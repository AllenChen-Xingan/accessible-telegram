//! `forward`：把同一会话里的一条或几条消息转发到另一个会话（#10，`forwardMessages` `td_api.tl:13180`）。
//! 规约：`specs/contracts/actions.md` 第一节第 3 小节「转发」、第 4 小节（闸）、第 5 小节（直发与重复提交拦截）。
//!
//! 流程照重发：[`prepare_forward`]（L0，只查不发：目标会话、来源会话、逐条在不在、能不能转）→ 闸开凭据 →
//! [`forward_and_wait`]（[`forward`]：验凭据 → 交出前逐条重取、和计划比对，变了就拒 → 直发先记账 → 交出；
//! 之后每条的新临时 id 用 [`wait`] 等、逐条 [`finish`]）。
//!
//! 几条定死的（人定 2026-11-17 先按默认，题面在大总管的待定清单）：
//! - 只做普通转发（带原发送人），不做 `send_copy`（复制会隐去原作者）；`remove_caption` 恒为假
//! - 一次最多 [`MAX_IDS`] 条（TDLib 上限 100）：误转一大批撤不回，重复提交拦截只拦同一批
//! - 留痕另记来源会话 id（[`Gated::log_source`]），不记内容
//!
//! 与重发不同，**进重复提交拦截**：同一批转两次对方就收到两份，撤不回。指纹 = 目标 + 来源会话 + 这几条 id（请求本身，
//! 不带每次都变的东西）。目标是论坛群时不带话题，TDLib 放进「General」话题（计划行写明）

use super::{
    Answer, Sealed, SealedKind, SendOrigin, Submitted, Waited, finish, submit_failed,
    submit_within, target_json, wait,
};
use crate::backend::Backend;
use crate::error::{CoreError, InvalidSendWhy, SendRefusedReason, UnknownWhy};
use crate::gate::{self, ConfirmMode, ConfirmToken, Gated};
use crate::link::{Envelope, EventStream};
use crate::model::{Body, ChatId, ChatTarget, Message, MsgId, SenderId};
use crate::tree::Time;
use serde_json::{Value, json};
use std::time::Duration;

/// 一次最多转几条（人定默认 B，2026-11-17）
pub const MAX_IDS: usize = 20;

/// 交出前要和计划比对的那部分：内容、临时内容、受限原因、敏感标记、编辑时间、发送人。
/// 准备到交出之间被编辑、受限、换了内容的，转出去的就不是计划里那条——拒
#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
struct Snapshot {
    content: Body,
    ephemeral: Option<Body>,
    restriction: Option<String>,
    sensitive: bool,
    edited: Option<Time>,
    sender: SenderId,
}

impl Snapshot {
    fn of(m: &Message) -> Self {
        Self {
            content: m.content.clone(),
            ephemeral: m.ephemeral.clone(),
            restriction: m.restriction.clone(),
            sensitive: m.sensitive,
            edited: m.edited,
            sender: m.sender.clone(),
        }
    }
}

/// 计划里的一条：消息 id + 纯文字正文（带附件、系统消息给不出，`None`）+ 比对用的快照
#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub struct ForwardItem {
    pub id: i64,
    pub text: Option<String>,
    snap: Snapshot,
}

/// 一份已定稿的转发：给人看的、交给 TDLib 的都是它（摘要绑请求 + 每条的 id 与正文）
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct ForwardPlan {
    target: ChatTarget,
    from: ChatTarget,
    items: Vec<ForwardItem>,
    request: Value,
    digest: [u8; 32],
    intent: [u8; 32],
    origin: SendOrigin,
    mode: ConfirmMode,
}

