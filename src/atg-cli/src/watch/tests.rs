//! `watch` 的单测：判定、渲染、直跑的起停与收尾

//! 假后端 + 假时钟：数据全是编的（名字、会话名只用 `fixtures/names.txt` 里的），不碰真实账号、不碰本机档案目录。
use super::*;
use crate::deadlines::TOPIC_TIMEOUT;
use atg_core::config::ServerKind;
use atg_core::fake::{Call, FakeAuth, FakeBackend};
use atg_core::link::block_on;
use std::sync::{Arc, Mutex};

mod ctrl_c;

const PEER: i64 = 101;
const GROUP: i64 = -1001;
/// 假时钟的「现在」（秒）
const NOW_S: i64 = 1_793_000_000;

fn clock() -> i64 {
    NOW_S * 1000
}

fn profile(tag: &str) -> Profile {
    let root = std::env::temp_dir().join(format!(
        "atg-cli-watch-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    Profile::create(&root, "main", ServerKind::Main).unwrap()
}

fn user(id: i64, name: &str) -> Value {
    json!({"@type": "updateUser", "user": {"@type": "user", "id": id, "first_name": name, "last_name": "",
        "usernames": null, "phone_number": "", "is_contact": true, "type": {"@type": "userTypeRegular"}}})
}

fn chat(id: i64, ty: Value, title: &str) -> Value {
    json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title, "type": ty,
        "positions": [], "unread_count": 0, "unread_mention_count": 0, "last_read_inbox_message_id": 0,
        "last_read_outbox_message_id": 0,
        "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0}}})
}

fn private() -> Value {
    json!({"@type": "chatTypePrivate", "user_id": PEER})
}

fn new_msg(chat_id: i64, id: i64, text: Value) -> Value {
    json!({"@type": "updateNewMessage", "message": {"@type": "message", "id": id, "chat_id": chat_id,
        "sender_id": {"@type": "messageSenderUser", "user_id": PEER}, "is_outgoing": false,
        "date": NOW_S - 10, "content": {"@type": "messageText", "text": text}}})
}

fn plain(t: &str) -> Value {
    json!({"@type": "formattedText", "text": t, "entities": []})
}

fn typing(action: &str) -> Value {
    json!({"@type": "updateChatAction", "chat_id": PEER, "topic_id": null,
        "sender_id": {"@type": "messageSenderUser", "user_id": PEER}, "action": {"@type": action}})
}

/// 收集输出；看到满足 `until` 的那一行就立「停」（相当于人按了 Ctrl+C）
struct Cap {
    lines: Arc<Mutex<Vec<String>>>,
    stop: &'static AtomicBool,
    until: fn(&Value) -> bool,
}

impl LineOut for Cap {
    fn line(&mut self, line: &str) -> bool {
        let v: Value = serde_json::from_str(line).unwrap_or(Value::Null);
        if (self.until)(&v) {
            self.stop.store(true, Ordering::SeqCst);
        }
        self.lines.lock().unwrap().push(line.to_owned());
        true
    }
}

fn cx(reveal: bool) -> SpeechCtx {
    crate::history::speech_ctx(Some("+08:00"), reveal).unwrap_or_else(|_| panic!("时区"))
}

fn backend() -> FakeBackend {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth {
        logged_in: true,
        ..FakeAuth::default()
    });
    b
}

/// 跑一次直跑的 `watch`，到 `until` 那行为止；回（退出码，每行 JSON）
fn run_watch(
    b: &FakeBackend,
    q: Query,
    reveal: bool,
    until: fn(&Value) -> bool,
) -> (u8, Vec<Value>) {
    let stop: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    let lines = Arc::new(Mutex::new(Vec::new()));
    let out = Cap {
        lines: lines.clone(),
        stop,
        until,
    };
    let p = profile("run");
    let code = run(
        b,
        &p,
        &q,
        cx(reveal),
        Format::Json,
        stop,
        Box::new(out),
        clock,
        Instant::now(),
    );
    let ls = lines
        .lock()
        .unwrap()
        .iter()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (code, ls)
}

fn events<'a>(ls: &'a [Value], ty: &str) -> Vec<&'a Value> {
    ls.iter()
        .filter(|l| l["kind"] == "event" && l["data"]["type"] == ty)
        .collect()
}

