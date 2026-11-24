//! `send::press` 的单测：假后端走全流程（数据全是编的）

use super::*;
use crate::config::{Profile, ServerKind};
use crate::error::{Holder, SendRefusedReason};
use crate::fake::{FakeAuth, FakeBackend, FakeSend};
use crate::gate::{DIRECT_LOG_FILE, Gate};
use crate::link::{EventStream, block_on};
use crate::status::AuthStep;
use std::path::PathBuf;

const ME: i64 = 4242;
const BOT: i64 = 900;
/// 回调数据：只许出现在交给 TDLib 的那份里
const DATA: &str = "c2VjcmV0LWRhdGE=";

struct Rig {
    b: FakeBackend,
    _stream: EventStream,
    dir: PathBuf,
}

fn new_chat(id: i64, title: &str, keyboard_at: i64) -> Value {
    json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title,
        "type": {"@type": "chatTypePrivate", "user_id": id}, "positions": [], "notification_settings": {},
        "reply_markup_message_id": keyboard_at}})
}

fn button(text: &str, ty: Value) -> Value {
    json!({"@type": "inlineKeyboardButton", "text": text, "type": ty})
}

/// 机器人发的一条：内联按钮「确认」（回调）、「官网」（网址）、「删号」（要密码的回调）
fn inline_msg(chat: i64, id: i64, first: &str) -> Value {
    json!({"@type": "message", "id": id, "chat_id": chat,
        "sender_id": {"@type": "messageSenderUser", "user_id": BOT}, "is_outgoing": false, "date": 1_792_755_000,
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": "选一个", "entities": []}},
        "reply_markup": {"@type": "replyMarkupInlineKeyboard", "rows": [
            [button(first, json!({"@type": "inlineKeyboardButtonTypeCallback", "data": DATA})),
             button("官网", json!({"@type": "inlineKeyboardButtonTypeUrl", "url": "https://example.org/a"}))],
            [button("删号", json!({"@type": "inlineKeyboardButtonTypeCallbackWithPassword", "data": DATA}))]]}})
}

/// 机器人换掉输入框的键盘：「好的」（文字）、「交手机号」
fn reply_msg(chat: i64, id: i64) -> Value {
    json!({"@type": "message", "id": id, "chat_id": chat,
        "sender_id": {"@type": "messageSenderUser", "user_id": BOT}, "is_outgoing": false, "date": 1_792_755_000,
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": "请回答", "entities": []}},
        "reply_markup": {"@type": "replyMarkupShowKeyboard", "rows": [[
            {"@type": "keyboardButton", "text": "好的", "type": {"@type": "keyboardButtonTypeText"}},
            {"@type": "keyboardButton", "text": "交手机号", "type": {"@type": "keyboardButtonTypeRequestPhoneNumber"}}]]}})
}

fn rig(tag: &str, test_dc: bool, keyboard_at: i64) -> Rig {
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
    b.set_chat(ME, vec![new_chat(ME, "我自己", keyboard_at)]);
    b.set_chat(BOT, vec![new_chat(BOT, "订票助手", keyboard_at)]);
    for chat in [ME, BOT] {
        b.set_history(chat, vec![inline_msg(chat, 5, "确认"), reply_msg(chat, 6)]);
    }
    let root = crate::testkit::temp_dir(&format!("press-{tag}"));
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
        _stream: stream,
        dir: profile.dir().to_path_buf(),
    }
}

fn id(chat: i64, message_id: i64) -> MsgId {
    MsgId {
        chat_id: chat,
        message_id,
    }
}

fn prepare(r: &Rig, chat: i64, msg: i64, n: usize) -> Result<Press, CoreError> {
    block_on(prepare_press(&r.b, id(chat, msg), n, SendOrigin::Cli))
}

fn callback(r: &Rig, chat: i64, n: usize) -> CallbackPlan {
    match prepare(r, chat, 5, n).unwrap() {
        Press::Callback(p) => p,
        other => panic!("{other:?}"),
    }
}

fn press_once(r: &Rig, plan: &CallbackPlan, again: bool) -> PressResult {
    let token = r.b.gate().issue(plan, again)?;
    press_within(&r.b, plan, token, Duration::from_secs(2))
}

type PressResult = Result<(ConfirmMode, Option<CallbackReply>), CoreError>;

fn why(e: CoreError) -> InvalidSendWhy {
    match e {
        CoreError::InvalidSend { why } => why,
        other => panic!("{other:?}"),
    }
}