impl ForwardPlan {
    pub fn target(&self) -> &ChatTarget {
        &self.target
    }
    pub fn from(&self) -> &ChatTarget {
        &self.from
    }
    pub fn items(&self) -> &[ForwardItem] {
        &self.items
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
    /// 计划行（`kind:"plan"` 的 `data`，`actions.md` 第一节第 6 小节 `ForwardPlan`）
    pub fn json(&self) -> Value {
        json!({
            "action": "forward",
            "target": target_json(&self.target),
            "from": target_json(&self.from),
            "messages": self.items.iter().map(|i| json!({"message_id": i.id, "text": i.text,
                "hidden_chars": i.text.as_deref().map(gate::hidden_count)})).collect::<Vec<_>>(),
            // 论坛群不带话题：落进「General」
            "topic": Value::Null,
            "origin": self.origin,
            "confirm": self.mode,
            "digest": self.digest_hex(),
            "note_zh": super::QUEUE_NOTE_ZH,
        })
    }
}

impl gate::Sealed for ForwardPlan {}
impl Gated for ForwardPlan {
    fn target(&self) -> ChatId {
        self.target.id
    }
    fn digest(&self) -> [u8; 32] {
        self.digest
    }
    fn intent(&self) -> [u8; 32] {
        self.intent
    }
    /// 进直发防重复：同一批转两次对方收到两份
    fn dedup(&self) -> bool {
        true
    }
    fn kind(&self) -> &'static str {
        "forward"
    }
    fn log_source(&self) -> Option<ChatId> {
        Some(self.from.id)
    }
}

/// `forward_prepare`（L0，只查不发）：`ids` 去重、按从小到大排（TDLib 要求严格递增），逐条查在不在、能不能转
pub async fn prepare_forward<B: Backend>(
    b: &B,
    to: ChatId,
    from: ChatId,
    ids: &[i64],
    origin: SendOrigin,
) -> Result<ForwardPlan, CoreError> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return Err(CoreError::InvalidInput { td: None });
    }
    if ids.len() > MAX_IDS {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::ForwardTooMany,
        });
    }
    let me = b.me().await?;
    let mut target = b.chat_target(to).await?;
    target.saved = target.id == me.saved_chat_id;
    let mode = b.gate().mode_for(target.id)?;
    let mut from = b.chat_target(from).await?;
    from.saved = from.id == me.saved_chat_id;
    let items = load_items(b, from.id, &ids).await?;
    let request = json!({
        "@type": "forwardMessages",
        "chat_id": target.id,
        "topic_id": null,
        "from_chat_id": from.id,
        "message_ids": ids,
        "options": null,
        "send_copy": false,
        "remove_caption": false,
    });
    let server = super::server_of(b)?;
    // 防重复看「同一意图」：请求本身（目标 + 来源 + 这几条），不看正文
    let intent = gate::digest(&request, me.user_id, server);
    let texts: Vec<Option<&str>> = items.iter().map(|i| i.text.as_deref()).collect();
    let digest = gate::digest(
        &json!({"request": request, "texts": texts}),
        me.user_id,
        server,
    );
    Ok(ForwardPlan {
        target,
        from,
        items,
        request,
        digest,
        intent,
        origin,
        mode,
    })
}

/// 逐条：在不在（`not_found`）、能不能转（[`Backend::message_caps`] 的 `can_forward`，即
/// `messageProperties.can_be_forwarded`；禁止转发的会话里都不能）
async fn load_items<B: Backend>(
    b: &B,
    chat: ChatId,
    ids: &[i64],
) -> Result<Vec<ForwardItem>, CoreError> {
    let mut items = Vec::with_capacity(ids.len());
    for &id in ids {
        let mid = MsgId {
            chat_id: chat,
            message_id: id,
        };
        let m = b.message_get(mid).await?;
        if !b.message_caps(mid).await?.can_forward {
            return Err(CoreError::InvalidSend {
                why: InvalidSendWhy::NotForwardable,
            });
        }
        items.push(ForwardItem {
            id,
            text: super::resending::confirmable_text(&m),
            snap: Snapshot::of(&m),
        });
    }
    Ok(items)
}

