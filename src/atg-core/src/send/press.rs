//! 按按钮：按一条消息上的第 N 个按钮（#13 13b-2；契约 `actions.md` 第一节第 3 小节「按按钮」）。
//!
//! 按种类分流（[`prepare_press`] 回 [`Press`]）：
//! - **回调按钮**（`callback`）：L2，过闸、**直发**（#113），进直发防重复（人定 2026-11-17 #13：同一条消息同一个按钮再按
//!   拦下，已按下的看 10 分钟、结果未知的看 24 小时，`--again` 放行），留痕 `kind:"press"`、`final_id` 记那条消息的 id。**回调数据不进计划、摘要、留痕**：
//!   请求只有会话、消息、第几个按钮与按钮文字，交出口在 td 层现取那条消息、核对文字、拿数据再交（`parse_request`）。
//!   机器人的回话经 [`Answer::Callback`] 回来；机器人没回话报成功（大总管定 2026-11-24）
//! - **键盘文字按钮**（`text`）：按下就是以你的名义发出按钮上的字——调用方拿 [`Press::Keyboard`] 的文字走 `send_text`
//!   （闸、防重复、留痕照 `send`）。不是私聊时回复键盘所在那条（机器人隐私模式下才收得到）。会话的键盘已换成别的消息上的、
//!   已收起的，照样发、计划里说一句（人定 B，2026-11-24）
//! - **网址 / 登录网址 / 复制文字 / 用户**：只读出，不按（人定）
//! - 其余一律拒（`button_not_supported`）
//!
//! 测试服务器上照样只许收藏夹（回调按钮在准备时由闸判；键盘文字按钮在 `send_text` 里判）。

use super::target_json;
use super::{Answer, CallbackReply, Sealed, SealedKind, SendOrigin, server_of, submit_failed_for};
use crate::backend::Backend;
use crate::error::{CoreError, InvalidSendWhy};
use crate::gate::{self, ConfirmMode, ConfirmToken, Gated, Outcome};
use crate::model::{ChatTarget, Message, MsgId, SenderId};
use crate::tree::{Button, ButtonAction, KeyboardKind};
use serde_json::{Value, json};
use std::time::Duration;

/// 一份已定稿的「按回调按钮」：交给闸与交出口的就是 `request`（摘要绑的就是它；不带回调数据）
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct CallbackPlan {
    target: ChatTarget,
    message: MsgId,
    /// 第几个按钮（1 起，跨行连续数）
    button: usize,
    text: String,
    /// 发这条消息的（一般是机器人）：计划里写明按的是谁的按钮
    sender: SenderId,
    request: Value,
    digest: [u8; 32],
    origin: SendOrigin,
    mode: ConfirmMode,
}

impl CallbackPlan {
    pub fn target(&self) -> &ChatTarget {
        &self.target
    }
    pub fn message(&self) -> MsgId {
        self.message
    }
    pub fn button(&self) -> usize {
        self.button
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn sender(&self) -> &SenderId {
        &self.sender
    }
    pub fn mode(&self) -> ConfirmMode {
        self.mode
    }
    pub fn origin(&self) -> SendOrigin {
        self.origin
    }
    pub fn digest_hex(&self) -> String {
        gate::hex(&self.digest)
    }
}

impl gate::Sealed for CallbackPlan {}
impl Gated for CallbackPlan {
    fn target(&self) -> crate::model::ChatId {
        self.target.id
    }
    fn digest(&self) -> [u8; 32] {
        self.digest
    }
    // 请求里没有每次都变的字段：同一条消息同一个按钮 = 同一个意图（人定：进防重复）
    fn intent(&self) -> [u8; 32] {
        self.digest
    }
    fn dedup(&self) -> bool {
        true
    }
    fn kind(&self) -> &'static str {
        "press"
    }
}

/// 按下这个按钮会怎样（[`prepare_press`] 的结果）
#[cfg_attr(test, derive(Debug))]
pub enum Press {
    /// 回调按钮：要过闸再交出
    Callback(CallbackPlan),
    /// 键盘文字按钮：调用方用 `text` 走 `send_text`。`reply`：不是私聊时要回复键盘所在那条（`message`）——同官方客户端；
    /// 机器人默认开隐私模式，群里只收得到命令与回复它的消息（自审 13b-2 第 1 轮 P2）。`keyboard`：会话此刻的键盘在哪
    Keyboard {
        target: ChatTarget,
        text: String,
        message: MsgId,
        reply: bool,
        keyboard: KeyboardNow,
    },
    /// 只读出、不按：网址、登录网址、复制文字、用户
    ReadOnly { button: Button },
}

