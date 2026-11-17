//! 起步前的同步准备：TDLib 日志降级改道、系统语言、组 `setTdlibParameters`（顺序见 `td.rs` 模块头）

use crate::config::{self, ApiCredentials};
use crate::error::CoreError;
use serde_json::{Value, json};
use std::future::Future;
use std::path::Path;
use tdlib_rs::{functions, types};

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
fn system_language() -> String {
    std::env::var("LANG")
        .ok()
        .and_then(|l| {
            let code: String = l.chars().take_while(char::is_ascii_lowercase).collect();
            (code.len() == 2).then_some(code)
        })
        .unwrap_or_else(|| "en".to_owned())
}

/// 组 `setTdlibParameters`（`use_test_dc` 只从档案标记来；钥匙与 `api_hash` 在这里复制进请求，调用方随后丢掉自己那份）
pub(super) fn parameters(
    use_test_dc: bool,
    db: &str,
    files: &str,
    key_b64: &str,
    creds: &ApiCredentials,
    client_id: i32,
) -> impl Future<Output = Result<(), types::Error>> + use<> {
    functions::set_tdlib_parameters(
        use_test_dc,
        db.to_owned(),
        files.to_owned(),
        key_b64.to_owned(),
        true,  // use_file_database
        true,  // use_chat_info_database
        true,  // use_message_database
        false, // use_secret_chats：私密聊天第一阶段不做
        creds.api_id,
        creds.api_hash.expose_str().to_owned(),
        system_language(),
        "Mac".to_owned(),
        String::new(),
        env!("CARGO_PKG_VERSION").to_owned(),
        client_id,
    )
}
