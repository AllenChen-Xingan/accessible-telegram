//! 交出口（`Backend::submit`）的真实实现：发文字、重发、删除、表情回应（#9）、编辑（#8 B）。交给 TDLib 的就是摘要算过的那份 JSON，
//! 由 `td/requests.rs` 逐段反序列化成 tdlib-rs 的类型，不另组一份。从 `td` 拆出（#60 行数线）。
//! 每种都经 [`TdBackend::detached`] 交出：交出未完成标记盖到应答登记完（#88）；表情回应没有临时 id，登记空列表

use super::{TdBackend, requests};
use crate::error::CoreError;
use crate::logline::{self, Field};
use crate::send::{Answer, Sealed, SealedKind};
use tdlib_rs::{enums, functions};

impl TdBackend {
    pub(super) async fn submit_sealed(&self, req: Sealed<'_>) -> Result<Answer, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        match req.into_kind() {
            SealedKind::Message { request, preview } => {
                let a = requests::message(request, preview)?;
                let kind = if preview { "preview" } else { "send" };
                logline::info("send_submit", &[Field::Name("kind", kind)]);
                let enums::Message::Message(m) = self
                    .detached(
                        functions::send_message(
                            a.chat,
                            a.topic,
                            a.reply,
                            Some(a.options),
                            a.content,
                            id,
                        ),
                        move |m| requests::sent(m, preview),
                    )
                    .await?;
                Self::map_message(m)
                    .map(Answer::Message)
                    .ok_or(CoreError::Internal {
                        what: "td_message_reserialize_failed",
                    })
            }
            SealedKind::Resend { request } => {
                let (chat, ids, quote, paid) = requests::resend(request)?;
                logline::info("send_submit", &[Field::Name("kind", "resend")]);
                let enums::Messages::Messages(ms) = self
                    .detached(
                        functions::resend_messages(chat, ids, quote, paid, id),
                        requests::resent,
                    )
                    .await?;
                Ok(Answer::Resent(
                    ms.messages
                        .into_iter()
                        .map(|m| m.and_then(Self::map_message))
                        .collect(),
                ))
            }
            SealedKind::Delete { request } => {
                let (chat, ids, revoke) = requests::delete(request)?;
                logline::info("send_submit", &[Field::Name("kind", "delete")]);
                let none = |_: &()| vec![];
                self.detached(functions::delete_messages(chat, ids, revoke, id), none)
                    .await?;
                Ok(Answer::Deleted)
            }
            // 应答 = 服务器已改完（不像发送先回临时消息），没有要登记的待回结果
            SealedKind::Edit { request } => {
                let (chat, message, content) = requests::edit(request)?;
                logline::info("send_submit", &[Field::Name("kind", "edit")]);
                let none = |_: &enums::Message| vec![];
                let enums::Message::Message(m) = self
                    .detached(
                        functions::edit_message_text(chat, message, content, id),
                        none,
                    )
                    .await?;
                Self::map_message(m)
                    .map(Answer::Message)
                    .ok_or(CoreError::Internal {
                        what: "td_message_reserialize_failed",
                    })
            }
            SealedKind::React { request } => {
                let a = requests::react(request)?;
                let kind = if a.remove { "unreact" } else { "react" };
                logline::info("send_submit", &[Field::Name("kind", kind)]);
                let none = |_: &()| vec![];
                if a.remove {
                    self.detached(
                        functions::remove_message_reaction(a.chat, a.message, a.kind, id),
                        none,
                    )
                    .await?;
                } else {
                    self.detached(
                        functions::add_message_reaction(
                            a.chat, a.message, a.kind, a.big, a.recent, id,
                        ),
                        none,
                    )
                    .await?;
                }
                Ok(Answer::Done)
            }
        }
    }
}
