//! 发送成功换 id、重发、撤销待发：位置与稳定键不变、旧 id 不复活。

use super::*;

fn pending(message_id: i64) -> Message {
    let mut p = msg(CHAT, message_id, text("你好"));
    p.outgoing = true;
    p.sender = SenderId::User(ME);
    p.sending = SendingState::Pending;
    p
}

#[test]
fn send_succeeded_swaps_id_keeps_place_and_stable_key() {
    let mut pending = msg(CHAT, 1_000_001, text("你好"));
    pending.outgoing = true;
    pending.sender = SenderId::User(ME);
    pending.sending = SendingState::Pending;
    let mut s = feed([CoreEvent::ChatAdded {
        chat: chat(CHAT, 1),
        last_message: None,
    }]);
    s.apply(Input::OpenChat { chat_id: CHAT });
    push(&mut s, new(pending.clone()));
    push(
        &mut s,
        CoreEvent::ChatLastMessageChanged {
            chat_id: CHAT,
            last_message: Some(pending.clone()),
            positions: None,
        },
    );
    let mut sent = pending.clone();
    sent.id = id(CHAT, 42);
    sent.sending = SendingState::Sent;
    push(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: pending.id,
            message: sent,
        },
    );
    assert!(s.entry(pending.id).is_none());
    assert_eq!(window_ids(&s), vec![42]);
    assert_eq!(s.chat(CHAT).unwrap().last_message, Some(42));
    assert_eq!(s.stable_key(id(CHAT, 42)), pending.id);
    assert_eq!(s.stable_key(id(CHAT, 7)), id(CHAT, 7));
    match project::message(&s, id(CHAT, 42)) {
        Some(Doc::Message(m)) => assert_eq!(m.tail.delivery, Some(tree::Delivery::Sent)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn send_failed_then_resend_keeps_first_key() {
    let mut p = msg(CHAT, 1_000_001, text("你好"));
    p.outgoing = true;
    p.sending = SendingState::Pending;
    let mut s = feed([new(p.clone())]);
    let mut failed = p.clone();
    failed.id = id(CHAT, 1_000_002);
    failed.sending = SendingState::Failed;
    push(
        &mut s,
        CoreEvent::SendFailed {
            old_id: p.id,
            message: failed.clone(),
            error: atg_core::error::map_td_error(400, ""),
            can_retry: true,
        },
    );
    let mut ok = failed.clone();
    ok.id = id(CHAT, 43);
    ok.sending = SendingState::Sent;
    push(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: failed.id,
            message: ok,
        },
    );
    assert_eq!(s.stable_key(id(CHAT, 43)), p.id);
}

#[test]
fn send_succeeded_for_unseen_pending_enters_open_window() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    let mut sent = msg(CHAT, 44, text("之前发的"));
    sent.outgoing = true;
    push(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: id(CHAT, 1_000_009),
            message: sent,
        },
    );
    assert_eq!(window_ids(&s), vec![44]);
}

/// 换 id 与晚到的历史应答（里面还是临时 id），两种先后结果一样：只有新 id，没有卡住的「待发送」
#[test]
fn swap_vs_late_history_any_order() {
    let run = |history_first: bool| {
        let mut s = Store::new();
        s.apply(Input::OpenChat { chat_id: CHAT });
        push(&mut s, new(pending(1_000_001)));
        s.apply(Input::Refetch);
        let t = s.ticket(CHAT);
        let mut sent = pending(1_000_001);
        sent.id = id(CHAT, 42);
        sent.sending = SendingState::Sent;
        let swap = CoreEvent::SendSucceeded {
            old_id: id(CHAT, 1_000_001),
            message: sent,
        };
        if history_first {
            s.apply(loaded(t, vec![pending(1_000_001)]));
            push(&mut s, swap);
        } else {
            push(&mut s, swap);
            s.apply(loaded(t, vec![pending(1_000_001)]));
        }
        assert!(s.entry(id(CHAT, 1_000_001)).is_none());
        project::window(&s)
    };
    let a = run(true);
    assert_eq!(a, run(false));
    assert!(matches!(a.as_slice(), [Doc::Message(m)] if m.id == id(CHAT, 42)));
}

