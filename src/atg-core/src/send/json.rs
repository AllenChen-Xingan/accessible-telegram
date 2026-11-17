//! 计划行（`kind:"plan"` 的 `data`，`actions.md` 第一节第 6 小节 `SendPlan` / `ReactPlan`）：发文字、重发、表情回应共用的输出形状。从 `send` 拆出（#60 行数线）

use super::react::ReactPlan;
use super::{QUEUE_NOTE_ZH, ResendPlan, TextPlan};
use crate::gate;
use crate::model::ChatTarget;
use serde_json::{Value, json};

/// 计划行（`kind:"plan"` 的 `data`，`actions.md` 第一节第 6 小节 `SendPlan`）。内容全文照给（这是给人确认用的）
pub fn plan_json(plan: &TextPlan) -> Value {
    json!({
        "action": "send_text",
        "target": target_json(&plan.target),
        "text": plan.text,
        // 正文里看不见的字符（零宽、标签字符……）有几个：人确认时看到的是转义后的，AI 读 JSON 时据此留意
        "hidden_chars": gate::hidden_count(&plan.text),
        "reply_to": plan.reply_to,
        "topic": plan.topic,
        "origin": plan.origin,
        "confirm": plan.mode,
        "digest": plan.digest_hex(),
        "note_zh": QUEUE_NOTE_ZH,
    })
}

pub fn resend_plan_json(plan: &ResendPlan) -> Value {
    json!({
        "action": "resend",
        "target": target_json(&plan.target),
        "messages": plan.items.iter().map(|i| json!({"message_id": i.id, "drops_reply": i.drops_reply,
            "text": i.text, "hidden_chars": i.text.as_deref().map(gate::hidden_count),
            "reply_to": i.reply.json()})).collect::<Vec<_>>(),
        "origin": plan.origin,
        "confirm": plan.mode,
        "digest": plan.digest_hex(),
        "note_zh": QUEUE_NOTE_ZH,
    })
}

pub(crate) fn target_json(t: &ChatTarget) -> Value {
    json!({
        "chat_id": t.id,
        "kind": t.kind,
        "title": t.title,
        "is_forum": t.is_forum,
        "saved": t.saved,
    })
}

/// 表情回应的计划行（#9）
pub fn react_plan_json(plan: &ReactPlan) -> Value {
    json!({
        "action": if plan.remove() { "unreact" } else { "react" },
        "target": target_json(plan.target()),
        "message_id": plan.message().message_id,
        "reaction": plan.kind(),
        "replaces": plan.replaces(),
        "origin": plan.origin(),
        "confirm": plan.mode(),
        "digest": plan.digest_hex(),
    })
}
