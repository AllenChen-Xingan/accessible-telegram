//! 假后端的读（第 5、8 块）：会话列表、会话、话题、历史、单条消息。
//! 从 `fake` 拆出（#60 R7）；`Backend` 的这几个方法在根上一行转给这里的 `do_*`，函数体照原样。

use super::{Call, FakeBackend, lock};
use crate::backend::{ChatsLoad, HistoryReq};
use crate::error::{CoreError, map_td_error};
use crate::link;
use crate::model::{ChatId, ChatList, ForumTopicState, Message, MsgId};
use crate::status::AuthStep;
use serde_json::Value;

impl FakeBackend {
    pub(super) async fn do_chats_load(
        &self,
        list: &ChatList,
        limit: u32,
    ) -> Result<ChatsLoad, CoreError> {
        self.started()?;
        // 本地检查拦下的不记账（同登录那组）
        if let ChatList::Unknown { .. } = list {
            return Err(CoreError::InvalidInput { td: None });
        }
        lock(&self.calls).push(Call::ChatsLoad {
            list: list.clone(),
            limit,
        });
        let hang = match lock(&self.chats_load_answers).as_mut() {
            Some(0) => true,
            Some(n) => {
                *n -= 1;
                false
            }
            None => false,
        };
        let injected = self.take_failure("chats_load");
        link::guard(self.shared.clone(), async move {
            if hang {
                std::future::pending::<()>().await;
            }
            match injected {
                Some(e) => Err(e),
                None => Ok(()),
            }
        })
        .await??;
        if self.step() != AuthStep::Ready {
            return Err(map_td_error(401, "Unauthorized"));
        }
        let batch: Vec<Vec<Value>> = {
            let mut all = lock(&self.chats);
            let q = all.entry(list.clone()).or_default();
            let n = q.len().min(usize::try_from(limit).unwrap_or(usize::MAX));
            q.drain(..n).collect()
        };
        if batch.is_empty() {
            return Ok(ChatsLoad::End);
        }
        for update in batch.into_iter().flatten() {
            self.push(update);
        }
        self.settle();
        Ok(ChatsLoad::Loaded)
    }

    pub(super) async fn do_chat_get(&self, chat: ChatId) -> Result<(), CoreError> {
        self.started()?;
        lock(&self.calls).push(Call::ChatGet(chat));
        self.ready_or_401("chat_get").await?;
        let Some(updates) = lock(&self.chat_info).get(&chat).cloned() else {
            return Err(CoreError::NotFound {
                what: "chat",
                td: None,
            });
        };
        for u in updates {
            self.push(u);
        }
        self.settle();
        Ok(())
    }

    pub(super) async fn do_chat_open(&self, chat: ChatId) -> Result<(), CoreError> {
        self.started()?;
        lock(&self.calls).push(Call::ChatOpen(chat));
        self.ready_or_401("chat_open").await?;
        if !lock(&self.chat_info).contains_key(&chat) {
            return Err(CoreError::NotFound {
                what: "chat",
                td: None,
            });
        }
        Ok(())
    }

    pub(super) async fn do_chat_close(&self, chat: ChatId) -> Result<(), CoreError> {
        self.started()?;
        lock(&self.calls).push(Call::ChatClose(chat));
        self.ready_or_401("chat_close").await?;
        Ok(())
    }

    pub(super) async fn do_forum_topic_get(
        &self,
        chat: ChatId,
        topic_id: i64,
    ) -> Result<ForumTopicState, CoreError> {
        self.started()?;
        lock(&self.calls).push(Call::ForumTopicGet(chat, topic_id));
        self.ready_or_401("forum_topic_get").await?;
        let t = lock(&self.forum_topics).get(&(chat, topic_id)).cloned();
        t.map(|t| crate::tdjson::forum_topic(&t))
            .ok_or(CoreError::NotFound {
                what: "forum_topic",
                td: None,
            })
    }