#[test]
fn backlog_then_sync_done_then_live_message() {
    let b = backend();
    b.push(user(PEER, "林小雨"));
    b.push(chat(PEER, private(), "林小雨"));
    b.push(new_msg(PEER, 1, plain("第一条")));
    b.push_net("connectionStateReady");
    b.push(new_msg(PEER, 2, plain("第二条")));
    let (code, ls) = run_watch(&b, Query { chat: None }, false, |v| {
        v["data"]["message_id"] == 2 && v["data"]["type"] == "new_message"
    });
    assert_eq!(code, 0);
    assert_eq!(ls[0]["kind"], "status");
    let msgs = events(&ls, "new_message");
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0]["data"]["notify"]["alert"], false);
    assert_eq!(msgs[0]["data"]["notify"]["reason"], "backlog");
    assert_eq!(msgs[1]["data"]["notify"]["alert"], true);
    assert_eq!(msgs[1]["data"]["notify"]["reason"], "new");
    assert_eq!(msgs[1]["data"]["notify"]["sound"], true);
    assert_eq!(msgs[1]["data"]["message"]["type"], "message");
    assert!(
        msgs[1]["speech"]["brief"]
            .as_str()
            .unwrap()
            .contains("第二条")
    );
    // seq 递增；sync_done 在两条之间
    let seq = |v: &Value| v["seq"].as_u64().unwrap();
    let sync = events(&ls, "sync_done");
    assert_eq!(sync.len(), 1);
    assert!(seq(msgs[0]) < seq(sync[0]) && seq(sync[0]) < seq(msgs[1]));
    let last = ls.last().unwrap();
    assert_eq!(last["kind"], "result");
    assert_eq!(last["data"]["reason"], "interrupted");
    assert_eq!(last["data"]["alerts"], 1);
    // 没给 --chat：不开任何会话
    assert!(!b.calls().iter().any(|c| matches!(c, Call::ChatOpen(_))));
}

#[test]
fn spoiler_hidden_unless_reveal() {
    // 暗号用汉字（#79）：整行里有真实时间戳，用数字当暗号会偶尔撞上时间
    let spoiler = json!({"@type": "formattedText", "text": "答案是丙丁，别说出去", "entities": [
        {"@type": "textEntity", "offset": 3, "length": 2, "type": {"@type": "textEntityTypeSpoiler"}}]});
    for reveal in [false, true] {
        let b = backend();
        b.push(chat(PEER, private(), "林小雨"));
        b.push_net("connectionStateReady");
        b.push(new_msg(PEER, 5, spoiler.clone()));
        let (_, ls) = run_watch(&b, Query { chat: None }, reveal, |v| {
            v["data"]["type"] == "new_message"
        });
        let m = events(&ls, "new_message")[0];
        // 只查消息本身与读法（#79）：不查整行，时间字段不归这条管
        let said = format!("{} {}", m["data"]["message"], m["speech"]);
        assert_eq!(said.contains("丙丁"), reveal, "{said}");
        assert!(m["speech"]["brief"].as_str().unwrap().contains("隐藏内容") != reveal);
    }
}

#[test]
fn chat_opened_for_typing_and_closed_at_end() {
    let b = Arc::new(backend());
    b.set_chat(
        PEER,
        vec![user(PEER, "林小雨"), chat(PEER, private(), "林小雨")],
    );
    b.push_net("connectionStateReady");
    // 会话开好之后再推一轮正在输入（真实里 TDLib 也是开了会话才推它的实时更新）
    let pusher = {
        let b = b.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !b.calls().contains(&Call::ChatOpen(PEER)) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            std::thread::sleep(Duration::from_millis(100));
            b.push(typing("chatActionTyping"));
            b.push(typing("chatActionTyping"));
            b.push(typing("chatActionCancel"));
        })
    };
    let stop: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    let lines = Arc::new(Mutex::new(Vec::new()));
    let out = Cap {
        lines: lines.clone(),
        stop,
        until: |v| v["data"]["action"] == "cancel",
    };
    let q = Query {
        chat: Some(ChatRef::Id(PEER)),
    };
    let p = profile("typing");
    let code = run(
        &*b,
        &p,
        &q,
        cx(false),
        Format::Json,
        stop,
        Box::new(out),
        clock,
        Instant::now(),
    );
    pusher.join().unwrap();
    let ls: Vec<Value> = lines
        .lock()
        .unwrap()
        .iter()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(code, 0);
    let acts = events(&ls, "chat_action");
    assert_eq!(acts.len(), 3, "{ls:?}");
    assert_eq!(acts[0]["data"]["notify"]["alert"], true);
    assert_eq!(acts[0]["data"]["notify"]["reason"], "typing");
    assert_eq!(acts[0]["speech"]["brief"], TYPING_ZH);
    assert_eq!(acts[1]["data"]["notify"]["reason"], "already_announced");
    assert!(acts[1].get("speech").is_none());
    assert!(acts[2]["data"].get("notify").is_none());
    let calls = b.calls();
    let open = calls.iter().position(|c| *c == Call::ChatOpen(PEER));
    let close = calls.iter().position(|c| *c == Call::ChatClose(PEER));
    assert!(
        open.is_some() && close.is_some() && open < close,
        "{calls:?}"
    );
}

