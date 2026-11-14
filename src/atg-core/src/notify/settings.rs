//! 有效通知设置：静音、提及、置顶按「话题 → 会话 → 范围」逐层取（003 收1）。「范围」是 TDLib 的三档默认设置
//! （私聊 / 群 / 频道）；「003 收N」等代号与提不提示的判定都在模块根 `notify.rs`（发送人会话静没静音的
//! `sender_muted` 属于判定，留在那里）。

use crate::model::{Chat, ChatNotify, NotifyScope, TopicNotify};
use crate::store::Store;

/// 静音落在哪一层
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Topic,
    Chat,
    Scope,
}

/// 逐层取出的有效通知设置
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effective {
    /// 静音的话是哪一层定的
    pub muted: Option<Layer>,
    pub disable_mention: bool,
    pub disable_pinned: bool,
}

/// 一条消息的有效通知设置：话题 → 会话 → 范围逐层取（003 收1）
pub fn effective(
    store: &Store,
    chat: &Chat,
    topic: Option<&TopicNotify>,
    now_ms: i64,
) -> Effective {
    let scope = NotifyScope::of(&chat.kind).and_then(|s| store.scope_notify(s));
    let topic_n = topic.map(|t| t.notify);
    // 静音
    let muted = match topic {
        Some(t) if !t.notify.use_default_mute_for => {
            (t.muted_until_ms > now_ms).then_some(Layer::Topic)
        }
        _ if !chat.notify.use_default_mute_for => (chat.notify.mute_for > 0).then_some(Layer::Chat),
        _ => scope
            .is_some_and(|s| s.mute_for > 0)
            .then_some(Layer::Scope),
    };
    // 提及、置顶：同一套取法
    let pick = |own: fn(&ChatNotify) -> Option<bool>, scope_v: bool| {
        topic_n
            .as_ref()
            .and_then(own)
            .or_else(|| own(&chat.notify))
            .unwrap_or(scope_v)
    };
    Effective {
        muted,
        disable_mention: pick(
            |n| (!n.use_default_disable_mention).then_some(n.disable_mention),
            scope.is_some_and(|s| s.disable_mention),
        ),
        disable_pinned: pick(
            |n| (!n.use_default_disable_pinned).then_some(n.disable_pinned),
            scope.is_some_and(|s| s.disable_pinned),
        ),
    }
}
