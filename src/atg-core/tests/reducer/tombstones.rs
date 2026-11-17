//! 撤回墓碑：留墓碑、不复活、和历史应答谁先到都一样、重拉与重开后保留。

use super::*;

#[test]
fn permanent_delete_leaves_tombstone_with_sender_and_date() {
    let mut s = feed([new(msg(CHAT, 5, text("原话")))]);
    s.apply(Input::OpenChat { chat_id: CHAT });
    push(&mut s, new(msg(CHAT, 6, text("后一条"))));
    push(&mut s, deleted(CHAT, &[6], true, false));
    match s.entry(id(CHAT, 6)) {
        Some(Entry::Tombstone(t)) => {
            assert_eq!(t.sender, Some(SenderId::User(LIN)));
            assert_eq!(t.date, Some(Time(1_792_755_420)));
        }
        other => panic!("应是墓碑：{other:?}"),
    }
    // 原位留着
    assert_eq!(window_ids(&s), vec![6]);
    // 原文不留
}

#[test]
fn cache_eviction_and_non_permanent_are_not_revokes() {
    for (is_permanent, from_cache) in [(true, true), (false, false), (false, true)] {
        let mut s = Store::new();
        s.apply(Input::OpenChat { chat_id: CHAT });
        push(&mut s, new(msg(CHAT, 5, text("还在"))));
        push(&mut s, deleted(CHAT, &[5], is_permanent, from_cache));
        assert!(
            !matches!(s.entry(id(CHAT, 5)), Some(Entry::Tombstone(_))),
            "permanent={is_permanent} from_cache={from_cache} 不许当撤回"
        );
        assert!(project::window(&s).is_empty());
    }
}

#[test]
fn tombstone_is_not_revived() {
    let mut s = feed([new(msg(CHAT, 5, text("原话")))]);
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t = s.ticket(CHAT);
    push(&mut s, deleted(CHAT, &[5], true, false));
    // 重复的新消息、内容更新、清缓存、晚到的应答都不让它复活
    push(&mut s, new(msg(CHAT, 5, text("原话"))));
    push(
        &mut s,
        CoreEvent::MessageContentChanged {
            id: id(CHAT, 5),
            content: text("改过"),
        },
    );
    push(&mut s, deleted(CHAT, &[5], true, true));
    s.apply(loaded(t, vec![msg(CHAT, 5, text("原话"))]));
    // 撤回后才发出的请求，应答里还带着它（服务器还没同步）：同样不复活
    let t2 = s.ticket(CHAT);
    s.apply(loaded(t2, vec![msg(CHAT, 5, text("原话"))]));
    assert!(matches!(s.entry(id(CHAT, 5)), Some(Entry::Tombstone(_))));
    assert!(matches!(
        project::window(&s).as_slice(),
        [Doc::Tombstone(_)]
    ));
}

#[test]
fn revoke_of_unseen_message_still_blocks_late_history() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t = s.ticket(CHAT);
    push(&mut s, deleted(CHAT, &[9], true, false));
    s.apply(loaded(t, vec![msg(CHAT, 9, text("撤回了的"))]));
    match s.entry(id(CHAT, 9)) {
        // 没见过原消息：发送人、时间从应答补记，原文不留
        Some(Entry::Tombstone(t)) => {
            assert_eq!(t.sender, Some(SenderId::User(LIN)));
            assert_eq!(t.date, Some(Time(1_792_755_420)));
        }
        other => panic!("应是墓碑：{other:?}"),
    }
    assert_eq!(window_ids(&s), vec![9]);
}

#[test]
fn tombstone_same_either_order_with_history() {
    // 应答先到、再撤回 vs 撤回先到、再晚到应答：墓碑一样
    let mut a = Store::new();
    a.apply(Input::OpenChat { chat_id: CHAT });
    let t = a.ticket(CHAT);
    a.apply(loaded(t, vec![msg(CHAT, 9, text("x"))]));
    push(&mut a, deleted(CHAT, &[9], true, false));

    let mut b = Store::new();
    b.apply(Input::OpenChat { chat_id: CHAT });
    let t = b.ticket(CHAT);
    push(&mut b, deleted(CHAT, &[9], true, false));
    b.apply(loaded(t, vec![msg(CHAT, 9, text("x"))]));

    assert_eq!(project::window(&a), project::window(&b));
}

