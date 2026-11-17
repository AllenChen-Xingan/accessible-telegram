//! `send`：发文字、回复、重发（M2.1 / M2.2，T04）。规约：`specs/contracts/actions.md` 第一节第 3、4、7 小节；
//! 先例与坑：`docs/research/003_send-receive-prior-art/README.md` 发1–发9。
//!
//! 几个词：**临时 id** = TDLib 交出时先给这条消息编的号（`messageSendingStatePending`），服务器收下后换成**正式 id**；
//! **应答** = `sendMessage` 这次请求的直接回复（带临时 id）；**结果** = 之后 TDLib 主动推的 `send_succeeded` /
//! `send_failed` 事件。应答与结果走两条路，谁先到没有保证（发4）。源码行号指 TDLib 1.8.69（`resources/td/`）。
//!
//! 与契约的名字对照（`specs/contracts/actions.md` 第一节第 3 小节）：`send_text_prepare` = [`prepare_text`]，
//! `send_text` = [`send_text`]（全程用 [`send_and_wait`]），`resend_prepare` = [`prepare_resend`]，`resend` = [`resend`]；
//! 契约的 `SendPlan` 在这里分成 [`TextPlan`] 与 [`ResendPlan`]，`PendingMessage` = [`Submitted`]。
//!
//! 一次发送的流程（CLI 与以后的 GUI 共用）：
//! 1. [`prepare_text`]（L0，只查不发）：自己是谁 → 目标会话 → 被回复那条（能不能回复、在哪个话题）→ 话题规则 →
//!    组好交给 TDLib 的那份请求（`clear_draft=false` 不动人在手机上的草稿；带 `sending_id`）→ 算摘要 →
//!    **调一次 `only_preview`**，回复对象与话题和请求逐项比对，不一致就拒（发6①：TDLib 会把不合规的回复悄悄去掉）
//! 2. 给人看 [`TextPlan`]，由闸开凭据（[`crate::gate::Gate::issue`]）
//! 3. [`send_text`]（L2）：闸验凭据 → **紧挨着交出前再预览比对一次**（发6②）→ 直发的话先记「已提交」→ 交给 TDLib，
//!    拿到临时 id
//! 4. [`wait`]（store = 本进程里的消息与会话缓存，界面和读法都从它取）：等 `send_succeeded` / `send_failed` / 临时 id 出现在 `messages_deleted` 里（发3，按结果未知报），
//!    30 秒上限（发9）；事件照常交给调用方喂 store（晚到的也不丢）。结果可能比应答先到（发4）：事件一直在流里排着，
//!    应答回来之后才开始看，所以不会漏
//! 5. [`finish`]：闸记结果（直发改防重复记录、写留痕）
//!
//! 重发（[`prepare_resend`] → [`resend_and_wait`]，内部是 [`resend`] → [`wait`] → 逐条 [`finish`]）：逐条查失败态（能否重发、要不要等、会不会去掉回复）→ 一张凭据 →
//! `resendMessages` → 每条的新临时 id 同样用 [`wait`] 等。重发不进直发防重复（见 `gate::Gated::dedup`）。
//!
//! 只有闸能发：[`Sealed`] 只能在本 crate 里造，后端的交出口 `Backend::submit` 收的就是它。
//!
//! 代号：「发N」「B17-2」见 `docs/research/003_send-receive-prior-art/`；「M2.1」等是 `docs/plan/002_roadmap.md` 的块，
//! 「T04」是 `specs/tasks.md` 的任务（发消息）。
//!
//! 文件分工：本文件放各步共用的底座（凭条 [`Submitted`]、确认框里的说明句、交出失败 / 超时按「结果未知」记账、
//! 计划行的目标部分）；`send/text.rs` 发文字（准备 → 交出 → 等结果的全程）；`send/outcome.rs` 等结果与记账
//! （[`fn@wait`]、[`finish`]）；`send/resending.rs` 重发。计划行（`plan_json` / `resend_plan_json`）各随自己的计划类型，R6 时一起换成 `Doc::Plan`

