//! 按字段合并：内容 / 编辑 / 已听各管各的、孤儿补丁只取比请求新的字段、临时内容不被 opened 动（临时内容本身优先显示在 `restricted.rs`）。

use super::*;

#[test]
fn content_and_edit_merge_in_any_order() {
    let base = || new(msg(CHAT, 5, text("旧")));
    let content = || CoreEvent::MessageContentChanged {
        id: id(CHAT, 5),
        content: text("新"),
    };
    let edited = || CoreEvent::MessageEdited {
        id: id(CHAT, 5),
        edit_date: Time(2_000_000_000),
    };
    let a = feed([base(), content(), edited()]);
    let b = feed([base(), edited(), content()]);
    for s in [&a, &b] {
        let m = s.message(id(CHAT, 5)).unwrap();
        assert_eq!(live_text(s, m.id).as_deref(), Some("新"));
        assert_eq!(m.edited, Some(Time(2_000_000_000)));
    }
    assert_eq!(
        project::message(&a, id(CHAT, 5)),
        project::message(&b, id(CHAT, 5))
    );
}

/// 编辑、回应、已听三种字段更新，任意顺序到达，结果都一样。
/// （内容替换不与「已听」交换：新内容自带已听标记，更新按序号到达，先后由 TDLib 定）
#[test]
fn field_updates_commute() {
    let evs = [
        CoreEvent::MessageContentOpened { id: id(CHAT, 5) },
        CoreEvent::MessageEdited {
            id: id(CHAT, 5),
            edit_date: Time(2_000_000_000),
        },
        CoreEvent::MessageInteractionChanged {
            id: id(CHAT, 5),
            reactions: vec![tree::Reaction {
                kind: tree::ReactionKind::Paid,
                count: 1,
                chosen: false,
            }],
        },
    ];
    let mut results = vec![];
    for perm in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut list = vec![new(msg(CHAT, 5, voice()))];
        list.extend(perm.iter().map(|&k| evs[k].clone()));
        results.push(project::message(&feed(list), id(CHAT, 5)));
    }
    assert!(results.windows(2).all(|w| w[0] == w[1]));
    match &results[0] {
        Some(Doc::Message(m)) => {
            assert_eq!(m.tail.edited, Some(Time(2_000_000_000)));
            assert_eq!(m.tail.reactions.len(), 1);
            assert!(matches!(
                m.content.attachment,
                Some(Attachment::Voice { listened: true, .. })
            ));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn content_opened_marks_listened() {
    let s = feed([
        new(msg(CHAT, 5, voice())),
        CoreEvent::MessageContentOpened { id: id(CHAT, 5) },
    ]);
    assert!(matches!(
        s.message(id(CHAT, 5)).unwrap().effective(),
        Body::Content(Content {
            attachment: Some(Attachment::Voice { listened: true, .. }),
            ..
        })
    ));
}

#[test]
fn orphan_patch_applies_only_fields_newer_than_request() {
    let mut s = Store::new();
    // 请求之前的内容补丁 v1（应答里已含更新的 v2）
    push(
        &mut s,
        CoreEvent::MessageContentChanged {
            id: id(CHAT, 5),
            content: text("v1"),
        },
    );
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t = s.ticket(CHAT);
    // 请求之后只来了编辑时间
    push(
        &mut s,
        CoreEvent::MessageEdited {
            id: id(CHAT, 5),
            edit_date: Time(9),
        },
    );
    s.apply(loaded(t, vec![msg(CHAT, 5, text("v2"))]));
    let m = s.message(id(CHAT, 5)).unwrap();
    assert_eq!(live_text(&s, m.id).as_deref(), Some("v2"));
    assert_eq!(m.edited, Some(Time(9)));
}

/// 请求发出后先「已听」、再换成一份未听的新内容：历史应答先到还是最后到，结果都是未听
#[test]
fn orphan_opened_then_new_content_any_order() {
    let unheard = || voice();
    let run = |history_first: bool| {
        let mut s = Store::new();
        s.apply(Input::OpenChat { chat_id: CHAT });
        let t = s.ticket(CHAT);
        let evs = [
            CoreEvent::MessageContentOpened { id: id(CHAT, 5) },
            CoreEvent::MessageContentChanged {
                id: id(CHAT, 5),
                content: unheard(),
            },
        ];
        if history_first {
            s.apply(loaded(t, vec![msg(CHAT, 5, voice())]));
            for e in evs {
                push(&mut s, e);
            }
        } else {
            for e in evs {
                push(&mut s, e);
            }
            s.apply(loaded(t, vec![msg(CHAT, 5, voice())]));
        }
        s.message(id(CHAT, 5)).unwrap().effective().clone()
    };
    let a = run(true);
    assert_eq!(a, run(false));
    assert!(matches!(
        a,
        Body::Content(Content {
            attachment: Some(Attachment::Voice {
                listened: false,
                ..
            }),
            ..
        })
    ));
}

/// 反过来：先换内容、再「已听」——两种先后都是已听
#[test]
fn orphan_new_content_then_opened_any_order() {
    let run = |history_first: bool| {
        let mut s = Store::new();
        s.apply(Input::OpenChat { chat_id: CHAT });
        let t = s.ticket(CHAT);
        let evs = [
            CoreEvent::MessageContentChanged {
                id: id(CHAT, 5),
                content: voice(),
            },
            CoreEvent::MessageContentOpened { id: id(CHAT, 5) },
        ];
        if history_first {
            s.apply(loaded(t, vec![msg(CHAT, 5, text("旧"))]));
            for e in evs {
                push(&mut s, e);
            }
        } else {
            for e in evs {
                push(&mut s, e);
            }
            s.apply(loaded(t, vec![msg(CHAT, 5, text("旧"))]));
        }
        s.message(id(CHAT, 5)).unwrap().effective().clone()
    };
    let a = run(true);
    assert_eq!(a, run(false));
    assert!(matches!(
        a,
        Body::Content(Content {
            attachment: Some(Attachment::Voice { listened: true, .. }),
            ..
        })
    ));
}

/// 两张请求前的凭据（都是序号 0）+ 消息没到时来的编辑：两份应答谁先到，结果都是「后发请求的正文 + 编辑时间」
#[test]
fn two_responses_with_orphan_edit_any_order() {
    let run = |older_first: bool| {
        let mut s = Store::new();
        s.apply(Input::OpenChat { chat_id: CHAT });
        let a = s.ticket(CHAT);
        let b = s.ticket(CHAT);
        push(
            &mut s,
            CoreEvent::MessageEdited {
                id: id(CHAT, 5),
                edit_date: Time(9),
            },
        );
        let ra = loaded(a, vec![msg(CHAT, 5, text("旧正文"))]);
        let rb = loaded(b, vec![msg(CHAT, 5, text("新正文"))]);
        if older_first {
            s.apply(ra);
            s.apply(rb);
        } else {
            s.apply(rb);
            s.apply(ra);
        }
        let m = s.message(id(CHAT, 5)).unwrap().clone();
        (live_text(&s, m.id), m.edited)
    };
    let x = run(true);
    assert_eq!(x, run(false));
    assert_eq!(x, (Some("新正文".to_owned()), Some(Time(9))));
}

/// 编辑时间为 0（TDLib 用来清掉编辑时间）：活消息、孤儿补丁、整条消息三个入口都得 None
#[test]
fn zero_edit_date_is_none_on_every_path() {
    let zero = || CoreEvent::MessageEdited {
        id: id(CHAT, 5),
        edit_date: Time(0),
    };
    // 活消息
    let mut edited = msg(CHAT, 5, text("a"));
    edited.edited = Some(Time(7));
    let live = feed([new(edited), zero()]);
    assert_eq!(live.message(id(CHAT, 5)).unwrap().edited, None);
    // 孤儿补丁
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t = s.ticket(CHAT);
    push(&mut s, zero());
    let mut resp = msg(CHAT, 5, text("a"));
    resp.edited = Some(Time(7));
    s.apply(loaded(t, vec![resp]));
    assert_eq!(s.message(id(CHAT, 5)).unwrap().edited, None);
    // 整条消息（夹具映射）
    let w = atg_core::tdjson::World::from_json(
        r#"[{"@type":"updateNewMessage","message":{"@type":"message","id":5,"chat_id":101,"date":1,"edit_date":0,
        "sender_id":{"@type":"messageSenderUser","user_id":7},
        "content":{"@type":"messageText","text":{"@type":"formattedText","text":"a","entities":[]}}}}]"#,
    )
    .unwrap();
    assert_eq!(w.store().message(id(CHAT, 5)).unwrap().edited, None);
    // 三个入口的投影一致
    assert_eq!(
        project::message(&live, id(CHAT, 5)),
        project::message(&s, id(CHAT, 5))
    );
}

