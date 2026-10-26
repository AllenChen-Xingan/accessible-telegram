//! `error`：对外唯一的错误类型 [`CoreError`] 与 TDLib 错误的映射 [`map_td_error`]。
//!
//! 规约正本：`specs/contracts/errors.md` 第一节第 1、2 小节。tdlib-rs 的错误类型不许渗出去——
//! `td` 模块拿到 TDLib 的 `{code, message}` 立刻过 [`map_td_error`]。
//!
//! 隐私（`errors.md` 第二节第 3 条）：TDLib 原文只在它是登记过的那几个码时才留（[`TdInfo::message`]）；
//! 码 406 连附件都不建；限流只留解析出的秒数。所以这个类型可以放心派生 `Debug`。

use crate::status::AuthStep;
use serde::Serialize;

/// 发送闸拒绝的原因（`errors.md` 第一节第 1 小节 `send_refused.reason`）。
/// 发送闸：L2 动作（发送、删除……）的唯一入口，只有拿到「确认凭据」（token：人确认过「发给谁 + 发什么」后
/// 开出的一次性凭证，60 秒有效）才发（`actions.md` 第一节第 4 小节）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SendRefusedReason {
    /// 没有凭据
    NoToken,
    /// 凭据开给的是别的会话
    TargetMismatch,
    /// 内容与确认时不一样
    ContentMismatch,
    /// 凭据过了 60 秒
    TokenExpired,
    /// 凭据已经用过
    TokenReused,
    /// 弹不出 Touch ID / 系统密码框（如经 SSH 运行）
    NotInteractive,
    /// Touch ID / 系统密码没过
    AuthFailed,
    /// 测试服务器上只许发收藏夹（和第二个测试号的私聊）
    TestDcSavedOnly,
    /// 账号被 Telegram 限制，L2 全停
    AccountLimited,
    /// 这一类 L1 副作用被整类关掉（第一阶段不会出现）
    L1Disabled,
}

/// 取钥匙失败的来源（`db_key_invalid.source`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeySource {
    KeychainDenied,
    WrongKey,
}

/// 占着档案的进程种类（`profile_busy.holder`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Holder {
    Cli,
    Gui,
}

/// TDLib 原错误的附件：数字码 + 只在登记过的码时才有的原文
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TdInfo {
    pub code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<&'static str>,
}

/// `atg-core` 所有对外函数失败时返回的唯一错误类型。变体与机读码一一对应（[`CoreError::code`]）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    AuthRequired {
        td: Option<TdInfo>,
    },
    AuthPhoneInvalid {
        td: Option<TdInfo>,
    },
    AuthCodeInvalid {
        td: Option<TdInfo>,
    },
    AuthCodeExpired {
        td: Option<TdInfo>,
    },
    AuthPasswordInvalid {
        td: Option<TdInfo>,
    },
    /// 号码与档案不配：测试档案收到非 `99966` 开头的号，或正式档案收到 `99966` 开头的测试号
    /// （`config-privacy.md` 第一节第 2 小节）。本地检查拦下，不交给 TDLib，所以没有 `td` 附件
    AuthPhoneNotForProfile,
    /// ⚠️ `expected` 是**现在所在**的那一步（「现在等的是这个」），不是调用方本想交的那一步。
    /// 字段名照 `errors.md` 第一节第 1 小节（「当前的 `AuthStep` 值」），会原样平铺进 CLI 的 JSON
    AuthWrongState {
        expected: AuthStep,
    },
    RateLimited {
        retry_after_s: u32,
        td: Option<TdInfo>,
    },
    AccountLimited {
        td: Option<TdInfo>,
    },
    NotFound {
        what: &'static str,
        td: Option<TdInfo>,
    },
    Forbidden {
        td: Option<TdInfo>,
    },
    InvalidInput {
        td: Option<TdInfo>,
    },
    Network {
        td: Option<TdInfo>,
    },
    Disconnected,
    ProfileBusy {
        holder: Option<Holder>,
    },
    DbKeyInvalid {
        source: KeySource,
    },
    SendRefused {
        reason: SendRefusedReason,
    },
    SendOutcomeUnknown {
        chat_id: i64,
        message_id: i64,
    },
    Cancelled,
    /// 406（附件都不建）或本项目自己的 bug。`what` 只放写死的说明，不放运行时的值
    Internal {
        what: &'static str,
    },
    Unknown {
        td: TdInfo,
    },
}