/// `forward`（L2）：一张凭据转这几条；返回每条的凭条（TDLib 说不能转的那条 `None`），之后用 [`wait`] 等、逐条 [`finish`]
pub async fn forward<B: Backend>(
    b: &B,
    plan: &ForwardPlan,
    token: ConfirmToken,
) -> Result<Vec<Option<Submitted>>, CoreError> {
    let mode = b.gate().verify(token, plan)?;
    // 交出前复核（同重发，codex 016 第 2 轮 #2）：准备到交出之间被编辑、删掉、变得不能转的，一律拒、什么都不交出
    let ids: Vec<i64> = plan.items.iter().map(|i| i.id).collect();
    let now = load_items(b, plan.from.id, &ids)
        .await
        .map_err(|_| content_mismatch())?;
    if now != plan.items {
        return Err(content_mismatch());
    }
    b.gate().before_submit(mode, plan)?;
    let sealed = Sealed {
        kind: SealedKind::Forward {
            request: &plan.request,
        },
    };
    match crate::send_limits::submit(b, sealed).await {
        // TDLib 按请求顺序逐条回（转不了的是 null）；条数对不上就认不出哪条是哪条——按内部错误报，留痕停在「已提交」，
        // 防重复照样拦着这一批（宁可要人去看一眼，也不让重跑多转一份）
        Ok(Answer::Resent(ms)) if ms.len() != plan.items.len() => Err(CoreError::Internal {
            what: "forward_answer_count",
        }),
        Ok(Answer::Resent(ms)) => Ok(ms
            .into_iter()
            .map(|m| m.map(|m| Submitted { mode, temp: m.id }))
            .collect()),
        Ok(Answer::Message(_) | Answer::Done | Answer::Deleted) => Err(CoreError::Internal {
            what: "forward_answer_shape",
        }),
        Err(e) => Err(submit_failed(b, mode, plan, e)),
    }
}

fn content_mismatch() -> CoreError {
    CoreError::SendRefused {
        reason: SendRefusedReason::ContentMismatch,
        last: None,
    }
}

/// 一条转发的下落
#[cfg_attr(test, derive(Debug))]
pub struct ForwardOutcome {
    /// 要转的那条（来源会话里的 id）
    pub source: i64,
    /// `None` = TDLib 说这条转不了（应答里是 null）；否则是目标会话里新的临时 id 与它的下落
    pub result: Option<(MsgId, Waited)>,
}

/// 转发的全程：交出 → 逐条等结果 → 逐条记账（CLI 与以后的 GUI 共用，漏不了记账）。交出连同应答也算进 `timeout`：
/// 到点还没回 → 可能已经交出，按结果未知记账、报 `send_outcome_unknown`（同发文字）
pub fn forward_and_wait<B: Backend>(
    b: &B,
    plan: &ForwardPlan,
    token: ConfirmToken,
    stream: &mut EventStream,
    timeout: Duration,
    on_event: &mut dyn FnMut(Envelope),
) -> Result<(ConfirmMode, Vec<ForwardOutcome>), CoreError> {
    let started = std::time::Instant::now();
    let news = submit_within(b, plan, plan.mode, timeout, forward(b, plan, token))?;
    let timeout = timeout.saturating_sub(started.elapsed());
    let mode = news
        .iter()
        .flatten()
        .next()
        .map_or(plan.mode, Submitted::mode);
    let temps: Vec<MsgId> = news.iter().flatten().map(Submitted::temp).collect();
    let mut waited = wait(stream, &temps, timeout, on_event).into_iter();
    let mut out = Vec::with_capacity(news.len());
    for (item, new) in plan.items.iter().zip(news) {
        let result = new.map(|sub| {
            let temp = sub.temp;
            let w = waited
                .next()
                .unwrap_or(Waited::Unknown(UnknownWhy::Timeout));
            finish(b, plan, sub, &w);
            (temp, w)
        });
        out.push(ForwardOutcome {
            source: item.id,
            result,
        });
    }
    Ok((mode, out))
}

#[cfg(test)]
mod tests;
