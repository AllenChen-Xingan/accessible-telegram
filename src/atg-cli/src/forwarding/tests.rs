//! 假后端：数据全是编的（名字取自 fixtures/names.txt），不碰真实账号与本机档案目录。
use super::*;
use atg_core::config::{Profile, ServerKind};
use atg_core::error::Holder;
use atg_core::fake::{FakeAuth, FakeBackend, FakeSend};
use atg_core::gate::Gate;
use serde_json::Value;
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
const FRIEND: i64 = 777;
const OTHER: i64 = 888;

fn chat(id: i64, title: &str) -> Value {
    json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title,
        "type": {"@type": "chatTypePrivate", "user_id": id}, "positions": [],
        "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0}}})
}

fn msg(id: i64, text: &str) -> Value {
    json!({"@type": "message", "id": id, "chat_id": FRIEND,
        "sender_id": {"@type": "messageSenderUser", "user_id": FRIEND}, "is_outgoing": false, "date": 1_792_755_000,
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": text, "entities": []}}})
}

fn backend() -> FakeBackend {
    let b = FakeBackend::with_gate(Gate::new(Holder::Cli)).unwrap();
    b.set_auth(FakeAuth {
        phone: "+15550001234".into(),
        logged_in: true,
        ..FakeAuth::default()
    });
    b.set_me(ME);
    b.set_chat(FRIEND, vec![chat(FRIEND, "林小雨")]);
    b.set_chat(OTHER, vec![chat(OTHER, "周大山")]);
    b.set_history(FRIEND, vec![msg(5, "第5条"), msg(7, "第7条")]);
    b.set_history(OTHER, vec![]);
    b.push_net("connectionStateReady");
    b
}

fn profile() -> Profile {
    static N: AtomicU32 = AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "atg-cli-forward-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&root);
    Profile::create(&root, "p", ServerKind::Main).unwrap()
}

fn fp(ids: &[i64], dry_run: bool, again: bool) -> ForwardParams {
    ForwardParams {
        from: FRIEND.to_string(),
        ids: ids.to_vec(),
        to: OTHER.to_string(),
        dry_run,
        again,
        tz: Some("+08:00".into()),
    }
}

fn lines(cap: &Cap) -> Vec<Value> {
    cap.out
        .iter()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// 在同一个后端上依次跑几次 forward
fn run(b: &FakeBackend, ps: &[ForwardParams]) -> Vec<(u8, Cap)> {
    crate::history::with_backend(b, &profile(), Instant::now(), |b, st, fe| {
        Ok(ps
            .iter()
            .map(|p| {
                let mut cap = Cap::default();
                let t = Duration::from_millis(800);
                let code = execute(b, st, fe, p, &mut cap, Format::Json, Instant::now(), t);
                (code, cap)
            })
            .collect::<Vec<_>>())
    })
    .unwrap_or_else(|_| panic!("起不来"))
}

#[test]
fn forwards_and_echoes_each_message() {
    let b = backend();
    let r = run(&b, &[fp(&[7, 5], false, false)]);
    let (code, cap) = &r[0];
    assert_eq!(*code, 0, "{:?}", cap.out);
    let ls = lines(cap);
    assert_eq!(ls[0]["kind"], "plan");
    assert_eq!(ls[0]["data"]["action"], "forward");
    assert_eq!(ls[0]["data"]["from"]["chat_id"], FRIEND);
    assert_eq!(ls[0]["data"]["target"]["chat_id"], OTHER);
    let echoed = ls.iter().filter(|l| l["kind"] == "message").count();
    assert_eq!(echoed, 2, "{ls:?}");
    let end = ls.last().unwrap();
    assert_eq!(end["kind"], "result");
    assert_eq!(end["data"]["from_chat_id"], FRIEND);
    let items = end["data"]["items"].as_array().unwrap();
    assert_eq!(items[0]["message_id"], 5);
    assert_eq!(items[1]["outcome"], "succeeded");
}

#[test]
fn dry_run_does_not_submit() {
    let b = backend();
    let r = run(&b, &[fp(&[5], true, false)]);
    let ls = lines(&r[0].1);
    assert_eq!(ls.last().unwrap()["data"]["dry_run"], true);
    assert!(b.submitted().is_empty());
}

#[test]
fn same_batch_again_is_held_then_released() {
    let b = backend();
    let r = run(
        &b,
        &[
            fp(&[5], false, false),
            fp(&[5], false, false),
            fp(&[5], false, true),
        ],
    );
    assert_eq!(r[0].0, 0);
    let held = lines(&r[1].1);
    let end = held.last().unwrap();
    assert_eq!(end["kind"], "error");
    assert_eq!(end["data"]["code"], "send_refused");
    assert_eq!(end["data"]["retryable"], false);
    assert_eq!(r[2].0, 0, "{:?}", r[2].1.out);
}

#[test]
fn unknown_outcome_exits_10_and_says_not_to_retry() {
    let b = backend();
    b.script_sends(vec![FakeSend::Silent]);
    let r = run(&b, &[fp(&[5], false, false)]);
    let (code, cap) = &r[0];
    assert_eq!(*code, 10, "{:?}", cap.out);
    let end = lines(cap).pop().unwrap();
    assert_eq!(end["data"]["code"], "send_outcome_unknown");
    assert_eq!(end["data"]["items"][0]["outcome"], "unknown");
    assert!(
        end["data"]["message_zh"]
            .as_str()
            .unwrap()
            .contains("不要再转")
    );
}

#[test]
fn too_many_is_refused_before_anything() {
    let b = backend();
    let ids: Vec<i64> = (1..=21).collect();
    let r = run(&b, &[fp(&ids, false, false)]);
    let end = lines(&r[0].1).pop().unwrap();
    assert_eq!(end["data"]["code"], "invalid_input");
    assert_eq!(end["data"]["why"], "forward_too_many");
    assert!(b.submitted().is_empty());
}

#[test]
fn partly_not_forwarded_does_not_say_nothing_was_sent() {
    let b = backend();
    let mut m7 = msg(7, "第7条");
    m7["_forward_null"] = json!(true);
    b.set_history(FRIEND, vec![msg(5, "第5条"), m7]);
    let r = run(&b, &[fp(&[5, 7], false, false)]);
    let end = lines(&r[0].1).pop().unwrap();
    assert_eq!(end["kind"], "error");
    assert_eq!(end["data"]["why"], "not_forwarded");
    assert_eq!(end["data"]["items"][1]["outcome"], "not_forwarded");
    let zh = end["data"]["message_zh"].as_str().unwrap();
    assert!(zh.contains("成功 1 条"), "{zh}");
    assert!(!zh.contains("什么都没转"), "{zh}");
}

#[test]
fn all_not_forwarded_says_nothing_was_sent() {
    let b = backend();
    let mut m5 = msg(5, "第5条");
    m5["_forward_null"] = json!(true);
    b.set_history(FRIEND, vec![m5, msg(7, "第7条")]);
    let r = run(&b, &[fp(&[5], false, false)]);
    let end = lines(&r[0].1).pop().unwrap();
    assert_eq!(end["data"]["why"], "not_forwardable");
    let zh = end["data"]["message_zh"].as_str().unwrap();
    assert!(zh.contains("什么都没转"), "{zh}");
}
