//! CLI 的 JSON 行：一份 [`Doc`] 一行。
//!
//! ⚠️ 外壳（`v` / `kind` / `speech` 这些顶层字段）归 infra 的 CLI 契约定，这里先按 2026-10-25
//! 与 infra 对齐的草案写；契约定稿后以它为准。树本身（`data`）归 `specs/content-tree.md`。
//! `at`、`seq` 由 CLI 的循环填，不归树。

use crate::speech::{self, SpeechCtx};
use crate::tree::{Doc, SCHEMA_V};
use serde_json::{Value, json};

/// 外壳里的 `kind`
pub fn kind(doc: &Doc) -> &'static str {
    match doc {
        Doc::Message(_) | Doc::Service(_) | Doc::Tombstone(_) => "message",
        Doc::ChatItem(_) => "chat",
        Doc::Alert(_) | Doc::Status(_) | Doc::Unknown(_) => "event",
    }
}

/// 一行 JSON 的值：`{"v":1,"kind":…,"data":<树>,"speech":{"brief":…}}`。
/// 没展开（`cx.reveal` 为假）时 `data` 里的剧透原文遮掉（[`crate::tree::mask`]，R1；大总管授权 2026-10-26）
pub fn value(doc: &Doc, cx: &SpeechCtx) -> Value {
    let data = if cx.reveal {
        json!(doc)
    } else {
        json!(crate::tree::mask::masked(doc))
    };
    json!({
        "v": SCHEMA_V,
        "kind": kind(doc),
        "data": data,
        "speech": { "brief": speech::brief(doc, cx) },
    })
}

/// 除 `brief` 外还要哪几层读法（`history --detail` / `--full`）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Layers {
    pub detail: bool,
    pub full: bool,
}

/// 同 [`value`]，`speech` 里按 `layers` 另带 `detail` / `full`
pub fn value_with(doc: &Doc, cx: &SpeechCtx, layers: Layers) -> Value {
    let mut v = value(doc, cx);
    if let Some(sp) = v.get_mut("speech").and_then(Value::as_object_mut) {
        if layers.detail {
            sp.insert("detail".into(), json!(speech::detail(doc, cx)));
        }
        if layers.full {
            sp.insert("full".into(), json!(speech::full(doc, cx)));
        }
    }
    v
}

/// 一行（不含换行符）
pub fn line(doc: &Doc, cx: &SpeechCtx) -> String {
    value(doc, cx).to_string()
}
