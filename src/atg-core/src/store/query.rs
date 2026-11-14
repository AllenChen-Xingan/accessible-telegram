//! Store 的纯查询（不改状态、不联网）：会话列表顺序、下一个有未读的会话、待发出的消息。
//! 契约：`specs/contracts/actions.md` 第一节第 3 小节「会话」表的 Store 行与「待发出的消息」。

use super::Store;
use crate::model::*;

impl Store {
    /// 某个列表当前的会话顺序：按 (order, 会话 id) 从大到小；order 为 0 的不在列表里
    pub fn chat_list(&self, list: &ChatList) -> Vec<ChatId> {
        let mut v: Vec<(i64, ChatId)> = self
            .chats
            .values()
            .filter_map(|c| {
                c.positions
                    .iter()
                    .find(|p| &p.list == list && p.order != 0)
                    .map(|p| (p.order, c.id))
            })
            .collect();
        v.sort_unstable_by(|a, b| b.cmp(a));
        v.into_iter().map(|(_, id)| id).collect()
    }

    /// 主列表里 `after` 之后第一个有未读的会话；到底不回绕（A3）。`after` 不在列表里时从头找
    pub fn next_unread_chat(&self, after: Option<ChatId>) -> Option<ChatId> {
        let list = self.chat_list(&ChatList::Main);
        let start = after
            .and_then(|a| list.iter().position(|&c| c == a))
            .map_or(0, |i| i + 1);
        list.into_iter()
            .skip(start)
            .find(|id| self.chats.get(id).is_some_and(|c| c.unread_count > 0))
    }

    /// 待发出的消息（`actions.md`「待发出的消息」）：store 里活着、发送状态还是「待发」的，按（会话 id, 消息 id）排。
    /// 来源是 TDLib 推的新消息：本进程交出的，和启动时 TDLib 从本地库续发的上次留下的（续发时临时 id 与时间都重编，
    /// TDLib 1.8.69 `MessagesManager.cpp:35851-35856`，所以与上一个进程报过的临时 id 对不上）。
    /// 非当前会话超过 `keep_per_chat` 被删旧的不在其内（待发的总是最新的几条，实际碰不到）
    pub fn pending_sends(&self) -> Vec<MsgId> {
        let mut v: Vec<MsgId> = self
            .table
            .live()
            .filter(|m| m.sending == SendingState::Pending)
            .map(|m| m.id)
            .collect();
        v.sort_unstable_by_key(|id| (id.chat_id, id.message_id));
        v
    }
}
