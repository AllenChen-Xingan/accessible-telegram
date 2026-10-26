//! 输出：每行 JSON 的外壳、结尾行、退出码（`specs/contracts/cli.md` 第一节第 1、3、4 小节）。

use crate::args::Format;
use atg_core::tree::{SCHEMA_V, Time};
use serde_json::{Value, json};
use std::io::Write;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

/// 骨架阶段用到的错误码，都是 CLI 自有码（`specs/contracts/errors.md` 第一节第 1 小节）
#[derive(Clone, Copy)]
pub enum Code {
    Usage,
    NotImplemented,
}

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Usage => "usage",
            Code::NotImplemented => "not_implemented",
        }
    }

    /// 退出码表：`usage` → 2；`not_implemented` 归「内部错误 / 未知」→ 1
    pub fn exit(self) -> u8 {
        match self {
            Code::Usage => 2,
            Code::NotImplemented => 1,
        }
    }
}

/// `at`：RFC3339 UTC，毫秒，以 `Z` 结尾
pub fn now_rfc3339_ms() -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = Time(i64::try_from(d.as_secs()).unwrap_or(i64::MAX)).rfc3339();
    // `…:SSZ` → `…:SS.mmmZ`
    let base = secs.strip_suffix('Z').unwrap_or(&secs);
    format!("{base}.{:03}Z", d.subsec_millis())
}

/// 一行外壳：`{"v","kind","at","data"}`。
/// 内容行（chat / message）以后从 `atg_core::json::value` 取 `{v,kind,data,speech}` 再补 `at`，不在这里另拼
pub fn envelope(kind: &str, data: Value) -> Value {
    json!({ "v": SCHEMA_V, "kind": kind, "at": now_rfc3339_ms(), "data": data })
}

/// 往标准输出写一行；管道被关掉时不 panic（退出码仍按最后一行定）
pub fn emit(line: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// 输出错误结尾行并给出退出码。JSON 模式出 `kind:"error"`；文字模式出一句中文
pub fn fail(format: Format, code: Code, message_zh: &str, extra: Value) -> ExitCode {
    match format {
        Format::Json => {
            let mut data = json!({
                "code": code.as_str(),
                "message_zh": message_zh,
                "retryable": false,
            });
            if let (Some(d), Value::Object(e)) = (data.as_object_mut(), extra) {
                d.extend(e);
            }
            emit(&envelope("error", data));
        }
        Format::Text => {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{message_zh}");
        }
    }
    ExitCode::from(code.exit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_has_millis_and_z() {
        let at = now_rfc3339_ms();
        // 2026-10-25T08:30:00.123Z
        assert_eq!(at.len(), 24, "{at}");
        assert_eq!(&at[19..20], ".");
        assert!(at.ends_with('Z'));
    }

    #[test]
    fn exit_codes_match_contract() {
        assert_eq!(Code::Usage.exit(), 2);
        assert_eq!(Code::NotImplemented.exit(), 1);
    }
}
