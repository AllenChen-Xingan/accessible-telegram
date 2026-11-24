//! 真实后端的读：会话列表、打开 / 关闭会话、话题、会话、历史、单条消息。
//! 从 `td/real.rs` 拆出（#60 R7）；根上 `Backend` 的这几个方法一行转给这里的 `do_*`，函数体照原样。

use super::TdBackend;
use crate::backend::{ChatsLoad, HistoryReq};
use crate::error::CoreError;
use crate::link;
use crate::logline::{self, Field};
use crate::model::{ChatId, ChatList, ForumTopicState, Message, MsgId};
use tdlib_rs::{enums, functions, types};

impl TdBackend {
    pub(super) async fn do_chats_load(
        &self,
        list: &ChatList,
        limit: u32,
    ) -> Result<ChatsLoad, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        let td_list = match list {
            ChatList::Main => enums::ChatList::Main,
            ChatList::Archive => enums::ChatList::Archive,
            ChatList::Folder(f) => {
                enums::ChatList::Folder(types::ChatListFolder { chat_folder_id: *f })
            }
            ChatList::Unknown { .. } => return Err(CoreError::InvalidInput { td: None }),
        };
        let limit = i32::try_from(limit).unwrap_or(i32::MAX);
        let r = link::guard(
            self.shared.clone(),
            functions::load_chats(Some(td_list), limit, id),
        )
        .await?;
        match r {
            Ok(()) => Ok(ChatsLoad::Loaded),
            // 没有更多了：TDLib 用 404 表示，吞掉（`actions.md` `chats_load`）
            Err(e) if e.code == 404 => Ok(ChatsLoad::End),
            Err(e) => {
                let e = self.map_error(&e);
                logline::info("chats_load_failed", &[Field::Name("code", e.code())]);
                Err(e)
            }
        }
    }

    pub(super) async fn do_chat_open(&self, chat: ChatId) -> Result<(), CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        // 只 openChat：绝不顺手调 viewMessages（不标已读，`actions.md` `chat_open`）
        link::guard(self.shared.clone(), functions::open_chat(chat, id))
            .await?
            .map_err(|e| self.read_error("chat", &e))
    }

    pub(super) async fn do_chat_close(&self, chat: ChatId) -> Result<(), CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        link::guard(self.shared.clone(), functions::close_chat(chat, id))
            .await?
            .map_err(|e| self.read_error("chat", &e))
    }

    pub(super) async fn do_forum_topic_get(
        &self,
        chat: ChatId,
        topic_id: i64,
    ) -> Result<ForumTopicState, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        let topic = i32::try_from(topic_id).map_err(|_| CoreError::InvalidInput { td: None })?;
        let r = link::guard(
            self.shared.clone(),
            functions::get_forum_topic(chat, topic, id),
        )
        .await?;
        match r {
            // 同其余映射：序列化回 TDLib JSON 交给 `tdjson`，不另写一份
            Ok(t) => serde_json::to_value(&t)
                .map(|v| crate::tdjson::forum_topic(&v))
                .map_err(|_| CoreError::Internal {
                    what: "td_forum_topic_reserialize_failed",
                }),
            Err(e) => Err(self.read_error("forum_topic", &e)),
        }
    }

    pub(super) async fn do_chat_get(&self, chat: ChatId) -> Result<(), CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        let r = link::guard(self.shared.clone(), functions::get_chat(chat, id)).await?;
        match r {
            Ok(_) => Ok(()),
            // 本地还没有这个私聊（如收藏夹）：取已有的，force=false 不新建（大总管授权 2026-10-26）
            Err(_) if chat > 0 => link::guard(
                self.shared.clone(),
                functions::create_private_chat(chat, false, id),
            )
            .await?
            .map(|_| ())
            .map_err(|e| self.read_error("chat", &e)),
            Err(e) => Err(self.read_error("chat", &e)),
        }
    }

    pub(super) async fn do_history(&self, req: HistoryReq) -> Result<Vec<Message>, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        req.check()?;
        let limit = i32::try_from(req.limit).unwrap_or(100);
        let r = link::guard(
            self.shared.clone(),
            functions::get_chat_history(req.chat, req.from, req.offset, limit, req.local_only, id),
        )
        .await?;
        match r {
            // 有一条转不回来就整次报错，不悄悄丢：丢了会让这一页缺一条，若只剩它还会被当成「到头了」
            Ok(enums::Messages::Messages(ms)) => ms
                .messages
                .into_iter()
                .flatten()
                .map(|m| {
                    Self::map_message(m).ok_or(CoreError::Internal {
                        what: "td_message_reserialize_failed",
                    })
                })
                .collect(),
            Err(e) => Err(self.read_error("history", &e)),
        }
    }

    pub(super) async fn do_message_get(&self, mid: MsgId) -> Result<Message, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        let r = link::guard(
            self.shared.clone(),
            functions::get_message(mid.chat_id, mid.message_id, id),
        )
        .await?;
        match r {
            Ok(enums::Message::Message(m)) => Self::map_message(m).ok_or(CoreError::Internal {
                what: "td_message_reserialize_failed",
            }),
            Err(e) => Err(self.read_error("message", &e)),
        }
    }
}