/// 会话此刻的键盘（键盘是会话级的，同一时刻只有一套，`reply_markup_message_id` 标在哪条上）
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardNow {
    /// 就在这条上
    Here,
    /// 已换成别的消息上的那套：照原文字发（人定 B），计划说「键盘已更换」
    Replaced,
    /// 已收起，此刻没有键盘：照原文字发，计划说「键盘已收起」
    Hidden,
}

/// 这条消息此刻显示的按钮里的第 `n` 个（显示的是临时内容时原按钮不算，同 `project`）
fn nth(m: &Message, n: usize) -> Result<(KeyboardKind, Button), CoreError> {
    let not_found = CoreError::InvalidSend {
        why: InvalidSendWhy::ButtonNotFound,
    };
    let k = match (&m.ephemeral, &m.buttons) {
        (None, Some(k)) => k,
        _ => return Err(not_found),
    };
    // 认不出的按钮区：读法说「有暂不支持的按钮」，这里同样说不支持，不说「没有这个按钮」（自审第 3 轮）
    if matches!(k.kind, KeyboardKind::Unknown { .. }) {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::ButtonNotSupported,
        });
    }
    let b = k.nth(n).ok_or(not_found)?;
    Ok((k.kind.clone(), b.clone()))
}

/// 交给闸与交出口的请求（本应用自己的形状，不是 TDLib 的：交出口据它现取数据再调 `getCallbackQueryAnswer`）
fn callback_request(id: MsgId, n: usize, text: &str) -> Value {
    json!({"@type": "pressCallback", "chat_id": id.chat_id, "message_id": id.message_id, "button": n, "text": text})
}

/// TDLib 的这个错 = 机器人没回话（按下已送到，大总管定报成功）。交出口用。TDLib 把服务器的 `BOT_RESPONSE_TIMEOUT`
/// 改写成 502「The bot is not responding」再交上来（`CallbackQueriesManager.cpp` `GetBotCallbackAnswerQuery::on_error`，
/// 自审 13b-2 第 2 轮 P1）；原串也认，防 TDLib 以后不改写
// 只有真后端（`tdlib-api`）的交出口调；不编 TDLib 时只有单测用它
#[cfg_attr(not(feature = "tdlib-api"), allow(dead_code))]
pub(crate) fn bot_silent(code: i32, message: &str) -> bool {
    (code == 502 && message == "The bot is not responding")
        || message.contains("BOT_RESPONSE_TIMEOUT")
}

/// 交出口拆请求：（会话、消息 id、第几个按钮、按钮文字）。真后端、假后端共用
pub(crate) fn parse_request(request: &Value) -> Result<(i64, i64, usize, String), CoreError> {
    let bad = || CoreError::Internal {
        what: "press_request_shape",
    };
    Ok((
        request["chat_id"].as_i64().ok_or_else(bad)?,
        request["message_id"].as_i64().ok_or_else(bad)?,
        request["button"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(bad)?,
        request["text"].as_str().ok_or_else(bad)?.to_owned(),
    ))
}

/// `press_prepare`：只查不发。`n` 是详情里读的第几个按钮（1 起）。没有这个按钮 `button_not_found`；不按的种类
/// `button_not_supported`；测试服务器上回调按钮只许收藏夹
pub async fn prepare_press<B: Backend>(
    b: &B,
    id: MsgId,
    n: usize,
    origin: SendOrigin,
) -> Result<Press, CoreError> {
    let me = b.me().await?;
    let mut target = b.chat_target(id.chat_id).await?;
    target.saved = target.id == me.saved_chat_id;
    let m = b.message_get(id).await?;
    let (kind, button) = nth(&m, n)?;
    let unsupported = Err(CoreError::InvalidSend {
        why: InvalidSendWhy::ButtonNotSupported,
    });
    match (&kind, &button.action) {
        (KeyboardKind::Inline, ButtonAction::Callback) => {
            let mode = b.gate().mode_for(target.id)?;
            let request = callback_request(id, n, &button.text);
            let digest = gate::digest(&request, me.user_id, server_of(b)?);
            Ok(Press::Callback(CallbackPlan {
                target,
                message: id,
                button: n,
                text: button.text,
                sender: m.sender,
                request,
                digest,
                origin,
                mode,
            }))
        }
        (KeyboardKind::Reply, ButtonAction::Text) => {
            let keyboard = match target.keyboard_message_id {
                k if k == id.message_id => KeyboardNow::Here,
                0 => KeyboardNow::Hidden,
                _ => KeyboardNow::Replaced,
            };
            let reply = target.kind != crate::tree::ChatKind::Private;
            Ok(Press::Keyboard {
                target,
                text: button.text,
                message: id,
                reply,
                keyboard,
            })
        }
        (
            _,
            ButtonAction::Url { .. }
            | ButtonAction::LoginUrl { .. }
            | ButtonAction::CopyText { .. }
            | ButtonAction::User { .. },
        ) => Ok(Press::ReadOnly { button }),
        _ => unsupported,
    }
}

/// 交出前复核：计划可能是早先准备的，机器人可能已经改了按钮。再取一次那条消息，那个位置上还得是同样文字的回调按钮，
/// 不然 `changed`（交出口在 td 层还会再核一次文字，拿数据时）
async fn recheck<B: Backend>(b: &B, plan: &CallbackPlan) -> Result<(), CoreError> {
    let m = b.message_get(plan.message).await?;
    let same = matches!(
        nth(&m, plan.button),
        Ok((KeyboardKind::Inline, ref x)) if x.action == ButtonAction::Callback && x.text == plan.text
    );
    if !same {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::Changed,
        });
    }
    Ok(())
}

