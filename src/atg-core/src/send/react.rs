//! 表情回应：给一条消息加 / 去掉一个 emoji 回应（#9；契约 `actions.md` 第一节第 3 小节 `react`）。
//!
//! 等级 L2（别人看得到），走同一道闸、同一个交出口，和发文字一样**直发、不确认**（人定 2026-11-16 去掉 Touch ID，#113）：
//! 照样验目标与摘要、照样 60 秒一次性、照样受账号受限与限流冷却管、照样写 `direct-send.log`（`kind` 为
//! `react` / `unreact`，另记消息 id 与可能挤掉几个，不记 emoji 本身）；不进直发防重复（同一个回应点两次
//! Telegram 那边不会多出一个）。测试服务器上照样只许收藏夹（`test_dc_saved_only`）。
//!
//! 流程：[`prepare_react`]（L0：自己是谁 → 目标会话 → 这条消息在不在、去掉的回应我点过没有 → 组请求、算摘要）→
//! 闸开凭据 → [`react_within`]（验凭据 → 留痕「已提交」→ 交给 TDLib → 记结果），交出连同应答有
//! [`super::RESULT_TIMEOUT`] 上限，到点按结果未知报。
//!
//! v0 只收 emoji（`reactionTypeEmoji`）；自定义表情、付费星星（要花钱）不做。

use super::{Answer, Sealed, SealedKind, SendOrigin, server_of, submit_failed, target_json};
use crate::backend::Backend;
use crate::error::{CoreError, InvalidSendWhy};
use crate::gate::{self, ConfirmMode, ConfirmToken, Gated, Outcome};
use crate::model::{ChatTarget, MsgId};
use crate::tree::ChatKind;
use crate::tree::ReactionKind;
use serde_json::{Value, json};
use std::time::Duration;

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
    /// 「我原先点了哪些」拿不拿得准（拿不准还放行只能是 `--replace`；留痕的 `reaction.may_replace` 记 `null`）
    replaces_known: bool,
    /// 准备时给的 `replace` / `synced`：交出前按同样的条件再判一次（codex 032 第 1 轮 #1）
    replace: bool,
    synced: bool,
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
    /// [`Self::replaces`] 是不是全的：假 = 拿不准我点过哪些（判法见 [`prepare_react`]；私聊、加密聊天只看本机，不问服务器）
    pub fn replaces_known(&self) -> bool {
        self.replaces_known
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
    // 没人当场看着：留痕记下点在哪条上、按最坏情况可能挤掉几个（只记个数，不记 emoji；拿不准记 null；去掉回应恒为 0）
    fn log_target(&self) -> Option<(i64, Option<usize>)> {
        Some((
            self.message.message_id,
            self.replaces_known.then_some(self.replaces.len()),
        ))
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
/// 人刚在手机上点的可能还没到（自审第 3 轮 P1）。拿得准的条件：调用方等到了补发结束（`synced`），且只有私聊、
/// 加密聊天（TDLib 照推回应变化；`MessagesManager.cpp` 里普通群、超级群、频道都靠打开时轮询，自审第 4 轮 P1）；
/// 其余另向服务器取一次「谁点了」（`getMessageAddedReactions`）里我点的那几个，取不到或一页没取全就算拿不准。
/// 服务器给全了也还要 `synced`：服务器名单只补「我点了」，本机缓存里刚取消、还没同步掉的那个会被误当成仍点着
/// ——误报只会多拒，但没同步完时整个本机视图都不可信，宁可一并按拿不准处理。
/// `replace` 为真两种都放行，计划里列出已知的（拿不准时 `replaces_known` 假）。
///
/// 去掉（`remove`）只看本机缓存判我点没点过：判错只会误报 `reaction_not_chosen` 或交给 TDLib 一个无效果的去掉，
/// 不会动到别的回应，所以不另问服务器
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
    let mode = b.gate().mode_for(target.id)?;
    let m = b.message_get(id).await?;
    if ask.remove && !m.reactions.iter().any(|r| r.chosen && r.kind == kind) {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::ReactionNotChosen,
        });
    }
    let (replaces, replaces_known) = if ask.remove {
        (Vec::new(), true)
    } else {
        let (replaces, known) = would_replace(b, id, &target, &kind, &m, ask.synced).await;
        if !ask.replace {
            refuse_replace(&replaces, known)?;
        }
        (replaces, known)
    };
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
        replaces_known,
        replace: ask.replace,
        synced: ask.synced,
        request,
        digest,
        origin,
        mode,
    })
}

