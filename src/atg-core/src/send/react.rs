//! 表情回应：给一条消息加 / 去掉一个 emoji 回应（#9；契约 `actions.md` 第一节第 3 小节 `react`）。
//!
//! 等级 L2（别人看得到），走同一道闸、同一个交出口；**不确认**（B18-2 人定：正式账号默认模式下也不用确认，
//! 可撤销；直发模式本来就免，D1）。所以计划的 [`Gated::needs_confirm`] 回假，闸开 `unconfirmed` 凭据——
//! 照样验目标与摘要、照样 60 秒一次性、照样受账号受限与限流冷却管、照样写 `direct-send.log`（`kind` 为
//! `react` / `unreact`，不记 emoji 本身）；不进直发防重复（同一个回应点两次 Telegram 那边不会多出一个）。
//! 测试服务器上照样只许收藏夹（`test_dc_saved_only`）。
//!
//! 流程：[`prepare_react`]（L0：自己是谁 → 目标会话 → 这条消息在不在、去掉的回应我点过没有 → 组请求、算摘要）→
//! 闸开凭据 → [`react_within`]（验凭据 → 留痕「已提交」→ 交给 TDLib → 记结果），交出连同应答有
//! [`super::RESULT_TIMEOUT`] 上限，到点按结果未知报。
//!
//! v0 只收 emoji（`reactionTypeEmoji`）；自定义表情、付费星星（要花钱）不做。

use super::{
    Answer, Sealed, SealedKind, SendOrigin, origin_zh, server_of, submit_failed, submit_within,
    target_zh,
};
use crate::backend::Backend;
use crate::error::{CoreError, InvalidSendWhy};
use crate::gate::{self, ConfirmMode, ConfirmToken, Gated, Outcome};
use crate::model::{ChatTarget, MsgId};
use crate::tree::ChatKind;
use crate::tree::ReactionKind;
use serde_json::{Value, json};
use std::time::Duration;

/// 表情回应要不要人确认：B18-2 人定不要（准备时算开法、开凭据与验凭据都只认这一处）
const NEEDS_CONFIRM: bool = false;

/// 一个 emoji 最多几个字符（Unicode 标量）：带肤色、零宽连接的组合 emoji 也就十来个，再长就不是一个 emoji
pub const EMOJI_MAX_CHARS: usize = 16;

/// 一份已定稿的回应：交给 TDLib 的就是 `request`（摘要绑的就是它）
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct ReactPlan {
    target: ChatTarget,
    message: MsgId,
    kind: ReactionKind,
    remove: bool,
    /// 加上时**可能**被挤掉的、我原先点的回应：每条能点几个有上限（非 Telegram Premium 会员 1 个，会员更多，会话也可另设），
    /// 上限多少本地拿不准，所以按最坏情况把我点过的别的全列出来；`--replace` 才放行
    replaces: Vec<ReactionKind>,
    request: Value,
    digest: [u8; 32],
    origin: SendOrigin,
    mode: ConfirmMode,
}