/// 论坛群里一条话题消息（话题 7）
fn forum_setup(b: &FakeBackend) {
    b.push(chat(
        GROUP,
        json!({"@type": "chatTypeSupergroup", "supergroup_id": 1, "is_channel": false}),
        "周末爬山群",
    ));
    b.push_net("connectionStateReady");
}

fn in_topic_7(id: i64) -> Value {
    let mut m = new_msg(GROUP, id, plain("话题里"));
    m["message"]["topic_id"] = json!({"@type": "messageTopicForum", "forum_topic_id": 7});
    m
}

/// 取话题设置一直不应答（#73）：最多等 TOPIC_TIMEOUT，之后照常往下走，这个话题本次不再取
#[test]
fn hung_forum_topic_gives_up_after_deadline() {
    let b = Arc::new(backend());
    forum_setup(&b);
    b.fail_next(
        "forum_topic_get",
        CoreError::Internal {
            what: FakeBackend::HANG,
        },
    );
    b.push(in_topic_7(1));
    let pusher = {
        let b = b.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !b.calls().contains(&Call::ForumTopicGet(GROUP, 7)) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            b.push(in_topic_7(2));
        })
    };
    let t0 = Instant::now();
    let (code, ls) = run_watch(&b, Query { chat: None }, false, |v| {
        v["data"]["message_id"] == 2
    });
    pusher.join().unwrap();
    assert_eq!(code, 0);
    let took = t0.elapsed();
    assert!(took >= TOPIC_TIMEOUT, "{took:?}");
    assert!(took < TOPIC_TIMEOUT + Duration::from_secs(5), "{took:?}");
    let msgs = events(&ls, "new_message");
    assert_eq!(msgs[1]["data"]["notify"]["reason"], "topic_unknown");
    let gets = b
        .calls()
        .iter()
        .filter(|c| **c == Call::ForumTopicGet(GROUP, 7))
        .count();
    assert_eq!(gets, 1);
}

/// 取话题设置时按了 Ctrl+C：不等到期限，直接回主循环收尾
#[test]
fn stop_while_forum_topic_hangs_returns_at_once() {
    let b = backend();
    let p = profile("topic-stop");
    let mut stream = b.subscribe().unwrap();
    block_on(b.start(&p)).unwrap();
    forum_setup(&b);
    b.fail_next(
        "forum_topic_get",
        CoreError::Internal {
            what: FakeBackend::HANG,
        },
    );
    let mut feed = Feed::new(Instant::now());
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    feed.hub.add(Sub {
        out: Box::new(Cap {
            lines: lines.clone(),
            stop: Box::leak(Box::new(AtomicBool::new(false))),
            until: |_| false,
        }),
        watcher: Watcher::new(cx(false), Format::Json, None, clock),
        chat: None,
        dead: false,
    });
    b.push(in_topic_7(1));
    // 喂到这条话题消息被判过（判的时候记下要取的话题）
    let t0 = Instant::now();
    while !lines
        .lock()
        .unwrap()
        .iter()
        .any(|l| l.contains("new_message"))
        && t0.elapsed() < Duration::from_secs(5)
    {
        feed.drain(&mut stream);
        std::thread::sleep(Duration::from_millis(10));
    }
    // 等假后端记下取话题的请求（已交出、卡住）再立停止标志，不靠固定延时
    let stop = AtomicBool::new(false);
    let t1 = Instant::now();
    std::thread::scope(|s| {
        s.spawn(|| {
            while !b.calls().contains(&Call::ForumTopicGet(GROUP, 7))
                && t1.elapsed() < Duration::from_secs(5)
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            stop.store(true, Ordering::SeqCst);
        });
        fetch_topics(&b, &mut feed, &stop);
    });
    assert!(t1.elapsed() < TOPIC_TIMEOUT, "{:?}", t1.elapsed());
    assert!(
        b.calls().contains(&Call::ForumTopicGet(GROUP, 7)),
        "确实去取了、卡在等应答"
    );
    let _ = b.shutdown(Duration::from_secs(2));
}