fn log_text(r: &Rig) -> String {
    std::fs::read_to_string(r.dir.join(DIRECT_LOG_FILE)).unwrap_or_default()
}

#[test]
fn callback_reads_the_reply_and_hands_data_only_to_tdlib() {
    let r = rig("cb", false, 0);
    r.b.script_callback(Some(CallbackReply {
        text: "已订好".into(),
        alert: true,
        url: String::new(),
    }));
    let plan = callback(&r, BOT, 1);
    let j = press_plan_json(&plan);
    assert_eq!(j["action"], "press");
    assert_eq!(j["text"], "确认");
    assert_eq!(j["sender"], json!({"type": "user", "user_id": BOT}));
    let (mode, reply) = press_once(&r, &plan, false).unwrap();
    assert_eq!(mode, ConfirmMode::DirectMode);
    let reply = reply.unwrap();
    assert_eq!(reply.text, "已订好");
    assert!(reply.alert);
    let sub = r.b.submitted();
    assert_eq!(sub.len(), 1);
    assert_eq!(sub[0]["@type"], "getCallbackQueryAnswer");
    assert_eq!(sub[0]["payload"]["data"], DATA);
    // 回调数据不进计划、摘要、留痕
    assert!(!j.to_string().contains(DATA));
    let log = log_text(&r);
    assert!(log.contains("\"kind\":\"press\""), "{log}");
    assert!(log.contains("\"final_id\":5"), "{log}");
    assert!(
        !log.contains(DATA) && !log.contains("确认") && !log.contains("已订好"),
        "{log}"
    );
}

/// 机器人没回话：报成功（按下已送到），回话为空
#[test]
fn no_reply_is_success() {
    let r = rig("silent", false, 0);
    let plan = callback(&r, BOT, 1);
    let (_, reply) = press_once(&r, &plan, false).unwrap();
    assert!(reply.is_none());
}

/// 人定：10 分钟内同一条消息同一个按钮再按拦下，`--again` 放行
#[test]
fn same_button_twice_is_blocked_unless_again() {
    let r = rig("dup", false, 0);
    let plan = callback(&r, BOT, 1);
    press_once(&r, &plan, false).unwrap();
    let again = callback(&r, BOT, 1);
    let e = press_once(&r, &again, false).unwrap_err();
    assert!(
        matches!(
            e,
            CoreError::SendRefused {
                reason: SendRefusedReason::DuplicateRecent,
                ..
            }
        ),
        "{e:?}"
    );
    let ok = press_once(&r, &again, true);
    assert!(ok.is_ok(), "{ok:?}");
    assert_eq!(r.b.submitted().len(), 2);
}

/// 准备之后机器人改了按钮：交出前复核拦下，什么都没按
#[test]
fn changed_button_is_refused_before_submit() {
    let r = rig("changed", false, 0);
    let plan = callback(&r, BOT, 1);
    let token = r.b.gate().issue(&plan, false).unwrap();
    r.b.set_history(BOT, vec![inline_msg(BOT, 5, "取消")]);
    let e = press_within(&r.b, &plan, token, Duration::from_secs(2)).unwrap_err();
    assert_eq!(why(e), InvalidSendWhy::Changed);
    assert!(r.b.submitted().is_empty());
}

#[test]
fn test_dc_other_chat_is_refused() {
    let r = rig("tdc", true, 0);
    let e = prepare(&r, BOT, 5, 1).unwrap_err();
    assert!(matches!(
        e,
        CoreError::SendRefused {
            reason: SendRefusedReason::TestDcSavedOnly,
            ..
        }
    ));
    // 收藏夹照样能按（测试服务器免确认）
    let plan = callback(&r, ME, 1);
    assert_eq!(
        press_once(&r, &plan, false).unwrap().0,
        ConfirmMode::TestExempt
    );
}

