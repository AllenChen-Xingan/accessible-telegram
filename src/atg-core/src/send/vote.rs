//! 投票：在一条投票消息里选一项或几项（#13 13b-1；契约 `actions.md` 第一节第 3 小节 `vote`）。
//!
//! 等级 L2（公开投票别人看得到你投了什么；不许改票的投了就收不回），走同一道闸、同一个交出口，和其他 L2 一样
//! **直发、不确认**（#113）：照样验目标与摘要、照样 60 秒一次性、照样受账号受限与限流冷却管、照样写 `direct-send.log`
//! （`kind` 为 `vote`，`final_id` 记投票那条消息的 id，**不记选了哪几项**）；不进直发防重复（人定 2026-11-17 #13：
//! 同样的票再投一次不会多出一票）。测试服务器上照样只许收藏夹（`test_dc_saved_only`）。
//!
//! 流程：[`prepare_vote`]（L0：自己是谁 → 目标会话 → 现取那条消息、是不是投票、能不能这样投 → 组请求、算摘要）→
//! 闸开凭据 → [`vote_within`]（验凭据 → **交出前再取一次那条消息复核** → 留痕「已提交」→ 交给 TDLib → 记结果），
//! 交出连同应答有 [`super::RESULT_TIMEOUT`] 上限，到点按结果未知报。
//!
//! 人定（`actions.md` 修订记录 2026-11-17 #13 与「人改定」）：测验许投；计划照这个投票的实际设置写投了能不能改
//! （`allows_revoting`，测验也一样）；测验另写明「投后会公布答案」。收回投过的票（空选项）v0 不做（大总管定 2026-11-24）。

use super::{Answer, Sealed, SealedKind, SendOrigin, server_of, submit_failed_for, target_json};
use crate::backend::Backend;
use crate::error::{CoreError, InvalidSendWhy};
use crate::gate::{self, ConfirmMode, ConfirmToken, Gated, Outcome};
use crate::model::{Body, ChatTarget, Message, MsgId};
use crate::tree::{Attachment, PollKind, PollOption};
use serde_json::{Value, json};
use std::time::Duration;

/// 选中的一项：显示顺序里的第几项（1 起，同读法「第 N 项」）、TDLib 的 0 起编号、选项文字
#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub struct Choice {
    pub number: u32,
    pub index: u32,
    /// 选项原文；含剧透的为 `None`（计划行不替人揭开）
    pub text: Option<String>,
}

/// 一份已定稿的投票：交给 TDLib 的就是 `request`（摘要绑的就是它）
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct VotePlan {
    target: ChatTarget,
    message: MsgId,
    choices: Vec<Choice>,
    /// 我现在投着、这次没再选上、会被丢掉的几项（`setPollAnswer` 整组替换；自审 13b-1 第 1 轮 P1、第 2 轮 P2）。
    /// 不空只可能是给了 `replace`；追加或与现在投的一样时为空
    replaces: Vec<Choice>,
    replace: bool,
    /// 投票的性质，计划行照实际设置说（人改定 6）
    revotable: bool,
    anonymous: bool,
    quiz: bool,
    /// 交出前复核用：准备时各选项的原文（按 TDLib 编号），变了就不投
    options: Vec<String>,
    request: Value,
    digest: [u8; 32],
    origin: SendOrigin,
    mode: ConfirmMode,
}