impl ReactPlan {
    pub fn target(&self) -> &ChatTarget {
        &self.target
    }
    pub fn message(&self) -> MsgId {
        self.message
    }
    pub fn kind(&self) -> &ReactionKind {
        &self.kind
    }
    /// 去掉（真）还是加上（假）
    pub fn remove(&self) -> bool {
        self.remove
    }
    /// 加上时可能被挤掉的我原先点的回应（只有 `replace` 放行时才不空）
    pub fn replaces(&self) -> &[ReactionKind] {
        &self.replaces
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

impl gate::Sealed for ReactPlan {}
impl Gated for ReactPlan {
    fn target(&self) -> crate::model::ChatId {
        self.target.id
    }
    fn digest(&self) -> [u8; 32] {
        self.digest
    }
    // 请求里没有每次都变的字段
    fn intent(&self) -> [u8; 32] {
        self.digest
    }
    fn dedup(&self) -> bool {
        false
    }
    fn kind(&self) -> &'static str {
        if self.remove { "unreact" } else { "react" }
    }
    fn needs_confirm(&self) -> bool {
        NEEDS_CONFIRM
    }
    // 不确认就没人当场看着：留痕记下点在哪条上、挤掉了几个（只记个数，不记 emoji）
    fn log_target(&self) -> Option<(i64, usize)> {
        Some((self.message.message_id, self.replaces.len()))
    }
    // 不确认就不弹框；闸要一段说明时（以后人推翻 B18-2）照发文字的样子给
    fn confirm_text(&self) -> String {
        format!(
            "{}{} 里 id {} 那条的回应 {}\n{}",
            if self.remove { "去掉" } else { "加上" },
            target_zh(&self.target),
            self.message.message_id,
            emoji_of(&self.kind),
            origin_zh(self.origin),
        )
    }
}

/// 给人看的 emoji（[`check_emoji`] 已拒掉除变体选择符、零宽连接符外的一切看不见的字符，所以原样给，不转义——
/// 转义会把 ❤️ 显示成 `❤\u{FE0F}`，VoiceOver 也照念）。不是 emoji 的种类给空串
pub fn emoji_of(k: &ReactionKind) -> String {
    match k {
        ReactionKind::Emoji { emoji } => emoji.clone(),
        ReactionKind::CustomEmoji { .. } | ReactionKind::Paid | ReactionKind::Unknown { .. } => {
            String::new()
        }
    }
}

/// 一个 emoji 的形状检查（本地的「明显给错了」防线；是不是这个会话允许的回应由 TDLib 判，不允许回 `invalid_input`）
pub fn check_emoji(emoji: &str) -> Result<(), CoreError> {
    let n = emoji.chars().count();
    // 至少一个非 ASCII 字符（键帽 emoji「1️⃣」开头是 ASCII 数字，所以不整个拒 ASCII）；看不见的字符只许 emoji
    // 自己要用的变体选择符 U+FE0F 与零宽连接符 U+200D（别的会改写显示、藏东西，见 `gate::visible`）
    let rest: String = emoji
        .chars()
        .filter(|c| !matches!(c, '\u{FE0F}' | '\u{200D}'))
        .collect();
    let bad = n == 0
        || n > EMOJI_MAX_CHARS
        || emoji.is_ascii()
        || emoji.chars().any(char::is_whitespace)
        || gate::hidden_count(&rest) > 0;
    if bad {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::ReactionInvalid,
        });
    }
    Ok(())
}

/// 交给 TDLib 的请求（TDLib 形状）。`is_big` 假（不放大动画）；`update_recent_reactions` 假（不改「最近用过」列表）
pub(crate) fn react_request(id: MsgId, kind: &ReactionKind, remove: bool) -> Option<Value> {
    let ty = crate::tdjson::reaction_type(kind)?;
    Some(if remove {
        json!({"@type": "removeMessageReaction", "chat_id": id.chat_id, "message_id": id.message_id,
            "reaction_type": ty})
    } else {
        json!({"@type": "addMessageReaction", "chat_id": id.chat_id, "message_id": id.message_id,
            "reaction_type": ty, "is_big": false, "update_recent_reactions": false})
    })
}

/// 一次点回应要什么（调用方给）
#[derive(Debug, Clone, Copy)]
pub struct ReactAsk<'a> {
    pub emoji: &'a str,
    /// 去掉（真）还是加上（假）
    pub remove: bool,
    /// 可能挤掉我原先点的回应也照点（CLI `--replace`）
    pub replace: bool,
    /// 调用方等到了补发结束（连接线到 `ready`）：本机库里「我点了哪些」才可能是新的
    pub synced: bool,
}

