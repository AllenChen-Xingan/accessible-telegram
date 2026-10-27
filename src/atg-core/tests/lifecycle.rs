//! 接 TDLib 的接线层，用假后端测（`testing.md` 第一节第 2、4 小节「收线程监督」）：
//! 收线程序号、三线合成、故障广播（等待中的请求以 `disconnected` 返回、之后的新请求被拒）、关闭流程、
//! 事件 → reducer 的批处理、重拉接线、重发应答喂法、日志里没有内容。
//! 假后端与真实后端共用同一个收线程运行器（`link`），所以这里测到的就是正式的那份。

use atg_core::backend::Backend;
use atg_core::config::{Profile, ServerKind};
use atg_core::error::CoreError;
use atg_core::fake::{Call, FakeBackend, FakeStart};
use atg_core::link::{Envelope, EventStream, block_on};
use atg_core::model::{CoreEvent, MsgId};
use atg_core::pump::{Pump, resent_inputs};
use atg_core::status::{AuthStep, CoreState};
use atg_core::store::Input;
use atg_core::tree::UserStatus;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

fn temp_root(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "atg-test-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn test_profile(tag: &str) -> Profile {
    Profile::create(&temp_root(tag), "test", ServerKind::Test).unwrap()
}

/// 把流里的东西取到流结束（故障 / 关闭后流会结束）
fn drain(stream: &mut EventStream) -> Vec<Envelope> {
    let mut all = Vec::new();
    while let Some(b) = stream.blocking_batch(64) {
        all.extend(b);
    }
    all
}

/// 取够 `n` 件（不等流结束）
fn take(stream: &mut EventStream, n: usize) -> Vec<Envelope> {
    let mut all = Vec::new();
    while all.len() < n {
        match stream.blocking_batch(n - all.len()) {
            Some(b) => all.extend(b),
            None => break,
        }
    }
    all
}

fn seqs_strictly_increase(evs: &[Envelope]) {
    for w in evs.windows(2) {
        assert!(w[0].seq < w[1].seq, "序号没有严格递增");
    }
}

#[test]
fn start_then_auth_steps_flow_with_increasing_seq() {
    let b = FakeBackend::new().unwrap();
    let mut stream = b.subscribe().unwrap();
    block_on(b.start(&test_profile("flow"))).unwrap();
    b.push_auth("authorizationStateWaitPhoneNumber");
    b.push(json!({"@type": "updateConnectionState", "state": {"@type": "connectionStateReady"}}));

    // start → 内核 running（状态仍是 connecting：授权还在等参数）
    // 授权 → wait_phone：AuthStepChanged + UserStatusChanged(signed_out)；连接 → ready：NetStateChanged
    let evs = take(&mut stream, 4);
    seqs_strictly_increase(&evs);
    let kinds: Vec<_> = evs.iter().map(|e| &e.event).collect();
    assert!(matches!(
        kinds[0],
        CoreEvent::CoreStateChanged(CoreState::Running)
    ));
    assert!(matches!(
        kinds[1],
        CoreEvent::AuthStepChanged(AuthStep::WaitPhone)
    ));
    assert!(matches!(
        kinds[2],
        CoreEvent::UserStatusChanged(UserStatus::SignedOut)
    ));
    assert!(matches!(kinds[3], CoreEvent::NetStateChanged(_)));
    assert_eq!(b.auth_state(), AuthStep::WaitPhone);
    assert_eq!(b.status().status, UserStatus::SignedOut);
    assert_eq!(
        b.calls()[1],
        Call::Start {
            profile: "test".into(),
            use_test_dc: true
        }
    );
}

#[test]
fn only_one_subscriber() {
    let b = FakeBackend::new().unwrap();
    let _s = b.subscribe().unwrap();
    assert_eq!(b.subscribe().err().map(|e| e.code()), Some("internal"));
}

