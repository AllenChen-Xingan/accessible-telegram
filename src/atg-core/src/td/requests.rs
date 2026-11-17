//! 交出口的请求 JSON → tdlib-rs 的参数（从 `td.rs` 的 `submit` 挪出）。交给 TDLib 的就是摘要算过的那份 JSON：
//! 逐字段反序列化，不另组一份；形状不对一律 `internal{send_request_shape}`

use crate::error::CoreError;
use crate::model::{MessageCaps, MsgId};
use serde_json::Value;
use tdlib_rs::{enums, types};

fn bad() -> CoreError {
    CoreError::Internal {
        what: "send_request_shape",
    }
}

fn part<T: serde::de::DeserializeOwned>(request: &Value, k: &str) -> Result<T, CoreError> {
    serde_json::from_value(request[k].clone()).map_err(|_| bad())
}

/// `sendMessage` 的参数
pub(super) struct MessageArgs {
    pub chat: i64,
    pub topic: Option<enums::MessageTopic>,
    pub reply: Option<enums::InputMessageReplyTo>,
    pub options: types::MessageSendOptions,
    pub content: enums::InputMessageContent,
}

pub(super) fn message(request: &Value, preview: bool) -> Result<MessageArgs, CoreError> {
    let mut options: types::MessageSendOptions = part(request, "options")?;
    options.only_preview = preview;
    Ok(MessageArgs {
        chat: request["chat_id"].as_i64().ok_or_else(bad)?,
        topic: part(request, "topic_id")?,
        reply: part(request, "reply_to")?,
        options,
        content: part(request, "input_message_content")?,
    })
}

/// `resendMessages`：（会话、消息 id、引用、付费星数）
pub(super) fn resend(
    request: &Value,
) -> Result<(i64, Vec<i64>, Option<types::InputTextQuote>, i64), CoreError> {
    Ok((
        request["chat_id"].as_i64().ok_or_else(bad)?,
        part(request, "message_ids")?,
        part(request, "quote")?,
        request["paid_message_star_count"]
            .as_i64()
            .ok_or_else(bad)?,
    ))
}

/// `deleteMessages`：（会话、消息 id、`revoke`）
pub(super) fn delete(request: &Value) -> Result<(i64, Vec<i64>, bool), CoreError> {
    Ok((
        request["chat_id"].as_i64().ok_or_else(bad)?,
        part(request, "message_ids")?,
        request["revoke"].as_bool().ok_or_else(bad)?,
    ))
}

/// `addMessageReaction` / `removeMessageReaction`（看 `@type`）：（会话、消息 id、种类、去不去掉、`is_big`、
/// `update_recent_reactions`）。去掉时后两个不看、给假
pub(super) struct ReactArgs {
    pub chat: i64,
    pub message: i64,
    pub kind: enums::ReactionType,
    pub remove: bool,
    pub big: bool,
    pub recent: bool,
}

pub(super) fn react(request: &Value) -> Result<ReactArgs, CoreError> {
    let remove = request["@type"] == "removeMessageReaction";
    let flag = |k: &str| {
        if remove {
            Ok(false)
        } else {
            request[k].as_bool().ok_or_else(bad)
        }
    };
    Ok(ReactArgs {
        chat: request["chat_id"].as_i64().ok_or_else(bad)?,
        message: request["message_id"].as_i64().ok_or_else(bad)?,
        kind: part(request, "reaction_type")?,
        remove,
        big: flag("is_big")?,
        recent: flag("update_recent_reactions")?,
    })
}

// ───── 应答里交出成功、成败还要等的消息（`TdBackend::detached` 登记，#88）─────

fn msg_id(m: &types::Message) -> MsgId {
    MsgId {
        chat_id: m.chat_id,
        message_id: m.id,
    }
}

/// `sendMessage`：预览不发，给空
pub(super) fn sent(m: &enums::Message, preview: bool) -> Vec<MsgId> {
    let enums::Message::Message(m) = m;
    if preview { vec![] } else { vec![msg_id(m)] }
}

/// `resendMessages`：能重发的那几条
pub(super) fn resent(ms: &enums::Messages) -> Vec<MsgId> {
    let enums::Messages::Messages(ms) = ms;
    ms.messages.iter().flatten().map(msg_id).collect()
}

/// `getMessageProperties` 的应答 → 本项目用到的那几项（[`MessageCaps`] 是唯一正本）
pub(super) fn caps(p: &types::MessageProperties) -> MessageCaps {
    MessageCaps {
        can_reply: p.can_be_replied,
        can_delete_for_all: p.can_be_deleted_for_all_users,
        can_edit: p.can_be_edited,
    }
}