impl VotePlan {
    pub fn target(&self) -> &ChatTarget {
        &self.target
    }
    pub fn message(&self) -> MsgId {
        self.message
    }
    pub fn choices(&self) -> &[Choice] {
        &self.choices
    }
    /// 这次会换掉的、我现在投着的几项（见 [`VotePlan`] 字段说明）
    pub fn replaces(&self) -> &[Choice] {
        &self.replaces
    }
    /// 投了还能改（`allows_revoting`）
    pub fn revotable(&self) -> bool {
        self.revotable
    }
    /// 匿名投票（假 = 别人看得到你投了什么）
    pub fn anonymous(&self) -> bool {
        self.anonymous
    }
    /// 测验：投后会公布答案
    pub fn quiz(&self) -> bool {
        self.quiz
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

impl gate::Sealed for VotePlan {}
impl Gated for VotePlan {
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
        "vote"
    }
}

/// 一个投票里 [`prepare_vote`] 要看的那几样
struct PollView<'a> {
    options: &'a [PollOption],
    multiple: bool,
    revotable: bool,
    anonymous: bool,
    quiz: bool,
    /// 没结束、没有不能投的原因、类型认得出（认不出的投票类型不投：不知道投了会怎样，读法也说「暂不支持」）、
    /// 上一次投票没在提交中（在提交时 TDLib 把我原来投着的也报成没选，判不了；自审第 3 轮 P2）
    open: bool,
}

/// 这条消息显示出来的是不是投票（受限的、临时内容盖住的都不算）
fn poll_of(m: &Message) -> Result<PollView<'_>, CoreError> {
    let not_poll = CoreError::InvalidSend {
        why: InvalidSendWhy::NotAPoll,
    };
    let Body::Content(c) = m.effective() else {
        return Err(not_poll);
    };
    let Some(Attachment::Poll {
        options,
        multiple,
        revotable,
        anonymous,
        kind,
        closed,
        blocked,
        ..
    }) = c.attachment.as_ref()
    else {
        return Err(not_poll);
    };
    Ok(PollView {
        options,
        multiple: *multiple,
        revotable: *revotable,
        anonymous: *anonymous,
        quiz: *kind == PollKind::Quiz,
        open: !*closed
            && blocked.is_none()
            && !matches!(kind, PollKind::Unknown { .. })
            && !options.iter().any(|o| o.pending),
    })
}

fn choice(o: &PollOption, number: u32) -> Choice {
    let hidden = o.text.iter().any(crate::tree::text::contains_spoiler);
    Choice {
        number,
        index: o.index,
        text: (!hidden).then(|| crate::tree::text::plain_text(&o.text)),
    }
}

/// 能不能这样投；能就回（选中的各项，会丢掉的我现在投着的各项），都按显示顺序。`numbers` 是显示顺序里的第几项（1 起）。
/// `setPollAnswer` 整组替换：我投过、许改票、现在投着的有一项这次没再选上，就会丢掉它，没给 `replace` 就拒
/// （`vote_would_replace`，同 react 的 `reaction_would_replace`：直发没人当场看着）；把原来投的也写上（追加）不丢、不拒
fn judge(
    p: &PollView<'_>,
    numbers: &[u32],
    replace: bool,
) -> Result<(Vec<Choice>, Vec<Choice>), CoreError> {
    let err = |why| Err(CoreError::InvalidSend { why });
    if !p.open {
        return err(InvalidSendWhy::VoteBlocked);
    }
    if !p.revotable && p.options.iter().any(|o| o.chosen) {
        return err(InvalidSendWhy::VoteNotRevotable);
    }
    let mut sorted = numbers.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let bad = numbers.is_empty()
        || sorted.len() != numbers.len()
        || (numbers.len() > 1 && !p.multiple)
        || sorted
            .iter()
            .any(|&n| n == 0 || n as usize > p.options.len());
    if bad {
        return err(InvalidSendWhy::VoteBadChoice);
    }
    let choices: Vec<Choice> = sorted
        .iter()
        .map(|&n| choice(&p.options[n as usize - 1], n))
        .collect();
    let mine: Vec<Choice> = (1u32..)
        .zip(p.options)
        .filter(|(_, o)| o.chosen)
        .map(|(n, o)| choice(o, n))
        .collect();
    // 真会丢掉的：现在投着、这次没再选上的（追加、和现在一样都不丢；自审第 2 轮 P2）
    let lost: Vec<Choice> = mine.into_iter().filter(|m| !choices.contains(m)).collect();
    if !lost.is_empty() && !replace {
        return err(InvalidSendWhy::VoteWouldReplace);
    }
    Ok((choices, lost))
}