#[test]
fn crash_fails_waiting_request_and_rejects_new_ones() {
    let b = FakeBackend::new().unwrap();
    let mut stream = b.subscribe().unwrap();
    b.set_start(FakeStart::Hang);
    let profile = test_profile("crash");
    std::thread::scope(|s| {
        let waiting = s.spawn(|| block_on(b.start(&profile)));
        // 让请求先挂上，再崩
        std::thread::sleep(Duration::from_millis(50));
        b.crash();
        assert_eq!(waiting.join().unwrap(), Err(CoreError::Disconnected));
    });
    b.join_receiver();
    // 之后的新请求直接拒（start 的应答本可以成功）
    b.set_start(FakeStart::Ok);
    assert_eq!(block_on(b.start(&profile)), Err(CoreError::Disconnected));
    assert_eq!(
        b.shutdown(Duration::from_secs(1)),
        Err(CoreError::Disconnected)
    );
    assert_eq!(b.status().status, UserStatus::Fault);
    assert_eq!(b.status().lines.core, CoreState::Faulted);
    // 流里有故障的那两件，然后结束
    let evs = drain(&mut stream);
    seqs_strictly_increase(&evs);
    assert!(
        evs.iter()
            .any(|e| matches!(e.event, CoreEvent::CoreStateChanged(CoreState::Faulted)))
    );
    assert!(matches!(
        evs.last().map(|e| &e.event),
        Some(CoreEvent::UserStatusChanged(UserStatus::Fault))
    ));
}

#[test]
fn source_ending_without_closed_is_a_fault() {
    let b = FakeBackend::new().unwrap();
    b.end();
    b.join_receiver();
    assert_eq!(b.status().status, UserStatus::Fault);
}

#[test]
fn start_error_is_returned_as_is() {
    let b = FakeBackend::new().unwrap();
    b.set_start(FakeStart::Fail(atg_core::error::map_td_error(
        401,
        "Wrong database encryption key",
    )));
    let r = block_on(b.start(&test_profile("dbkey")));
    assert_eq!(r.err().map(|e| e.code()), Some("db_key_invalid"));
}

#[test]
fn shutdown_waits_for_closed_and_ends_stream() {
    let b = FakeBackend::new().unwrap();
    let mut stream = b.subscribe().unwrap();
    block_on(b.start(&test_profile("close"))).unwrap();
    b.push_auth("authorizationStateWaitPhoneNumber");
    assert_eq!(b.shutdown(Duration::from_secs(5)), Ok(()));
    b.join_receiver();
    let st = b.status();
    assert_eq!(st.lines.core, CoreState::Closed);
    assert_eq!(st.lines.auth, AuthStep::Closed);
    assert_eq!(st.status, UserStatus::Stopping);
    let evs = drain(&mut stream);
    seqs_strictly_increase(&evs);
    // 关闭不是故障
    assert!(
        !evs.iter()
            .any(|e| matches!(e.event, CoreEvent::UserStatusChanged(UserStatus::Fault)))
    );
}

#[test]
fn shutdown_times_out_when_closed_never_comes() {
    let b = FakeBackend::new().unwrap();
    block_on(b.start(&test_profile("hang"))).unwrap();
    b.set_close_hangs(true);
    let r = b.shutdown(Duration::from_millis(100));
    assert_eq!(
        r,
        Err(CoreError::Internal {
            what: "close_timeout"
        })
    );
}

#[test]
fn updates_reach_the_store_through_pump() {
    let b = FakeBackend::new().unwrap();
    let mut stream = b.subscribe().unwrap();
    b.push(json!({
        "@type": "updateNewChat",
        "chat": {"@type": "chat", "id": 777, "title": "林小雨", "type": {"@type": "chatTypePrivate", "user_id": 777},
                 "positions": [{"@type": "chatPosition", "list": {"@type": "chatListMain"}, "order": "100", "is_pinned": false}]}
    }));
    b.push(json!({
        "@type": "updateNewMessage",
        "message": {"@type": "message", "id": 5, "chat_id": 777, "date": 1792755420,
                    "sender_id": {"@type": "messageSenderUser", "user_id": 777},
                    "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": "晚上好", "entities": []}}}
    }));
    b.push(json!({
        "@type": "updateChatDraftMessage", "chat_id": 777,
        "positions": [{"@type": "chatPosition", "list": {"@type": "chatListMain"}, "order": "300", "is_pinned": false}]
    }));
    let mut pump = Pump::default();
    let evs = take(&mut stream, 3);
    assert!(matches!(evs[2].event, CoreEvent::ChatDraftChanged { .. }));
    let out = pump.apply_batch(evs);
    assert_eq!(out.applied, 3);
    assert_eq!(out.refetch, None);
    let s = pump.store();
    assert!(
        s.message(MsgId {
            chat_id: 777,
            message_id: 5
        })
        .is_some()
    );
    assert_eq!(s.chat(777).map(|c| c.positions[0].order), Some(300));
}

