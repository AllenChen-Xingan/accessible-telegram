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
        "atg-cli-delete-{}-{}",
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
        b.set_history(
            id,
            vec![own(id, 5, "atg 测试 删除"), own(id, 6, "atg 测试 留着")],
        );
    }
    b
}

fn run_with(
    b: &FakeBackend,
    chat: &str,
    ids: &[i64],
    dry_run: bool,
    format: Format,
    timeout: Duration,
) -> (u8, Cap) {
    let p = profile();
    history::with_backend(b, &p, Instant::now(), |b, st, fe| {
        let mut cap = Cap::default();
        let params = DeleteParams {
            chat: chat.into(),
            ids: ids.to_vec(),
            dry_run,
            tz: Some("+08:00".into()),
        };
        let code = execute_within(
            b,
            st,
            fe,
            &params,
            &mut cap,
            format,
            Instant::now(),
            timeout,
        );
        Ok((code, cap))
    })
    .unwrap_or_else(|_| panic!("起不来"))
}

fn run(b: &FakeBackend, chat: &str, ids: &[i64], dry_run: bool) -> (u8, Cap) {
    run_with(b, chat, ids, dry_run, Format::Json, QUERY_TIMEOUT)
}

fn line(cap: &Cap, i: usize) -> Value {
    serde_json::from_str(&cap.out[i]).unwrap()
}

fn deletes(b: &FakeBackend) -> Vec<Call> {
    b.calls()
        .into_iter()
        .filter(|c| matches!(c, Call::Delete { .. }))
        .collect()
}

#[test]
fn deletes_for_everyone_directly() {
    let b = backend();
    let (code, cap) = run(&b, &FRIEND.to_string(), &[5], false);
    assert_eq!(code, 0, "{:?}", cap.out);
    let plan = line(&cap, 0);
    assert_eq!(plan["kind"], "plan");
    assert_eq!(plan["data"]["action"], "delete");
    assert_eq!(plan["data"]["confirm"], "direct_mode");
    let item = &plan["data"]["items"][0];
    assert_eq!(item["message_id"], 5);
    assert_eq!(item["text"], "atg 测试 删除");
    assert!(item["brief"].as_str().unwrap().contains("atg 测试 删除"));
    let last = line(&cap, cap.out.len() - 1);
    assert_eq!(last["kind"], "result");
    assert_eq!(last["data"]["deleted"], json!([5]));
    assert_eq!(
        deletes(&b),
        vec![Call::Delete {
            chat: FRIEND,
            ids: vec![5],
            revoke: true
        }]
    );
}

#[test]
fn dry_run_deletes_nothing() {
    let b = backend();
    let (code, cap) = run(&b, &FRIEND.to_string(), &[5, 6], true);
    assert_eq!(code, 0);
    assert_eq!(cap.out.len(), 2);
    assert_eq!(line(&cap, 0)["data"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(line(&cap, 1)["data"]["dry_run"], true);
    assert!(deletes(&b).is_empty());
}

#[test]
fn saved_is_forbidden_with_why() {
    let b = backend();
    let (code, cap) = run(&b, "saved", &[5], false);
    assert_eq!(code, 7);
    assert_eq!(cap.out.len(), 1);
    let e = line(&cap, 0);
    assert_eq!(e["data"]["code"], "forbidden");
    assert_eq!(e["data"]["why"], "saved");
    assert_eq!(e["data"]["retryable"], false);
    assert!(deletes(&b).is_empty());
}

#[test]
fn disconnected_after_submit_is_outcome_unknown() {
    let b = backend();
    b.fail_next("delete", CoreError::Disconnected);
    let (code, cap) = run(&b, &FRIEND.to_string(), &[5], false);
    assert_eq!(code, 10);
    let last = line(&cap, cap.out.len() - 1);
    assert_eq!(last["data"]["code"], "send_outcome_unknown");
    assert_eq!(last["data"]["message_zh"], DELETE_UNKNOWN_ZH);
}

#[test]
fn submit_timeout_is_outcome_unknown() {
    let b = backend();
    b.fail_next(
        "delete",
        CoreError::Internal {
            what: FakeBackend::HANG,
        },
    );
    let (code, cap) = run_with(
        &b,
        &FRIEND.to_string(),
        &[5],
        false,
        Format::Json,
        Duration::from_millis(300),
    );
    assert_eq!(code, 10);
    let last = line(&cap, cap.out.len() - 1);
    assert_eq!(last["data"]["code"], "send_outcome_unknown");
    assert_eq!(last["data"]["why"], "timeout");
    assert_eq!(last["data"]["temp_message_id"], 0);
}

#[test]
fn text_format_reads_plan_and_result() {
    let b = backend();
    let (code, cap) = run_with(
        &b,
        &FRIEND.to_string(),
        &[5],
        false,
        Format::Text,
        QUERY_TIMEOUT,
    );
    assert_eq!(code, 0);
    assert!(cap.out[0].starts_with("将给所有人删除 "), "{}", cap.out[0]);
    assert!(cap.out[0].contains("id 5："));
    assert!(cap.out.last().unwrap().starts_with("已在本机删除 1 条"));
}

#[test]
fn bad_ids_from_the_interface_are_usage() {
    let b = backend();
    let (code, _) = run(&b, &FRIEND.to_string(), &[], false);
    assert_eq!(code, 2);
    assert!(deletes(&b).is_empty());
}
