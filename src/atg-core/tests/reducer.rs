//! reducer 与投影的单测：全用手写的假事件，不经 JSON、不需要 TDLib。
//! 重点：乱序到达、重复到达、晚到的旧应答、撤回 vs 清缓存、发送成功换 id（架构方案第一节第 3 小节第 6 条）。

use atg_core::model::*;
use atg_core::project;
use atg_core::store::{Entry, HistoryTicket, Input, Store, reduce};
use atg_core::tree::{self, Doc, Sender, Span};

// ───── 造假数据 ─────

const ME: i64 = 1;
const LIN: i64 = 7;
const CHAT: i64 = 101;
const GROUP: i64 = -1001;

fn id(chat_id: i64, message_id: i64) -> MsgId {
    MsgId {
        chat_id,
        message_id,
    }
}

fn text(t: &str) -> Body {
    Body::Content(Content {
        body: vec![Span::Text { text: t.into() }],
        attachment: None,
        sensitive: false,
    })
}

fn voice() -> Body {
    Body::Content(Content {
        body: vec![],
        attachment: Some(Attachment::Voice {
            duration: 3,
            listened: false,
        }),
        sensitive: false,
    })
}

fn msg(chat_id: i64, message_id: i64, body: Body) -> Message {
    Message {
        id: id(chat_id, message_id),
        sender: SenderId::User(LIN),
        date: Time(1_792_755_420),
        outgoing: false,
        reply_to: None,
        forward: None,
        album_id: None,
        topic: None,
        content: body,
        ephemeral: None,
        restriction: None,
        sensitive: false,
        edited: None,
        reactions: vec![],
        sending: SendingState::Sent,
    }
}

fn chat(chat_id: i64, order: i64) -> Chat {
    Chat {
        id: chat_id,
        title: format!("会话{chat_id}"),
        kind: ChatKind::Private,
        positions: vec![pos(ChatList::Main, order)],
        unread_count: 0,
        unread_mention_count: 0,
        last_read_inbox: 0,
        last_read_outbox: 0,
        notify: ChatNotify::default(),
        last_message: None,
    }
}

fn pos(list: ChatList, order: i64) -> ChatPosition {
    ChatPosition {
        list,
        order,
        is_pinned: false,
    }
}

/// 按顺序喂一串事件，序号从 1 起
fn feed(events: impl IntoIterator<Item = CoreEvent>) -> Store {
    let mut s = Store::new();
    for e in events {
        let seq = s.last_seq() + 1;
        s.apply(Input::Update { seq, event: e });
    }
    s
}

fn push(s: &mut Store, e: CoreEvent) {
    let seq = s.last_seq() + 1;
    s.apply(Input::Update { seq, event: e });
}

fn new(m: Message) -> CoreEvent {
    CoreEvent::NewMessage(m)
}

fn deleted(chat_id: i64, ids: &[i64], is_permanent: bool, from_cache: bool) -> CoreEvent {
    CoreEvent::MessagesDeleted {
        chat_id,
        message_ids: ids.to_vec(),
        is_permanent,
        from_cache,
    }
}

fn live_text(s: &Store, i: MsgId) -> Option<String> {
    match s.message(i)?.effective() {
        Body::Content(c) => Some(tree::text::plain_text(&c.body)),
        Body::Service(_) => None,
    }
}

fn window_ids(s: &Store) -> Vec<i64> {
    s.window()
        .map(|w| w.ids.iter().copied().collect())
        .unwrap_or_default()
}

fn loaded(ticket: HistoryTicket, messages: Vec<Message>) -> Input {
    Input::HistoryLoaded { ticket, messages }
}

// ───── 会话排序 ─────

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

// ───── 撤回墓碑 ─────

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

// ───── 按字段合并 ─────

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

// ───── 版本边界与请求代次 ─────

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

// ───── 重复到达 ─────

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

// ───── 发送成功换 id ─────

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
            td_error_code: 400,
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

// ───── 有效内容 · 受限内容 ─────

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

// ───── 投影 ─────

