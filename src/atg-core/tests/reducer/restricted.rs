//! 有效内容与受限内容：临时内容优先、受限不留原文、回复里的引用清除。临时内容和孤儿补丁的先后在 `fields.rs`。

use super::*;

fn restricted_target_and_reply() -> (Message, Message) {
    let mut a = msg(CHAT, 5, text("机密"));
    a.restriction = Some("版权".into());
    let mut b = msg(CHAT, 6, text("回"));
    b.reply_to = Some(ReplyRef::Message {
        target: id(CHAT, 5),
        quote: Some(vec![Span::Text {
            text: "机密引用".into(),
        }]),
        origin: None,
    });
    (a, b)
}

fn quote_leaks(s: &Store) -> bool {
    serde_json::to_string(&project::message(s, id(CHAT, 6)))
        .unwrap()
        .contains("机密引用")
}

#[test]
fn ephemeral_content_wins_and_falls_back() {
    let mut m = msg(CHAT, 5, text("原内容"));
    m.ephemeral = Some(text("临时内容"));
    let mut s = feed([new(m)]);
    assert_eq!(live_text(&s, id(CHAT, 5)).as_deref(), Some("临时内容"));
    push(
        &mut s,
        CoreEvent::MessageEphemeralChanged {
            id: id(CHAT, 5),
            ephemeral: None,
        },
    );
    assert_eq!(live_text(&s, id(CHAT, 5)).as_deref(), Some("原内容"));
    // 原内容换了，临时内容又出现：仍显示临时的
    push(
        &mut s,
        CoreEvent::MessageEphemeralChanged {
            id: id(CHAT, 5),
            ephemeral: Some(text("又一次临时")),
        },
    );
    push(
        &mut s,
        CoreEvent::MessageContentChanged {
            id: id(CHAT, 5),
            content: text("原内容 2"),
        },
    );
    assert_eq!(live_text(&s, id(CHAT, 5)).as_deref(), Some("又一次临时"));
}