#[test]
fn stale_window_triggers_refetch_with_fresh_ticket() {
    use atg_core::model::*;
    use atg_core::store::{Store, StoreConfig};
    // 非当前会话只留 1 条：1 被挤掉后，早于挤掉的旧应答里的 1 不收，窗口标「要重拉」
    let store = Store::with_config(StoreConfig { keep_per_chat: 1 });
    let mut pump = Pump::new(store);
    let msg = |id| Message {
        id: MsgId {
            chat_id: 9,
            message_id: id,
        },
        sender: SenderId::User(9),
        date: Time(1),
        outgoing: false,
        reply_to: None,
        forward: None,
        album_id: None,
        topic: None,
        content: Body::Content(Content {
            body: vec![],
            attachment: None,
            sensitive: false,
        }),
        ephemeral: None,
        restriction: None,
        sensitive: false,
        edited: None,
        reactions: vec![],
        sending: SendingState::Sent,
        mentions_me: false,
    };
    let up = |seq, event| Envelope { seq, event };
    let t = pump.store_mut().ticket(9);
    pump.apply_batch(vec![
        up(1, CoreEvent::NewMessage(msg(1))),
        up(2, CoreEvent::NewMessage(msg(2))),
    ]);
    // 打开会话，再把旧应答（早于挤掉）喂进来
    assert_eq!(pump.apply(Input::OpenChat { chat_id: 9 }), None);
    let t_old = atg_core::store::HistoryTicket {
        generation: pump.store().window().map(|w| w.generation).unwrap(),
        ..t
    };
    let refetch = pump.apply(Input::HistoryLoaded {
        ticket: t_old,
        messages: vec![msg(1), msg(2)],
    });
    let ticket = refetch.expect("要重拉");
    assert_eq!(ticket.chat_id, 9);
    assert!(!pump.store().needs_refetch(), "Refetch 已发");
    assert_eq!(
        ticket.generation,
        pump.store().window().map(|w| w.generation).unwrap()
    );
}

#[test]
fn resent_answers_skip_nulls_and_keep_order() {
    let inputs = resent_inputs(7, &[10, 11, 12], &[Some(20), None, Some(22)]);
    assert_eq!(inputs.len(), 2);
    let pair = |i: &Input| match i {
        Input::Resent { old, new } => (old.message_id, new.message_id, old.chat_id),
        _ => panic!("不是 Resent"),
    };
    assert_eq!(pair(&inputs[0]), (10, 20, 7));
    assert_eq!(pair(&inputs[1]), (12, 22, 7));
    // 条数对不上：只配对得上的
    assert_eq!(resent_inputs(7, &[1, 2], &[Some(3)]).len(), 1);
}

// ───── 日志里没有内容 ─────

struct Capture(Mutex<Vec<String>>);
impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, r: &log::Record) {
        self.0.lock().unwrap().push(r.args().to_string());
    }
    fn flush(&self) {}
}
static CAPTURE: Capture = Capture(Mutex::new(Vec::new()));

const SECRET: &str = "暗号-FAKE-SECRET-占位";