/// 加上 `kind` 可能挤掉的我原先点的回应，以及拿不拿得准（见 [`prepare_react`]）
async fn would_replace<B: Backend>(
    b: &B,
    id: MsgId,
    target: &ChatTarget,
    kind: &ReactionKind,
    m: &crate::model::Message,
    synced: bool,
) -> (Vec<ReactionKind>, bool) {
    let (mine, known) = my_reactions(b, id, target, m, synced).await;
    (mine.into_iter().filter(|k| k != kind).collect(), known)
}

/// 没给 `replace` 时：可能挤掉、或拿不准，都拒
fn refuse_replace(replaces: &[ReactionKind], known: bool) -> Result<(), CoreError> {
    let why = if !replaces.is_empty() {
        InvalidSendWhy::ReactionWouldReplace
    } else if !known {
        InvalidSendWhy::ReactionStateUnknown
    } else {
        return Ok(());
    };
    Err(CoreError::InvalidSend { why })
}

/// 交出前复核（codex 032 第 1 轮 #1、第 2 轮 #1 #3）：计划可能是早先准备的，期间我又点了别的（先后准备 👍、❤️ 两份
/// 计划再依次交出，第二份就会挤掉第一份），也可能断过线、补发还没完。所以加回应在交出前按同样的条件再判一次：
/// 「补发完了」要准备时完了**且**现在连接线仍是 `ready`；没给 `replace` 的照旧拒，给了的不拒、只用新算的结果留痕。
/// 回刷新后的计划（去掉回应原样回）
///
/// `pending`：同一条消息上还有放弃了的请求（[`InFlight`]）——它可能随时点上，所以这次「会挤掉几个」一律算拿不准
/// （codex 032 第 4 轮 #1）
async fn recheck<B: Backend>(
    b: &B,
    plan: &ReactPlan,
    pending: bool,
) -> Result<ReactPlan, CoreError> {
    if plan.remove {
        return Ok(plan.clone());
    }
    let synced = !pending
        && plan.synced
        && b.wait_net(Duration::ZERO, &|n| *n == crate::status::NetState::Ready)
            .is_some();
    let m = b.message_get(plan.message).await?;
    let (replaces, known) =
        would_replace(b, plan.message, &plan.target, &plan.kind, &m, synced).await;
    if !plan.replace {
        refuse_replace(&replaces, known)?;
    }
    Ok(ReactPlan {
        replaces,
        replaces_known: known,
        ..plan.clone()
    })
}

/// 同一条消息同时只许一个回应在「复核 → 交出 → 等应答」之间：两个并发的加回应都先复核、再各自交出，复核就白做了。
/// 第二个直接按拿不准拒（`reaction_state_unknown`），不排队等。键带上闸的地址：一个后端一道闸，不同后端（含并行的测试）互不相拦。
///
/// 交出后没等到应答（[`react_within`] 到点丢掉了这次等待，或交出口报连接断了、按结果未知记账），请求却可能还在 TDLib
/// 里走（codex 032 第 2 轮 #2、第 3 轮 #1）：这次的槽位记成「放弃」**一直留着**（本进程内不清），之后再点这条一律按拿不准拒，
/// `replace` 才放行（人已接受可能挤掉）。每个槽位带序号，收尾只删自己那一个，别人的「放弃」不会被顺手删掉
static IN_FLIGHT: std::sync::Mutex<Vec<Slot>> = std::sync::Mutex::new(Vec::new());
static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
struct Slot {
    key: (usize, MsgId),
    serial: u64,
    /// 真 = 交出后没等到应答就放弃了
    abandoned: bool,
}

struct InFlight {
    serial: u64,
    /// 拿到时同一条消息上已有放弃了的请求（只有 `replace` 能走到这）
    pending: bool,
    /// 已交出、还没确定结束：这时被丢掉就算「放弃」
    armed: bool,
}

