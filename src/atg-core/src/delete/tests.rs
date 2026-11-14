//! 假后端走撤销待发消息的全流程（数据全是编的；名字取自 fixtures/names.txt）
use super::*;
use crate::config::{Profile, ServerKind};
use crate::error::{Holder, SendRefusedReason};
use crate::fake::{Call, FakeAuth, FakeBackend};
use crate::gate::{Authenticator, Gate};
use crate::link::block_on;
use crate::status::AuthStep;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

const ME: i64 = 4242;
const FRIEND: i64 = 777;

struct Auth(Arc<AtomicU32>, Result<(), SendRefusedReason>);
impl Authenticator for Auth {
    fn authenticate(&self, _: &str) -> Result<(), SendRefusedReason> {
        self.0.fetch_add(1, Ordering::SeqCst);
        self.1
    }
}

fn own(chat: i64, id: i64, text: &str, state: Value) -> Value {
    json!({"@type": "message", "id": id, "chat_id": chat,
        "sender_id": {"@type": "messageSenderUser", "user_id": ME}, "is_outgoing": true,
        "date": 1_792_755_000, "sending_state": state,
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": text, "entities": []}}})
}

fn pending_state() -> Value {
    json!({"@type": "messageSendingStatePending", "sending_id": 0})
}

fn rig(direct: bool, test_dc: bool, asks: Arc<AtomicU32>) -> FakeBackend {
    static N: AtomicU32 = AtomicU32::new(0);
    let gate = Gate::new(direct, Holder::Cli, Box::new(Auth(asks, Ok(()))));
    let b = FakeBackend::with_gate(gate).unwrap();
    b.set_auth(FakeAuth {
        phone: if test_dc {
            "+9996612345"
        } else {
            "+15550001234"
        }
        .into(),
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
    }
    let hist = |chat| {
        vec![
            own(chat, 1_048_577, "atg 测试 待发", pending_state()),
            own(chat, 5, "atg 测试 已发", Value::Null),
        ]
    };
    b.set_history(ME, hist(ME));
    b.set_history(FRIEND, hist(FRIEND));
    let root = std::env::temp_dir().join(format!(
        "atg-delete-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&root);
    let kind = if test_dc {
        ServerKind::Test
    } else {
        ServerKind::Main
    };
    let profile = Profile::create(&root, "p", kind).unwrap();
    drop(b.subscribe().unwrap());
    block_on(b.start(&profile)).unwrap();
    b.wait_auth(Duration::from_secs(5), &|s| s == AuthStep::Ready)
        .unwrap();
    b
}

fn deletes(b: &FakeBackend) -> Vec<Call> {
    b.calls()
        .into_iter()
        .filter(|c| matches!(c, Call::Delete { .. }))
        .collect()
}

#[test]
fn test_saved_cancel_is_exempt_and_only_deletes_here() {
    let asks = Arc::new(AtomicU32::new(0));
    let b = rig(false, true, asks.clone());
    let plan = block_on(prepare_cancel(&b, ME, &[1_048_577], SendOrigin::Cli)).unwrap();
    assert_eq!(plan.mode(), ConfirmMode::TestExempt);
    assert!(plan.target().saved);
    assert_eq!(plan.items()[0].text.as_deref(), Some("atg 测试 待发"));
    let token = b.gate().issue(&plan, false).unwrap();
    assert_eq!(
        block_on(cancel(&b, &plan, token)).unwrap(),
        ConfirmMode::TestExempt
    );
    assert_eq!(asks.load(Ordering::SeqCst), 0);
    assert_eq!(
        deletes(&b),
        vec![Call::Delete {
            chat: ME,
            ids: vec![1_048_577],
            revoke: false
        }]
    );
}

#[test]
fn main_server_asks_and_says_it_may_revoke() {
    let asks = Arc::new(AtomicU32::new(0));
    let b = rig(false, false, asks.clone());
    let plan = block_on(prepare_cancel(&b, FRIEND, &[1_048_577], SendOrigin::Cli)).unwrap();
    assert_eq!(plan.mode(), ConfirmMode::HumanAuth);
    let t = plan.confirm_text();
    assert!(t.starts_with("撤销待发 · 私聊 · 林小雨 · id 777\n"), "{t}");
    assert!(
        t.contains("可能是 AI") && t.contains("已送达的全员撤回，刚发出的只删本机"),
        "{t}"
    );
    assert!(t.contains("atg 测试 待发"), "{t}");
    assert!(plan.confirm_fits(), "{t}");
    let token = b.gate().issue(&plan, false).unwrap();
    block_on(cancel(&b, &plan, token)).unwrap();
    assert_eq!(asks.load(Ordering::SeqCst), 1);
    assert_eq!(deletes(&b).len(), 1);
}

#[test]
fn test_server_other_chat_refused() {
    let b = rig(false, true, Arc::default());
    let e = block_on(prepare_cancel(&b, FRIEND, &[1_048_577], SendOrigin::Cli)).unwrap_err();
    assert_eq!(e.code(), "send_refused");
    assert!(deletes(&b).is_empty());
}

#[test]
fn sent_message_is_not_pending() {
    let b = rig(true, false, Arc::default());
    let e = block_on(prepare_cancel(&b, FRIEND, &[5], SendOrigin::Cli)).unwrap_err();
    assert_eq!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::NotPending
        }
    );
    let e = block_on(prepare_cancel(&b, FRIEND, &[], SendOrigin::Cli)).unwrap_err();
    assert_eq!(e.code(), "invalid_input");
}

/// 带附件的待发消息：正文给不出、绑不进凭据，整批不撤（codex 029 #3）
#[test]
fn non_text_pending_is_refused() {
    let b = rig(true, false, Arc::default());
    let mut m = own(FRIEND, 1_048_579, "", pending_state());
    m["content"] = json!({"@type": "messagePhoto", "caption": {"@type": "formattedText", "text": "atg 测试 图", "entities": []},
        "photo": {"@type": "photo", "has_stickers": false, "sizes": []}});
    b.set_history(FRIEND, vec![m]);
    let e = block_on(prepare_cancel(&b, FRIEND, &[1_048_579], SendOrigin::Cli)).unwrap_err();
    assert_eq!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::NotText
        }
    );
}

#[test]
fn sent_meanwhile_is_refused_before_submit() {
    let b = rig(true, false, Arc::default());
    let plan = block_on(prepare_cancel(&b, FRIEND, &[1_048_577], SendOrigin::Cli)).unwrap();
    assert_eq!(plan.mode(), ConfirmMode::DirectMode);
    let token = b.gate().issue(&plan, false).unwrap();
    // 准备之后、交出之前它发出去了：再删就是给所有人删除，不是撤销
    b.set_history(
        FRIEND,
        vec![own(FRIEND, 1_048_577, "atg 测试 待发", Value::Null)],
    );
    let e = block_on(cancel(&b, &plan, token)).unwrap_err();
    assert_eq!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::NotPending
        }
    );
    assert!(deletes(&b).is_empty());
}

#[test]
fn token_for_another_plan_is_refused() {
    let b = rig(true, false, Arc::default());
    let a = block_on(prepare_cancel(&b, FRIEND, &[1_048_577], SendOrigin::Cli)).unwrap();
    let other = block_on(prepare_cancel(&b, ME, &[1_048_577], SendOrigin::Cli)).unwrap();
    let token = b.gate().issue(&other, false).unwrap();
    assert_eq!(
        block_on(cancel(&b, &a, token)).unwrap_err().code(),
        "send_refused"
    );
    assert!(deletes(&b).is_empty());
}
