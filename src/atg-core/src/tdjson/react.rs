//! 表情回应：消息上的回应汇总、「谁点了回应」一页（#9）。从 `tdjson` 拆出（#60 行数线）

use super::{arr, b, i, s, sender, str_or_num, ty, u};
use crate::model::{AddedReaction, AddedReactions};
use crate::tree::{Reaction, ReactionKind};
use serde_json::{Value, json};

/// `messageInteractionInfo.reactions` → 回应汇总（种类、次数、我点了没有）
pub(super) fn reactions(info: &Value) -> Vec<Reaction> {
    arr(&info["reactions"], "reactions")
        .iter()
        .map(|r| Reaction {
            kind: reaction_kind(&r["type"]),
            count: u(r, "total_count"),
            chosen: b(r, "is_chosen"),
        })
        .collect()
}

pub(super) fn reaction_kind(v: &Value) -> ReactionKind {
    match ty(v) {
        "reactionTypeEmoji" => ReactionKind::Emoji {
            emoji: s(v, "emoji").to_owned(),
        },
        "reactionTypeCustomEmoji" => ReactionKind::CustomEmoji {
            custom_emoji_id: str_or_num(&v["custom_emoji_id"]),
        },
        "reactionTypePaid" => ReactionKind::Paid,
        other => ReactionKind::Unknown {
            td_type: other.to_owned(),
        },
    }
}

/// TDLib `addedReactions`（`getMessageAddedReactions` 的应答，`td_api.tl:7933`）→ 一页「谁点了回应」
pub fn added_reactions(v: &Value) -> AddedReactions {
    AddedReactions {
        total: u(v, "total_count"),
        items: arr(v, "reactions")
            .iter()
            .map(|r| AddedReaction {
                kind: reaction_kind(&r["type"]),
                sender: sender(&r["sender_id"]),
                outgoing: b(r, "is_outgoing"),
                date: i(r, "date"),
            })
            .collect(),
        next_offset: s(v, "next_offset").to_owned(),
    }
}

/// 回应种类 → TDLib `ReactionType` 的 JSON（交给 TDLib 的请求用）。不认识的种类给不出：`None`
pub fn reaction_type(k: &ReactionKind) -> Option<Value> {
    Some(match k {
        ReactionKind::Emoji { emoji } => json!({"@type": "reactionTypeEmoji", "emoji": emoji}),
        ReactionKind::CustomEmoji { custom_emoji_id } => {
            json!({"@type": "reactionTypeCustomEmoji", "custom_emoji_id": custom_emoji_id.parse::<i64>().ok()?})
        }
        ReactionKind::Paid => json!({"@type": "reactionTypePaid"}),
        ReactionKind::Unknown { .. } => return None,
    })
}
