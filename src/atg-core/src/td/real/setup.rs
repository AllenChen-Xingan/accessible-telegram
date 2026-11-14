//! 真实后端建客户端之前的两步：TDLib 日志降到 0、改写到档案自己的日志文件；取系统语言（从 `td.rs` 挪出）

use crate::config;
use crate::error::CoreError;
use serde_json::{Value, json};
use std::path::Path;

/// TDLib 原生日志单文件上限（`config-privacy.md` 第一节第 5 小节）
const TD_LOG_MAX_BYTES: i64 = 10 * 1024 * 1024;

/// 建客户端之前：TDLib 日志级别降到 0、输出改到档案自己的日志文件
pub(super) fn quiet_tdlib_log(log_dir: &Path, profile: &str) -> Result<(), CoreError> {
    let ok = |r: Option<Value>| r.is_some_and(|v| v["@type"] == "ok");
    let level = tdlib_rs::execute(json!({
        "@type": "setLogVerbosityLevel",
        "new_verbosity_level": 0,
    }));
    if !ok(level) {
        return Err(CoreError::Internal {
            what: "td_log_level",
        });
    }
    config::ensure_private_dir(log_dir)?;
    let path = log_dir.join(format!("tdlib-{profile}.log"));
    let path = path.to_str().ok_or(CoreError::Internal {
        what: "td_log_path",
    })?;
    let stream = tdlib_rs::execute(json!({
        "@type": "setLogStream",
        "log_stream": {
            "@type": "logStreamFile",
            "path": path,
            "max_file_size": TD_LOG_MAX_BYTES,
            "redirect_stderr": false,
        },
    }));
    if !ok(stream) {
        return Err(CoreError::Internal {
            what: "td_log_stream",
        });
    }
    Ok(())
}

/// 系统语言（`setTdlibParameters` 必填）：取 `LANG` 的语言部分，取不到用 `en`
pub(super) fn system_language() -> String {
    std::env::var("LANG")
        .ok()
        .and_then(|l| {
            let code: String = l.chars().take_while(char::is_ascii_lowercase).collect();
            (code.len() == 2).then_some(code)
        })
        .unwrap_or_else(|| "en".to_owned())
}
