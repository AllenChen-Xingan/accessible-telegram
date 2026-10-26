//! 输出：每行 JSON 的外壳、结尾行、退出码（`specs/contracts/cli.md` 第一节第 1、3、4 小节）。

use crate::args::Format;
use atg_core::error::CoreError;
use atg_core::status::AuthStep;
use atg_core::tree::{SCHEMA_V, Time};
use serde_json::{Value, json};
use std::io::Write;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

/// CLI 自有码（`specs/contracts/errors.md` 第一节第 1 小节）
#[derive(Clone, Copy)]
pub enum Code {
    Usage,
    NotImplemented,
    Interrupted,
}

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Usage => "usage",
            Code::NotImplemented => "not_implemented",
            Code::Interrupted => "interrupted",
        }
    }

    /// 退出码表：`usage` → 2；`not_implemented` 归「内部错误 / 未知」→ 1；`interrupted` → 130
    pub fn exit(self) -> u8 {
        match self {
            Code::Usage => 2,
            Code::NotImplemented => 1,
            Code::Interrupted => 130,
        }
    }
}

/// 一条命令怎么失败的（结尾的 `error` 行由它出）
pub enum Fail {
    /// `atg-core` 报的错：码、说明、附加字段照 `errors.md`
    Core(CoreError),
    /// 参数不对 / 用法不对（如要终端却读不到 `/dev/tty`）。说明是写死的话，不带用户输入
    // 现在只有要终端的路径（真实后端）用到
    #[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
    Usage(&'static str),
    /// 还没做（带子命令名）
    NotImplemented {
        command: &'static str,
        message_zh: &'static str,
    },
    /// Ctrl+C
    #[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
    Interrupted,
    /// 授权走到本应用第一阶段不支持的一步（扫码确认、未知步骤、正式档案上要注册）。
    /// 规约没有专门的码，按 `auth_wrong_state` 报、`expected` 写这一步（已报大总管）
    Unsupported(AuthStep),
}

/// 退出码表（`cli.md` 第一节第 4 小节）里 `CoreError` 那几档。按变体穷举：新增变体时编译器会逼着来这里定码
pub fn core_exit(e: &CoreError) -> u8 {
    use CoreError as E;
    match e {
        E::AuthRequired { .. }
        | E::AuthPhoneInvalid { .. }
        | E::AuthCodeInvalid { .. }
        | E::AuthCodeExpired { .. }
        | E::AuthPasswordInvalid { .. }
        | E::AuthPhoneNotForProfile
        | E::AuthWrongState { .. } => EXIT_AUTH,
        E::Network { .. } | E::Disconnected => 4,
        E::RateLimited { .. } | E::AccountLimited { .. } => 5,
        E::NotFound { .. } => 6,
        E::Forbidden { .. } | E::InvalidInput { .. } => 7,
        E::SendRefused { .. } | E::Cancelled => 8,
        E::ProfileBusy { .. } | E::DbKeyInvalid { .. } => 9,
        E::SendOutcomeUnknown { .. } => 10,
        E::Internal { .. } | E::Unknown { .. } => 1,
    }
}

/// 「要登录 / 登录失败」那一档（`auth_*`；第一阶段不支持的授权步骤也落这里）
pub const EXIT_AUTH: u8 = 3;

fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// `CoreError` → `error.data`（`errors.md` 第一节第 1 小节「`error.data` 的字段」：码、说明、能否重试、
/// 各码附加字段平铺、`td` 附件）。附加字段都是写死的名字、数字或枚举，不带内容
pub fn core_error_data(e: &CoreError) -> Value {
    let mut d = json!({
        "code": e.code(),
        "message_zh": e.message_zh(),
        "retryable": e.retryable(),
    });
    let extra = match e {
        CoreError::AuthWrongState { expected } => json!({ "expected": to_value(expected) }),
        CoreError::RateLimited { retry_after_s, .. } => json!({ "retry_after_s": retry_after_s }),
        CoreError::NotFound { what, .. } | CoreError::Internal { what } => json!({ "what": what }),
        CoreError::ProfileBusy { holder: Some(h) } => json!({ "holder": to_value(h) }),
        CoreError::DbKeyInvalid { source } => json!({ "source": to_value(source) }),
        CoreError::SendRefused { reason } => json!({ "reason": to_value(reason) }),
        CoreError::SendOutcomeUnknown {
            chat_id,
            message_id,
        } => json!({ "chat_id": chat_id, "message_id": message_id }),
        CoreError::Unknown { td } => json!({ "td_code": td.code }),
        _ => json!({}),
    };
    if let (Some(d), Value::Object(x)) = (d.as_object_mut(), extra) {
        d.extend(x);
    }
    if let (Some(d), Some(td)) = (d.as_object_mut(), e.td()) {
        d.insert("td".into(), to_value(td));
    }
    d
}

/// 第一阶段不支持的授权步骤，给人的话（占位措辞）
pub const UNSUPPORTED_STEP_ZH: &str = "这一步本应用暂不支持，请先在官方客户端完成";

/// 出结尾的 `error` 行并给出退出码
pub fn fail_with(format: Format, f: &Fail) -> ExitCode {
    match f {
        Fail::Core(e) => {
            let data = core_error_data(e);
            match format {
                Format::Json => emit(&envelope("error", data)),
                Format::Text => text_line(e.message_zh()),
            }
            ExitCode::from(core_exit(e))
        }
        Fail::Usage(m) => fail(format, Code::Usage, m, json!({})),
        Fail::NotImplemented {
            command,
            message_zh,
        } => fail(
            format,
            Code::NotImplemented,
            message_zh,
            json!({ "command": command }),
        ),
        Fail::Interrupted => fail(format, Code::Interrupted, "已中断", json!({})),
        Fail::Unsupported(step) => {
            let data = json!({
                "code": "auth_wrong_state",
                "message_zh": UNSUPPORTED_STEP_ZH,
                "retryable": false,
                "expected": to_value(step),
            });
            match format {
                Format::Json => emit(&envelope("error", data)),
                Format::Text => text_line(UNSUPPORTED_STEP_ZH),
            }
            ExitCode::from(EXIT_AUTH)
        }
    }
}

/// 文字模式的一行（给人看）
pub fn text_line(s: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{s}");
    let _ = out.flush();
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
        Format::Text => text_line(message_zh),
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
        assert_eq!(Code::Interrupted.exit(), 130);
        use atg_core::error::{Holder, map_td_error};
        let cases = [
            (map_td_error(400, "PHONE_CODE_INVALID"), 3),
            (
                CoreError::AuthWrongState {
                    expected: AuthStep::WaitCode,
                },
                3,
            ),
            (map_td_error(500, "x"), 4),
            (CoreError::Disconnected, 4),
            (map_td_error(429, "retry after 3"), 5),
            (map_td_error(400, "PHONE_NUMBER_FLOOD"), 5),
            (map_td_error(404, "x"), 6),
            (CoreError::Forbidden { td: None }, 7),
            (CoreError::AuthPhoneNotForProfile, 3),
            (CoreError::Cancelled, 8),
            (
                CoreError::ProfileBusy {
                    holder: Some(Holder::Gui),
                },
                9,
            ),
            (CoreError::Internal { what: "x" }, 1),
            (map_td_error(502, "x"), 1),
        ];
        for (e, want) in cases {
            assert_eq!(core_exit(&e), want, "{}", e.code());
        }
    }

    #[test]
    fn core_error_data_shape() {
        use atg_core::error::map_td_error;
        let d = core_error_data(&map_td_error(400, "PHONE_CODE_INVALID"));
        assert_eq!(d["code"], "auth_code_invalid");
        assert_eq!(d["message_zh"], "验证码不对，请重新输入");
        assert_eq!(d["retryable"], false);
        assert_eq!(d["td"]["code"], 400);
        assert_eq!(d["td"]["message"], "PHONE_CODE_INVALID");
        let d = core_error_data(&CoreError::AuthWrongState {
            expected: AuthStep::WaitPassword,
        });
        assert_eq!(d["expected"], "wait_password");
        let d = core_error_data(&map_td_error(420, "FLOOD_WAIT_30"));
        assert_eq!(d["retry_after_s"], 30);
        assert!(d["td"].get("message").is_none());
    }
}
