//! 假后端：数据全是编的（名字取自 `src/atg-core/fixtures/names.txt`），不碰真实账号与本机档案目录。
use super::*;
use atg_core::config::{Profile, ServerKind};
use atg_core::error::{Holder, SendRefusedReason};
use atg_core::fake::{FakeAuth, FakeBackend, FakeSend};
use atg_core::gate::{Authenticator, Gate};
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Default)]
struct Cap {
    out: Vec<String>,
    err: Vec<String>,
}

impl Sink for Cap {
    fn out(&mut self, line: &str) {
        self.out.push(line.to_owned());
    }
    fn err(&mut self, line: &str) {
        self.err.push(line.to_owned());
    }
}

struct Deny;
impl Authenticator for Deny {
    fn authenticate(&self, _: &str) -> Result<(), SendRefusedReason> {
        Err(SendRefusedReason::AuthFailed)
    }
}

const ME: i64 = 4242;
const FRIEND: i64 = 777;

fn backend(direct: bool, test_phone: bool) -> FakeBackend {
    let b = FakeBackend::with_gate(Gate::new(direct, Holder::Cli, Box::new(Deny))).unwrap();
    b.set_auth(FakeAuth {
        phone: if test_phone {
            "+9996612345"
        } else {
            "+15550001234"
        }
        .into(),
        logged_in: true,
        ..FakeAuth::default()
    });
    b.set_me(ME);
    let chat = |id: i64, title: &str| {
        vec![
            json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title,
            "type": {"@type": "chatTypePrivate", "user_id": id}, "positions": [],
            "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0}}}),
        ]
    };
    b.set_chat(ME, chat(ME, "我自己"));
    b.set_chat(FRIEND, chat(FRIEND, "林小雨"));
    b.push_net("connectionStateReady");
    b
}

fn profile(kind: ServerKind) -> Profile {
    static N: AtomicU32 = AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "atg-cli-send-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&root);
    Profile::create(&root, "p", kind).unwrap()
}

fn params(chat: &str, text: &str) -> SendParams {
    SendParams {
        chat: chat.into(),
        text: text.into(),
        reply_to: None,
        dry_run: false,
        again: false,
        tz: Some("+08:00".into()),
    }
}

/// 在同一个后端上连发几次，每次的输出分开收
fn run_many(b: &FakeBackend, p: &Profile, ps: &[SendParams]) -> Vec<(u8, Cap)> {
    crate::history::with_backend(b, p, Instant::now(), |b, st, fe| {
        Ok(ps
            .iter()
            .map(|sp| {
                let mut cap = Cap::default();
                let code = execute_send(
                    b,
                    st,
                    fe,
                    sp,
                    &mut cap,
                    Format::Json,
                    Instant::now(),
                    Duration::from_millis(500),
                );
                (code, cap)
            })
            .collect())
    })
    .unwrap_or_else(|_| panic!("起不来"))
}

fn last(cap: &Cap) -> Value {
    serde_json::from_str(cap.out.last().unwrap()).unwrap()
}

#[test]
fn direct_duplicate_is_refused_with_when_and_what() {
    let b = backend(true, false);
    let p = profile(ServerKind::Main);
    let mut again = params("777", "好的");
    again.again = true;
    let r = run_many(
        &b,
        &p,
        &[params("777", "好的"), params("777", "好的"), again],
    );
    assert_eq!(r[0].0, 0);
    let e = last(&r[1].1);
    assert_eq!(r[1].0, 8);
    assert_eq!(e["kind"], "error");
    assert_eq!(e["data"]["reason"], "duplicate_recent");
    assert_eq!(e["data"]["last_outcome"], "succeeded");
    assert_eq!(e["data"]["retryable"], false);
    let zh = e["data"]["message_zh"].as_str().unwrap();
    assert!(
        zh.contains("秒前") && zh.contains("已发出") && zh.contains("--again"),
        "{zh}"
    );
    // 被拒的那次：plan 照出，没有交出去
    assert_eq!(b.submitted().len(), 2);
    assert_eq!(r[2].0, 0);
}

#[test]
fn failure_ends_with_error_carrying_failed_id_and_resend_flag() {
    let b = backend(false, true);
    b.script_sends(vec![FakeSend::Fail {
        code: 400,
        message: "MESSAGE_TOO_LONG".into(),
        can_retry: true,
    }]);
    let p = profile(ServerKind::Test);
    let r = run_many(&b, &p, &[params("saved", "测试")]);
    let (code, cap) = &r[0];
    assert_eq!(*code, 7);
    let e = last(cap);
    assert_eq!(e["data"]["code"], "invalid_input");
    assert_eq!(e["data"]["can_resend"], true);
    assert_eq!(e["data"]["retryable"], false);
    assert!(e["data"]["message_id"].as_i64().is_some());
}

