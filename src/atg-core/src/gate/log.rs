//! 直发留痕 `direct-send.log` 的一行与追加（`actions.md` 第一节第 5 小节「本地留痕」）。从 `gate` 拆出（#60 行数线）

use super::{ConfirmMode, DIRECT_LOG_KEEP_S, Outcome, write_atomic};
use crate::error::Holder;
use crate::model::ChatId;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

fn open_append(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(path)
}

#[derive(Serialize)]
pub(super) struct DirectLogLine {
    pub(super) at: i64,
    pub(super) proc: Holder,
    pub(super) pid: u32,
    pub(super) chat_id: ChatId,
    pub(super) temp_id: i64,
    pub(super) final_id: Option<i64>,
    pub(super) kind: &'static str,
    /// 怎么开的凭据（直发 / 免确认的动作种类）
    pub(super) confirm: ConfirmMode,
    pub(super) result: Outcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<&'static str>,
    /// 动在哪条消息上、挤掉了几个原先的回应（[`super::Gated::log_target`]；只有表情回应有）
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "target_fields"
    )]
    pub(super) target: Option<(i64, usize)>,
}

fn target_fields<S: serde::Serializer>(t: &Option<(i64, usize)>, s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    let mut m = s.serialize_map(Some(2))?;
    if let Some((id, n)) = t {
        m.serialize_entry("message_id", id)?;
        m.serialize_entry("replaced", n)?;
    }
    m.end()
}

/// 追加一行；文件头一行已超过 30 天时先把过期的整理掉（整体重写）
pub(super) fn append_direct_log(
    path: &Path,
    line: &DirectLogLine,
    now: i64,
) -> std::io::Result<()> {
    let text = serde_json::to_string(line).unwrap_or_default();
    if let Ok(old) = fs::read_to_string(path) {
        let stale = |l: &str| {
            serde_json::from_str::<serde_json::Value>(l)
                .ok()
                .and_then(|v| v["at"].as_i64())
                .is_some_and(|at| now.saturating_sub(at) >= DIRECT_LOG_KEEP_S)
        };
        if old.lines().next().is_some_and(stale) {
            let mut keep: String = old
                .lines()
                .filter(|l| !stale(l))
                .flat_map(|l| [l, "\n"])
                .collect();
            keep.push_str(&text);
            keep.push('\n');
            return write_atomic(path, keep.as_bytes());
        }
    }
    let mut f = open_append(path)?;
    writeln!(f, "{text}")
}