use crate::backend::Backend;
use crate::config::ServerKind;
use crate::error::{CoreError, InvalidSendWhy, SendRefusedReason, UnknownWhy};
use crate::gate::{self, ConfirmMode, ConfirmToken, Gated, Outcome};
use crate::link::{Envelope, EventStream, block_on_timeout};
use crate::model::{ChatId, ChatKind, CoreEvent, Message, MsgId, ReplyRef, Topic};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

mod outcome;
pub(crate) mod resending;
mod text;

pub use outcome::{Waited, finish, wait};
pub use resending::{
    Fed, ResendItem, ResendOutcome, ResendPlan, ResendReply, prepare_resend, resend,
    resend_and_wait, resend_plan_json,
};
pub use text::{Draft, TextPlan, check_kept, plan_json, prepare_text, send_and_wait, send_text};
// `td.rs` 的测试（tdlib-api 特性）与本模块测试用
#[cfg(test)]
pub(crate) use text::text_request;

/// 正文最多多少字节（UTF-8）。这是本地的「明显给错了」防线，不是 Telegram 的上限：Telegram 单条文字上限是
/// 4096 字（`getOption message_text_length_max`），在两者之间的正文由 TDLib 的预览当场拒（`invalid_input`，
/// 原文 `MESSAGE_TOO_LONG`），不会自动拆成几条、也不会交出去。
/// 发送动作的规则，准备步骤（[`prepare_text`]）里查，CLI 与 GUI 都受它管
pub const TEXT_MAX_BYTES: u64 = 64 * 1024;

/// `send` 等结果的上限（`cli.md` `send` 行；发9）
pub const RESULT_TIMEOUT: Duration = Duration::from_secs(30);

/// 确认时说给人的「网络中断也会发」（`actions.md` 第一节第 2 小节）
pub const QUEUE_NOTE_ZH: &str = "若网络中断，恢复后仍会发出";

pub use crate::model::ChatTarget;

/// 谁发起的（确认说明里写；命令行注明「可能是 AI」）
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SendOrigin {
    Cli,
    Gui,
}

/// 只能在本 crate 里造的「交给 TDLib 的请求」：后端的交出口只收它，所以 crate 外的代码绕不过闸
pub struct Sealed<'a> {
    // 私有：只有本模块（验过凭据之后）造得出；后端用 [`Sealed::into_kind`] 取（codex 016 第 1 轮 #7）
    kind: SealedKind<'a>,
}

impl<'a> Sealed<'a> {
    /// 后端的交出口拆开用
    pub(crate) fn into_kind(self) -> SealedKind<'a> {
        self.kind
    }
    /// `deleteMessages`：只由 [`crate::delete`] 在闸验过凭据之后造
    pub(crate) fn delete(request: &'a Value) -> Self {
        let kind = SealedKind::Delete { request };
        Self { kind }
    }
}