/// `press`（L2）：验凭据 → 复核 → 留痕「已提交」（直发防重复在这里登记）→ 交给 TDLib → 记结果。
/// 成功回（凭据的开法，机器人的回话；`None` = 机器人没回话）
pub async fn press<B: Backend>(
    b: &B,
    plan: &CallbackPlan,
    token: ConfirmToken,
) -> Result<(ConfirmMode, Option<CallbackReply>), CoreError> {
    press_inner(b, plan, token, &std::sync::atomic::AtomicBool::new(false)).await
}

async fn press_inner<B: Backend>(
    b: &B,
    plan: &CallbackPlan,
    token: ConfirmToken,
    sent: &std::sync::atomic::AtomicBool,
) -> Result<(ConfirmMode, Option<CallbackReply>), CoreError> {
    // 进防重复：`--again` 的放行绑在这张凭据上，一路带到交出前那次查（同 send_text / send_file，#77）
    let (mode, again) = b.gate().verify_again(token, plan)?;
    recheck(b, plan).await?;
    b.gate().before_submit_again(mode, plan, again)?;
    let sealed = Sealed {
        kind: SealedKind::Callback {
            request: &plan.request,
        },
    };
    sent.store(true, std::sync::atomic::Ordering::SeqCst);
    let id = Some(plan.message.message_id);
    match crate::send_limits::submit(b, sealed).await {
        Ok(Answer::Callback(reply)) => {
            b.gate()
                .after_outcome(mode, plan, 0, id, Outcome::Succeeded, None);
            Ok((mode, reply))
        }
        Ok(Answer::Message(_) | Answer::Resent(_) | Answer::Done | Answer::Deleted) => {
            Err(CoreError::Internal {
                what: "press_answer_shape",
            })
        }
        Err(e) => Err(submit_failed_for(b, mode, plan, e, id)),
    }
}

/// [`press`] 连同应答最多等 `timeout`。到点时：已交出 → 记「结果未知」、报 `send_outcome_unknown`（可能已按下）；
/// 还没交出（卡在复核里）→ 报 `network`，可以重试
pub fn press_within<B: Backend>(
    b: &B,
    plan: &CallbackPlan,
    token: ConfirmToken,
    timeout: Duration,
) -> Result<(ConfirmMode, Option<CallbackReply>), CoreError> {
    let sent = std::sync::atomic::AtomicBool::new(false);
    if let Some(r) = crate::link::block_on_timeout(press_inner(b, plan, token, &sent), timeout) {
        return r;
    }
    if sent.load(std::sync::atomic::Ordering::SeqCst) {
        let id = Some(plan.message.message_id);
        b.gate()
            .after_outcome(plan.mode, plan, 0, id, Outcome::Unknown, None);
        Err(CoreError::SendOutcomeUnknown {
            chat_id: Some(plan.target.id),
            temp_message_id: 0,
            why: crate::error::UnknownWhy::Timeout,
        })
    } else {
        Err(CoreError::Network { td: None })
    }
}

/// 计划行（`kind:"plan"` 的 `data`，`actions.md` 第一节第 6 小节 `CallbackPlan`）。不带回调数据
pub fn press_plan_json(plan: &CallbackPlan) -> Value {
    let sender = match &plan.sender {
        SenderId::User(u) => json!({"type": "user", "user_id": u}),
        SenderId::Chat(c) => json!({"type": "chat", "chat_id": c}),
        SenderId::Unknown { .. } => json!({"type": "unknown"}),
    };
    json!({
        "action": "press",
        "target": target_json(plan.target()),
        "message_id": plan.message().message_id,
        "button": plan.button(),
        "text": plan.text(),
        "sender": sender,
        "origin": plan.origin(),
        "confirm": plan.mode(),
        "digest": plan.digest_hex(),
    })
}

#[cfg(test)]
mod tests;
