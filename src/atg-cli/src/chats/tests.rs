//! 假后端单测：数据全是编的（名字「会话N」、正文「第N条」），不碰真实账号。
use super::*;
use atg_core::config::ServerKind;
use atg_core::error::map_td_error;
use atg_core::fake::{Call, FakeAuth, FakeBackend};
use atg_core::speech::chat_brief;
use atg_core::tree::{ChatKind, ChatLast};
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_root(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "atg-cli-chats-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn profile(tag: &str) -> Profile {
    Profile::create(&temp_root(tag), "main", ServerKind::Main).unwrap()
}

/// 已登录的假后端
fn backend() -> FakeBackend {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth {
        logged_in: true,
        ..FakeAuth::default()
    });
    b
}

fn list_json(list: &ChatList) -> Value {
    match list {
        ChatList::Archive => json!({"@type": "chatListArchive"}),
        _ => json!({"@type": "chatListMain"}),
    }
}

fn position(id: i64, list: &ChatList, order: i64, pinned: bool) -> Value {
    json!({"@type": "updateChatPosition", "chat_id": id, "position": {
        "@type": "chatPosition", "list": list_json(list), "order": order.to_string(), "is_pinned": pinned,
    }})
}

/// 一个私聊会话：`updateNewChat`（TDLib 加载时位置是空的）+ 位置
fn chat_in(list: &ChatList, id: i64, order: i64, patch: Value) -> Vec<Value> {
    let mut c = json!({
        "@type": "chat", "id": id, "title": format!("会话{id}"),
        "type": {"@type": "chatTypePrivate", "user_id": id},
        "positions": [], "unread_count": 0, "unread_mention_count": 0,
        "last_read_inbox_message_id": 0, "last_read_outbox_message_id": 0,
        "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0},
    });
    if let (Some(c), Value::Object(p)) = (c.as_object_mut(), patch) {
        c.extend(p);
    }
    vec![
        json!({"@type": "updateNewChat", "chat": c}),
        position(id, list, order, false),
    ]
}

fn chat(id: i64, order: i64) -> Vec<Value> {
    chat_in(&ChatList::Main, id, order, json!({}))
}

fn last_message(chat_id: i64, msg_id: i64, text: &str, outgoing: bool) -> Value {
    json!({"@type": "updateChatLastMessage", "chat_id": chat_id, "last_message": {
        "@type": "message", "id": msg_id, "chat_id": chat_id,
        "sender_id": {"@type": "messageSenderUser", "user_id": if outgoing { 1 } else { chat_id }},
        "is_outgoing": outgoing, "date": 1_792_755_420, "edit_date": 0,
        "media_album_id": "0",
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": text, "entities": []}},
    }})
}

fn query(limit: usize) -> Query {
    Query {
        list: ChatList::Main,
        limit,
        unread: false,
        after: None,
    }
}

fn ids(l: &Listing) -> Vec<i64> {
    l.items.iter().map(|c| c.chat_id).collect()
}

/// 起后端之前先把连接线推到 ready（补发完了）
fn ready(b: &FakeBackend) {
    b.push_net("connectionStateReady");
}

fn fast() -> Limits {
    Limits {
        load: Duration::from_secs(5),
        sync: Duration::from_millis(300),
    }
}

fn run_fast(b: &FakeBackend, p: &Profile, q: &Query) -> Result<Listing, Fail> {
    run_with(b, p, q, Instant::now(), fast())
}

fn ok(r: Result<Listing, Fail>) -> Listing {
    match r {
        Ok(l) => l,
        Err(Fail::Core(e)) => panic!("出错：{}", e.code()),
        Err(_) => panic!("出错"),
    }
}

fn core_err(r: Result<Listing, Fail>) -> CoreError {
    match r {
        Err(Fail::Core(e)) => e,
        Err(_) => panic!("不是 CoreError"),
        Ok(_) => panic!("没出错"),
    }
}

