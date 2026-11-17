//! `edit`：改自己发的一条纯文字消息的正文（#8 B 段，M5 ①），经发送闸的 `editMessageText`（`td_api.tl:13252`）。
//! 规约：`specs/contracts/actions.md`「发送」表 `edit_prepare` / `edit_text` 两行、第一节第 4、5 小节（闸，直发）。
//!
//! **只改自己发的、已发出的、纯文字的**（大总管定 2026-11-17）：不是自己发的 `forbidden why:not_own`（频道管理员改
//! 别人帖子的能力不开，待人看）；还没发出的 `not_sent`；带附件的 `invalid_input why:not_text`（说明文字第二批）；
//! 能不能改（编辑时限一般 48 小时、收藏夹不限时，转发来的不行……，`MessagesManager.cpp:23113-23222`）全听
//! `getMessageProperties.can_be_edited`，不自己算，不行的 `not_editable`。新文与原文一样 `unchanged`（本地先拦）。
//! 新文只有纯文字：原文带格式（加粗、文字链接……）的改后会丢，计划里标 `formatting_dropped`（待人看）；链接预览按
//! 默认重新生成（请求带 `link_preview_options:null`，原消息的预览设置本项目还没读进来，见 `EDIT_NOTE_ZH`；自审 #8 B P2，开 issue）。
//!
//! `editMessageText` **等服务器改完才应答**（与 `sendMessage` 先回临时消息不同）：应答到 = 改成，带回改后的那条；
//! 交出前再查一遍（同准备时的全部条件），原文在列计划之后变了 `invalid_input why:changed`，什么都不改。
//!
//! 流程：[`prepare_edit`]（L0）→ 闸开凭据 → [`edit_text`]（验凭据 → 复查 → 交出）

use crate::backend::Backend;
use crate::error::{CoreError, InvalidSendWhy, RefusedWhy};
use crate::gate::{self, ConfirmMode, ConfirmToken, Gated, Outcome};
use crate::model::{Body, ChatId, ChatTarget, Message, MsgId, SendingState, Time};
use crate::send::{self, Answer, Sealed, SendOrigin, TEXT_MAX_BYTES};
use crate::tree::Span;
use serde_json::{Value, json};

/// 计划行说明
pub const EDIT_NOTE_ZH: &str = "改的是已发出的消息：对方那边也跟着变，标「已编辑」；改前的文字对方可能已经看到。文字里有网址的，链接预览按默认重新生成（原来关掉或调过的预览不保留）";

/// 一份已定稿的编辑：给人看的、交给 TDLib 的都是它（摘要绑请求 + 原文）
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct EditPlan {
    target: ChatTarget,
    id: i64,
    old_text: String,
    text: String,
    formatting_dropped: bool,
    /// 准备时看到的：复查拿它比（整份内容 + 编辑时间，同 `delete`）
    seen: Seen,
    request: Value,
    digest: [u8; 32],
    origin: SendOrigin,
    mode: ConfirmMode,
}

#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
struct Seen {
    content: Body,
    edited: Option<Time>,
}