impl CoreError {
    /// 稳定的机读码（snake_case，CLI 原样输出）
    pub fn code(&self) -> &'static str {
        match self {
            Self::AuthRequired { .. } => "auth_required",
            Self::AuthPhoneInvalid { .. } => "auth_phone_invalid",
            Self::AuthCodeInvalid { .. } => "auth_code_invalid",
            Self::AuthCodeExpired { .. } => "auth_code_expired",
            Self::AuthPasswordInvalid { .. } => "auth_password_invalid",
            Self::AuthPhoneNotForProfile => "auth_phone_not_for_profile",
            Self::AuthWrongState { .. } => "auth_wrong_state",
            Self::RateLimited { .. } => "rate_limited",
            Self::AccountLimited { .. } => "account_limited",
            Self::NotFound { .. } => "not_found",
            Self::Forbidden { .. } => "forbidden",
            Self::InvalidInput { .. } => "invalid_input",
            Self::Network { .. } => "network",
            Self::Disconnected => "disconnected",
            Self::ProfileBusy { .. } => "profile_busy",
            Self::DbKeyInvalid { .. } => "db_key_invalid",
            Self::SendRefused { .. } => "send_refused",
            Self::SendOutcomeUnknown { .. } => "send_outcome_unknown",
            Self::Cancelled => "cancelled",
            Self::Internal { .. } => "internal",
            Self::Unknown { .. } => "unknown",
        }
    }

    /// 能不能自动重试（`errors.md` 表最后一列）。**L2 动作的错误由调用方一律当 false**（L2 不自动重试）
    pub fn retryable(&self) -> bool {
        matches!(self, Self::RateLimited { .. } | Self::Network { .. })
    }

    /// TDLib 原错误的附件（有就给）
    pub fn td(&self) -> Option<&TdInfo> {
        match self {
            Self::AuthRequired { td }
            | Self::AuthPhoneInvalid { td }
            | Self::AuthCodeInvalid { td }
            | Self::AuthCodeExpired { td }
            | Self::AuthPasswordInvalid { td }
            | Self::RateLimited { td, .. }
            | Self::AccountLimited { td }
            | Self::NotFound { td, .. }
            | Self::Forbidden { td }
            | Self::InvalidInput { td }
            | Self::Network { td } => td.as_ref(),
            Self::Unknown { td } => Some(td),
            Self::AuthWrongState { .. }
            | Self::AuthPhoneNotForProfile
            | Self::Disconnected
            | Self::ProfileBusy { .. }
            | Self::DbKeyInvalid { .. }
            | Self::SendRefused { .. }
            | Self::SendOutcomeUnknown { .. }
            | Self::Cancelled
            | Self::Internal { .. } => None,
        }
    }

    /// 一句中文说明：发生了什么 + 下一步能做什么。**占位措辞**——正本是读法快照，人定标后搬
    /// `specs/style/`（`errors.md` 第一节第 5 小节）；不带 TDLib 原文、不带内容
    pub fn message_zh(&self) -> &'static str {
        match self {
            Self::AuthRequired { .. } => "还没有登录，或登录已失效，请重新登录",
            Self::AuthPhoneInvalid { .. } => "手机号格式不对，请检查后重新输入",
            Self::AuthCodeInvalid { .. } => "验证码不对，请重新输入",
            Self::AuthCodeExpired { .. } => "验证码已过期，请重发验证码",
            Self::AuthPasswordInvalid { .. } => "二步验证密码不对，请重新输入",
            Self::AuthPhoneNotForProfile => {
                "这个号码不能用在这个档案上：测试档案只收 99966 开头的测试号，正式档案不收测试号"
            }
            Self::AuthWrongState { .. } => "当前登录步骤不收这项输入",
            Self::RateLimited { .. } => "请求太频繁，请等一会儿再试",
            Self::AccountLimited { .. } => "账号被 Telegram 暂时限制，已停止发送和登录，请人工处理",
            Self::NotFound { .. } => "要找的会话、消息或用户不存在",
            Self::Forbidden { .. } => "没有权限做这件事",
            Self::InvalidInput { .. } => "参数不对",
            Self::Network { .. } => "网络出错，连接恢复后可以再试",
            Self::Disconnected => "与 Telegram 的连接已断，请重新启动",
            Self::ProfileBusy { .. } => "本地档案正被另一个程序使用",
            Self::DbKeyInvalid { .. } => {
                "本地缓存打不开，可以清掉缓存重新登录（聊天记录在服务器上，不会丢）"
            }
            Self::SendRefused { .. } => "发送被拒绝",
            Self::SendOutcomeUnknown { .. } => "消息已交出，但还不知道发没发出去，请查看待发消息",
            Self::Cancelled => "已取消",
            Self::Internal { .. } => "程序内部出错",
            Self::Unknown { .. } => "出现了无法识别的错误",
        }
    }
}