#[test]
fn order_follows_store_not_load_order() {
    let b = backend();
    // TDLib 交出的先后与 order 不一致；同 order 按会话 id 从大到小
    b.set_chats(
        ChatList::Main,
        vec![chat(10, 300), chat(11, 500), chat(12, 300), chat(13, 100)],
    );
    ready(&b);
    let l = ok(run_fast(&b, &profile("order"), &query(10)));
    assert_eq!(ids(&l), vec![11, 12, 10, 13]);
    assert!(!l.more);
    assert_eq!(l.next_cursor, None);
    assert!(l.synced);
    assert!(l.first_chat_ms.is_some());
}

#[test]
fn updates_while_waiting_for_sync_reorder_the_list() {
    let b = backend();
    b.set_chats(
        ChatList::Main,
        vec![chat(1, 300), chat(2, 200), chat(3, 100)],
    );
    let p = profile("reorder");
    let l = std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(100));
            // 补发途中：会话 3 来了新消息挪到最前，会话 1 被挪出主列表（如归档）
            b.push(position(3, &ChatList::Main, 900, false));
            b.push(position(1, &ChatList::Main, 0, false));
            b.push(last_message(3, 1 << 20, "第一条", false));
            ready(&b);
        });
        ok(run_with(
            &b,
            &p,
            &query(10),
            Instant::now(),
            Limits {
                load: Duration::from_secs(5),
                sync: Duration::from_secs(5),
            },
        ))
    });
    assert!(l.synced);
    assert_eq!(ids(&l), vec![3, 2]);
    let last = l.items[0].last.as_deref();
    assert!(matches!(last, Some(ChatLast::Message(_))));
}

#[test]
fn fields_unread_mentions_mute_pin_last() {
    let b = backend();
    b.set_chats(
        ChatList::Main,
        vec![
            // 自己设了静音、有未读与提及、置顶
            [
                chat_in(
                    &ChatList::Main,
                    1,
                    900,
                    json!({
                        "unread_count": 7, "unread_mention_count": 2,
                        "notification_settings": {"@type": "chatNotificationSettings",
                            "use_default_mute_for": false, "mute_for": 3600},
                    }),
                ),
                vec![
                    position(1, &ChatList::Main, 900, true),
                    last_message(1, 2 << 20, "明天见。", false),
                ],
            ]
            .concat(),
            // 跟随默认：私聊范围静音 → 有效静音
            [
                chat_in(&ChatList::Main, 2, 800, json!({})),
                vec![last_message(2, 3 << 20, "收到", true)],
            ]
            .concat(),
            // 一个群：跟随默认，群范围不静音
            chat_in(
                &ChatList::Main,
                3,
                700,
                json!({"type": {"@type": "chatTypeBasicGroup", "basic_group_id": 3}}),
            ),
        ],
    );
    b.push(json!({"@type": "updateScopeNotificationSettings",
        "scope": {"@type": "notificationSettingsScopePrivateChats"},
        "notification_settings": {"@type": "scopeNotificationSettings", "mute_for": 86400}}));
    b.push(json!({"@type": "updateScopeNotificationSettings",
        "scope": {"@type": "notificationSettingsScopeGroupChats"},
        "notification_settings": {"@type": "scopeNotificationSettings", "mute_for": 0}}));
    ready(&b);
    let l = ok(run_fast(&b, &profile("fields"), &query(10)));
    assert_eq!(ids(&l), vec![1, 2, 3]);
    let [a, c2, g] = l.items.as_slice() else {
        panic!("要三个")
    };
    assert_eq!((a.unread_count, a.unread_mention_count), (7, 2));
    assert!(a.muted && !a.mute_inherited && a.pinned);
    assert_eq!(chat_brief(a), "会话1，7 条未读，2 条提到你，明天见");
    assert!(c2.muted && c2.mute_inherited && !c2.pinned);
    assert_eq!(chat_brief(c2), "会话2，我：收到");
    assert_eq!(g.kind, ChatKind::Group);
    assert!(!g.muted && g.mute_inherited);
    assert!(g.last.is_none());
    assert_eq!(chat_brief(g), "会话3");
}

