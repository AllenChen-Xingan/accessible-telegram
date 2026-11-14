//! 假后端：数据全是编的，不碰真实账号与本机档案目录。
use super::*;
use atg_core::config::{Profile, ServerKind};
use atg_core::fake::{FakeAuth, FakeBackend};
use atg_core::model::{Message, SenderId};
use atg_core::store::Input;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Default)]
struct Cap {
    out: Vec<String>,
}

impl Sink for Cap {
    fn out(&mut self, line: &str) {
        self.out.push(line.to_owned());
    }
    fn err(&mut self, _: &str) {}
}

const ME: i64 = 4242;

fn profile() -> Profile {
    static N: AtomicU32 = AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "atg-cli-pending-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&root);
    Profile::create(&root, "p", ServerKind::Test).unwrap()
}

fn backend() -> FakeBackend {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth {
        phone: "+9996612345".into(),
        logged_in: true,
        ..FakeAuth::default()
    });
    b.set_me(ME);
    b
}

/// TDLib 形状的一条自己发的消息
fn own(id: i64, state: Value) -> Value {
    json!({"@type": "message", "id": id, "chat_id": ME,
        "sender_id": {"@type": "messageSenderUser", "user_id": ME},
        "is_outgoing": true, "date": 1_792_755_420,
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": "atg 测试 待发", "entities": []}},
        "sending_state": state})
}

fn run(b: &FakeBackend, format: Format) -> (u8, Cap) {
    let p = profile();
    history::with_backend(b, &p, Instant::now(), |b, st, fe| {
        let mut cap = Cap::default();
        let code = execute_list(
            b,
            st,
            fe,
            &PendingParams {
                tz: Some("+08:00".into()),
            },
            &mut cap,
            format,
            Instant::now(),
        );
        Ok((code, cap))
    })
    .unwrap_or_else(|_| panic!("起不来"))
}

fn line(cap: &Cap, i: usize) -> Value {
    serde_json::from_str(&cap.out[i]).unwrap()
}

#[test]
fn lists_replayed_pending_and_skips_sent() {
    let b = backend();
    // TDLib 启动时续发上次留下的：推一条「待发」新消息；另有一条早已发出的不算
    b.push(json!({"@type": "updateNewMessage",
        "message": own(1_048_577, json!({"@type": "messageSendingStatePending", "sending_id": 0}))}));
    b.push(json!({"@type": "updateNewMessage", "message": own(9, Value::Null)}));
    b.push_net("connectionStateReady");
    let (code, cap) = run(&b, Format::Json);
    assert_eq!(code, 0);
    assert_eq!(cap.out.len(), 2, "{:?}", cap.out);
    let m = line(&cap, 0);
    assert_eq!(m["kind"], "message");
    assert_eq!(m["data"]["id"]["message_id"], 1_048_577);
    let r = line(&cap, 1);
    assert_eq!(r["kind"], "result");
    assert_eq!(r["data"]["count"], 1);
    assert_eq!(r["data"]["pending_sends"][0]["chat_id"], ME);
    assert_eq!(r["data"]["pending_sends"][0]["temp_message_id"], 1_048_577);
    assert_eq!(
        r["data"]["pending_sends"][0]["date"],
        "2026-10-23T11:37:00Z"
    );
    assert_eq!(r["data"]["synced"], true);
    assert_eq!(r["data"]["resolved"], json!([]));
}

#[test]
fn empty_queue_text() {
    let b = backend();
    b.push_net("connectionStateReady");
    let (code, cap) = run(&b, Format::Text);
    assert_eq!(code, 0);
    assert_eq!(cap.out, vec!["没有待发出的消息".to_owned()]);
}

#[test]
fn not_logged_in_is_auth_required() {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth {
        logged_in: false,
        ..FakeAuth::default()
    });
    let (code, cap) = run(&b, Format::Json);
    assert_ne!(code, 0);
    assert_eq!(line(&cap, 0)["data"]["code"], "auth_required");
}

fn pending_msg(id: i64) -> Message {
    let mut m = atg_core::tdjson::message(&own(
        id,
        json!({"@type": "messageSendingStatePending", "sending_id": 0}),
    ));
    m.sender = SenderId::User(ME);
    m
}