#[test]
fn unknown_forum_topic_is_fetched_then_judged_by_topic() {
    let b = Arc::new(backend());
    b.push(chat(
        GROUP,
        json!({"@type": "chatTypeSupergroup", "supergroup_id": 1, "is_channel": false}),
        "周末爬山群",
    ));
    b.set_forum_topic(GROUP, 7, json!({"@type": "forumTopic", "last_read_inbox_message_id": 0,
        "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": false, "mute_for": 3600}}));
    b.push_net("connectionStateReady");
    let in_topic = |id: i64| {
        let mut m = new_msg(GROUP, id, plain("第一条"));
        m["message"]["topic_id"] = json!({"@type": "messageTopicForum", "forum_topic_id": 7});
        m
    };
    b.push(in_topic(1));
    // 取过话题之后再来一条
    let pusher = {
        let b = b.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !b.calls().contains(&Call::ForumTopicGet(GROUP, 7)) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            std::thread::sleep(Duration::from_millis(100));
            let mut m = new_msg(GROUP, 2, plain("第二条"));
            m["message"]["topic_id"] = json!({"@type": "messageTopicForum", "forum_topic_id": 7});
            b.push(m);
        })
    };
    let (code, ls) = run_watch(&b, Query { chat: None }, false, |v| {
        v["data"]["message_id"] == 2
    });
    pusher.join().unwrap();
    assert_eq!(code, 0);
    let msgs = events(&ls, "new_message");
    assert_eq!(msgs[0]["data"]["notify"]["reason"], "topic_unknown");
    assert_eq!(msgs[1]["data"]["notify"]["reason"], "muted_topic");
    assert_eq!(
        b.calls()
            .iter()
            .filter(|c| **c == Call::ForumTopicGet(GROUP, 7))
            .count(),
        1
    );
}

#[test]
fn feed_tracks_sync_from_what_it_fed_not_live_state() {
    // 中途接入判「已补发完」按 Feed 喂到的：连接线到 ready 但事件还没喂时不算
    let b = backend();
    let p = profile("feedsync");
    let mut stream = b.subscribe().unwrap();
    block_on(b.start(&p)).unwrap();
    let mut feed = Feed::new(Instant::now());
    b.push_net("connectionStateReady");
    chats::ensure_ready(&b).unwrap_or_else(|_| panic!("没登录"));
    let _ = b.wait_net(Duration::from_secs(5), &|n| {
        *n == atg_core::status::NetState::Ready
    });
    assert!(!feed.synced(), "还没喂就不算");
    feed.drain(&mut stream);
    assert!(feed.synced());
    b.push_net("connectionStateUpdating");
    let _ = b.wait_net(Duration::from_secs(5), &|n| {
        *n == atg_core::status::NetState::Updating
    });
    feed.drain(&mut stream);
    assert!(!feed.synced(), "重连补发中");
    let _ = b.shutdown(SHUTDOWN_TIMEOUT);
}

#[test]
fn reconnect_summary_line() {
    let b = backend();
    b.push(chat(PEER, private(), "林小雨"));
    b.push_net("connectionStateReady");
    b.push_net("connectionStateConnecting");
    b.push(new_msg(PEER, 1, plain("第一条")));
    b.push(new_msg(PEER, 2, plain("第二条")));
    b.push_net("connectionStateReady");
    let (code, ls) = run_watch(&b, Query { chat: None }, false, |v| {
        v["data"]["type"] == "sync_done" && v["data"]["missed"].is_object()
    });
    assert_eq!(code, 0);
    let syncs = events(&ls, "sync_done");
    assert_eq!(syncs.len(), 2);
    assert!(syncs[0]["data"].get("notify").is_none(), "启动那次不汇总");
    let s = syncs[1];
    assert_eq!(s["data"]["missed"], json!({"messages": 2, "chats": 1}));
    assert_eq!(s["data"]["notify"]["reason"], "reconnect_summary");
    assert_eq!(s["data"]["notify"]["sound"], true);
    assert_eq!(s["speech"]["brief"], missed_zh(2, 1));
    // 补发来的两条逐条不提示
    for m in events(&ls, "new_message") {
        assert_eq!(m["data"]["notify"]["reason"], "backlog");
    }
    assert_eq!(ls.last().unwrap()["data"]["alerts"], 1);
}