impl EditPlan {
    pub fn target(&self) -> &ChatTarget {
        &self.target
    }
    pub fn message_id(&self) -> i64 {
        self.id
    }
    pub fn old_text(&self) -> &str {
        &self.old_text
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn formatting_dropped(&self) -> bool {
        self.formatting_dropped
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
    /// 计划行（JSON，`kind:"plan"` 的 `data`）
    pub fn json(&self) -> Value {
        json!({
            "action": "edit",
            "target": send::target_json(&self.target),
            "message_id": self.id,
            "old_text": self.old_text,
            "text": self.text,
            "hidden_chars": gate::hidden_count(&self.text),
            "formatting_dropped": self.formatting_dropped,
            "note_zh": EDIT_NOTE_ZH,
            "origin": self.origin,
            "confirm": self.mode.as_str(),
            "digest": self.digest_hex(),
        })
    }
}

impl gate::Sealed for EditPlan {}
impl Gated for EditPlan {
    fn target(&self) -> ChatId {
        self.target.id
    }
    fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// 不进直发防重复（[`Gated::dedup`] 为假），这个值不用
    fn intent(&self) -> [u8; 32] {
        self.digest
    }
    /// 不拦：改成同样的字第二次在准备时就 `unchanged`
    fn dedup(&self) -> bool {
        false
    }
    fn kind(&self) -> &'static str {
        "edit"
    }
}

/// `edit_prepare`（L0，只查不改）
pub async fn prepare_edit<B: Backend>(
    b: &B,
    chat: ChatId,
    id: i64,
    text: &str,
    origin: SendOrigin,
) -> Result<EditPlan, CoreError> {
    if text.trim().is_empty() {
        return Err(invalid(InvalidSendWhy::EmptyText));
    }
    if u64::try_from(text.len()).unwrap_or(u64::MAX) > TEXT_MAX_BYTES {
        return Err(invalid(InvalidSendWhy::TooLong));
    }
    let me = b.me().await?;
    let mut target = b.chat_target(chat).await?;
    target.saved = target.id == me.saved_chat_id;
    let mode = b.gate().mode_for(target.id)?;
    let (m, old_text) = load(b, target.id, id).await?;
    if old_text == text {
        return Err(invalid(InvalidSendWhy::Unchanged));
    }
    let formatting_dropped = match &m.content {
        Body::Content(c) => c.body.iter().any(lost_on_plain_edit),
        Body::Service(_) => false,
    };
    let request = json!({
        "@type": "editMessageText",
        "chat_id": target.id,
        "message_id": id,
        "input_message_content": {
            "@type": "inputMessageText",
            "text": {"@type": "formattedText", "text": text, "entities": []},
            "link_preview_options": null,
            "clear_draft": false,
        },
    });
    let digest = gate::digest(
        &json!({"request": request, "old_text": old_text}),
        me.user_id,
        send::server_of(b)?,
    );
    Ok(EditPlan {
        target,
        id,
        old_text,
        text: text.to_owned(),
        formatting_dropped,
        seen: seen(&m),
        request,
        digest,
        origin,
        mode,
    })
}

/// `edit_text`（L2）：验凭据 → 复查（不再能改、原文变了就拒，什么都不改）→ 交出。返回确认方式与改后的那条
pub async fn edit_text<B: Backend>(
    b: &B,
    plan: &EditPlan,
    token: ConfirmToken,
) -> Result<(ConfirmMode, Message), CoreError> {
    let mode = b.gate().verify(token, plan)?;
    let (m, old_text) = load(b, plan.target.id, plan.id).await?;
    if old_text != plan.old_text || seen(&m) != plan.seen {
        return Err(invalid(InvalidSendWhy::Changed));
    }
    b.gate().before_submit(mode, plan)?;
    match crate::send_limits::submit(b, Sealed::edit(&plan.request)).await {
        Ok(Answer::Message(m)) => {
            b.gate()
                .after_outcome(mode, plan, 0, Some(plan.id), Outcome::Succeeded, None);
            Ok((mode, m))
        }
        Ok(_) => {
            b.gate()
                .after_outcome(mode, plan, 0, Some(plan.id), Outcome::Failed, None);
            Err(CoreError::Internal {
                what: "edit_answer_shape",
            })
        }
        // 同发送：断线、网络、应答转不回来 = 可能已交出，按结果未知记账、报 `send_outcome_unknown`
        Err(e) => Err(send::submit_failed_for(b, mode, plan, e, Some(plan.id))),
    }
}

/// 查这一条能不能改，取原文（准备与复查共用）
async fn load<B: Backend>(b: &B, chat: ChatId, id: i64) -> Result<(Message, String), CoreError> {
    let mid = MsgId {
        chat_id: chat,
        message_id: id,
    };
    let m = b.message_get(mid).await?;
    // 还没发出的 TDLib 本就说不能改（`:23118`），这里先拦好给出准确的原因
    let refused = if !m.outgoing {
        Some(RefusedWhy::NotOwn)
    } else if m.sending != SendingState::Sent {
        Some(RefusedWhy::NotSent)
    } else {
        None
    };
    if let Some(why) = refused {
        return Err(CoreError::ActionRefused { why });
    }
    let old_text =
        send::resending::confirmable_text(&m).ok_or(invalid(InvalidSendWhy::EditNotText))?;
    if !b.message_caps(mid).await?.can_edit {
        return Err(CoreError::ActionRefused {
            why: RefusedWhy::NotEditable,
        });
    }
    Ok((m, old_text))
}

fn seen(m: &Message) -> Seen {
    let mut content = m.content.clone();
    content.mark_opened();
    Seen {
        content,
        edited: m.edited,
    }
}

fn invalid(why: InvalidSendWhy) -> CoreError {
    CoreError::InvalidSend { why }
}

/// 改成纯文字后会丢的格式：服务器会自己重新认出来的（网址、@用户名、话题标签……）不算
fn lost_on_plain_edit(s: &Span) -> bool {
    match s {
        Span::Text { .. } => false,
        Span::Code { .. }
        | Span::Pre { .. }
        | Span::DateTime { .. }
        | Span::Bold { .. }
        | Span::Italic { .. }
        | Span::Underline { .. }
        | Span::Strikethrough { .. }
        | Span::Spoiler { .. }
        | Span::Quote { .. }
        | Span::Link { .. }
        | Span::MentionName { .. }
        | Span::CustomEmoji { .. }
        | Span::Unknown { .. } => true,
        Span::Url { children }
        | Span::Mention { children }
        | Span::Hashtag { children }
        | Span::Cashtag { children }
        | Span::BotCommand { children }
        | Span::Email { children }
        | Span::Phone { children }
        | Span::BankCard { children }
        | Span::TonAddress { children }
        | Span::MediaTimestamp { children, .. } => children.iter().any(lost_on_plain_edit),
    }
}

#[cfg(test)]
mod tests;
