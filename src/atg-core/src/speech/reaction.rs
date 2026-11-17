//! 表情回应的读法（#9）：消息详情里的回应汇总，`atg-cli reactions` 的每一行（`Doc::Reactor`）。
//! **句子是 AI 草稿，待人实听**；正本是快照。从 `speech` 拆出（#60 行数线）
//!
//! 「谁点了回应」一行：「张三，👍，19:33」；自己点的读「我」；查不到名字读「未知发送人」（同消息发送人，V18）

use super::{ME, SpeechCtx, UNKNOWN_SENDER, hm, sender_name};
use crate::tree::{Reaction, ReactionKind, Reactor};

/// 一种回应怎么叫：emoji 照读；自定义表情、付费星星、不认识的给说法
pub(super) fn label(k: &ReactionKind) -> String {
    match k {
        ReactionKind::Emoji { emoji } => emoji.clone(),
        ReactionKind::CustomEmoji { .. } => "自定义表情".to_owned(),
        ReactionKind::Paid => "星星".to_owned(),
        ReactionKind::Unknown { .. } => "暂不支持的回应".to_owned(),
    }
}

/// 「回应：3 个👍、1 个❤️，我点了👍」
pub(super) fn reactions(list: &[Reaction]) -> String {
    let all: Vec<String> = list
        .iter()
        .map(|r| format!("{} 个{}", r.count, label(&r.kind)))
        .collect();
    let mine: Vec<String> = list
        .iter()
        .filter(|r| r.chosen)
        .map(|r| label(&r.kind))
        .collect();
    let mut s = format!("回应：{}", all.join("、"));
    if !mine.is_empty() {
        s.push_str(&format!("，我点了{}", mine.join("、")));
    }
    s
}

/// 「谁点了回应」的一行
pub(super) fn reactor(r: &Reactor, cx: &SpeechCtx) -> String {
    let who = if r.outgoing {
        ME.to_owned()
    } else {
        sender_name(&r.sender, UNKNOWN_SENDER)
    };
    format!("{who}，{}，{}", label(&r.kind), hm(r.date, cx))
}
