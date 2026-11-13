//! 假后端的调用记账（测试据它断言「调了什么、没调什么」）。

use crate::backend::HistoryReq;
use crate::model::{ChatId, ChatList, MsgId};

/// 一次调用的记账
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    Start {
        profile: String,
        use_test_dc: bool,
    },
    Subscribe,
    /// `chat_get`
    ChatGet(ChatId),
    /// `history`：原样记下请求
    History(HistoryReq),
    /// `message_get`
    MessageGet(MsgId),
    /// `file_download`：文件 id
    FileDownload(i32),
    /// `mark_listened`
    MarkListened(MsgId),
    /// `chat_open` / `chat_close`
    ChatOpen(ChatId),
    ChatClose(ChatId),
    /// `forum_topic_get`
    ForumTopicGet(ChatId, i64),
    Shutdown,
    /// 登录动作：只记名字（`phone` `code` `resend` `email` `email_code` `password` `register` `logout`），
    /// 不记值。只记真交到「TDLib」那一侧的——本地检查拦下的不记
    Auth(&'static str),
    /// `chats_load`：哪个列表、要几个
    ChatsLoad {
        list: ChatList,
        limit: u32,
    },
}