/// `react_prepare`：只查不发。`remove` 时我得点过这个回应（不然 `reaction_not_chosen`）。
///
/// 加上时可能挤掉我原先点的别的回应（每条能点几个有上限，超了 TDLib 自动去掉最早的）：不确认就没人看得到，所以
/// 我点过别的就拒（`reaction_would_replace`）；**我点过哪些拿不准**也拒（`reaction_state_unknown`）——本机库里的是缓存，
/// 人刚在手机上点的可能还没到（自审第 3 轮 P1）。拿得准的条件：调用方等到了补发结束（`synced`），且私聊、普通群
/// （回应变化 TDLib 照推）；超级群、频道没打开时不一定推，另向服务器取一次「谁点了」（`getMessageAddedReactions`）里
/// 我点的那几个，取不到或一页没取全就算拿不准。`replace` 为真两种都放行，计划里列出已知的
pub async fn prepare_react<B: Backend>(
    b: &B,
    id: MsgId,
    ask: ReactAsk<'_>,
    origin: SendOrigin,
) -> Result<ReactPlan, CoreError> {
    check_emoji(ask.emoji)?;
    let kind = ReactionKind::Emoji {
        emoji: ask.emoji.to_owned(),
    };
    let me = b.me().await?;
    let mut target = b.chat_target(id.chat_id).await?;
    target.saved = target.id == me.saved_chat_id;
    let mode = b.gate().mode_for(target.id, NEEDS_CONFIRM)?;
    let m = b.message_get(id).await?;
    if ask.remove && !m.reactions.iter().any(|r| r.chosen && r.kind == kind) {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::ReactionNotChosen,
        });
    }
    let mut replaces: Vec<ReactionKind> = Vec::new();
    if !ask.remove {
        let (mine, known) = my_reactions(b, id, &target, &m, ask.synced).await;
        replaces = mine.into_iter().filter(|k| *k != kind).collect();
        let why = if !replaces.is_empty() {
            Some(InvalidSendWhy::ReactionWouldReplace)
        } else if !known {
            Some(InvalidSendWhy::ReactionStateUnknown)
        } else {
            None
        };
        if let (Some(why), false) = (why, ask.replace) {
            return Err(CoreError::InvalidSend { why });
        }
    }
    let request = react_request(id, &kind, ask.remove).ok_or(CoreError::Internal {
        what: "react_request_shape",
    })?;
    let digest = gate::digest(&request, me.user_id, server_of(b)?);
    Ok(ReactPlan {
        target,
        message: id,
        kind,
        remove: ask.remove,
        replaces,
        request,
        digest,
        origin,
        mode,
    })
}

/// 我在这条上点了哪些回应（去重），以及拿不拿得准（见 [`prepare_react`]）
async fn my_reactions<B: Backend>(
    b: &B,
    id: MsgId,
    target: &ChatTarget,
    m: &crate::model::Message,
    synced: bool,
) -> (Vec<ReactionKind>, bool) {
    let mut mine: Vec<ReactionKind> = m
        .reactions
        .iter()
        .filter(|r| r.chosen)
        .map(|r| r.kind.clone())
        .collect();
    let pushed = matches!(
        target.kind,
        ChatKind::Private | ChatKind::Group | ChatKind::Secret
    );
    let mut known = synced && pushed;
    if !pushed && let Ok(page) = b.added_reactions(id, None, "", SERVER_PAGE).await {
        for a in page.items.iter().filter(|a| a.outgoing) {
            if !mine.contains(&a.kind) {
                mine.push(a.kind.clone());
            }
        }
        // 一页取全了才算数（我点的可能落在没取到的那页）
        known = synced && page.next_offset.is_empty();
    }
    (mine, known)
}

/// 向服务器取「谁点了」时一页要多少（TDLib 上限 100）
const SERVER_PAGE: u32 = 100;

/// `react`（L2）：验凭据 → 留痕「已提交」→ 交给 TDLib → 记结果。成功回凭据的开法
pub async fn react<B: Backend>(
    b: &B,
    plan: &ReactPlan,
    token: ConfirmToken,
) -> Result<ConfirmMode, CoreError> {
    let mode = b.gate().verify(token, plan)?;
    b.gate().before_submit(mode, plan)?;
    let sealed = Sealed {
        kind: SealedKind::React {
            request: &plan.request,
        },
    };
    match crate::send_limits::submit(b, sealed).await {
        Ok(Answer::Done) => {
            b.gate()
                .after_outcome(mode, plan, 0, None, Outcome::Succeeded, None);
            Ok(mode)
        }
        Ok(Answer::Message(_) | Answer::Resent(_)) => Err(CoreError::Internal {
            what: "react_answer_shape",
        }),
        Err(e) => Err(submit_failed(b, mode, plan, e)),
    }
}

/// [`react`] 连同应答最多等 `timeout`；到点按结果未知记账、报 `send_outcome_unknown`（回应可能已经点上）
pub fn react_within<B: Backend>(
    b: &B,
    plan: &ReactPlan,
    token: ConfirmToken,
    timeout: Duration,
) -> Result<ConfirmMode, CoreError> {
    submit_within(b, plan, plan.mode, timeout, react(b, plan, token))
}

#[cfg(test)]
mod tests;