fn slots() -> std::sync::MutexGuard<'static, Vec<Slot>> {
    IN_FLIGHT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl InFlight {
    fn take(gate: &gate::Gate, id: MsgId, replace: bool) -> Result<Self, CoreError> {
        let key = (std::ptr::from_ref(gate) as usize, id);
        let mut v = slots();
        let busy = v.iter().any(|s| s.key == key && (!s.abandoned || !replace));
        let pending = v.iter().any(|s| s.key == key);
        if busy {
            return Err(CoreError::InvalidSend {
                why: InvalidSendWhy::ReactionStateUnknown,
            });
        }
        let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        v.push(Slot {
            key,
            serial,
            abandoned: false,
        });
        Ok(Self {
            serial,
            pending,
            armed: false,
        })
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        let mut v = slots();
        match v.iter_mut().find(|s| s.serial == self.serial) {
            Some(s) if self.armed => s.abandoned = true,
            _ => v.retain(|s| s.serial != self.serial),
        }
    }
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
    let pushed = matches!(target.kind, ChatKind::Private | ChatKind::Secret);
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

/// `react`（L2）：验凭据 → 复核会不会挤掉（`recheck`，同一条消息串行）→ 留痕「已提交」→ 交给 TDLib → 记结果。
/// 成功回凭据的开法
pub async fn react<B: Backend>(
    b: &B,
    plan: &ReactPlan,
    token: ConfirmToken,
) -> Result<ConfirmMode, CoreError> {
    react_inner(b, plan, token, &std::sync::Mutex::new(None)).await
}

/// `sent`：交出前放进复核后的计划，[`react_within`] 到点时拿它记账（没放就是还没交出）
async fn react_inner<B: Backend>(
    b: &B,
    plan: &ReactPlan,
    token: ConfirmToken,
    sent: &std::sync::Mutex<Option<ReactPlan>>,
) -> Result<ConfirmMode, CoreError> {
    let mode = b.gate().verify(token, plan)?;
    let mut one = InFlight::take(b.gate(), plan.message, plan.replace)?;
    let plan = &recheck(b, plan, one.pending).await?;
    *sent
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(plan.clone());
    b.gate().before_submit(mode, plan)?;
    let sealed = Sealed {
        kind: SealedKind::React {
            request: &plan.request,
        },
    };
    one.armed = true;
    let answer = crate::send_limits::submit(b, sealed).await;
    match answer {
        Ok(Answer::Done) => {
            one.armed = false;
            b.gate()
                .after_outcome(mode, plan, 0, None, Outcome::Succeeded, None);
            Ok(mode)
        }
        Ok(Answer::Message(_) | Answer::Resent(_) | Answer::Deleted) => {
            one.armed = false;
            Err(CoreError::Internal {
                what: "react_answer_shape",
            })
        }
        Err(e) => {
            let e = submit_failed(b, mode, plan, e);
            // 按结果未知报的（连接断了、内核关了……）请求可能已经交出：槽位留成「放弃」
            one.armed = matches!(e, CoreError::SendOutcomeUnknown { .. });
            Err(e)
        }
    }
}

/// [`react`] 连同应答最多等 `timeout`。到点时：已交出 → 按交出时复核后的计划记「结果未知」、报 `send_outcome_unknown`
/// （回应可能已经点上；codex 032 第 3 轮 #3）；还没交出（卡在验凭据后的复核里）→ 报 `network`，可以重试
pub fn react_within<B: Backend>(
    b: &B,
    plan: &ReactPlan,
    token: ConfirmToken,
    timeout: Duration,
) -> Result<ConfirmMode, CoreError> {
    let sent = std::sync::Mutex::new(None);
    if let Some(r) = crate::link::block_on_timeout(react_inner(b, plan, token, &sent), timeout) {
        return r;
    }
    let sent = sent
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match sent {
        Some(fresh) => {
            b.gate()
                .after_outcome(plan.mode, &fresh, 0, None, Outcome::Unknown, None);
            Err(CoreError::SendOutcomeUnknown {
                chat_id: Some(plan.target.id),
                temp_message_id: 0,
                why: crate::error::UnknownWhy::Timeout,
            })
        }
        None => Err(CoreError::Network { td: None }),
    }
}

/// 计划行（`kind:"plan"` 的 `data`，`actions.md` 第一节第 6 小节 `ReactPlan`）
pub fn react_plan_json(plan: &ReactPlan) -> Value {
    json!({
        "action": if plan.remove() { "unreact" } else { "react" },
        "target": target_json(plan.target()),
        "message_id": plan.message().message_id,
        "reaction": plan.kind(),
        "replaces": plan.replaces(),
        "replaces_known": plan.replaces_known(),
        "origin": plan.origin(),
        "confirm": plan.mode(),
        "digest": plan.digest_hex(),
    })
}

#[cfg(test)]
mod tests;