#[test]
fn projection_fills_late_names_and_tombstone_sender() {
    let mut s = feed([new(msg(CHAT, 5, text("a")))]);
    push(&mut s, deleted(CHAT, &[5], true, false));
    // 名字最后才到
    push(
        &mut s,
        CoreEvent::UserChanged(User {
            id: LIN,
            first_name: "小雨".into(),
            last_name: String::new(),
        }),
    );
    match project::message(&s, id(CHAT, 5)) {
        Some(Doc::Tombstone(t)) => {
            assert_eq!(
                t.sender,
                Some(Sender::User {
                    user_id: LIN,
                    name: Some("小雨".into())
                })
            );
            assert_eq!(t.date, Some(Time(1_792_755_420)));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn reply_sender_from_store_else_origin() {
    let mut reply = msg(CHAT, 6, text("回你"));
    reply.reply_to = Some(ReplyRef::Message {
        target: id(CHAT, 5),
        quote: None,
        origin: Some(OriginId::User(99)),
    });
    let mut orphan = msg(CHAT, 7, text("回一条不在的"));
    orphan.reply_to = Some(ReplyRef::Message {
        target: id(GROUP, 1),
        quote: None,
        origin: Some(OriginId::User(99)),
    });
    let s = feed([new(msg(CHAT, 5, text("原"))), new(reply), new(orphan)]);
    let sender_of = |mid| match project::message(&s, id(CHAT, mid)) {
        Some(Doc::Message(m)) => match m.head.reply_to {
            Some(tree::ReplyTo::Message { sender, .. }) => sender,
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    assert_eq!(
        sender_of(6),
        Some(Sender::User {
            user_id: LIN,
            name: None
        })
    );
    assert_eq!(
        sender_of(7),
        Some(Sender::User {
            user_id: 99,
            name: None
        })
    );
}

#[test]
fn delivery_read_follows_outbox() {
    let mut m = msg(CHAT, 5, text("a"));
    m.outgoing = true;
    let mut s = feed([
        CoreEvent::ChatAdded {
            chat: chat(CHAT, 1),
            last_message: None,
        },
        new(m),
    ]);
    let delivery = |s: &Store| match project::message(s, id(CHAT, 5)) {
        Some(Doc::Message(m)) => m.tail.delivery,
        other => panic!("{other:?}"),
    };
    assert_eq!(delivery(&s), Some(tree::Delivery::Sent));
    push(
        &mut s,
        CoreEvent::ChatReadOutbox {
            chat_id: CHAT,
            last_read_outbox_message_id: 5,
        },
    );
    assert_eq!(delivery(&s), Some(tree::Delivery::Read));
}

#[test]
fn inherited_mute_uses_scope_setting() {
    let mut c = chat(GROUP, 1);
    c.kind = ChatKind::Supergroup;
    c.notify = ChatNotify {
        use_default_mute_for: true,
        mute_for: 0,
    };
    let mut s = feed([CoreEvent::ChatAdded {
        chat: c,
        last_message: None,
    }]);
    let item = |s: &Store| project::chat_item(s, GROUP, &ChatList::Main).unwrap();
    assert!(!item(&s).muted && item(&s).mute_inherited);
    push(
        &mut s,
        CoreEvent::ScopeNotificationChanged {
            scope: NotifyScope::Group,
            mute_for: 3600,
        },
    );
    assert!(item(&s).muted);
}

#[test]
fn chat_last_can_be_tombstone() {
    let mut s = feed([CoreEvent::ChatAdded {
        chat: chat(CHAT, 1),
        last_message: Some(msg(CHAT, 5, text("a"))),
    }]);
    push(&mut s, deleted(CHAT, &[5], true, false));
    assert!(matches!(
        project::chat_item(&s, CHAT, &ChatList::Main)
            .unwrap()
            .last
            .as_deref(),
        Some(tree::ChatLast::Tombstone(_))
    ));
}

#[test]
fn reduce_is_a_pure_function_of_inputs() {
    let inputs = [
        Input::OpenChat { chat_id: CHAT },
        Input::Update {
            seq: 1,
            event: new(msg(CHAT, 5, text("a"))),
        },
        Input::Update {
            seq: 2,
            event: deleted(CHAT, &[5], true, false),
        },
    ];
    let run = || inputs.iter().cloned().fold(Store::new(), reduce);
    assert_eq!(project::window(&run()), project::window(&run()));
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

#[test]
fn user_status_is_kept_including_stopping() {
    let mut s = feed([CoreEvent::UserStatusChanged(UserStatus::Online)]);
    assert_eq!(s.status(), Some(UserStatus::Online));
    push(&mut s, CoreEvent::UserStatusChanged(UserStatus::Stopping));
    assert_eq!(s.status(), Some(UserStatus::Stopping));
    let j = serde_json::to_value(tree::Status {
        status: UserStatus::Stopping,
    })
    .unwrap();
    assert_eq!(j["status"], "stopping");
}

fn pending(message_id: i64) -> Message {
    let mut p = msg(CHAT, message_id, text("你好"));
    p.outgoing = true;
    p.sender = SenderId::User(ME);
    p.sending = SendingState::Pending;
    p
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
            td_error_code: 500,
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
                        name: None
                    })
                );
            }
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
    match project::message(&s, id(CHAT, 51)) {
        Some(Doc::Service(sv)) => {
            assert_eq!(
                sv.action,
                tree::Action::Pinned {
                    pinned: id(CHAT, 42)
                }
            )
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
                td_error_code: 500,
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
            td_error_code: 500,
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

// ───── 拍板 006 ─────

use atg_core::store::StoreConfig;

fn small(keep: usize) -> Store {
    Store::with_config(StoreConfig {
        keep_per_chat: keep,
    })
}

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
                td_error_code: 500,
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

// ───── 第八轮 ─────

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

// ───── 第九轮 ─────

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

// ───── 第十轮 ─────

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