/// 甲3：重拉 / 切走再回来，窗口里的撤回占位原位保留；带回来的只在已加载范围内显示
#[test]
fn tombstones_survive_refetch_and_reopen() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    for k in [4, 5, 6] {
        push(&mut s, new(msg(CHAT, k, text("x"))));
    }
    push(&mut s, deleted(CHAT, &[5], true, false));
    let kinds = |s: &Store| -> Vec<(i64, &'static str)> {
        project::window(s)
            .iter()
            .map(|d| match d {
                Doc::Message(m) => (m.id.message_id, "msg"),
                Doc::Tombstone(t) => (t.id.message_id, "tomb"),
                _ => (0, "?"),
            })
            .collect()
    };
    // 重拉：窗口清空，应答（服务器已没有 5）回来后 5 仍在原位
    s.apply(Input::Refetch);
    assert!(kinds(&s).is_empty(), "还没加载时带来的占位先不显示");
    let t = s.ticket(CHAT);
    s.apply(loaded(
        t,
        vec![msg(CHAT, 4, text("x")), msg(CHAT, 6, text("x"))],
    ));
    assert_eq!(kinds(&s), vec![(4, "msg"), (5, "tomb"), (6, "msg")]);
    // 切走再回来，同样
    s.apply(Input::OpenChat { chat_id: GROUP });
    s.apply(Input::OpenChat { chat_id: CHAT });
    let t = s.ticket(CHAT);
    s.apply(loaded(t, vec![msg(CHAT, 6, text("x"))]));
    // 只加载到 6：比它早的 5 不孤零零挂在顶上
    assert_eq!(kinds(&s), vec![(6, "msg")]);
    let t = s.ticket(CHAT);
    s.apply(loaded(t, vec![msg(CHAT, 4, text("x"))]));
    assert_eq!(kinds(&s), vec![(4, "msg"), (5, "tomb"), (6, "msg")]);
}

/// 窗口里只剩撤回占位、重拉回来的历史是空的：占位仍原位显示
#[test]
fn only_tombstones_survive_empty_refetch() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    push(&mut s, new(msg(CHAT, 5, text("唯一一条"))));
    push(&mut s, deleted(CHAT, &[5], true, false));
    s.apply(Input::Refetch);
    let t = s.ticket(CHAT);
    s.apply(loaded(t, vec![]));
    assert!(matches!(
        project::window(&s).as_slice(),
        [Doc::Tombstone(t)] if t.id == id(CHAT, 5)
    ));
}

/// 空应答恢复显示的撤回占位，之后再来新消息也不被挤掉
#[test]
fn restored_tombstone_survives_next_new_message() {
    let mut s = Store::new();
    s.apply(Input::OpenChat { chat_id: CHAT });
    push(&mut s, new(msg(CHAT, 5, text("唯一一条"))));
    push(&mut s, deleted(CHAT, &[5], true, false));
    s.apply(Input::Refetch);
    let t = s.ticket(CHAT);
    s.apply(loaded(t, vec![]));
    push(&mut s, new(msg(CHAT, 9, text("新来的"))));
    let ids: Vec<i64> = project::window(&s)
        .iter()
        .map(|d| match d {
            Doc::Message(m) => m.id.message_id,
            Doc::Tombstone(t) => t.id.message_id,
            _ => 0,
        })
        .collect();
    assert_eq!(ids, vec![5, 9]);
}

/// 重拉中新消息与空历史谁先到，撤回占位都原位恢复（加载边界只由应答推进）
#[test]
fn tombstone_restore_independent_of_new_message_order() {
    let run = |new_first: bool| {
        let mut s = Store::new();
        s.apply(Input::OpenChat { chat_id: CHAT });
        push(&mut s, new(msg(CHAT, 5, text("唯一一条"))));
        push(&mut s, deleted(CHAT, &[5], true, false));
        s.apply(Input::Refetch);
        let t = s.ticket(CHAT);
        if new_first {
            push(&mut s, new(msg(CHAT, 9, text("新来的"))));
            s.apply(loaded(t, vec![]));
        } else {
            s.apply(loaded(t, vec![]));
            push(&mut s, new(msg(CHAT, 9, text("新来的"))));
        }
        project::window(&s)
            .iter()
            .map(|d| match d {
                Doc::Message(m) => m.id.message_id,
                Doc::Tombstone(t) => t.id.message_id,
                _ => 0,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(run(true), vec![5, 9]);
    assert_eq!(run(false), vec![5, 9]);
}