#[test]
fn last_message_is_not_truncated() {
    // R2：最后一条不截断
    let long = "很长".repeat(100);
    let b = backend();
    b.set_chats(
        ChatList::Main,
        vec![[chat(1, 9), vec![last_message(1, 1 << 20, &long, false)]].concat()],
    );
    ready(&b);
    let l = ok(run_fast(&b, &profile("long"), &query(1)));
    assert_eq!(chat_brief(&l.items[0]), format!("会话1，{long}"));
}

#[test]
fn empty_list() {
    let b = backend();
    ready(&b);
    let l = ok(run_fast(&b, &profile("empty"), &query(100)));
    assert!(l.items.is_empty());
    assert!(!l.more);
    assert_eq!(l.next_cursor, None);
    assert_eq!(l.first_chat_ms, None);
    assert!(l.synced);
    assert!(b.calls().contains(&Call::ChatsLoad {
        list: ChatList::Main,
        limit: 101,
    }));
}

/// 超多会话的 order：TDLib 的 `loadChats` 从列表顶上往下交，所以按交出的先后递减
fn order_of(id: i64) -> i64 {
    1_000_000 - id
}

#[test]
fn many_chats_page_with_cursor_covers_everything_once() {
    const N: i64 = 2500;
    let b = backend();
    b.set_chats(
        ChatList::Main,
        (1..=N).map(|id| chat(id, order_of(id))).collect(),
    );
    ready(&b);
    let p = profile("many");
    let first = ok(run_fast(&b, &p, &query(100)));
    assert_eq!(first.items.len(), 100);
    assert!(first.more);
    let cursor = first.next_cursor.clone().unwrap();
    // 只加载了够一页的，没把 2500 个全拉下来
    let loads: u32 = b
        .calls()
        .iter()
        .filter_map(|c| match c {
            Call::ChatsLoad { limit, .. } => Some(*limit),
            _ => None,
        })
        .sum();
    assert!(loads < 300, "{loads}");

    // 一次全列：与 store 顺序一致、严格递减、不重
    let b = backend();
    b.set_chats(
        ChatList::Main,
        (1..=N).map(|id| chat(id, order_of(id))).collect(),
    );
    ready(&b);
    let all = ok(run_fast(&b, &profile("many-all"), &query(100_000)));
    assert_eq!(all.items.len(), usize::try_from(N).unwrap());
    assert!(!all.more);
    let keys: Vec<(i64, i64)> = all
        .items
        .iter()
        .map(|c| (order_of(c.chat_id), c.chat_id))
        .collect();
    assert!(keys.windows(2).all(|w| w[0] > w[1]));
    assert_eq!(ids(&first), ids(&all)[..100].to_vec());

    // 用游标接着翻：每页接上、整体等于全列
    let mut got = ids(&first);
    let mut cur = Some(cursor);
    while let Some(c) = cur {
        let b = backend();
        b.set_chats(
            ChatList::Main,
            (1..=N).map(|id| chat(id, order_of(id))).collect(),
        );
        ready(&b);
        let q = Query {
            after: Cursor::parse(&c),
            ..query(1000)
        };
        let page = ok(run_fast(&b, &profile("many-page"), &q));
        got.extend(ids(&page));
        cur = page.next_cursor;
    }
    assert_eq!(got, ids(&all));
}

