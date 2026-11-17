//! 版本边界与请求代次、重复到达、晚到的历史应答。

use super::*;

#[test]
fn late_old_history_does_not_overwrite_newer_update() {
    let mut s = feed([new(msg(CHAT, 5, text("v1")))]);
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t = s.ticket(CHAT);
    // 请求发出后，内容更新先到
    push(
        &mut s,
        CoreEvent::MessageContentChanged {
            id: id(CHAT, 5),
            content: text("v2"),
        },
    );
    // 应答带着旧内容晚到；同批里另一条没被碰过，照收
    s.apply(loaded(
        t,
        vec![msg(CHAT, 5, text("v1")), msg(CHAT, 4, text("更早"))],
    ));
    assert_eq!(live_text(&s, id(CHAT, 5)).as_deref(), Some("v2"));
    assert_eq!(live_text(&s, id(CHAT, 4)).as_deref(), Some("更早"));
    assert_eq!(window_ids(&s), vec![4, 5]);
}

#[test]
fn history_replaces_untouched_copy() {
    let mut s = feed([new(msg(CHAT, 5, text("缓存里的")))]);
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t = s.ticket(CHAT);
    s.apply(loaded(t, vec![msg(CHAT, 5, text("服务器上的"))]));
    assert_eq!(live_text(&s, id(CHAT, 5)).as_deref(), Some("服务器上的"));
}

#[test]
fn two_responses_out_of_order_keep_the_newer() {
    let mut s = feed([new(msg(CHAT, 5, text("v1")))]);
    s.apply(Input::OpenChat { chat_id: CHAT });
    let old = s.ticket(CHAT);
    push(
        &mut s,
        CoreEvent::Unknown {
            td_type: "x".into(),
        },
    );
    let newer = s.ticket(CHAT);
    // 新请求的应答先到，旧请求的晚到
    s.apply(loaded(newer, vec![msg(CHAT, 5, text("v3"))]));
    s.apply(loaded(old, vec![msg(CHAT, 5, text("v2"))]));
    assert_eq!(live_text(&s, id(CHAT, 5)).as_deref(), Some("v3"));
}

#[test]
fn update_before_message_is_kept_when_old_history_arrives() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t = s.ticket(CHAT);
    // 消息本身还没进 store，它的编辑和内容更新先到
    push(
        &mut s,
        CoreEvent::MessageContentChanged {
            id: id(CHAT, 5),
            content: text("v2"),
        },
    );
    push(
        &mut s,
        CoreEvent::MessageEdited {
            id: id(CHAT, 5),
            edit_date: Time(2_000_000_000),
        },
    );
    let mut old = msg(CHAT, 5, text("v1"));
    old.reactions = vec![tree::Reaction {
        kind: tree::ReactionKind::Paid,
        count: 3,
        chosen: false,
    }];
    s.apply(loaded(t, vec![old]));
    let m = s.message(id(CHAT, 5)).unwrap();
    assert_eq!(live_text(&s, m.id).as_deref(), Some("v2"));
    assert_eq!(m.edited, Some(Time(2_000_000_000)));
    // 没被更新碰过的字段取应答的
    assert_eq!(m.reactions.len(), 1);
}

#[test]
fn stale_generation_only_fills_cache() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t_old = s.ticket(CHAT);
    // 用户切走了
    s.apply(Input::OpenChat { chat_id: GROUP });
    s.apply(loaded(t_old, vec![msg(CHAT, 5, text("旧窗口的"))]));
    assert_eq!(s.window().unwrap().chat_id, GROUP);
    assert!(window_ids(&s).is_empty());
    assert_eq!(live_text(&s, id(CHAT, 5)).as_deref(), Some("旧窗口的"));
    // 切回来再拉，代次又加一；之前那张凭据也不算当前
    s.apply(Input::OpenChat { chat_id: CHAT });
    s.apply(loaded(t_old, vec![msg(CHAT, 6, text("还是旧的"))]));
    assert!(window_ids(&s).is_empty());
    let t = s.ticket(CHAT);
    s.apply(loaded(t, vec![msg(CHAT, 5, text("新的"))]));
    assert_eq!(window_ids(&s), vec![5]);
}

