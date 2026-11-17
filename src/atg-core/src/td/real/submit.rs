//! 交出口（`Backend::submit`）的真实实现：发文字、重发、表情回应（#9）。交给 TDLib 的就是摘要算过的那份 JSON，
//! 逐段反序列化成 tdlib-rs 的类型，不另组一份。从 `td` 拆出（#60 行数线）

use super::TdBackend;
use crate::error::CoreError;
use crate::logline::{self, Field};
use crate::send::{Answer, Sealed, SealedKind};
use tdlib_rs::{enums, functions, types};

impl TdBackend {
    pub(super) async fn submit_sealed(&self, req: Sealed<'_>) -> Result<Answer, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        match req.into_kind() {
            SealedKind::Message { request, preview } => {
                // 交给 TDLib 的就是摘要算过的那份 JSON：逐段反序列化成 tdlib-rs 的类型，不另组一份
                let bad = || CoreError::Internal {
                    what: "send_request_shape",
                };
                let part = |k: &str| request[k].clone();
                let chat = request["chat_id"].as_i64().ok_or_else(bad)?;
                let topic: Option<enums::MessageTopic> =
                    serde_json::from_value(part("topic_id")).map_err(|_| bad())?;
                let reply: Option<enums::InputMessageReplyTo> =
                    serde_json::from_value(part("reply_to")).map_err(|_| bad())?;
                let mut options: types::MessageSendOptions =
                    serde_json::from_value(part("options")).map_err(|_| bad())?;
                options.only_preview = preview;
                let content: enums::InputMessageContent =
                    serde_json::from_value(part("input_message_content")).map_err(|_| bad())?;
                logline::info(
                    "send_submit",
                    &[Field::Name(
                        "kind",
                        if preview { "preview" } else { "send" },
                    )],
                );
                let enums::Message::Message(m) = self
                    .detached(functions::send_message(
                        chat,
                        topic,
                        reply,
                        Some(options),
                        content,
                        id,
                    ))
                    .await?;
                Self::map_message(m)
                    .map(Answer::Message)
                    .ok_or(CoreError::Internal {
                        what: "td_message_reserialize_failed",
                    })
            }
            SealedKind::Resend { request } => {
                // 同发文字：交出去的就是摘要算过的那份，逐字段取，不另组
                let bad = || CoreError::Internal {
                    what: "send_request_shape",
                };
                let chat = request["chat_id"].as_i64().ok_or_else(bad)?;
                let ids: Vec<i64> =
                    serde_json::from_value(request["message_ids"].clone()).map_err(|_| bad())?;
                let quote: Option<types::InputTextQuote> =
                    serde_json::from_value(request["quote"].clone()).map_err(|_| bad())?;
                let paid = request["paid_message_star_count"]
                    .as_i64()
                    .ok_or_else(bad)?;
                logline::info("send_submit", &[Field::Name("kind", "resend")]);
                let enums::Messages::Messages(ms) = self
                    .detached(functions::resend_messages(chat, ids, quote, paid, id))
                    .await?;
                Ok(Answer::Resent(
                    ms.messages
                        .into_iter()
                        .map(|m| m.and_then(Self::map_message))
                        .collect(),
                ))
            }
            SealedKind::React { request } => {
                // 同上：交出去的就是摘要算过的那份
                let bad = || CoreError::Internal {
                    what: "send_request_shape",
                };
                let chat = request["chat_id"].as_i64().ok_or_else(bad)?;
                let msg = request["message_id"].as_i64().ok_or_else(bad)?;
                let ty: enums::ReactionType =
                    serde_json::from_value(request["reaction_type"].clone()).map_err(|_| bad())?;
                let remove = request["@type"] == "removeMessageReaction";
                logline::info(
                    "send_submit",
                    &[Field::Name(
                        "kind",
                        if remove { "unreact" } else { "react" },
                    )],
                );
                if remove {
                    self.detached(functions::remove_message_reaction(chat, msg, ty, id))
                        .await?;
                } else {
                    let flag = |k: &str| request[k].as_bool().ok_or_else(bad);
                    let (big, recent) = (flag("is_big")?, flag("update_recent_reactions")?);
                    self.detached(functions::add_message_reaction(
                        chat, msg, ty, big, recent, id,
                    ))
                    .await?;
                }
                Ok(Answer::Done)
            }
        }
    }
}