#[test]
fn unknown_outcome_exits_10_and_says_do_not_resend() {
    for (step, why) in [
        (FakeSend::Silent, "timeout"),
        (FakeSend::Delete, "pending_deleted"),
    ] {
        let b = backend(false, true);
        b.script_sends(vec![step]);
        let p = profile(ServerKind::Test);
        let r = run_many(&b, &p, &[params("saved", "测试")]);
        let (code, cap) = &r[0];
        assert_eq!(*code, 10, "{why}");
        let e = last(cap);
        assert_eq!(e["data"]["code"], "send_outcome_unknown");
        assert_eq!(e["data"]["why"], why);
        assert!(
            e["data"]["message_zh"]
                .as_str()
                .unwrap()
                .contains("不要重发")
        );
    }
}

#[test]
fn main_server_without_touch_id_sends_nothing() {
    let b = backend(false, false);
    let p = profile(ServerKind::Main);
    let r = run_many(&b, &p, &[params("saved", "测试")]);
    let (code, cap) = &r[0];
    assert_eq!(*code, 8);
    assert_eq!(last(cap)["data"]["reason"], "auth_failed");
    // plan 先出，确认方式写明要 Touch ID
    let plan: Value = serde_json::from_str(&cap.out[0]).unwrap();
    assert_eq!(plan["kind"], "plan");
    assert_eq!(plan["data"]["confirm"], "human_auth");
    assert_eq!(plan["data"]["target"]["saved"], true);
    // 弹框前在标准错误说一句人话（自审第 12 轮 P2）
    assert!(cap.err.iter().any(|l| l == HUMAN_AUTH_PROMPT_ZH));
    assert!(b.submitted().is_empty());
}

#[test]
fn dry_run_only_plans() {
    let b = backend(true, false);
    let p = profile(ServerKind::Main);
    let mut sp = params("777", "好的");
    sp.dry_run = true;
    let r = run_many(&b, &p, &[sp]);
    assert_eq!(r[0].0, 0);
    assert_eq!(r[0].1.out.len(), 2);
    assert_eq!(last(&r[0].1)["data"]["dry_run"], true);
    assert!(b.submitted().is_empty());
}

#[test]
fn test_dc_other_chat_refused() {
    let b = backend(true, true);
    let p = profile(ServerKind::Test);
    let r = run_many(&b, &p, &[params("777", "好的")]);
    assert_eq!(r[0].0, 8);
    assert_eq!(last(&r[0].1)["data"]["reason"], "test_dc_saved_only");
}

fn resend_params(ids: Vec<i64>) -> ResendParams {
    ResendParams {
        chat: "saved".into(),
        ids,
        dry_run: false,
        tz: Some("+08:00".into()),
    }
}

/// 先发一条失败的，再按 error 行里的 message_id 重发：plan → 回显 → result（items 逐条）
#[test]
fn resend_after_failure_reports_each_item() {
    let b = backend(false, true);
    b.script_sends(vec![FakeSend::Fail {
        code: 400,
        message: "MESSAGE_TOO_LONG".into(),
        can_retry: true,
    }]);
    let p = profile(ServerKind::Test);
    let out = crate::history::with_backend(&b, &p, Instant::now(), |b, st, fe| {
        let mut first = Cap::default();
        let c1 = execute_send(
            b,
            st,
            fe,
            &params("saved", "测试"),
            &mut first,
            Format::Json,
            Instant::now(),
            Duration::from_millis(500),
        );
        let failed_id = last(&first)["data"]["message_id"].as_i64().unwrap();
        let mut second = Cap::default();
        let c2 = execute_resend(
            b,
            st,
            fe,
            &resend_params(vec![failed_id]),
            &mut second,
            Format::Json,
            Instant::now(),
            Duration::from_millis(500),
        );
        Ok((c1, c2, failed_id, second))
    })
    .unwrap_or_else(|_| panic!("起不来"));
    let (c1, c2, failed_id, cap) = out;
    assert_eq!((c1, c2), (7, 0));
    let kinds: Vec<String> = cap
        .out
        .iter()
        .map(|l| serde_json::from_str::<Value>(l).unwrap()["kind"].to_string())
        .collect();
    assert_eq!(kinds, ["\"plan\"", "\"message\"", "\"result\""]);
    let r = last(&cap);
    assert_eq!(r["data"]["items"][0]["message_id"], failed_id);
    assert_eq!(r["data"]["items"][0]["outcome"], "succeeded");
    assert!(r["data"]["items"][0]["new_message_id"].as_i64().is_some());
}

