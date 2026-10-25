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

/// 一行 JSON 的值：`{"v":1,"kind":…,"data":<树>,"speech":{"brief":…}}`
pub fn value(doc: &Doc, cx: &SpeechCtx) -> Value {
    json!({
        "v": SCHEMA_V,
        "kind": kind(doc),
        "data": doc,
        "speech": { "brief": speech::brief(doc, cx) },
    })
}

/// 一行（不含换行符）
pub fn line(doc: &Doc, cx: &SpeechCtx) -> String {
    value(doc, cx).to_string()
}