#[test]
fn refetch_bumps_generation_and_clears_window() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t = s.ticket(CHAT);
    s.apply(loaded(t, vec![msg(CHAT, 5, text("a"))]));
    assert_eq!(window_ids(&s), vec![5]);
    s.apply(Input::Refetch);
    assert!(window_ids(&s).is_empty());
    // 重新拉之前发出的那张凭据作废
    s.apply(loaded(t, vec![msg(CHAT, 6, text("b"))]));
    assert!(window_ids(&s).is_empty());
}

#[test]
fn history_ignores_other_chats_messages() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t = s.ticket(CHAT);
    s.apply(loaded(t, vec![msg(GROUP, 5, text("混进来的"))]));
    assert!(s.entry(id(GROUP, 5)).is_none());
}

#[test]
fn non_revoke_delete_after_request_blocks_old_copy() {
    let mut s = feed([new(msg(CHAT, 5, text("a")))]);
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t = s.ticket(CHAT);
    push(&mut s, deleted(CHAT, &[5], false, false));
    s.apply(loaded(t, vec![msg(CHAT, 5, text("a"))]));
    assert!(s.entry(id(CHAT, 5)).is_none());
}

#[test]
fn duplicate_seq_is_ignored() {
    let mut s = feed([new(msg(CHAT, 5, text("a")))]);
    s.apply(Input::Update {
        seq: 1,
        event: CoreEvent::MessageContentChanged {
            id: id(CHAT, 5),
            content: text("重放的"),
        },
    });
    assert_eq!(live_text(&s, id(CHAT, 5)).as_deref(), Some("a"));
}

#[test]
fn later_full_message_replaces_copy() {
    // 整条消息随更新到达 = 那个序号时的完整状态，整条替换
    let mut later = msg(CHAT, 5, text("b"));
    later.edited = Some(Time(2_000_000_000));
    let s = feed([new(msg(CHAT, 5, text("a"))), new(later)]);
    assert_eq!(live_text(&s, id(CHAT, 5)).as_deref(), Some("b"));
    assert_eq!(
        s.message(id(CHAT, 5)).unwrap().edited,
        Some(Time(2_000_000_000))
    );
}

/// 旧历史先落库、完整更新后到 vs 完整更新先到、旧历史后到：结果一样（取更新那份）
#[test]
fn history_vs_full_update_any_order() {
    let run = |history_first: bool| {
        let mut s = Store::new();
        s.apply(Input::OpenChat { chat_id: CHAT });
        let t = s.ticket(CHAT);
        let upd = new(msg(CHAT, 5, text("v2")));
        if history_first {
            s.apply(loaded(t, vec![msg(CHAT, 5, text("v1"))]));
            push(&mut s, upd);
        } else {
            push(&mut s, upd);
            s.apply(loaded(t, vec![msg(CHAT, 5, text("v1"))]));
        }
        live_text(&s, id(CHAT, 5))
    };
    assert_eq!(run(true).as_deref(), Some("v2"));
    assert_eq!(run(false).as_deref(), Some("v2"));
}

/// 孤儿补丁满了被挤掉后，请求之前的旧应答不能拿旧内容盖掉已收到的更新：
/// 应答先到 → 新内容；应答后到 → 不收、窗口标「要重拉」，重拉后拿到新的
#[test]
fn evicted_orphan_vs_old_history_any_order() {
    let flood = |s: &mut Store| {
        for k in 0..4096 {
            push(
                s,
                CoreEvent::MessageEdited {
                    id: id(GROUP, k),
                    edit_date: Time(1),
                },
            );
        }
    };
    // 应答先到
    let mut a = Store::new();
    a.apply(Input::OpenChat { chat_id: CHAT });
    let t = a.ticket(CHAT);
    a.apply(loaded(t, vec![msg(CHAT, 5, text("旧版"))]));
    push(
        &mut a,
        CoreEvent::MessageContentChanged {
            id: id(CHAT, 5),
            content: text("新版"),
        },
    );
    flood(&mut a);
    assert_eq!(live_text(&a, id(CHAT, 5)).as_deref(), Some("新版"));
    // 应答后到：A 的补丁被挤掉
    let mut b = Store::new();
    b.apply(Input::OpenChat { chat_id: CHAT });
    let t = b.ticket(CHAT);
    push(
        &mut b,
        CoreEvent::MessageContentChanged {
            id: id(CHAT, 5),
            content: text("新版"),
        },
    );
    flood(&mut b);
    b.apply(loaded(t, vec![msg(CHAT, 5, text("旧版"))]));
    assert_ne!(live_text(&b, id(CHAT, 5)).as_deref(), Some("旧版"));
    assert!(b.needs_refetch());
    b.apply(Input::Refetch);
    assert!(!b.needs_refetch());
    let t2 = b.ticket(CHAT);
    b.apply(loaded(t2, vec![msg(CHAT, 5, text("新版"))]));
    assert_eq!(live_text(&b, id(CHAT, 5)).as_deref(), Some("新版"));
    assert_eq!(window_ids(&b), vec![5]);
}