#[test]
fn text_mode_summary_line_counts_as_alert() {
    use atg_core::status::NetState;
    use atg_core::store::Input;
    let mut w = Watcher::new(cx(false), Format::Text, None, clock);
    let mut store = Store::new();
    let mut seq = 0;
    let mut feed = |w: &mut Watcher, store: &mut Store, e: CoreEvent| {
        seq += 1;
        store.apply(Input::Update {
            seq,
            event: e.clone(),
        });
        w.render(store, seq, &e)
    };
    let ev = |v: Value| atg_core::tdjson::event(&v);
    feed(&mut w, &mut store, ev(chat(PEER, private(), "林小雨")));
    // 启动时的第一次补发结束：只出补发结束那句，不算要提示的
    let first = feed(&mut w, &mut store, CoreEvent::SyncDone);
    assert_eq!(first.as_deref(), Some(SYNC_DONE_ZH));
    // 断网、补发来两条（逐条不提示）、补发结束：出汇总那一行，算一条要提示的
    feed(
        &mut w,
        &mut store,
        CoreEvent::NetStateChanged(NetState::Connecting),
    );
    feed(&mut w, &mut store, ev(new_msg(PEER, 1, plain("第一条"))));
    feed(&mut w, &mut store, ev(new_msg(PEER, 2, plain("第二条"))));
    let line = feed(&mut w, &mut store, CoreEvent::SyncDone).expect("要出汇总行");
    assert_eq!(line, format!("{}。{SYNC_DONE_ZH}", missed_zh(2, 1)));
    assert_eq!(w.alerts, 1);
    // 转发那头按同一口径数
    let (mut events, mut alerts) = (0, 0);
    crate::forward::count_line(Format::Text, &line, &mut events, &mut alerts);
    assert_eq!((events, alerts), (1, 1));
}

#[test]
fn missing_chat_is_not_found_and_nothing_opened() {
    let b = backend();
    b.push_net("connectionStateReady");
    let (code, ls) = run_watch(
        &b,
        Query {
            chat: Some(ChatRef::Id(GROUP)),
        },
        false,
        |_| false,
    );
    assert_eq!(code, 6);
    assert_eq!(ls.last().unwrap()["data"]["code"], "not_found");
    assert!(!b.calls().iter().any(|c| matches!(c, Call::ChatOpen(_))));
}

#[test]
fn not_logged_in_is_auth_required() {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth::default());
    let (code, ls) = run_watch(&b, Query { chat: None }, false, |_| false);
    assert_eq!(code, 3);
    assert_eq!(ls.len(), 1);
    assert_eq!(ls[0]["data"]["code"], "auth_required");
}

#[test]
fn backend_fault_ends_with_disconnected() {
    let b = backend();
    b.push_net("connectionStateReady");
    b.crash();
    let (code, ls) = run_watch(&b, Query { chat: None }, false, |_| false);
    assert_eq!(code, 4);
    assert_eq!(ls.last().unwrap()["data"]["code"], "disconnected");
}

#[test]
fn event_names_match_contract_table() {
    // 契约表里的事件名（actions.md 第一节第 7 小节，本 issue 新增的三个也在内）
    let contract = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../specs/contracts/actions.md"
    ))
    .unwrap();
    let samples = [
        CoreEvent::SyncDone,
        CoreEvent::ChatAction {
            chat_id: 1,
            topic: None,
            sender: SenderId::User(1),
            action: ChatActionKind::Typing,
        },
        CoreEvent::ForumTopicChanged {
            chat_id: 1,
            topic_id: 1,
            last_read_inbox: 0,
            notify: ChatNotify::default(),
            received_ms: None,
        },
        CoreEvent::Unknown {
            td_type: "x".into(),
        },
    ];
    for e in &samples {
        let n = event_name(e);
        assert!(contract.contains(&format!("| `{n}`")), "{n} 不在契约表里");
    }
    let _ = GROUP;
}
