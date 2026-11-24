//! 表情回应的节点（`specs/content-tree.md` 第三节）：消息尾部的回应汇总 [`Reaction`]，「谁点了回应」一行一个的
//! [`Reactor`]（`Doc::Reactor`，`atg-cli reactions`；#9）。从 `tree` 拆出（#60 行数线）

use super::{Sender, Time};
use serde::Serialize;

#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Reaction {
    pub kind: ReactionKind,
    pub count: u32,
    /// 我自己点了没有
    pub chosen: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReactionKind {
    Emoji {
        emoji: String,
    },
    /// int64，JSON 里写字符串
    CustomEmoji {
        custom_emoji_id: String,
    },
    Paid,
    Unknown {
        td_type: String,
    },
}

/// 谁点了哪个回应（`getMessageAddedReactions` 的一项，投影时填名字）。句子只由 `speech` 出
#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Reactor {
    pub kind: ReactionKind,
    pub sender: Sender,
    /// 是不是我自己点的
    pub outgoing: bool,
    pub date: Time,
}