/// 各选项原文，按 TDLib 编号排（交出前比对用）
fn texts(options: &[PollOption]) -> Vec<String> {
    let mut v: Vec<(u32, String)> = options
        .iter()
        .map(|o| (o.index, crate::tree::text::plain_text(&o.text)))
        .collect();
    v.sort_by_key(|(i, _)| *i);
    v.into_iter().map(|(_, t)| t).collect()
}

/// 交给 TDLib 的请求（TDLib 形状）：选项给 0 起编号，从小到大
pub(crate) fn vote_request(id: MsgId, choices: &[Choice]) -> Value {
    let mut ids: Vec<u32> = choices.iter().map(|c| c.index).collect();
    ids.sort_unstable();
    json!({"@type": "setPollAnswer", "chat_id": id.chat_id, "message_id": id.message_id, "option_ids": ids})
}

/// `vote_prepare`：只查不发。拒的情形（都是 `invalid_input`）：这条显示的不是投票 `not_a_poll`；已结束、有不能投的原因、
/// 类型认不出 `vote_blocked`；我投过、这个投票不许改 `vote_not_revotable`；我投过、新选的不一样、没给 `replace`
/// `vote_would_replace`；没选、选重了、编号越界、不许多选却选了几项 `vote_bad_choice`
pub async fn prepare_vote<B: Backend>(
    b: &B,
    id: MsgId,
    numbers: &[u32],
    replace: bool,
    origin: SendOrigin,
) -> Result<VotePlan, CoreError> {
    let me = b.me().await?;
    let mut target = b.chat_target(id.chat_id).await?;
    target.saved = target.id == me.saved_chat_id;
    let mode = b.gate().mode_for(target.id)?;
    let m = b.message_get(id).await?;
    let p = poll_of(&m)?;
    let (choices, replaces) = judge(&p, numbers, replace)?;
    let request = vote_request(id, &choices);
    let digest = gate::digest(&request, me.user_id, server_of(b)?);
    Ok(VotePlan {
        target,
        message: id,
        choices,
        replaces,
        replace,
        revotable: p.revotable,
        anonymous: p.anonymous,
        quiz: p.quiz,
        options: texts(p.options),
        request,
        digest,
        origin,
        mode,
    })
}

/// 交出前复核：计划可能是早先准备的，期间投票可能结束了、我在手机上投过或改了票（不许改的就不能再投；许改的会被这次
/// 整组换掉）、发起人改了选项。再取一次那条消息，按同样的规矩判；会换掉的那几项、性质（匿名、能否改、测验）或选项原文
/// 变了 → `changed`（计划里说的已不对）
async fn recheck<B: Backend>(b: &B, plan: &VotePlan) -> Result<(), CoreError> {
    let m = b.message_get(plan.message).await?;
    let p = poll_of(&m)?;
    let numbers: Vec<u32> = plan.choices.iter().map(|c| c.number).collect();
    let (again, replaces) = judge(&p, &numbers, plan.replace)?;
    let same = again == plan.choices
        && replaces == plan.replaces
        && texts(p.options) == plan.options
        && p.revotable == plan.revotable
        && p.anonymous == plan.anonymous
        && p.quiz == plan.quiz;
    if !same {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::Changed,
        });
    }
    Ok(())
}

/// 在途的投票（键带上闸的地址：一个后端一道闸，并行的测试互不相拦）。放开后（交完、失败、到点被丢掉）由 TDLib 自己的
/// `is_being_chosen`（[`PollOption::pending`]）接着挡：`setPollAnswer` 一交出 TDLib 就标上，直到服务器回话
static IN_FLIGHT: std::sync::Mutex<Vec<(usize, MsgId)>> = std::sync::Mutex::new(Vec::new());

pub(super) struct InFlight(usize, MsgId);