/// 临时内容那一支：请求后先「已听」、再换一份未听的临时内容——两种先后都是未听
#[test]
fn orphan_opened_then_new_ephemeral_any_order() {
    let run = |history_first: bool| {
        let mut s = Store::new();
        s.apply(Input::OpenChat { chat_id: CHAT });
        let t = s.ticket(CHAT);
        let evs = [
            CoreEvent::MessageContentOpened { id: id(CHAT, 5) },
            CoreEvent::MessageEphemeralChanged {
                id: id(CHAT, 5),
                ephemeral: Some(voice()),
            },
        ];
        let mut base = msg(CHAT, 5, text("原"));
        base.ephemeral = Some(voice());
        if history_first {
            s.apply(loaded(t, vec![base]));
            for e in evs {
                push(&mut s, e);
            }
        } else {
            for e in evs {
                push(&mut s, e);
            }
            s.apply(loaded(t, vec![base]));
        }
        s.message(id(CHAT, 5)).unwrap().effective().clone()
    };
    let a = run(true);
    assert_eq!(a, run(false));
    assert!(matches!(
        a,
        Body::Content(Content {
            attachment: Some(Attachment::Voice {
                listened: false,
                ..
            }),
            ..
        })
    ));
}

/// 「已听」只改原内容：已有的临时语音保持未听（活消息、孤儿补丁两条路径）
#[test]
fn opened_does_not_touch_ephemeral() {
    let mut base = msg(CHAT, 5, voice());
    base.ephemeral = Some(voice());
    let heard = |b: &Body| {
        matches!(
            b,
            Body::Content(Content {
                attachment: Some(Attachment::Voice { listened: true, .. }),
                ..
            })
        )
    };
    // 活消息
    let s = feed([
        new(base.clone()),
        CoreEvent::MessageContentOpened { id: id(CHAT, 5) },
    ]);
    let m = s.message(id(CHAT, 5)).unwrap();
    assert!(heard(&m.content));
    assert!(!heard(m.ephemeral.as_ref().unwrap()));
    // 孤儿补丁
    let mut o = Store::new();
    o.apply(Input::OpenChat { chat_id: CHAT });
    let t = o.ticket(CHAT);
    push(&mut o, CoreEvent::MessageContentOpened { id: id(CHAT, 5) });
    o.apply(loaded(t, vec![base]));
    let m2 = o.message(id(CHAT, 5)).unwrap();
    assert!(heard(&m2.content));
    assert!(!heard(m2.ephemeral.as_ref().unwrap()));
    assert_eq!(m, m2);
}