/// 允许原文进附件的 TDLib 错误消息（`errors.md` 第一节第 1 小节末段），其余一律只留数字码
const REGISTERED: &[&str] = &[
    "PHONE_NUMBER_INVALID",
    "PHONE_CODE_INVALID",
    "PHONE_CODE_EXPIRED",
    "PASSWORD_HASH_INVALID",
    "PEER_FLOOD",
    "PHONE_NUMBER_FLOOD",
    "CHAT_WRITE_FORBIDDEN",
];

/// 解析不出秒数时按 60 秒（`errors.md` 第一节第 2 小节第 3 条）
const DEFAULT_RETRY_S: u32 = 60;

fn td_info(code: i32, message: &str) -> Option<TdInfo> {
    Some(TdInfo {
        code,
        message: REGISTERED.iter().find(|r| **r == message).copied(),
    })
}

/// 从限流消息里取秒数：`FLOOD_WAIT_30` / `SLOWMODE_WAIT_30` / `Too Many Requests: retry after 30`
fn retry_after(message: &str) -> u32 {
    let digits: String = message
        .rsplit(|c: char| !c.is_ascii_digit())
        .find(|s| !s.is_empty())
        .unwrap_or("")
        .to_owned();
    digits.parse().unwrap_or(DEFAULT_RETRY_S)
}

/// TDLib 错误 → [`CoreError`]。纯函数，按 `errors.md` 第一节第 2 小节的顺序先中先出
pub fn map_td_error(code: i32, message: &str) -> CoreError {
    // 0. 两条本地错误（码会和别的撞）
    if message == "Wrong database encryption key" {
        return CoreError::DbKeyInvalid {
            source: KeySource::WrongKey,
        };
    }
    if message.contains("already in use") {
        return CoreError::ProfileBusy { holder: None };
    }
    // 1. 406：TDLib 约定这类错误「不得以任何方式处理或展示」（`td_api.tl:16`），所以绝不显示、绝不读出，连附件都不建
    if code == 406 {
        return CoreError::Internal { what: "td_406" };
    }
    let td = td_info(code, message);
    // 2. 账号被限
    if message == "PEER_FLOOD" || message == "PHONE_NUMBER_FLOOD" {
        return CoreError::AccountLimited { td };
    }
    // 3. 限流：只带秒数，不带原文
    if code == 429 || message.starts_with("FLOOD_WAIT_") || message.starts_with("SLOWMODE_WAIT_") {
        return CoreError::RateLimited {
            retry_after_s: retry_after(message),
            td: Some(TdInfo {
                code,
                message: None,
            }),
        };
    }
    // 420 是 Telegram 的「洪水」类码：除了带秒数的 FLOOD_WAIT（上面已按限流处理），其余视为账号被限
    if code == 420 {
        return CoreError::AccountLimited { td };
    }
    match code {
        401 => CoreError::AuthRequired { td },
        403 => CoreError::Forbidden { td },
        404 => CoreError::NotFound { what: "td", td },
        _ if message.ends_with("_NOT_FOUND") || message.ends_with("_ID_INVALID") => {
            CoreError::NotFound { what: "td", td }
        }
        400 => match message {
            "PHONE_NUMBER_INVALID" => CoreError::AuthPhoneInvalid { td },
            // 邮箱验证码错按验证码错报（`actions.md` `auth_submit_email_code`「同验证码」）；原文未登记，附件只带数字码
            "PHONE_CODE_INVALID" | "EMAIL_CODE_INVALID" => CoreError::AuthCodeInvalid { td },
            "PHONE_CODE_EXPIRED" => CoreError::AuthCodeExpired { td },
            "PASSWORD_HASH_INVALID" => CoreError::AuthPasswordInvalid { td },
            _ => CoreError::InvalidInput { td },
        },
        500 => CoreError::Network { td },
        _ => CoreError::Unknown {
            td: TdInfo {
                code,
                message: td.and_then(|t| t.message),
            },
        },
    }
}