#[test]
fn fate_follows_the_id_swap() {
    use atg_core::model::CoreEvent;
    let mut s = Store::new();
    let mut seq = 0;
    let mut feed = |s: &mut Store, event: CoreEvent| {
        seq += 1;
        s.apply(Input::Update { seq, event });
    };
    let (a, b, c) = (pending_msg(1), pending_msg(2), pending_msg(3));
    for m in [&a, &b, &c] {
        feed(&mut s, CoreEvent::NewMessage(m.clone()));
    }
    let mut sent = a.clone();
    sent.id.message_id = 50;
    sent.sending = SendingState::Sent;
    feed(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: a.id,
            message: sent,
        },
    );
    let mut failed = b.clone();
    failed.id.message_id = 4;
    failed.sending = SendingState::Failed;
    feed(
        &mut s,
        CoreEvent::SendFailed {
            old_id: b.id,
            message: failed,
            error: atg_core::error::map_td_error(400, ""),
            can_retry: true,
        },
    );
    feed(
        &mut s,
        CoreEvent::MessagesDeleted {
            chat_id: ME,
            message_ids: vec![3],
            is_permanent: true,
            from_cache: false,
        },
    );
    let f = |id| fate(&s, id).unwrap();
    assert_eq!(f(a.id)["outcome"], "succeeded");
    assert_eq!(f(a.id)["message_id"], 50);
    assert_eq!(f(b.id)["outcome"], "failed");
    assert_eq!(f(c.id)["outcome"], "deleted");
    assert_eq!(f(c.id)["message_id"], Value::Null);
}

// ───── cancel ─────

fn cancel_run(b: &FakeBackend, chat: &str, dry_run: bool) -> (u8, Cap) {
    let p = profile();
    history::with_backend(b, &p, Instant::now(), |b, st, fe| {
        let mut cap = Cap::default();
        let params = CancelParams {
            chat: chat.into(),
            dry_run,
        };
        let code = execute_cancel(b, st, fe, &params, &mut cap, Format::Json, Instant::now());
        Ok((code, cap))
    })
    .unwrap_or_else(|_| panic!("起不来"))
}

/// 收藏夹里有这几条待发（续发时 TDLib 推的新消息 + `getMessage` 查得到的同一份）和一条已发出的
fn saved_backend_with(pending: &[i64]) -> FakeBackend {
    let b = backend();
    b.set_chat(
        ME,
        vec![json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": ME, "title": "我自己",
            "type": {"@type": "chatTypePrivate", "user_id": ME}, "positions": [], "notification_settings": {}}})],
    );
    let st = || json!({"@type": "messageSendingStatePending", "sending_id": 0});
    let mut hist: Vec<Value> = pending.iter().map(|&id| own(id, st())).collect();
    hist.push(own(9, Value::Null));
    b.set_history(ME, hist);
    for &id in pending {
        b.push(json!({"@type": "updateNewMessage", "message": own(id, st())}));
    }
    b.push(json!({"@type": "updateNewMessage", "message": own(9, Value::Null)}));
    b.push_net("connectionStateReady");
    b
}

fn saved_backend() -> FakeBackend {
    saved_backend_with(&[1_048_577])
}

#[test]
fn cancel_saved_on_test_server() {
    let b = saved_backend();
    let (code, cap) = cancel_run(&b, "saved", false);
    assert_eq!(code, 0, "{:?}", cap.out);
    let plan = line(&cap, 0);
    assert_eq!(plan["kind"], "plan");
    assert_eq!(plan["data"]["action"], "pending_cancel");
    assert_eq!(plan["data"]["confirm"], "test_exempt");
    assert_eq!(plan["data"]["items"][0]["text"], "atg 测试 待发");
    assert_eq!(plan["data"]["items"][0]["temp_message_id"], 1_048_577);
    assert_eq!(
        plan["data"]["items"].as_array().unwrap().len(),
        1,
        "已发出的那条不在计划里"
    );
    let r = line(&cap, cap.out.len() - 1);
    assert_eq!(r["kind"], "result");
    assert_eq!(r["data"]["items"][0]["outcome"], "cancelled");
    assert_eq!(r["data"]["all_cancelled"], true);
    assert!(r["data"]["note_zh"].as_str().unwrap().contains("尽力"));
}

