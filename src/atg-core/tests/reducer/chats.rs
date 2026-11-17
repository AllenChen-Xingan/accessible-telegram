//! 会话列表：排序、位置、草稿、置顶、提及计数、重复的 ChatAdded。

use super::*;

#[test]
fn chats_sorted_by_order_then_id_desc() {
    let s = feed([
        CoreEvent::ChatAdded {
            chat: chat(1, 10),
            last_message: None,
        },
        CoreEvent::ChatAdded {
            chat: chat(2, 30),
            last_message: None,
        },
        CoreEvent::ChatAdded {
            chat: chat(3, 10),
            last_message: None,
        },
    ]);
    assert_eq!(s.chat_list(&ChatList::Main), vec![2, 3, 1]);
}

#[test]
fn position_updates_move_and_remove_chats() {
    let mut s = feed([
        CoreEvent::ChatAdded {
            chat: chat(1, 10),
            last_message: None,
        },
        CoreEvent::ChatAdded {
            chat: chat(2, 20),
            last_message: None,
        },
    ]);
    push(
        &mut s,
        CoreEvent::ChatPositionChanged {
            chat_id: 1,
            position: pos(ChatList::Main, 99),
        },
    );
    assert_eq!(s.chat_list(&ChatList::Main), vec![1, 2]);
    // 进了归档：主列表不受影响
    push(
        &mut s,
        CoreEvent::ChatPositionChanged {
            chat_id: 2,
            position: pos(ChatList::Archive, 5),
        },
    );
    assert_eq!(s.chat_list(&ChatList::Main), vec![1, 2]);
    assert_eq!(s.chat_list(&ChatList::Archive), vec![2]);
    // order 0 = 移出主列表
    push(
        &mut s,
        CoreEvent::ChatPositionChanged {
            chat_id: 1,
            position: pos(ChatList::Main, 0),
        },
    );
    assert_eq!(s.chat_list(&ChatList::Main), vec![2]);
    // 最后一条变了，连带整体换位置
    push(
        &mut s,
        CoreEvent::ChatLastMessageChanged {
            chat_id: 1,
            last_message: Some(msg(1, 50, text("新的"))),
            positions: Some(vec![pos(ChatList::Main, 100)]),
        },
    );
    assert_eq!(s.chat_list(&ChatList::Main), vec![1, 2]);
    assert_eq!(s.chat(1).unwrap().last_message, Some(50));
    assert_eq!(s.chat_list(&ChatList::Archive), vec![2]);
}

#[test]
fn next_unread_chat_does_not_wrap() {
    let mut a = chat(1, 30);
    a.unread_count = 2;
    let b = chat(2, 20);
    let mut c = chat(3, 10);
    c.unread_count = 1;
    let s = feed([a, b, c].into_iter().map(|chat| CoreEvent::ChatAdded {
        chat,
        last_message: None,
    }));
    assert_eq!(s.next_unread_chat(None), Some(1));
    assert_eq!(s.next_unread_chat(Some(1)), Some(3));
    assert_eq!(s.next_unread_chat(Some(3)), None);
}

#[test]
fn duplicate_chat_added_keeps_last_message() {
    let s = feed([
        CoreEvent::ChatAdded {
            chat: chat(CHAT, 1),
            last_message: Some(msg(CHAT, 5, text("a"))),
        },
        CoreEvent::ChatAdded {
            chat: chat(CHAT, 1),
            last_message: None,
        },
    ]);
    assert_eq!(s.chat(CHAT).unwrap().last_message, Some(5));
}

#[test]
fn draft_update_replaces_positions() {
    let mut s = feed([
        CoreEvent::ChatAdded {
            chat: chat(1, 10),
            last_message: None,
        },
        CoreEvent::ChatAdded {
            chat: chat(2, 20),
            last_message: None,
        },
    ]);
    push(
        &mut s,
        CoreEvent::ChatDraftChanged {
            chat_id: 1,
            positions: vec![pos(ChatList::Main, 30), pos(ChatList::Folder(3), 7)],
        },
    );
    assert_eq!(s.chat_list(&ChatList::Main), vec![1, 2]);
    assert_eq!(s.chat_list(&ChatList::Folder(3)), vec![1]);
}

#[test]
fn pinned_is_per_list() {
    let mut c = chat(1, 10);
    c.positions = vec![
        pos(ChatList::Main, 10),
        ChatPosition {
            list: ChatList::Archive,
            order: 5,
            is_pinned: true,
        },
    ];
    let s = feed([CoreEvent::ChatAdded {
        chat: c,
        last_message: None,
    }]);
    assert!(!project::chat_item(&s, 1, &ChatList::Main).unwrap().pinned);
    assert!(
        project::chat_item(&s, 1, &ChatList::Archive)
            .unwrap()
            .pinned
    );
}

#[test]
fn mention_read_update_lowers_mention_count() {
    let w = atg_core::tdjson::World::from_json(
        r#"[{"@type":"updateNewChat","chat":{"@type":"chat","id":101,"title":"x","type":{"@type":"chatTypePrivate"},
          "unread_mention_count":1,"positions":[{"@type":"chatPosition","list":{"@type":"chatListMain"},"order":"1"}]}},
          {"@type":"updateMessageMentionRead","chat_id":101,"message_id":5,"unread_mention_count":0}]"#,
    )
    .unwrap();
    assert_eq!(w.store().chat(CHAT).unwrap().unread_mention_count, 0);
    // 不落进「不认识的更新」
    assert!(w.docs().is_empty());
}