#[test]
fn restricted_content_keeps_no_original_text() {
    let mut m = msg(CHAT, 5, text("机密原文"));
    m.ephemeral = Some(text("临时机密"));
    m.restriction = Some("版权".into());
    let mut s = feed([new(m)]);
    // 之后来的内容更新同样被换掉
    push(
        &mut s,
        CoreEvent::MessageContentChanged {
            id: id(CHAT, 5),
            content: text("又一段机密"),
        },
    );
    // 存下来的原内容、临时内容都已是受限节点（原文不留）
    let m = s.message(id(CHAT, 5)).unwrap();
    for body in std::iter::once(&m.content).chain(m.ephemeral.as_ref()) {
        assert!(matches!(
            body,
            Body::Content(Content {
                attachment: Some(Attachment::Restricted { .. }),
                ..
            })
        ));
    }
    match project::message(&s, id(CHAT, 5)) {
        Some(Doc::Message(d)) => {
            assert!(d.content.body.is_empty());
            assert_eq!(
                d.content.attachment,
                Some(Attachment::Restricted {
                    reason: "版权".into()
                })
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn sensitive_flag_survives_content_change() {
    let mut m = msg(CHAT, 5, text("a"));
    m.sensitive = true;
    let s = feed([
        new(m),
        CoreEvent::MessageContentChanged {
            id: id(CHAT, 5),
            content: text("b"),
        },
    ]);
    match s.message(id(CHAT, 5)).unwrap().effective() {
        Body::Content(c) => assert!(c.sensitive),
        Body::Service(_) => panic!(),
    }
}

#[test]
fn restricted_service_message_keeps_no_title() {
    let mut m = msg(
        GROUP,
        5,
        Body::Service(tree::Action::TitleChanged {
            title: "机密群名".into(),
        }),
    );
    m.restriction = Some("违规".into());
    let s = feed([new(m)]);
    match project::message(&s, id(GROUP, 5)) {
        Some(Doc::Message(d)) => assert_eq!(
            d.content.attachment,
            Some(Attachment::Restricted {
                reason: "违规".into()
            })
        ),
        other => panic!("应是受限节点：{other:?}"),
    }
    let j = serde_json::to_string(&project::message(&s, id(GROUP, 5))).unwrap();
    assert!(!j.contains("机密群名"));
}

/// 乙11：被回复那条受限，回复里的引用不出原文
#[test]
fn quote_cleared_when_reply_target_restricted() {
    let mut target = msg(CHAT, 5, text("机密"));
    target.restriction = Some("版权".into());
    let mut reply = msg(CHAT, 6, text("回"));
    reply.reply_to = Some(ReplyRef::Message {
        target: id(CHAT, 5),
        quote: Some(vec![Span::Text {
            text: "机密引用".into(),
        }]),
        origin: None,
    });
    let s = feed([new(target), new(reply.clone())]);
    let j = serde_json::to_string(&project::message(&s, id(CHAT, 6))).unwrap();
    assert!(!j.contains("机密引用"), "{j}");
    // 被回复那条不受限时引用照出
    let s2 = feed([new(msg(CHAT, 5, text("公开"))), new(reply)]);
    let j2 = serde_json::to_string(&project::message(&s2, id(CHAT, 6))).unwrap();
    assert!(j2.contains("机密引用"));
}

/// 受限目标被删旧后，回复里的引用不重新出现
#[test]
fn quote_stays_cleared_after_target_pruned() {
    let (a, b) = restricted_target_and_reply();
    let mut s = small(1);
    s.apply(Input::OpenChat { chat_id: CHAT });
    push(&mut s, new(a));
    push(&mut s, new(b));
    assert!(!quote_leaks(&s));
    s.apply(Input::OpenChat { chat_id: GROUP });
    assert!(s.message(id(CHAT, 5)).is_none(), "A 应被删旧");
    assert!(s.message(id(CHAT, 6)).is_some());
    assert!(!quote_leaks(&s));
}

/// 受限目标被撤回后，回复里的引用不重新出现
#[test]
fn quote_stays_cleared_after_target_revoked() {
    let (a, b) = restricted_target_and_reply();
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    push(&mut s, new(a));
    push(&mut s, new(b));
    push(&mut s, deleted(CHAT, &[5], true, false));
    assert!(matches!(s.entry(id(CHAT, 5)), Some(Entry::Tombstone(_))));
    assert!(!quote_leaks(&s));
}

/// 回复先到、受限目标后到：存着的引用也被清掉
#[test]
fn quote_cleared_when_restricted_target_arrives_later() {
    let (a, b) = restricted_target_and_reply();
    let mut s = feed([new(b)]);
    assert!(quote_leaks(&s), "目标没到时还不知道受限");
    push(&mut s, new(a));
    assert!(!quote_leaks(&s));
    // 存的那份也清了：目标再被删旧 / 撤回都不会回来
    match &s.message(id(CHAT, 6)).unwrap().reply_to {
        Some(ReplyRef::Message { quote, .. }) => assert!(quote.is_none()),
        other => panic!("{other:?}"),
    }
}

/// 受限目标没加载过就被撤回、受限的历史后到（或反过来）：两种先后引用都清掉、都认得受限
#[test]
fn restricted_history_vs_delete_any_order() {
    let run = |delete_first: bool| {
        let (a, b) = restricted_target_and_reply();
        let mut s = Store::new();
        s.apply(Input::OpenChat { chat_id: CHAT });
        let t = s.ticket(CHAT);
        push(&mut s, new(b));
        if delete_first {
            push(&mut s, deleted(CHAT, &[5], true, false));
            s.apply(loaded(t, vec![a]));
        } else {
            s.apply(loaded(t, vec![a]));
            push(&mut s, deleted(CHAT, &[5], true, false));
        }
        assert!(s.is_restricted(id(CHAT, 5)), "delete_first={delete_first}");
        assert!(!quote_leaks(&s), "delete_first={delete_first}");
        match &s.message(id(CHAT, 6)).unwrap().reply_to {
            Some(ReplyRef::Message { quote, .. }) => assert!(quote.is_none()),
            other => panic!("{other:?}"),
        }
    };
    run(true);
    run(false);
}