/// 一次撤几条：TDLib 只推一条 `updateDeleteMessages`，每条都要认出来（自审第 1 轮 P1）
#[test]
fn cancel_several_in_one_go() {
    let b = saved_backend_with(&[1_048_577, 1_048_578]);
    let t0 = Instant::now();
    let (code, cap) = cancel_run(&b, "saved", false);
    assert_eq!(code, 0, "{:?}", cap.out);
    let r = line(&cap, cap.out.len() - 1);
    assert_eq!(r["data"]["items"][0]["outcome"], "cancelled");
    assert_eq!(r["data"]["items"][1]["outcome"], "cancelled");
    assert_eq!(r["data"]["all_cancelled"], true);
    assert!(t0.elapsed() < CANCEL_WAIT, "不该等到超时");
}

#[test]
fn text_list_gives_ids_for_cancel() {
    let b = backend();
    b.push(json!({"@type": "updateNewMessage",
        "message": own(1_048_577, json!({"@type": "messageSendingStatePending", "sending_id": 0}))}));
    b.push_net("connectionStateReady");
    let (_, cap) = run(&b, Format::Text);
    assert!(
        cap.out[0].starts_with(&format!("会话 {ME} · 临时 id 1048577：")),
        "{:?}",
        cap.out
    );
}

#[test]
fn cancel_dry_run_deletes_nothing() {
    let b = saved_backend();
    let (code, cap) = cancel_run(&b, "saved", true);
    assert_eq!(code, 0);
    assert_eq!(line(&cap, 1)["data"]["dry_run"], true);
    assert!(
        !b.calls()
            .iter()
            .any(|c| matches!(c, atg_core::fake::Call::Delete { .. }))
    );
}

#[test]
fn nothing_pending_is_refused() {
    let b = saved_backend_with(&[]);
    let (code, cap) = cancel_run(&b, "saved", false);
    assert_ne!(code, 0);
    let e = line(&cap, 0);
    assert_eq!(e["data"]["code"], "invalid_input");
    assert_eq!(e["data"]["why"], "not_pending");
    assert_eq!(e["data"]["retryable"], false);
}

#[test]
fn execute_rejects_bad_params() {
    let b = saved_backend();
    let p = profile();
    let code = history::with_backend(&b, &p, Instant::now(), |b, st, fe| {
        let mut cap = Cap::default();
        Ok(execute(
            b,
            st,
            fe,
            "pending_cancel",
            json!({"chat": 1}),
            &mut cap,
            Format::Json,
            Instant::now(),
        ))
    })
    .unwrap_or_else(|_| panic!("起不来"));
    assert_ne!(code, 0);
}

/// 没赶上、已发出：删除被 TDLib 换成正式 id、和发出成功同一批到（自审第 4 轮 P1）——按 store 判，不靠第二次等待
#[test]
fn raced_cancel_is_sent_deleted_here() {
    use atg_core::model::CoreEvent;
    let p = pending_msg(1_048_577);
    let mut sent = p.clone();
    sent.id.message_id = 50;
    sent.sending = SendingState::Sent;
    let w = Waited::Succeeded(sent.clone());
    let mut s = Store::new();
    s.apply(Input::Update {
        seq: 1,
        event: CoreEvent::NewMessage(p.clone()),
    });
    s.apply(Input::Update {
        seq: 2,
        event: CoreEvent::SendSucceeded {
            old_id: p.id,
            message: sent.clone(),
        },
    });
    assert_eq!(cancel_outcome(&s, &w), ("already_sent", Some(50)));
    s.apply(Input::Update {
        seq: 3,
        event: CoreEvent::MessagesDeleted {
            chat_id: ME,
            message_ids: vec![50],
            is_permanent: true,
            from_cache: false,
        },
    });
    assert_eq!(cancel_outcome(&s, &w), ("sent_deleted_here", Some(50)));
    let gone = Waited::Unknown(atg_core::error::UnknownWhy::PendingDeleted);
    assert_eq!(cancel_outcome(&s, &gone), ("cancelled", None));
}