fn slots() -> std::sync::MutexGuard<'static, Vec<(usize, MsgId)>> {
    IN_FLIGHT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl InFlight {
    pub(super) fn take(gate: &gate::Gate, id: MsgId) -> Result<Self, CoreError> {
        let key = (std::ptr::from_ref(gate) as usize, id);
        let mut v = slots();
        if v.contains(&key) {
            return Err(CoreError::InvalidSend {
                why: InvalidSendWhy::VoteBlocked,
            });
        }
        v.push(key);
        Ok(Self(key.0, key.1))
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        let key = (self.0, self.1);
        slots().retain(|k| *k != key);
    }
}

/// `vote`（L2）：验凭据 → 同一条投票串行 → 复核 → 留痕「已提交」→ 交给 TDLib → 记结果。成功回凭据的开法
pub async fn vote<B: Backend>(
    b: &B,
    plan: &VotePlan,
    token: ConfirmToken,
) -> Result<ConfirmMode, CoreError> {
    vote_inner(b, plan, token, &std::sync::atomic::AtomicBool::new(false)).await
}

/// `sent`：交出前置真，[`vote_within`] 到点时看它（没置就是还没交出）
async fn vote_inner<B: Backend>(
    b: &B,
    plan: &VotePlan,
    token: ConfirmToken,
    sent: &std::sync::atomic::AtomicBool,
) -> Result<ConfirmMode, CoreError> {
    let mode = b.gate().verify(token, plan)?;
    // 同一条投票同时只许一个在「复核 → 交出 → 等应答」之间（codex 062 第 1 轮 #1）：两个都先复核、再各自交出，后一个
    // 会整组覆盖前一个，`vote_would_replace` 与 `pending` 都挡不住（交出前 TDLib 还没标「在提交」）。第二个按「还在提交」拒
    let _one = InFlight::take(b.gate(), plan.message)?;
    recheck(b, plan).await?;
    b.gate().before_submit(mode, plan)?;
    let sealed = Sealed {
        kind: SealedKind::Vote {
            request: &plan.request,
        },
    };
    sent.store(true, std::sync::atomic::Ordering::SeqCst);
    let id = Some(plan.message.message_id);
    match crate::send_limits::submit(b, sealed).await {
        Ok(Answer::Done) => {
            b.gate()
                .after_outcome(mode, plan, 0, id, Outcome::Succeeded, None);
            Ok(mode)
        }
        Ok(Answer::Message(_) | Answer::Resent(_) | Answer::Deleted | Answer::Callback(_)) => {
            Err(CoreError::Internal {
                what: "vote_answer_shape",
            })
        }
        Err(e) => Err(submit_failed_for(b, mode, plan, e, id)),
    }
}

/// [`vote`] 连同应答最多等 `timeout`。到点时：已交出 → 记「结果未知」、报 `send_outcome_unknown`（票可能已投上）；
/// 还没交出（卡在复核里）→ 报 `network`，可以重试
pub fn vote_within<B: Backend>(
    b: &B,
    plan: &VotePlan,
    token: ConfirmToken,
    timeout: Duration,
) -> Result<ConfirmMode, CoreError> {
    let sent = std::sync::atomic::AtomicBool::new(false);
    if let Some(r) = crate::link::block_on_timeout(vote_inner(b, plan, token, &sent), timeout) {
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

/// 计划行（`kind:"plan"` 的 `data`，`actions.md` 第一节第 6 小节 `VotePlan`）。选项文字含剧透的给 `null`
pub fn vote_plan_json(plan: &VotePlan) -> Value {
    // `replaces` 形状同 `choices`（契约写明）：同一个函数出
    let one = |c: &Choice| json!({"number": c.number, "index": c.index, "text": c.text});
    let choices: Vec<Value> = plan.choices.iter().map(one).collect();
    json!({
        "action": "vote",
        "target": target_json(plan.target()),
        "message_id": plan.message().message_id,
        "choices": choices,
        "replaces": plan.replaces.iter().map(one).collect::<Vec<_>>(),
        "revotable": plan.revotable,
        "anonymous": plan.anonymous,
        "quiz": plan.quiz,
        "origin": plan.origin(),
        "confirm": plan.mode(),
        "digest": plan.digest_hex(),
    })
}

#[cfg(test)]
mod tests;
