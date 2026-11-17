//! `send::forward` 的单测：假后端走全流程（数据全是编的；名字取自 fixtures/names.txt）

use super::*;
use crate::config::{Profile, ServerKind};
use crate::error::Holder;
use crate::fake::{Cap, FakeAuth, FakeBackend, FakeSend};
use crate::gate::{DIRECT_LOG_FILE, Gate};
use crate::link::block_on;
use crate::status::AuthStep;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

const ME: i64 = 4242;
const FRIEND: i64 = 777;
const OTHER: i64 = 888;

struct Rig {
    b: FakeBackend,
    stream: EventStream,
    dir: PathBuf,
}

fn new_chat(id: i64, title: &str) -> Value {
    json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title,
        "type": {"@type": "chatTypePrivate", "user_id": id}, "positions": [], "notification_settings": {}}})
}

fn msg(chat: i64, id: i64, text: &str, extra: Value) -> Value {
    let mut m = json!({"@type": "message", "id": id, "chat_id": chat,
        "sender_id": {"@type": "messageSenderUser", "user_id": FRIEND}, "is_outgoing": false,
        "date": 1_792_755_000,
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": text, "entities": []}}});
    if let (Some(o), Value::Object(x)) = (m.as_object_mut(), extra) {
        o.extend(x);
    }
    m
}

fn friend_history() -> Vec<Value> {
    vec![
        msg(FRIEND, 5, "第5条", json!({})),
        msg(FRIEND, 6, "第6条", json!({})),
        msg(FRIEND, 7, "第7条", json!({})),
    ]
}

fn rig(tag: &str, test_dc: bool) -> Rig {
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
    b.set_chat(ME, vec![new_chat(ME, "我自己")]);
    b.set_chat(FRIEND, vec![new_chat(FRIEND, "林小雨")]);
    b.set_chat(OTHER, vec![new_chat(OTHER, "周大山")]);
    b.set_history(ME, vec![]);
    b.set_history(FRIEND, friend_history());
    b.set_history(OTHER, vec![]);
    // 第 6 条转不了（模拟来源会话禁止转发）
    b.deny(
        MsgId {
            chat_id: FRIEND,
            message_id: 6,
        },
        Cap::Forward,
    );
    let root = std::env::temp_dir().join(format!(
        "atg-forward-{tag}-{}-{}",
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
    let stream = b.subscribe().unwrap();
    block_on(b.start(&profile)).unwrap();
    b.wait_auth(Duration::from_secs(5), &|s| s == AuthStep::Ready)
        .unwrap();
    Rig {
        b,
        stream,
        dir: profile.dir().to_path_buf(),
    }
}

fn prepare(r: &Rig, to: i64, ids: &[i64]) -> Result<ForwardPlan, CoreError> {
    block_on(prepare_forward(&r.b, to, FRIEND, ids, SendOrigin::Cli))
}

/// 准备 → 开凭据 → 交出 → 等结果
fn once(
    r: &mut Rig,
    to: i64,
    ids: &[i64],
    again: bool,
) -> Result<(ConfirmMode, Vec<ForwardOutcome>), CoreError> {
    let plan = prepare(r, to, ids)?;
    let token = r.b.gate().issue(&plan, again)?;
    forward_and_wait(
        &r.b,
        &plan,
        token,
        &mut r.stream,
        Duration::from_secs(2),
        &mut |_| {},
    )
}

fn log_lines(r: &Rig) -> Vec<Value> {
    std::fs::read_to_string(r.dir.join(DIRECT_LOG_FILE))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn why(e: CoreError) -> InvalidSendWhy {
    match e {
        CoreError::InvalidSend { why } => why,
        other => panic!("不是 invalid_input：{other:?}"),
    }
}

fn refused(e: CoreError) -> SendRefusedReason {
    match e {
        CoreError::SendRefused { reason, .. } => reason,
        other => panic!("不是 send_refused：{other:?}"),
    }
}

#[test]
fn forwards_sorted_deduped_and_logs_source_chat() {
    let mut r = rig("ok", false);
    let (mode, out) = once(&mut r, OTHER, &[7, 5, 7], false).unwrap();
    assert_eq!(mode, ConfirmMode::DirectMode);
    let sources: Vec<i64> = out.iter().map(|o| o.source).collect();
    assert_eq!(sources, vec![5, 7]);
    assert!(
        out.iter()
            .all(|o| matches!(o.result, Some((_, Waited::Succeeded(_)))))
    );
    let sub = r.b.submitted();
    let req = sub.last().unwrap();
    assert_eq!(req["@type"], "forwardMessages");
    assert_eq!(req["message_ids"], json!([5, 7]));
    assert_eq!(req["send_copy"], false);
    // 留痕：交出前一行 + 每条结果一行，都带来源会话 id，不带正文
    let lines = log_lines(&r);
    assert_eq!(lines.len(), 3, "{lines:?}");
    for l in &lines {
        assert_eq!(l["kind"], "forward");
        assert_eq!(l["from_chat_id"], FRIEND);
        assert_eq!(l["chat_id"], OTHER);
        assert!(!l.to_string().contains("第5条"));
    }
}

#[test]
fn plan_json_names_both_chats() {
    let r = rig("plan", false);
    let plan = prepare(&r, OTHER, &[5]).unwrap();
    let j = plan.json();
    assert_eq!(j["action"], "forward");
    assert_eq!(j["target"]["chat_id"], OTHER);
    assert_eq!(j["from"]["chat_id"], FRIEND);
    assert_eq!(j["messages"][0]["message_id"], 5);
    assert_eq!(j["messages"][0]["text"], "第5条");
    assert_eq!(j["confirm"], "direct_mode");
}

#[test]
fn test_dc_only_forwards_to_saved() {
    let mut r = rig("tdc", true);
    let e = prepare(&r, OTHER, &[5]).unwrap_err();
    assert_eq!(refused(e), SendRefusedReason::TestDcSavedOnly);
    let (mode, _) = once(&mut r, ME, &[5], false).unwrap();
    assert_eq!(mode, ConfirmMode::TestExempt);
}

#[test]
fn too_many_is_refused_whole() {
    let r = rig("many", false);
    let ids: Vec<i64> = (1..=21).collect();
    assert_eq!(
        why(prepare(&r, OTHER, &ids).unwrap_err()),
        InvalidSendWhy::ForwardTooMany
    );
    assert!(r.b.submitted().is_empty());
}

#[test]
fn not_forwardable_refuses_the_batch() {
    let r = rig("nofwd", false);
    assert_eq!(
        why(prepare(&r, OTHER, &[5, 6]).unwrap_err()),
        InvalidSendWhy::NotForwardable
    );
}

#[test]
fn missing_message_is_not_found() {
    let r = rig("missing", false);
    assert!(matches!(
        prepare(&r, OTHER, &[99]).unwrap_err(),
        CoreError::NotFound { .. }
    ));
    assert!(matches!(
        prepare(&r, OTHER, &[]).unwrap_err(),
        CoreError::InvalidInput { .. }
    ));
}

#[test]
fn edited_after_prepare_is_refused_before_submit() {
    let r = rig("edited", false);
    let plan = prepare(&r, OTHER, &[5]).unwrap();
    let token = r.b.gate().issue(&plan, false).unwrap();
    let mut h = friend_history();
    h[0] = msg(
        FRIEND,
        5,
        "第5条（改过）",
        json!({"edit_date": 1_792_755_100}),
    );
    r.b.set_history(FRIEND, h);
    let e = block_on(forward(&r.b, &plan, token)).unwrap_err();
    assert_eq!(refused(e), SendRefusedReason::ContentMismatch);
    assert!(r.b.submitted().is_empty());
    assert!(log_lines(&r).is_empty());
}

#[test]
fn same_batch_twice_is_held_unless_again() {
    let mut r = rig("dup", false);
    once(&mut r, OTHER, &[5], false).unwrap();
    let e = once(&mut r, OTHER, &[5], false).unwrap_err();
    assert_eq!(refused(e), SendRefusedReason::DuplicateRecent);
    // 别的一批、别的目标不拦；--again 放行
    once(&mut r, OTHER, &[7], false).unwrap();
    once(&mut r, ME, &[5], false).unwrap();
    once(&mut r, OTHER, &[5], true).unwrap();
}

#[test]
fn each_message_gets_its_own_outcome() {
    let mut r = rig("mixed", false);
    r.b.script_sends(vec![
        FakeSend::Succeed,
        FakeSend::Fail {
            code: 400,
            message: "CHAT_WRITE_FORBIDDEN".into(),
            can_retry: false,
        },
    ]);
    let (_, out) = once(&mut r, OTHER, &[5, 7], false).unwrap();
    assert!(matches!(out[0].result, Some((_, Waited::Succeeded(_)))));
    assert!(matches!(out[1].result, Some((_, Waited::Failed { .. }))));
}

#[test]
fn no_answer_in_time_is_outcome_unknown() {
    let mut r = rig("silent", false);
    r.b.script_sends(vec![FakeSend::Silent]);
    let plan = prepare(&r, OTHER, &[5]).unwrap();
    let token = r.b.gate().issue(&plan, false).unwrap();
    let (_, out) = forward_and_wait(
        &r.b,
        &plan,
        token,
        &mut r.stream,
        Duration::from_millis(300),
        &mut |_| {},
    )
    .unwrap();
    assert!(matches!(
        out[0].result,
        Some((_, Waited::Unknown(UnknownWhy::Timeout)))
    ));
    // 结果未知的同一批 24 小时内照样拦
    let e = once(&mut r, OTHER, &[5], false).unwrap_err();
    assert_eq!(refused(e), SendRefusedReason::DuplicateRecent);
}

#[test]
fn null_at_submit_is_not_forwarded_and_others_still_count() {
    let mut r = rig("null", false);
    let mut h = friend_history();
    h[2] = msg(FRIEND, 7, "第7条", json!({"_forward_null": true}));
    r.b.set_history(FRIEND, h);
    let (_, out) = once(&mut r, OTHER, &[5, 7], false).unwrap();
    assert!(matches!(out[0].result, Some((_, Waited::Succeeded(_)))));
    assert!(out[1].result.is_none());
}

#[test]
fn all_null_at_submit_closes_the_record_as_failed() {
    let mut r = rig("allnull", false);
    let mut h = friend_history();
    h[0] = msg(FRIEND, 5, "第5条", json!({"_forward_null": true}));
    r.b.set_history(FRIEND, h);
    let (_, out) = once(&mut r, OTHER, &[5], false).unwrap();
    assert!(out[0].result.is_none());
    // 一条都没转出：记「失败」收尾，不停在「已提交」——同一批再转不被拦
    let last = log_lines(&r).pop().unwrap();
    assert_eq!(last["result"], "failed");
    let mut h = friend_history();
    h[0] = msg(FRIEND, 5, "第5条", json!({}));
    r.b.set_history(FRIEND, h);
    once(&mut r, OTHER, &[5], false).unwrap();
}
