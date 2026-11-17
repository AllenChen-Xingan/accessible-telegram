//! 假后端：数据全是编的（名字取自 fixtures/names.txt），不碰真实账号与本机档案目录
use super::*;
use atg_core::config::{Profile, ServerKind};
use atg_core::fake::{Call, FakeAuth, FakeBackend};
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

fn profile() -> Profile {
    static N: AtomicU32 = AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "atg-cli-edit-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&root);
    Profile::create(&root, "p", ServerKind::Main).unwrap()
}

fn own(chat: i64, id: i64, text: &str) -> Value {
    json!({"@type": "message", "id": id, "chat_id": chat,
        "sender_id": {"@type": "messageSenderUser", "user_id": ME},
        "is_outgoing": true, "date": 1_792_755_420,
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": text, "entities": []}}})
}

/// 正式服务器（假的）：自己的收藏夹 + 与「林小雨」的私聊，各有一条自己发的
fn backend() -> FakeBackend {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth {
        phone: "+15550001234".into(),
        logged_in: true,
        ..FakeAuth::default()
    });
    b.set_me(ME);
    for (id, title) in [(ME, "我自己"), (FRIEND, "林小雨")] {
        b.set_chat(
            id,
            vec![json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title,
                "type": {"@type": "chatTypePrivate", "user_id": id}, "positions": [], "notification_settings": {}}})],
        );
        b.set_history(id, vec![own(id, 5, "atg 测试 原文")]);
    }
    b
}

fn params(chat: &str, id: i64, text: &str, dry_run: bool) -> EditParams {
    EditParams {
        chat: chat.into(),
        message_id: id,
        text: text.into(),
        dry_run,
        tz: Some("+08:00".into()),
    }
}

fn run_with(b: &FakeBackend, p: &EditParams, format: Format, timeout: Duration) -> (u8, Cap) {
    let pr = profile();
    history::with_backend(b, &pr, Instant::now(), |b, st, fe| {
        let mut cap = Cap::default();
        let code = execute_within(b, st, fe, p, &mut cap, format, Instant::now(), timeout);
        Ok((code, cap))
    })
    .unwrap_or_else(|_| panic!("起不来"))
}

fn run(b: &FakeBackend, p: &EditParams) -> (u8, Cap) {
    run_with(b, p, Format::Json, QUERY_TIMEOUT)
}

fn line(cap: &Cap, i: usize) -> Value {
    serde_json::from_str(&cap.out[i]).unwrap()
}

fn edits(b: &FakeBackend) -> Vec<Call> {
    b.calls()
        .into_iter()
        .filter(|c| matches!(c, Call::Edit { .. }))
        .collect()
}

#[test]
fn edits_directly_and_echoes_the_new_text() {
    let b = backend();
    let (code, cap) = run(&b, &params(&FRIEND.to_string(), 5, "atg 测试 改后", false));
    assert_eq!(code, 0, "{:?}", cap.out);
    let plan = line(&cap, 0);
    assert_eq!(plan["kind"], "plan");
    assert_eq!(plan["data"]["action"], "edit");
    assert_eq!(plan["data"]["old_text"], "atg 测试 原文");
    assert_eq!(plan["data"]["confirm"], "direct_mode");
    let echo = line(&cap, 1);
    assert_eq!(echo["kind"], "message");
    assert!(cap.out[1].contains("atg 测试 改后"));
    let last = line(&cap, cap.out.len() - 1);
    assert_eq!(last["kind"], "result");
    assert_eq!(last["data"]["message_id"], 5);
    assert!(last["data"]["edit_date"].is_i64());
    assert_eq!(
        edits(&b),
        vec![Call::Edit {
            chat: FRIEND,
            id: 5,
            text: "atg 测试 改后".into()
        }]
    );
}

#[test]
fn dry_run_changes_nothing() {
    let b = backend();
    let (code, cap) = run(&b, &params("saved", 5, "atg 测试 改后", true));
    assert_eq!(code, 0);
    assert_eq!(cap.out.len(), 2);
    assert_eq!(line(&cap, 0)["data"]["target"]["saved"], true);
    assert_eq!(line(&cap, 1)["data"]["dry_run"], true);
    assert!(edits(&b).is_empty());
}

#[test]
fn unchanged_text_is_invalid_input_with_why() {
    let b = backend();
    let (code, cap) = run(&b, &params(&FRIEND.to_string(), 5, "atg 测试 原文", false));
    assert_eq!(code, 7);
    let e = line(&cap, 0);
    assert_eq!(e["data"]["code"], "invalid_input");
    assert_eq!(e["data"]["why"], "unchanged");
    assert_eq!(e["data"]["retryable"], false);
    assert!(edits(&b).is_empty());
}

#[test]
fn disconnected_after_submit_is_outcome_unknown() {
    let b = backend();
    b.fail_next("edit", CoreError::Disconnected);
    let (code, cap) = run(&b, &params(&FRIEND.to_string(), 5, "atg 测试 改后", false));
    assert_eq!(code, 10);
    let last = line(&cap, cap.out.len() - 1);
    assert_eq!(last["data"]["code"], "send_outcome_unknown");
    assert_eq!(last["data"]["message_zh"], EDIT_UNKNOWN_ZH);
}

#[test]
fn submit_timeout_is_outcome_unknown() {
    let b = backend();
    b.fail_next(
        "edit",
        CoreError::Internal {
            what: FakeBackend::HANG,
        },
    );
    let p = params(&FRIEND.to_string(), 5, "atg 测试 改后", false);
    let (code, cap) = run_with(&b, &p, Format::Json, Duration::from_millis(300));
    assert_eq!(code, 10);
    let last = line(&cap, cap.out.len() - 1);
    assert_eq!(last["data"]["why"], "timeout");
    assert_eq!(last["data"]["temp_message_id"], 0);
}

#[test]
fn text_format_reads_plan_echo_and_result() {
    let b = backend();
    let p = params(&FRIEND.to_string(), 5, "atg 测试 改后", false);
    let (code, cap) = run_with(&b, &p, Format::Text, QUERY_TIMEOUT);
    assert_eq!(code, 0);
    assert!(cap.out[0].starts_with("将把 "), "{}", cap.out[0]);
    assert!(cap.out[0].contains("「atg 测试 原文」改成「atg 测试 改后」"));
    assert!(cap.out[1].contains("atg 测试 改后"));
    assert_eq!(cap.out.last().unwrap(), "已改好");
}
