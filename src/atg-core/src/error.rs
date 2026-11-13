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
    /// 测试服务器上只许发收藏夹。规约另有「第二个自己的测试号私聊」例外（`actions.md` 第一节第 4 小节第 2 条，N7），
    /// **还没做**（落在 M2.7 删除那块）；现在只放行收藏夹
    TestDcSavedOnly,
    /// 账号被 Telegram 限制，L2 全停
    AccountLimited,
    /// 这一类 L1 副作用被整类关掉（第一阶段不会出现）
    L1Disabled,
    /// 直发模式下，最近已向同一会话提交过同样的内容（B17-2）：上次已发出的看 10 分钟、结果未知的看 24 小时；
    /// 前端各自给放行的办法（CLI 是 `--again`）
    DuplicateRecent,
    /// TDLib 的预览把回复关系去掉了（被回复那条找不到、不能回复或不在同一话题，`MessagesManager.cpp:21144-21170`）
    ReplyDropped,
    /// TDLib 的预览把消息放进了别的话题
    TopicChanged,
    /// 正文太长，Touch ID 确认框放不下全文（人看不到全文就不让按；用直发模式或以后的界面发）
    TooLongToConfirm,
}

/// 直发防重复拦下时，上一次是什么时候、什么结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastSend {
    /// Unix 秒
    pub at: i64,
    /// 距现在几秒
    pub ago_s: i64,
    pub outcome: crate::gate::Outcome,
}

impl LastSend {
    /// 被防重复拦下时给人的话：几秒前、什么结果（GUI 与 CLI 同一句；大总管 2026-10-26 要求说清）。怎么放行由前端
    /// 补（CLI 是 `--again`，界面是另一个按键），这里不写死某一边的说法
    pub fn message_zh(&self) -> String {
        use crate::gate::Outcome;
        let when = match self.ago_s {
            s if s < 60 => format!("{s} 秒前"),
            s if s < 3600 => format!("{} 分 {} 秒前", s / 60, s % 60),
            s => format!("{} 小时 {} 分前", s / 3600, s % 3600 / 60),
        };
        let what = match self.outcome {
            Outcome::Succeeded => "已发出",
            Outcome::Unknown => "结果未知，可能已发出",
            Outcome::Submitted => "已交出、没等到结果，可能已发出",
            Outcome::Failed => "失败",
        };
        format!("{when}已向同一会话提交过同样的内容（结果：{what}），这次没有发")
    }
}

/// `send_outcome_unknown` 是怎么来的
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownWhy {
    /// 30 秒内既没等到成功也没等到失败
    Timeout,
    /// 待发那条被删掉了（TDLib 不来失败；可能已经送达，`MessagesManager.cpp:27848-27851`）
    PendingDeleted,
    /// 等结果途中与 TDLib 的连接断了
    Disconnected,
}

/// 发送前本地就能判定的「这样发不行」（机读码仍是 `invalid_input`，`why` 平铺进 `error.data`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidSendWhy {
    /// 话题群里不回复任何一条的新消息，第一阶段拒发（`actions.md` `send_text_prepare`）
    ForumNeedsReply,
    /// 被回复的那条不能回复（`messageProperties.can_be_replied` 为假）
    ReplyNotAllowed,
    /// 正文是空的
    EmptyText,
    /// 正文超过上限（`send::TEXT_MAX_BYTES`，64 KB）
    TooLong,
    /// 要重发的那条不是发送失败的消息
    NotFailed,
    /// TDLib 说这条不能重发（`can_retry` 为假）
    CannotRetry,
    /// 重发要换发送身份或换引用（第一阶段不做）
    NeedsOther,
    /// 重发要付星（付费发送不做）
    Paid,
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
        /// 只有 `duplicate_recent` 带：上一次什么时候、什么结果
        last: Option<LastSend>,
    },
    SendOutcomeUnknown {
        /// 经本机接口转发、会话写的是 `saved` 而发起方认不出 id 时为 `None`（契约里的 `null`）
        chat_id: Option<i64>,
        /// 临时 id（TDLib 交出时编的号，不是正式 id，不能拿去重发）；交出途中就断了、没拿到时为 0
        temp_message_id: i64,
        why: UnknownWhy,
    },
    /// 机读码 `invalid_input`：发送前本地判定不能这样发，带 `why`
    InvalidSend {
        why: InvalidSendWhy,
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
            Self::InvalidInput { .. } | Self::InvalidSend { .. } => "invalid_input",
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
            | Self::InvalidSend { .. }
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
            Self::SendRefused { reason, .. } => match reason {
                SendRefusedReason::DuplicateRecent => {
                    "刚才已向同一会话提交过同样的内容，这次没有发；确要再发一次请明确要求再发"
                }
                SendRefusedReason::ReplyDropped => {
                    "被回复的那条找不到或不能回复，Telegram 会把它当普通消息发出，已拦下没发"
                }
                SendRefusedReason::TopicChanged => "这条会落到别的话题里，已拦下没发",
                SendRefusedReason::TooLongToConfirm => {
                    "内容太长，确认框放不下全文，没有发；请拆短，或用直发模式"
                }
                SendRefusedReason::TestDcSavedOnly => "测试服务器上只许发自己的收藏夹",
                SendRefusedReason::AuthFailed => "没有通过 Touch ID 或系统密码，没有发",
                SendRefusedReason::NotInteractive => {
                    "这里弹不出 Touch ID 或系统密码框（比如经 SSH），没有发"
                }
                SendRefusedReason::AccountLimited => {
                    "账号被 Telegram 限制，发送已全部停止；人处理好之后删掉本地档案目录里的 account-limited 文件才恢复"
                }
                SendRefusedReason::NoToken => "没有确认就不能发，没有发",
                SendRefusedReason::TargetMismatch => "确认的是别的会话，没有发；请重新发起",
                SendRefusedReason::ContentMismatch => {
                    "要发的内容和确认时不一样，没有发；请重新发起"
                }
                SendRefusedReason::TokenExpired => "确认已超过 60 秒失效，没有发；请重新发起",
                SendRefusedReason::TokenReused => "这次确认已经用过，没有再发；请重新发起",
                SendRefusedReason::L1Disabled => "这类操作已被关掉",
            },
            Self::SendOutcomeUnknown { why, .. } => match why {
                UnknownWhy::PendingDeleted => {
                    "待发消息已被删除，是否已送达未知；不要重发，先去会话里看一眼最近的消息"
                }
                UnknownWhy::Timeout | UnknownWhy::Disconnected => {
                    "消息已交出，但还不知道发没发出去，没发出的下次启动会自动补发；不要重发，先去会话里看一眼最近的消息"
                }
            },
            Self::InvalidSend { why } => match why {
                InvalidSendWhy::ForumNeedsReply => "话题群里请先选一条消息回复",
                InvalidSendWhy::ReplyNotAllowed => "这条消息不能回复",
                InvalidSendWhy::EmptyText => "要发的内容是空的",
                InvalidSendWhy::TooLong => "要发的内容太长（超过 64 KB）",
                InvalidSendWhy::NotFailed => "这条消息不是发送失败的，不用重发",
                InvalidSendWhy::CannotRetry => "这条消息 Telegram 说不能重发",
                InvalidSendWhy::NeedsOther => {
                    "这条要换个身份发（比如以频道名义）或换引用才能重发，本应用暂不支持"
                }
                InvalidSendWhy::Paid => {
                    "这条重发要付 Telegram Stars（Telegram 的付费星星），本应用不做付费发送"
                }
            },
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