#[test]
fn nothing_from_updates_reaches_the_log() {
    let _ = log::set_logger(&CAPTURE);
    log::set_max_level(log::LevelFilter::Trace);
    let b = FakeBackend::new().unwrap();
    let mut stream = b.subscribe().unwrap();
    // 不认识的更新、类型错位的更新、带暗号的连接状态类型名、普通消息
    b.push(json!({"@type": "updateFutureThing", "text": SECRET, "chat_id": SECRET}));
    b.push(json!({"@type": "updateNewMessage", "message": {"chat_id": SECRET, "content": {"@type": "messageText", "text": {"text": SECRET}}}}));
    b.push(json!({"@type": "updateConnectionState", "state": {"@type": SECRET}}));
    b.push(json!({"@type": "updateChatTitle", "chat_id": 1, "title": SECRET}));
    b.crash();
    b.join_receiver();
    let evs = drain(&mut stream);
    assert!(evs.len() >= 4, "暗号确实进过系统");
    let lines = CAPTURE.0.lock().unwrap().clone();
    assert!(
        lines.iter().any(|l| l.contains("core_fault")),
        "日志确实写过"
    );
    for l in &lines {
        assert!(!l.contains("暗号"), "日志漏了内容");
    }
    // 事件的 Debug 也漏不出（类型级脱敏）
    for e in &evs {
        assert!(!format!("{:?}", e.event).contains("暗号"));
    }
}

// ───── 起停前置（两份后端一致）与「内核自己先关了」 ─────

#[test]
fn shutdown_after_core_closed_itself_returns_promptly() {
    let b = FakeBackend::new().unwrap();
    block_on(b.start(&test_profile("selfclose"))).unwrap();
    // 别处退出登录：TDLib 自己走到 Closed，收线程正常退出
    b.push_auth("authorizationStateLoggingOut");
    b.push_auth("authorizationStateClosed");
    b.join_receiver();
    let t0 = std::time::Instant::now();
    assert_eq!(b.shutdown(Duration::from_secs(5)), Ok(()));
    assert!(t0.elapsed() < Duration::from_secs(1), "不该等到超时");
    assert!(!b.status().previous_unclean);
}

#[test]
fn request_pending_when_core_closes_returns_disconnected() {
    let b = FakeBackend::new().unwrap();
    b.set_start(FakeStart::Hang);
    let profile = test_profile("pendclose");
    std::thread::scope(|s| {
        let waiting = s.spawn(|| block_on(b.start(&profile)));
        std::thread::sleep(Duration::from_millis(50));
        b.push_auth("authorizationStateClosed");
        assert_eq!(waiting.join().unwrap(), Err(CoreError::Disconnected));
    });
    // 关了不是故障
    assert_eq!(b.status().lines.core, CoreState::Closed);
}

#[test]
fn second_backend_on_same_profile_is_busy() {
    let profile = test_profile("busy");
    let a = FakeBackend::new().unwrap();
    block_on(a.start(&profile)).unwrap();
    let b = FakeBackend::new().unwrap();
    assert_eq!(
        block_on(b.start(&profile)).err().map(|e| e.code()),
        Some("profile_busy")
    );
}

#[test]
fn start_twice_and_shutdown_before_start() {
    let b = FakeBackend::new().unwrap();
    assert_eq!(
        b.shutdown(Duration::from_secs(1)),
        Ok(()),
        "没起过就关：什么都不做"
    );
    let profile = test_profile("twice");
    block_on(b.start(&profile)).unwrap();
    assert_eq!(
        block_on(b.start(&profile)),
        Err(CoreError::Internal {
            what: "already_started"
        })
    );
}

#[test]
fn unclean_exit_is_reported_on_next_start() {
    let profile = test_profile("unclean");
    {
        let a = FakeBackend::new().unwrap();
        block_on(a.start(&profile)).unwrap();
        // 没调 shutdown 就丢掉：「运行中」标记留着
    }
    let b = FakeBackend::new().unwrap();
    block_on(b.start(&profile)).unwrap();
    assert!(b.status().previous_unclean);
    assert_eq!(b.shutdown(Duration::from_secs(5)), Ok(()));
    let c = FakeBackend::new().unwrap();
    block_on(c.start(&profile)).unwrap();
    assert!(!c.status().previous_unclean, "正常关闭后标记已拿掉");
}