pub(crate) enum SealedKind<'a> {
    /// `sendMessage`：请求 JSON（TDLib 形状）；`preview` 为真时改成 `only_preview`，不发
    Message { request: &'a Value, preview: bool },
    /// `resendMessages`：请求 JSON（TDLib 形状；摘要算过的就是它）
    Resend { request: &'a Value },
    /// `deleteMessages`（`chat_id` · `message_ids` · `revoke`）：撤销待发消息、给所有人删除
    Delete { request: &'a Value },
}

/// 交出口的应答
#[allow(clippy::large_enum_variant)]
pub enum Answer {
    /// `sendMessage`：待发的那条（临时 id）；预览时是 TDLib 造的假消息
    Message(Message),
    /// `resendMessages`：按请求顺序，不能重发的那条是 `None`
    Resent(Vec<Option<Message>>),
    /// `deleteMessages`：TDLib 回了 `ok`
    Deleted,
}

/// 交出之后拿到的凭条：确认方式 + 临时 id。只能由本模块造出；等到结果后**必须**交给 [`finish`]（它吃掉凭条）
/// 记账——漏了，直发的防重复记录就一直停在「已提交」、留痕少一行。文字发送用 [`send_and_wait`] 一步走完
#[derive(Debug, PartialEq, Eq)]
#[must_use = "等到结果后要交给 send::finish 记账（直发的防重复与留痕）"]
pub struct Submitted {
    mode: ConfirmMode,
    temp: MsgId,
}

impl Submitted {
    pub fn mode(&self) -> ConfirmMode {
        self.mode
    }
    /// 临时 id：TDLib 交出时先给消息编的号，服务器收下后换成正式 id（`send_succeeded` 里带）
    pub fn temp(&self) -> MsgId {
        self.temp
    }
}

// ───────────────────────── 给人看的会话说法 ─────────────────────────

/// 「私聊 · 名字 · id 777」（名字转义控制字符）
pub fn target_zh(t: &ChatTarget) -> String {
    let kind = if t.saved {
        "收藏夹"
    } else {
        match t.kind {
            ChatKind::Private => "私聊",
            ChatKind::Secret => "加密聊天",
            ChatKind::Group => "群",
            ChatKind::Supergroup if t.is_forum => "话题群",
            ChatKind::Supergroup => "超级群",
            ChatKind::Channel => "频道",
            ChatKind::Unknown { .. } => "会话",
        }
    };
    format!("{kind} · {} · id {}", gate::visible(&t.title), t.id)
}

// ───────────────────────── 交出出了岔子：按「结果未知」记账（发文字、重发共用）─────────────────────────

pub(crate) fn server_of(b: &impl Backend) -> Result<ServerKind, CoreError> {
    match b.gate().use_test_dc() {
        Some(true) => Ok(ServerKind::Test),
        Some(false) => Ok(ServerKind::Main),
        None => Err(CoreError::Internal {
            what: "gate_not_started",
        }),
    }
}

/// 交出口回了错：分清「TDLib 明确拒了、没发出」和「请求可能已经交出、只是没等到应答」。
/// 后一种（连接断了、内核关了、请求中止、应答转不回来）按结果未知记账并报 `send_outcome_unknown`——
/// 记成失败会让直发防重复放过重跑，对方就收到两条（自审第 1 轮 P1）
pub(crate) fn submit_failed<B: Backend>(
    b: &B,
    mode: ConfirmMode,
    plan: &dyn Gated,
    e: CoreError,
) -> CoreError {
    let maybe_left = matches!(
        e,
        CoreError::Disconnected
            | CoreError::Network { .. }
            | CoreError::Internal {
                what: "td_message_reserialize_failed",
            }
    );
    if maybe_left {
        b.gate()
            .after_outcome(mode, plan, 0, None, Outcome::Unknown, None);
        CoreError::SendOutcomeUnknown {
            chat_id: Some(plan.target()),
            temp_message_id: 0,
            why: UnknownWhy::Disconnected,
        }
    } else {
        b.gate()
            .after_outcome(mode, plan, 0, None, Outcome::Failed, Some(&e));
        e
    }
}

/// 交出（验凭据 → 复核 → 交给 TDLib）也算进同一个上限：TDLib 迟迟不回交出的应答时，命令与 serve 不许一直卡着
/// （codex 016 第 1 轮 #3）。到点还没回 → 可能已经交出，按结果未知记账并报 `send_outcome_unknown`（`timeout`）
fn submit_within<B: Backend, T>(
    b: &B,
    plan: &dyn Gated,
    mode: ConfirmMode,
    timeout: Duration,
    fut: impl std::future::Future<Output = Result<T, CoreError>>,
) -> Result<T, CoreError> {
    match crate::link::block_on_timeout(fut, timeout) {
        Some(r) => r,
        None => {
            b.gate()
                .after_outcome(mode, plan, 0, None, Outcome::Unknown, None);
            Err(CoreError::SendOutcomeUnknown {
                chat_id: Some(plan.target()),
                temp_message_id: 0,
                why: UnknownWhy::Timeout,
            })
        }
    }
}

// ───────────────────────── 计划行的目标部分（两种计划行共用）─────────────────────────

pub(crate) fn target_json(t: &ChatTarget) -> Value {
    json!({
        "chat_id": t.id,
        "kind": t.kind,
        "title": t.title,
        "is_forum": t.is_forum,
        "saved": t.saved,
    })
}

#[cfg(test)]
mod tests;
