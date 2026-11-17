//! 假后端的 `getMessageProperties`（[`MessageCaps`]）：默认照 TDLib 的规矩算，测试可逐条关掉某项能力

use super::{FakeBackend, lock};
use crate::error::CoreError;
use crate::model::{MessageCaps, MsgId};

/// 可以单独关掉的能力
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cap {
    Reply,
    DeleteForAll,
    Edit,
}

impl FakeBackend {
    /// 这条消息不能回复
    pub fn set_no_reply(&self, id: MsgId) {
        self.deny(id, Cap::Reply);
    }

    /// 关掉这条消息的某项能力（如「过了时限，不能给所有人删」）
    pub fn deny(&self, id: MsgId, cap: Cap) {
        lock(&self.denied).push((id, cap));
    }

    /// 默认：都能回复；给所有人删、编辑只限自己发的、已在服务器上的、不在收藏夹里的（TDLib 不许给所有人删收藏夹里的，
    /// `MessagesManager.cpp:8346`）。被 [`FakeBackend::deny`] 关掉的照关
    pub(super) fn caps(&self, id: MsgId) -> Result<MessageCaps, CoreError> {
        let m = self.find_message(id).ok_or(CoreError::NotFound {
            what: "message",
            td: None,
        })?;
        let on = |c: Cap| !lock(&self.denied).contains(&(id, c));
        let sent = m["sending_state"].is_null();
        let own = m["is_outgoing"].as_bool() == Some(true) && sent;
        let saved = *lock(&self.me_id) == Some(id.chat_id);
        Ok(MessageCaps {
            can_reply: on(Cap::Reply),
            can_delete_for_all: own && !saved && on(Cap::DeleteForAll),
            can_edit: own && on(Cap::Edit),
        })
    }
}