    pub(super) async fn do_history(&self, req: HistoryReq) -> Result<Vec<Message>, CoreError> {
        self.started()?;
        req.check()?;
        lock(&self.calls).push(Call::History(req));
        self.ready_or_401("history").await?;
        if !lock(&self.chat_info).contains_key(&req.chat) {
            return Err(CoreError::NotFound {
                what: "chat",
                td: None,
            });
        }
        let mut all: Vec<Value> = lock(&self.histories)
            .get(&req.chat)
            .cloned()
            .unwrap_or_default();
        let id = |v: &Value| v["id"].as_i64().unwrap_or(0);
        all.sort_by_key(|v| std::cmp::Reverse(id(v)));
        // 照 TDLib 1.8.69 的切法（`td/telegram/OrderedMessage.cpp` `OrderedMessages::get_history`）：
        // - `from` 为 0：从最新一条起（含它）往旧
        // - `offset` 为 0：**不含** `from` 那条，从比它旧的起
        // - `offset` 为 -k：`from` 那条算进去，另给比它新的 k-1 条，再往旧接上
        let from_end = req.from == 0;
        let k = usize::try_from(-i64::from(req.offset)).unwrap_or(0);
        let newer_n = k.saturating_sub(1);
        let mut newer: Vec<Value> = all
            .iter()
            .filter(|v| !from_end && id(v) > req.from)
            .cloned()
            .collect();
        let skip = newer.len().saturating_sub(newer_n);
        let newer = newer.split_off(skip);
        let older: Vec<Value> = all
            .iter()
            .filter(|v| from_end || id(v) < req.from || (k > 0 && id(v) == req.from))
            .cloned()
            .collect();
        // 停住：只回起点那一条（有的话），一条新的都不给——内存里只有它、更早的还在后台拉
        let stalled = {
            let mut st = lock(&self.stall);
            if st.0 > 0 {
                st.0 -= 1;
                false
            } else {
                let s = st.1 > 0;
                st.1 = st.1.saturating_sub(1);
                s
            }
        };
        // 从最新一条起时「起点」就是最新那条
        let at_from = || {
            if from_end {
                all.first().cloned()
            } else {
                all.iter().find(|v| id(v) == req.from).cloned()
            }
        };
        if stalled {
            return Ok(at_from().iter().map(crate::tdjson::message).collect());
        }
        // TDLib 首次常只回 1 条（内存里只有挨着起点的那一条）：TDLib 给的总是连着起点的一段，所以是离起点最近的那条
        let short = lock(&self.short_first)
            .as_mut()
            .is_some_and(|seen| seen.insert(req.chat));
        if short {
            let one = older.first().cloned().or_else(|| newer.last().cloned());
            return Ok(one.iter().map(crate::tdjson::message).collect());
        }
        // 限了条数：留离起点最近的那几条（往新时是起点和紧挨着它的新的，往旧时是紧挨着它的旧的）
        let (newer, older) = match *lock(&self.cap) {
            Some(c) if k > 0 => {
                let keep_newer = c.saturating_sub(1).min(newer.len());
                let newer = newer[newer.len() - keep_newer..].to_vec();
                let older = older.into_iter().take(c - keep_newer).collect();
                (newer, older)
            }
            Some(c) => (newer, older.into_iter().take(c).collect()),
            None => (newer, older),
        };
        let mut out: Vec<Value> = newer.into_iter().chain(older).collect();
        out.truncate(usize::try_from(req.limit).unwrap_or(usize::MAX));
        Ok(out.iter().map(crate::tdjson::message).collect())
    }

    pub(super) async fn do_message_get(&self, id: MsgId) -> Result<Message, CoreError> {
        self.started()?;
        lock(&self.calls).push(Call::MessageGet(id));
        self.ready_or_401("message_get").await?;
        lock(&self.histories)
            .get(&id.chat_id)
            .and_then(|ms| {
                ms.iter()
                    .find(|v| v["id"].as_i64() == Some(id.message_id))
                    .map(crate::tdjson::message)
            })
            .ok_or(CoreError::NotFound {
                what: "message",
                td: None,
            })
    }
}