/// 几条一起重发，一条又失败、一条结果未知：结尾报结果未知（退出码 10），说明里逐项计数、点名未知那条不要再重发
/// （codex 016 第 2 轮 #3：原来只报第一条失败，文字模式看不到未知项）
#[test]
fn mixed_resend_reports_unknown_first() {
    let b = backend(false, true);
    let fail = || FakeSend::Fail {
        code: 400,
        message: "MESSAGE_TOO_LONG".into(),
        can_retry: true,
    };
    b.script_sends(vec![fail(), fail(), fail(), FakeSend::Silent]);
    let p = profile(ServerKind::Test);
    let (code, cap) = crate::history::with_backend(&b, &p, Instant::now(), |b, st, fe| {
        let mut ids = Vec::new();
        for text in ["测试一", "测试二"] {
            let mut c = Cap::default();
            execute_send(
                b,
                st,
                fe,
                &params("saved", text),
                &mut c,
                Format::Json,
                Instant::now(),
                Duration::from_millis(500),
            );
            ids.push(last(&c)["data"]["message_id"].as_i64().unwrap());
        }
        let mut second = Cap::default();
        let c = execute_resend(
            b,
            st,
            fe,
            &resend_params(ids),
            &mut second,
            Format::Json,
            Instant::now(),
            Duration::from_millis(500),
        );
        Ok((c, second))
    })
    .unwrap_or_else(|_| panic!("起不来"));
    assert_eq!(code, 10);
    let e = last(&cap);
    assert_eq!(e["data"]["code"], "send_outcome_unknown");
    let zh = e["data"]["message_zh"].as_str().unwrap();
    assert!(
        zh.contains("失败 1 条") && zh.contains("结果未知 1 条") && zh.contains("不要再重发"),
        "{zh}"
    );
}

/// 重发又失败：结尾是 error，码与字段取那一条，items 照带
#[test]
fn resend_that_fails_again_ends_with_error() {
    let b = backend(false, true);
    let fail = || FakeSend::Fail {
        code: 400,
        message: "MESSAGE_TOO_LONG".into(),
        can_retry: true,
    };
    b.script_sends(vec![fail(), fail()]);
    let p = profile(ServerKind::Test);
    let (code, cap) = crate::history::with_backend(&b, &p, Instant::now(), |b, st, fe| {
        let mut first = Cap::default();
        execute_send(
            b,
            st,
            fe,
            &params("saved", "测试"),
            &mut first,
            Format::Json,
            Instant::now(),
            Duration::from_millis(500),
        );
        let id = last(&first)["data"]["message_id"].as_i64().unwrap();
        let mut second = Cap::default();
        let c = execute_resend(
            b,
            st,
            fe,
            &resend_params(vec![id]),
            &mut second,
            Format::Json,
            Instant::now(),
            Duration::from_millis(500),
        );
        Ok((c, second))
    })
    .unwrap_or_else(|_| panic!("起不来"));
    assert_eq!(code, 7);
    let e = last(&cap);
    assert_eq!(e["kind"], "error");
    assert_eq!(e["data"]["code"], "invalid_input");
    assert_eq!(e["data"]["retryable"], false);
    assert_eq!(e["data"]["items"][0]["outcome"], "failed");
    assert_eq!(e["data"]["items"][0]["can_resend"], true);
}

#[test]
fn resend_of_message_that_did_not_fail_is_invalid() {
    let b = backend(false, true);
    let p = profile(ServerKind::Test);
    let (code, cap) = crate::history::with_backend(&b, &p, Instant::now(), |b, st, fe| {
        let mut first = Cap::default();
        execute_send(
            b,
            st,
            fe,
            &params("saved", "测试"),
            &mut first,
            Format::Json,
            Instant::now(),
            Duration::from_millis(500),
        );
        let id = last(&first)["data"]["message_id"].as_i64().unwrap();
        let mut second = Cap::default();
        let c = execute_resend(
            b,
            st,
            fe,
            &resend_params(vec![id]),
            &mut second,
            Format::Json,
            Instant::now(),
            Duration::from_millis(500),
        );
        Ok((c, second))
    })
    .unwrap_or_else(|_| panic!("起不来"));
    assert_eq!(code, 7);
    assert_eq!(last(&cap)["data"]["why"], "not_failed");
}