#[test]
fn kinds_route_to_readonly_keyboard_or_refusal() {
    let r = rig("kinds", false, 6);
    assert!(matches!(
        prepare(&r, BOT, 5, 2).unwrap(),
        Press::ReadOnly { button } if matches!(button.action, ButtonAction::Url { .. })
    ));
    assert_eq!(
        why(prepare(&r, BOT, 5, 3).unwrap_err()),
        InvalidSendWhy::ButtonNotSupported
    );
    assert_eq!(
        why(prepare(&r, BOT, 5, 4).unwrap_err()),
        InvalidSendWhy::ButtonNotFound
    );
    assert_eq!(
        why(prepare(&r, BOT, 5, 0).unwrap_err()),
        InvalidSendWhy::ButtonNotFound
    );
    // 键盘文字按钮：交给调用方走 send_text；会话的键盘就在这条上 = 没换过
    match prepare(&r, BOT, 6, 1).unwrap() {
        Press::Keyboard {
            text,
            keyboard,
            reply,
            ..
        } => {
            assert_eq!(text, "好的");
            assert_eq!(keyboard, KeyboardNow::Here);
            assert!(!reply, "私聊不用回复");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        why(prepare(&r, BOT, 6, 2).unwrap_err()),
        InvalidSendWhy::ButtonNotSupported
    );
    assert!(r.b.submitted().is_empty());
}

/// 人定 B：键盘已换到别的消息上 / 已收起，照样给文字，计划里说一句（收起不说成更换，自审第 1 轮 P2）
#[test]
fn replaced_or_hidden_keyboard_still_presses() {
    let r = rig("hidden", false, 0);
    match prepare(&r, BOT, 6, 1).unwrap() {
        Press::Keyboard { keyboard, .. } => assert_eq!(keyboard, KeyboardNow::Hidden),
        other => panic!("{other:?}"),
    }
    let r = rig("replaced", false, 99);
    match prepare(&r, BOT, 6, 1).unwrap() {
        Press::Keyboard { keyboard, .. } => assert_eq!(keyboard, KeyboardNow::Replaced),
        other => panic!("{other:?}"),
    }
}

#[test]
fn rejected_submit_is_logged_as_failed_with_message_id() {
    let r = rig("reject", false, 0);
    r.b.script_sends(vec![FakeSend::Reject(CoreError::InvalidInput { td: None })]);
    let plan = callback(&r, BOT, 1);
    let e = press_once(&r, &plan, false).unwrap_err();
    assert_eq!(e.code(), "invalid_input");
    let log = log_text(&r);
    assert!(log.contains("\"result\":\"failed\""), "{log}");
    assert!(log.contains("\"final_id\":5"), "{log}");
}

/// 自审 13b-2 第 2 轮 P1：TDLib 收到服务器的 `BOT_RESPONSE_TIMEOUT` 先改写成 502「The bot is not responding」再交上来
/// （`CallbackQueriesManager.cpp` `GetBotCallbackAnswerQuery::on_error`）。认不出就会按「失败」报，而按下其实已送到——
/// 失败不进防重复，人再按一次机器人就收到两次
#[test]
fn bot_silence_is_recognised_as_tdlib_reports_it() {
    assert!(bot_silent(502, "The bot is not responding"));
    assert!(bot_silent(400, "BOT_RESPONSE_TIMEOUT"));
    assert!(!bot_silent(400, "MESSAGE_ID_INVALID"));
    assert!(!bot_silent(502, "Bad Gateway"));
}

/// 自审 13b-2 第 3 轮：认不出的按钮区（读法说「有暂不支持的按钮」）按的时候报「不支持」，不报「没有这个按钮」
#[test]
fn unknown_keyboard_kind_is_not_supported() {
    let r = rig("unknown-kb", false, 0);
    let mut m = inline_msg(BOT, 5, "确认");
    m["reply_markup"] = json!({"@type": "replyMarkupFutureThing", "rows": []});
    r.b.set_history(BOT, vec![m]);
    assert_eq!(
        why(prepare(&r, BOT, 5, 1).unwrap_err()),
        InvalidSendWhy::ButtonNotSupported
    );
}

/// codex 065 第 1 轮 #1：防重复认「同一条消息同一个按钮」（人定），不认按钮文字——机器人按下后常把「提交」改成「处理中」、
/// 数据照旧，换了字再按也得拦下，`--again` 才放行
#[test]
fn relabelled_button_is_still_the_same_button() {
    let r = rig("relabel", false, 0);
    let plan = callback(&r, BOT, 1);
    press_once(&r, &plan, false).unwrap();
    r.b.set_history(BOT, vec![inline_msg(BOT, 5, "处理中"), reply_msg(BOT, 6)]);
    let again = callback(&r, BOT, 1);
    assert_eq!(again.text(), "处理中");
    let e = press_once(&r, &again, false).unwrap_err();
    assert!(
        matches!(
            e,
            CoreError::SendRefused {
                reason: SendRefusedReason::DuplicateRecent,
                ..
            }
        ),
        "{e:?}"
    );
    assert!(press_once(&r, &again, true).is_ok());
}
