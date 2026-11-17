//! 缓存上限与删旧（拍板 006，`docs/plan/006_decisions-b7.md`）。

use super::*;

/// 甲1：非当前会话只留最近 N 条；最后一条、墓碑不删；当前会话不限；切走后再删
#[test]
fn cache_cap_keeps_recent_last_and_tombstones() {
    let mut s = small(3);
    push(
        &mut s,
        CoreEvent::ChatAdded {
            chat: chat(GROUP, 1),
            last_message: Some(msg(GROUP, 1, text("最后一条（id 最小）"))),
        },
    );
    for k in 2..=6 {
        push(&mut s, new(msg(GROUP, k, text("x"))));
    }
    push(&mut s, deleted(GROUP, &[2], true, false));
    let live: Vec<i64> = (1..=6)
        .filter(|&k| s.message(id(GROUP, k)).is_some())
        .collect();
    // 最后一条 1 留着，另留最近 3 条里的活消息（2 已撤回、是墓碑）
    assert!(live.contains(&1));
    assert_eq!(live.len(), 3, "{live:?}");
    assert!(live.contains(&6) && live.contains(&5));
    assert!(matches!(s.entry(id(GROUP, 2)), Some(Entry::Tombstone(_))));

    // 当前会话不限
    s.apply(Input::OpenChat { chat_id: CHAT });
    for k in 1..=10 {
        push(&mut s, new(msg(CHAT, k, text("y"))));
    }
    assert_eq!(window_ids(&s).len(), 10);
    // 切走后 CHAT 变成非当前，删到 3 条
    s.apply(Input::OpenChat { chat_id: GROUP });
    let left = (1..=10)
        .filter(|&k| s.message(id(CHAT, k)).is_some())
        .count();
    assert_eq!(left, 3);
    assert!(s.message(id(CHAT, 10)).is_some());
}

/// 甲1：被删掉的旧消息，之后来了更新，早于删除的请求的应答不能拿旧内容盖回来
#[test]
fn pruned_message_not_restored_by_old_history() {
    let mut s = small(1);
    let t = s.ticket(GROUP);
    push(&mut s, new(msg(GROUP, 1, text("v1"))));
    push(&mut s, new(msg(GROUP, 2, text("新"))));
    // 1 被删；之后它的内容更新落成补丁
    assert!(s.message(id(GROUP, 1)).is_none());
    push(
        &mut s,
        CoreEvent::MessageContentChanged {
            id: id(GROUP, 1),
            content: text("v2"),
        },
    );
    s.apply(loaded(t, vec![msg(GROUP, 1, text("v0 旧应答"))]));
    assert_ne!(live_text(&s, id(GROUP, 1)).as_deref(), Some("v0 旧应答"));
    // 删之后发的新请求照常收（会话打开着，不再删）
    s.apply(Input::OpenChat { chat_id: GROUP });
    let t2 = s.ticket(GROUP);
    s.apply(loaded(t2, vec![msg(GROUP, 1, text("v2"))]));
    assert_eq!(live_text(&s, id(GROUP, 1)).as_deref(), Some("v2"));
}

/// #28 之一：补丁被挤掉后又来一条同消息的局部补丁，也不能放旧应答进来（两种先后的正文一致）
#[test]
fn evicted_then_new_partial_patch_still_blocks_old_history() {
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
    let edit_a = || CoreEvent::MessageEdited {
        id: id(CHAT, 5),
        edit_date: Time(9),
    };
    // 应答后到
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
    push(&mut b, edit_a());
    b.apply(loaded(t, vec![msg(CHAT, 5, text("旧版"))]));
    assert_ne!(live_text(&b, id(CHAT, 5)).as_deref(), Some("旧版"));
    assert!(b.needs_refetch());
    b.apply(Input::Refetch);
    let t2 = b.ticket(CHAT);
    // 重拉的应答在所有更新之后，已含新版和编辑时间
    let mut fresh = msg(CHAT, 5, text("新版"));
    fresh.edited = Some(Time(9));
    b.apply(loaded(t2, vec![fresh]));
    assert_eq!(live_text(&b, id(CHAT, 5)).as_deref(), Some("新版"));
    assert_eq!(b.message(id(CHAT, 5)).unwrap().edited, Some(Time(9)));
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
    push(&mut a, edit_a());
    assert_eq!(live_text(&a, id(CHAT, 5)), live_text(&b, id(CHAT, 5)));
}

/// 删旧后，同一序号、更早编号的请求的应答不能把旧版写回
#[test]
fn pruned_then_same_seq_older_request_rejected() {
    let mut s = small(1);
    push(
        &mut s,
        CoreEvent::Unknown {
            td_type: "x".into(),
        },
    );
    let t1 = s.ticket(GROUP);
    let t2 = s.ticket(GROUP);
    assert_eq!(t1.since_seq, t2.since_seq);
    s.apply(loaded(
        t2,
        vec![msg(GROUP, 5, text("新版")), msg(GROUP, 6, text("x"))],
    ));
    assert!(s.message(id(GROUP, 5)).is_none(), "5 应被删旧");
    s.apply(Input::OpenChat { chat_id: GROUP });
    s.apply(loaded(t1, vec![msg(GROUP, 5, text("旧版"))]));
    assert_ne!(live_text(&s, id(GROUP, 5)).as_deref(), Some("旧版"));
}