/// TDLib 重发：先永久删掉失败的那条，再来一条新的待发送消息。删除不是撤回；
/// 新旧的关联由重发请求的应答（`Input::Resent`）给，稳定键沿用
#[test]
fn resend_is_not_a_revoke_and_keeps_key() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    push(&mut s, new(pending(1_000_001)));
    let mut failed = pending(1_000_002);
    failed.sending = SendingState::Failed;
    push(
        &mut s,
        CoreEvent::SendFailed {
            old_id: id(CHAT, 1_000_001),
            message: failed,
            error: atg_core::error::map_td_error(500, ""),
            can_retry: true,
        },
    );
    push(&mut s, deleted(CHAT, &[1_000_002], true, false));
    assert!(s.entry(id(CHAT, 1_000_002)).is_none(), "不许留撤回占位");
    push(&mut s, new(pending(1_000_003)));
    s.apply(Input::Resent {
        old: id(CHAT, 1_000_002),
        new: id(CHAT, 1_000_003),
    });
    assert_eq!(s.stable_key(id(CHAT, 1_000_003)), id(CHAT, 1_000_001));
    assert_eq!(window_ids(&s), vec![1_000_003]);
}

#[test]
fn cancel_pending_is_not_a_revoke() {
    let mut s = feed([new(pending(1_000_001))]);
    push(&mut s, deleted(CHAT, &[1_000_001], true, false));
    assert!(s.entry(id(CHAT, 1_000_001)).is_none());
    // 取消后紧接着另发一条新的：不凭相邻猜测，不继承稳定键
    push(&mut s, new(pending(1_000_002)));
    assert_eq!(s.stable_key(id(CHAT, 1_000_002)), id(CHAT, 1_000_002));
}

#[test]
fn fixture_harness_follows_id_swap() {
    let w = atg_core::tdjson::World::from_json(
        r#"[
        {"@type":"updateNewMessage","message":{"@type":"message","id":1000001,"chat_id":101,
          "is_outgoing":true,"date":1,"sender_id":{"@type":"messageSenderUser","user_id":1},
          "sending_state":{"@type":"messageSendingStatePending"},
          "content":{"@type":"messageText","text":{"@type":"formattedText","text":"你好","entities":[]}}}},
        {"@type":"updateMessageSendSucceeded","old_message_id":1000001,"message":{"@type":"message","id":42,"chat_id":101,
          "is_outgoing":true,"date":1,"sender_id":{"@type":"messageSenderUser","user_id":1},
          "content":{"@type":"messageText","text":{"@type":"formattedText","text":"你好","entities":[]}}}}
    ]"#,
    )
    .unwrap();
    assert!(matches!(w.docs().as_slice(), [Doc::Message(m)] if m.id == id(CHAT, 42)));
}

#[test]
fn repeated_swap_does_not_revive_revoked_target() {
    let mut s = feed([new(pending(1_000_001))]);
    let mut sent = pending(1_000_001);
    sent.id = id(CHAT, 42);
    sent.sending = SendingState::Sent;
    let swap = || CoreEvent::SendSucceeded {
        old_id: id(CHAT, 1_000_001),
        message: sent.clone(),
    };
    push(&mut s, swap());
    push(&mut s, deleted(CHAT, &[42], true, false));
    push(&mut s, swap());
    assert!(matches!(s.entry(id(CHAT, 42)), Some(Entry::Tombstone(_))));
    assert!(s.entry(id(CHAT, 1_000_001)).is_none());
}