#[test]
fn unread_filter_keeps_loading_until_page_is_full() {
    let b = backend();
    // 300 个会话，每 50 个里第一个有未读
    b.set_chats(
        ChatList::Main,
        (1..=300)
            .map(|id| {
                let unread = if id % 50 == 1 { 3 } else { 0 };
                chat_in(
                    &ChatList::Main,
                    id,
                    1000 - id,
                    json!({ "unread_count": unread }),
                )
            })
            .collect(),
    );
    ready(&b);
    let q = Query {
        unread: true,
        ..query(5)
    };
    let l = ok(run_fast(&b, &profile("unread"), &q));
    assert_eq!(ids(&l), vec![1, 51, 101, 151, 201]);
    assert!(l.more);
    assert!(l.items.iter().all(|c| c.unread_count > 0));
}

#[test]
fn archive_is_a_separate_list() {
    let b = backend();
    b.set_chats(ChatList::Main, vec![chat(1, 10)]);
    b.set_chats(
        ChatList::Archive,
        vec![
            chat_in(&ChatList::Archive, 2, 20, json!({})),
            chat_in(&ChatList::Archive, 3, 30, json!({})),
        ],
    );
    ready(&b);
    let q = Query {
        list: ChatList::Archive,
        ..query(10)
    };
    let l = ok(run_fast(&b, &profile("archive"), &q));
    assert_eq!(ids(&l), vec![3, 2]);
    assert!(b.calls().iter().all(|c| !matches!(
        c,
        Call::ChatsLoad {
            list: ChatList::Main,
            ..
        }
    )));
}

#[test]
fn not_synced_when_connection_never_ready() {
    // 断网：会话照样从（假）本地库出，只是 synced=false
    let b = backend();
    b.set_chats(ChatList::Main, vec![chat(1, 10)]);
    b.push_net("connectionStateWaitingForNetwork");
    let l = ok(run_fast(&b, &profile("offline"), &query(10)));
    assert_eq!(ids(&l), vec![1]);
    assert!(!l.synced);
}

#[test]
fn load_that_never_answers_stops_at_the_deadline() {
    // 断网、本地库不够一页：第一批出了本地的，再要就一直没应答。到点照出已有的，不卡死
    let b = backend();
    b.set_chats(ChatList::Main, vec![chat(1, 20), chat(2, 10)]);
    b.hang_chats_load_after(1);
    b.push_net("connectionStateWaitingForNetwork");
    let t = Instant::now();
    let l = ok(run_with(
        &b,
        &profile("hang"),
        &query(100),
        Instant::now(),
        Limits {
            load: Duration::from_millis(300),
            sync: Duration::from_secs(30),
        },
    ));
    assert!(
        t.elapsed() < Duration::from_secs(5),
        "加载超时后不该再等补发"
    );
    assert_eq!(ids(&l), vec![1, 2]);
    assert!(l.load_timed_out && l.more && !l.synced);
    assert_eq!(l.next_cursor.as_deref(), Some("10:2"));
}

#[test]
fn not_logged_in_is_auth_required() {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth::default());
    let e = core_err(run_fast(&b, &profile("noauth"), &query(10)));
    assert_eq!(e.code(), "auth_required");
    // 后端照样关掉
    assert_eq!(b.calls().last(), Some(&Call::Shutdown));
}

#[test]
fn load_error_is_reported_and_backend_closed() {
    let b = backend();
    b.set_chats(ChatList::Main, vec![chat(1, 10)]);
    b.fail_next(
        "chats_load",
        map_td_error(429, "Too Many Requests: retry after 7"),
    );
    ready(&b);
    let e = core_err(run_fast(&b, &profile("loaderr"), &query(10)));
    assert_eq!(e.code(), "rate_limited");
    assert_eq!(b.calls().last(), Some(&Call::Shutdown));
}

#[test]
fn cursor_roundtrip_and_rejects_garbage() {
    let c = Cursor {
        order: 9_221_294_780_217_032_704,
        chat_id: -1_001_234_567_890,
    };
    assert_eq!(Cursor::parse(&c.encode()), Some(c));
    for bad in ["", "1", "a:b", "1:2:3", ":5", "5:"] {
        assert_eq!(Cursor::parse(bad), None, "{bad}");
    }
}
