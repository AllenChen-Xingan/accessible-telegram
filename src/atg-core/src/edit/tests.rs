//! 假后端走编辑的全流程（#8 B；数据全是编的，名字取自 fixtures/names.txt）
use super::*;
use crate::config::{Profile, ServerKind};
use crate::error::Holder;
use crate::fake::{Call, Cap, FakeAuth, FakeBackend};
use crate::gate::Gate;
use crate::link::block_on;
use crate::status::AuthStep;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

const ME: i64 = 4242;
const FRIEND: i64 = 777;

fn own(chat: i64, id: i64, text: &str, state: Value) -> Value {
    json!({"@type": "message", "id": id, "chat_id": chat,
        "sender_id": {"@type": "messageSenderUser", "user_id": ME}, "is_outgoing": true,
        "date": 1_792_755_000, "sending_state": state,
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": text, "entities": []}}})
}

fn rig(test_dc: bool) -> FakeBackend {
    static N: AtomicU32 = AtomicU32::new(0);
    let b = FakeBackend::with_gate(Gate::new(Holder::Cli)).unwrap();
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
        b.set_history(id, vec![own(id, 5, "atg 测试 原文", Value::Null)]);
    }
    let root = std::env::temp_dir().join(format!(
        "atg-edit-{}-{}",
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

fn edits(b: &FakeBackend) -> Vec<Call> {
    b.calls()
        .into_iter()
        .filter(|c| matches!(c, Call::Edit { .. }))
        .collect()
}

fn refused(why: RefusedWhy) -> CoreError {
    CoreError::ActionRefused { why }
}

fn invalid_why(why: InvalidSendWhy) -> CoreError {
    CoreError::InvalidSend { why }
}

#[test]
fn own_text_message_is_edited_directly() {
    let b = rig(false);
    let plan = block_on(prepare_edit(
        &b,
        FRIEND,
        5,
        "atg 测试 改后",
        SendOrigin::Cli,
    ))
    .unwrap();
    assert_eq!(plan.mode(), ConfirmMode::DirectMode);
    let j = plan.json();
    assert_eq!(j["action"], "edit");
    assert_eq!(j["old_text"], "atg 测试 原文");
    assert_eq!(j["text"], "atg 测试 改后");
    assert_eq!(j["formatting_dropped"], false);
    let token = b.gate().issue(&plan, false).unwrap();
    let (mode, m) = block_on(edit_text(&b, &plan, token)).unwrap();
    assert_eq!(mode, ConfirmMode::DirectMode);
    assert_eq!(m.id.message_id, 5);
    assert_eq!(
        send::resending::confirmable_text(&m).as_deref(),
        Some("atg 测试 改后")
    );
    assert!(m.edited.is_some());
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
fn saved_messages_can_be_edited_on_both_servers() {
    for test_dc in [false, true] {
        let b = rig(test_dc);
        let plan = block_on(prepare_edit(&b, ME, 5, "atg 测试 改后", SendOrigin::Cli)).unwrap();
        assert!(plan.target().saved);
        let token = b.gate().issue(&plan, false).unwrap();
        block_on(edit_text(&b, &plan, token)).unwrap();
        assert_eq!(edits(&b).len(), 1);
    }
}

#[test]
fn test_server_other_chat_is_refused_by_the_gate() {
    let b = rig(true);
    let e = block_on(prepare_edit(
        &b,
        FRIEND,
        5,
        "atg 测试 改后",
        SendOrigin::Cli,
    ))
    .unwrap_err();
    assert_eq!(e.code(), "send_refused");
}

#[test]
fn bad_drafts_are_refused_before_asking_tdlib() {
    let b = rig(false);
    for (text, why) in [
        ("  ", InvalidSendWhy::EmptyText),
        ("atg 测试 原文", InvalidSendWhy::Unchanged),
    ] {
        let e = block_on(prepare_edit(&b, FRIEND, 5, text, SendOrigin::Cli)).unwrap_err();
        assert_eq!(e, invalid_why(why));
    }
    let long = "字".repeat(30_000);
    let e = block_on(prepare_edit(&b, FRIEND, 5, &long, SendOrigin::Cli)).unwrap_err();
    assert_eq!(e, invalid_why(InvalidSendWhy::TooLong));
}

#[test]
fn what_cannot_be_edited_is_refused() {
    let b = rig(false);
    let mut other = own(FRIEND, 6, "atg 测试 对方发的", Value::Null);
    other["is_outgoing"] = json!(false);
    other["sender_id"] = json!({"@type": "messageSenderUser", "user_id": FRIEND});
    let pending = own(
        FRIEND,
        1_048_577,
        "atg 测试 待发",
        json!({"@type": "messageSendingStatePending", "sending_id": 0}),
    );
    let mut photo = own(FRIEND, 7, "", Value::Null);
    photo["content"] = json!({"@type": "messagePhoto", "caption": {"@type": "formattedText", "text": "atg 测试 图", "entities": []},
        "photo": {"@type": "photo", "has_stickers": false, "sizes": []}});
    b.set_history(
        FRIEND,
        vec![
            own(FRIEND, 5, "atg 测试 原文", Value::Null),
            other,
            pending,
            photo,
        ],
    );
    let try_edit = |id| {
        block_on(prepare_edit(
            &b,
            FRIEND,
            id,
            "atg 测试 改后",
            SendOrigin::Cli,
        ))
    };
    assert_eq!(try_edit(6).unwrap_err(), refused(RefusedWhy::NotOwn));
    assert_eq!(
        try_edit(1_048_577).unwrap_err(),
        refused(RefusedWhy::NotSent)
    );
    assert_eq!(
        try_edit(7).unwrap_err(),
        invalid_why(InvalidSendWhy::EditNotText)
    );
    assert_eq!(try_edit(99).unwrap_err().code(), "not_found");
    // 过了时限等：TDLib 说不行就不行
    b.deny(
        MsgId {
            chat_id: FRIEND,
            message_id: 5,
        },
        Cap::Edit,
    );
    assert_eq!(try_edit(5).unwrap_err(), refused(RefusedWhy::NotEditable));
    assert!(edits(&b).is_empty());
}

#[test]
fn formatting_that_would_be_lost_is_flagged() {
    let b = rig(false);
    let mut m = own(FRIEND, 8, "atg 测试 加粗 https://example.org", Value::Null);
    m["content"]["text"]["entities"] = json!([
        {"@type": "textEntity", "offset": 8, "length": 2, "type": {"@type": "textEntityTypeBold"}}
    ]);
    let mut plain_link = own(FRIEND, 9, "atg 测试 https://example.org", Value::Null);
    plain_link["content"]["text"]["entities"] = json!([
        {"@type": "textEntity", "offset": 8, "length": 19, "type": {"@type": "textEntityTypeUrl"}}
    ]);
    b.set_history(FRIEND, vec![m, plain_link]);
    let p = block_on(prepare_edit(
        &b,
        FRIEND,
        8,
        "atg 测试 改后",
        SendOrigin::Cli,
    ))
    .unwrap();
    assert!(p.formatting_dropped());
    // 网址服务器会自己再认出来，不算丢
    let p = block_on(prepare_edit(
        &b,
        FRIEND,
        9,
        "atg 测试 改后",
        SendOrigin::Cli,
    ))
    .unwrap();
    assert!(!p.formatting_dropped());
}

#[test]
fn changed_after_prepare_is_refused_before_submit() {
    let b = rig(false);
    let plan = block_on(prepare_edit(
        &b,
        FRIEND,
        5,
        "atg 测试 改后",
        SendOrigin::Cli,
    ))
    .unwrap();
    let token = b.gate().issue(&plan, false).unwrap();
    b.set_history(
        FRIEND,
        vec![own(FRIEND, 5, "atg 测试 别处改过", Value::Null)],
    );
    let e = block_on(edit_text(&b, &plan, token)).unwrap_err();
    assert_eq!(e, invalid_why(InvalidSendWhy::Changed));
    assert!(edits(&b).is_empty());
}

#[test]
fn no_longer_editable_at_submit_is_refused() {
    let b = rig(false);
    let plan = block_on(prepare_edit(
        &b,
        FRIEND,
        5,
        "atg 测试 改后",
        SendOrigin::Cli,
    ))
    .unwrap();
    let token = b.gate().issue(&plan, false).unwrap();
    b.deny(
        MsgId {
            chat_id: FRIEND,
            message_id: 5,
        },
        Cap::Edit,
    );
    let e = block_on(edit_text(&b, &plan, token)).unwrap_err();
    assert_eq!(e, refused(RefusedWhy::NotEditable));
    assert!(edits(&b).is_empty());
}

#[test]
fn token_for_another_plan_is_refused() {
    let b = rig(false);
    let a = block_on(prepare_edit(
        &b,
        FRIEND,
        5,
        "atg 测试 改后",
        SendOrigin::Cli,
    ))
    .unwrap();
    let other = block_on(prepare_edit(
        &b,
        FRIEND,
        5,
        "atg 测试 另一种",
        SendOrigin::Cli,
    ))
    .unwrap();
    let token = b.gate().issue(&other, false).unwrap();
    assert_eq!(
        block_on(edit_text(&b, &a, token)).unwrap_err().code(),
        "send_refused"
    );
    assert!(edits(&b).is_empty());
}

#[test]
fn disconnect_while_submitting_is_outcome_unknown() {
    let b = rig(false);
    let plan = block_on(prepare_edit(
        &b,
        FRIEND,
        5,
        "atg 测试 改后",
        SendOrigin::Cli,
    ))
    .unwrap();
    let token = b.gate().issue(&plan, false).unwrap();
    b.fail_next("edit", CoreError::Disconnected);
    let e = block_on(edit_text(&b, &plan, token)).unwrap_err();
    assert_eq!(e.code(), "send_outcome_unknown");
}