#[test]
fn duplicate_snapshot_after_request_blocks_old_response() {
    let mut s = feed([new(msg(CHAT, 5, text("v1")))]);
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t = s.ticket(CHAT);
    push(
        &mut s,
        CoreEvent::MessageEdited {
            id: id(CHAT, 4),
            edit_date: Time(1),
        },
    );
    // 请求之后又整条到了一次（如会话最后一条）：store 那份到这时都是最新的
    push(
        &mut s,
        CoreEvent::ChatLastMessageChanged {
            chat_id: CHAT,
            last_message: Some(msg(CHAT, 5, text("v1"))),
            positions: None,
        },
    );
    push(&mut s, new(msg(CHAT, 5, text("v1"))));
    s.apply(loaded(t, vec![msg(CHAT, 5, text("旧应答"))]));
    assert_eq!(live_text(&s, id(CHAT, 5)).as_deref(), Some("v1"));
}

#[test]
fn same_seq_older_generation_does_not_win() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    let a = s.ticket(CHAT);
    s.apply(Input::Refetch);
    let b = s.ticket(CHAT);
    assert_eq!(a.since_seq, b.since_seq);
    s.apply(loaded(b, vec![msg(CHAT, 5, text("新"))]));
    s.apply(loaded(a, vec![msg(CHAT, 5, text("旧"))]));
    assert_eq!(live_text(&s, id(CHAT, 5)).as_deref(), Some("新"));
    assert_eq!(window_ids(&s), vec![5]);
}

#[test]
fn two_requests_without_updates_later_one_wins() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    let a = s.ticket(CHAT);
    let b = s.ticket(CHAT);
    assert_eq!(a.since_seq, b.since_seq);
    s.apply(loaded(b, vec![msg(CHAT, 5, text("新"))]));
    s.apply(loaded(a, vec![msg(CHAT, 5, text("旧"))]));
    assert_eq!(live_text(&s, id(CHAT, 5)).as_deref(), Some("新"));
}

/// 最后一条未知期间新消息只随「最后一条」到达（`td_api.tl:11312`）：会话正开着就进窗口
#[test]
fn last_message_only_arrival_enters_open_window() {
    let mut s = feed([CoreEvent::ChatAdded {
        chat: chat(CHAT, 1),
        last_message: None,
    }]);
    s.apply(Input::OpenChat { chat_id: CHAT });
    push(
        &mut s,
        CoreEvent::ChatLastMessageChanged {
            chat_id: CHAT,
            last_message: None,
            positions: None,
        },
    );
    push(
        &mut s,
        CoreEvent::ChatLastMessageChanged {
            chat_id: CHAT,
            last_message: Some(msg(CHAT, 7, text("只从这里来"))),
            positions: None,
        },
    );
    assert_eq!(window_ids(&s), vec![7]);
    // 别的会话的最后一条不进当前窗口
    push(
        &mut s,
        CoreEvent::ChatAdded {
            chat: chat(GROUP, 2),
            last_message: Some(msg(GROUP, 8, text("别处"))),
        },
    );
    assert_eq!(window_ids(&s), vec![7]);
}