#[test]
fn reply_and_pin_follow_swapped_id() {
    let mut s = Store::new();
    push(&mut s, new(pending(1_000_001)));
    let mut reply = msg(CHAT, 50, text("回你"));
    reply.reply_to = Some(ReplyRef::Message {
        target: id(CHAT, 1_000_001),
        quote: None,
        origin: None,
    });
    push(&mut s, new(reply));
    let mut pin = msg(
        CHAT,
        51,
        Body::Service(tree::Action::Pinned {
            pinned: id(CHAT, 1_000_001),
            content: None,
        }),
    );
    pin.sender = SenderId::User(LIN);
    push(&mut s, new(pin));
    let mut sent = pending(1_000_001);
    sent.id = id(CHAT, 42);
    sent.sending = SendingState::Sent;
    push(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: id(CHAT, 1_000_001),
            message: sent,
        },
    );
    match project::message(&s, id(CHAT, 50)) {
        Some(Doc::Message(m)) => match m.head.reply_to {
            Some(tree::ReplyTo::Message { target, sender, .. }) => {
                assert_eq!(target, id(CHAT, 42));
                assert_eq!(
                    sender,
                    Some(Sender::User {
                        user_id: ME,
                        name: None,
                        deleted: false,
                    })
                );
            }
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
    match project::message(&s, id(CHAT, 51)) {
        Some(Doc::Service(sv)) => {
            assert!(matches!(
                sv.action,
                tree::Action::Pinned { pinned, .. } if pinned == id(CHAT, 42)
            ))
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn retired_id_is_not_revived_by_late_updates() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    push(&mut s, new(pending(1_000_001)));
    let mut sent = pending(1_000_001);
    sent.id = id(CHAT, 42);
    sent.sending = SendingState::Sent;
    push(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: id(CHAT, 1_000_001),
            message: sent,
        },
    );
    push(&mut s, new(pending(1_000_001)));
    push(
        &mut s,
        CoreEvent::MessageEdited {
            id: id(CHAT, 1_000_001),
            edit_date: Time(1),
        },
    );
    let t = s.ticket(CHAT);
    s.apply(loaded(t, vec![pending(1_000_001)]));
    assert!(s.entry(id(CHAT, 1_000_001)).is_none());
    assert_eq!(window_ids(&s), vec![42]);
}

/// 重发应答（`Resent`）与发送成功（换 id）两种先后：稳定键都是最初那条
#[test]
fn resent_vs_send_succeeded_any_order() {
    let run = |resent_first: bool| {
        let mut s = Store::new();
        push(&mut s, new(pending(1_000_001))); // F 的前身
        let mut failed = pending(1_000_002); // F
        failed.sending = SendingState::Failed;
        push(
            &mut s,
            CoreEvent::SendFailed {
                old_id: id(CHAT, 1_000_001),
                message: failed,
                error: atg_core::error::map_td_error(500, ""),
                can_retry: true,
            },
        );
        push(&mut s, deleted(CHAT, &[1_000_002], true, false));
        push(&mut s, new(pending(1_000_003))); // B
        let resent = Input::Resent {
            old: id(CHAT, 1_000_002),
            new: id(CHAT, 1_000_003),
        };
        let mut sent = pending(1_000_003);
        sent.id = id(CHAT, 42); // C
        sent.sending = SendingState::Sent;
        let swap = CoreEvent::SendSucceeded {
            old_id: id(CHAT, 1_000_003),
            message: sent,
        };
        if resent_first {
            s.apply(resent);
            push(&mut s, swap);
        } else {
            push(&mut s, swap);
            s.apply(resent);
        }
        s.stable_key(id(CHAT, 42))
    };
    assert_eq!(run(true), id(CHAT, 1_000_001));
    assert_eq!(run(false), id(CHAT, 1_000_001));
}

/// 换 id 后又到一条「最后一条 = 旧 id」：会话的最后一条指向新 id，预览不丢
#[test]
fn last_message_with_retired_id_points_to_current() {
    let mut s = feed([CoreEvent::ChatAdded {
        chat: chat(CHAT, 1),
        last_message: None,
    }]);
    push(&mut s, new(pending(1_000_001)));
    let mut sent = pending(1_000_001);
    sent.id = id(CHAT, 42);
    sent.sending = SendingState::Sent;
    push(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: id(CHAT, 1_000_001),
            message: sent,
        },
    );
    push(
        &mut s,
        CoreEvent::ChatLastMessageChanged {
            chat_id: CHAT,
            last_message: Some(pending(1_000_001)),
            positions: None,
        },
    );
    assert_eq!(s.chat(CHAT).unwrap().last_message, Some(42));
    assert!(matches!(
        project::chat_item(&s, CHAT, &ChatList::Main)
            .unwrap()
            .last
            .as_deref(),
        Some(tree::ChatLast::Message(m)) if m.id == id(CHAT, 42)
    ));
    assert!(s.entry(id(CHAT, 1_000_001)).is_none());
}

/// 没加载过的失败消息被永久删除（重发的第一步），与带着它的历史应答交错：两种先后都不留撤回占位
#[test]
fn unseen_failed_delete_vs_history_any_order() {
    let run = |history_first: bool| {
        let mut s = Store::new();
        s.apply(Input::OpenChat { chat_id: CHAT });
        let t = s.ticket(CHAT);
        let mut failed = pending(5);
        failed.sending = SendingState::Failed;
        let h = loaded(t, vec![failed]);
        if history_first {
            s.apply(h);
            push(&mut s, deleted(CHAT, &[5], true, false));
        } else {
            push(&mut s, deleted(CHAT, &[5], true, false));
            s.apply(h);
        }
        assert!(
            s.entry(id(CHAT, 5)).is_none(),
            "history_first={history_first}"
        );
        // 删除之后再来一份请求之前的旧应答也带不回来
        let mut again = pending(5);
        again.sending = SendingState::Failed;
        s.apply(loaded(t, vec![again]));
        assert!(s.entry(id(CHAT, 5)).is_none());
        project::window(&s)
    };
    let a = run(true);
    assert_eq!(a, run(false));
    assert!(a.is_empty());
}

/// 连续两次重发、应答乱序：最终消息的稳定键总是最初那条
#[test]
fn two_resends_out_of_order_keep_first_key() {
    let a = id(CHAT, 1_000_001);
    let (f1, b, f2, c, sent) = (
        id(CHAT, 1_000_002),
        id(CHAT, 1_000_003),
        id(CHAT, 1_000_004),
        id(CHAT, 1_000_005),
        id(CHAT, 42),
    );
    let fail = |old: MsgId, new: MsgId| {
        let mut m = pending(new.message_id);
        m.sending = SendingState::Failed;
        CoreEvent::SendFailed {
            old_id: old,
            message: m,
            error: atg_core::error::map_td_error(500, ""),
            can_retry: true,
        }
    };
    let mut ok = pending(c.message_id);
    ok.id = sent;
    ok.sending = SendingState::Sent;
    let run = |first_resent_late: bool| {
        let mut s = feed([new(pending(a.message_id))]);
        push(&mut s, fail(a, f1));
        push(&mut s, deleted(CHAT, &[f1.message_id], true, false));
        push(&mut s, new(pending(b.message_id)));
        if !first_resent_late {
            s.apply(Input::Resent { old: f1, new: b });
        }
        push(&mut s, fail(b, f2));
        push(&mut s, deleted(CHAT, &[f2.message_id], true, false));
        push(&mut s, new(pending(c.message_id)));
        s.apply(Input::Resent { old: f2, new: c });
        push(
            &mut s,
            CoreEvent::SendSucceeded {
                old_id: c,
                message: ok.clone(),
            },
        );
        if first_resent_late {
            s.apply(Input::Resent { old: f1, new: b });
        }
        s.stable_key(sent)
    };
    assert_eq!(run(false), a);
    assert_eq!(run(true), a);
}

/// #28 之二：重发链超过 64 条边，稳定键仍是最初那条
#[test]
fn long_resend_chain_keeps_first_key() {
    let mut s = Store::new();
    let a0 = id(CHAT, 1_000_000);
    push(&mut s, new(pending(a0.message_id)));
    let mut cur = a0;
    for i in 0..40 {
        let f = id(CHAT, 2_000_000 + i);
        let next = id(CHAT, 3_000_000 + i);
        let mut failed = pending(f.message_id);
        failed.sending = SendingState::Failed;
        push(
            &mut s,
            CoreEvent::SendFailed {
                old_id: cur,
                message: failed,
                error: atg_core::error::map_td_error(500, ""),
                can_retry: true,
            },
        );
        push(&mut s, deleted(CHAT, &[f.message_id], true, false));
        push(&mut s, new(pending(next.message_id)));
        s.apply(Input::Resent { old: f, new: next });
        cur = next;
    }
    assert_eq!(s.stable_key(cur), a0);
    let mut ok = pending(cur.message_id);
    ok.id = id(CHAT, 42);
    ok.sending = SendingState::Sent;
    push(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: cur,
            message: ok,
        },
    );
    assert_eq!(s.stable_key(id(CHAT, 42)), a0);
}
