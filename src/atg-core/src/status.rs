//! `status`：三条状态线（内核 / 授权 / 连接）与合成函数 [`user_status`]。
//!
//! 规约正本：`specs/contracts/errors.md` 第一节第 3 小节。三条线分开存，合成时才排优先级；
//! 合成是纯函数，表驱动单测穷举全部组合。TDLib 类型名 → 线上取值的映射也在这里（纯函数，不碰 tdlib-rs）。

use crate::tree::UserStatus;
use serde::Serialize;

/// 内核线：本项目自己的（收线程监督，`docs/plan/001_arch-stack.md` 第一节第 4 小节）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreState {
    Starting,
    Running,
    Faulted,
    Closing,
    Closed,
}

impl CoreState {
    pub const ALL: [Self; 5] = [
        Self::Starting,
        Self::Running,
        Self::Faulted,
        Self::Closing,
        Self::Closed,
    ];
}

/// 授权线：`updateAuthorizationState`（`td_api.tl:11193`；各状态 `:206-263`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStep {
    /// TDLib 刚起，等本项目送启动参数（`setTdlibParameters`）
    WaitParameters,
    /// 等人输手机号
    WaitPhone,
    /// 等人输验证码（发到 Telegram 或短信）
    WaitCode,
    /// 等人输登录邮箱
    WaitEmail,
    /// 等人输邮箱里收到的验证码
    WaitEmailCode,
    /// 等人输二步验证密码
    WaitPassword,
    /// 号码还没注册，等填名字注册（第一阶段只在测试档案上做）
    WaitRegistration,
    /// 等在另一台已登录的设备上确认（扫码登录；对读屏不友好，第一阶段不做）
    WaitOtherDevice,
    /// 已登录
    Ready,
    /// 正在退出登录
    LoggingOut,
    /// 正在关闭
    Closing,
    /// 已关闭（客户端没了）
    Closed,
    /// 本项目第一阶段不支持的步骤（如 `authorizationStateWaitPremiumPurchase`），不崩
    Unsupported,
}

impl AuthStep {
    pub const ALL: [Self; 13] = [
        Self::WaitParameters,
        Self::WaitPhone,
        Self::WaitCode,
        Self::WaitEmail,
        Self::WaitEmailCode,
        Self::WaitPassword,
        Self::WaitRegistration,
        Self::WaitOtherDevice,
        Self::Ready,
        Self::LoggingOut,
        Self::Closing,
        Self::Closed,
        Self::Unsupported,
    ];

    /// `AuthorizationState` 的 TDLib 类型名 → 授权步骤；不认识的落 `Unsupported`
    pub fn from_td_type(td_type: &str) -> Self {
        match td_type {
            "authorizationStateWaitTdlibParameters" => Self::WaitParameters,
            "authorizationStateWaitPhoneNumber" => Self::WaitPhone,
            "authorizationStateWaitCode" => Self::WaitCode,
            "authorizationStateWaitEmailAddress" => Self::WaitEmail,
            "authorizationStateWaitEmailCode" => Self::WaitEmailCode,
            "authorizationStateWaitPassword" => Self::WaitPassword,
            "authorizationStateWaitRegistration" => Self::WaitRegistration,
            "authorizationStateWaitOtherDeviceConfirmation" => Self::WaitOtherDevice,
            "authorizationStateReady" => Self::Ready,
            "authorizationStateLoggingOut" => Self::LoggingOut,
            "authorizationStateClosing" => Self::Closing,
            "authorizationStateClosed" => Self::Closed,
            _ => Self::Unsupported,
        }
    }
}

/// 连接线：`updateConnectionState`（`td_api.tl:11818`；各状态 `:10657-10669`）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetState {
    WaitingForNetwork,
    ConnectingToProxy,
    Connecting,
    Updating,
    Ready,
    /// TDLib 升级后新增的状态；合成时当 `connecting`
    Unknown {
        td_type: String,
    },
}

impl NetState {
    /// `ConnectionState` 的 TDLib 类型名 → 连接状态
    pub fn from_td_type(td_type: &str) -> Self {
        match td_type {
            "connectionStateWaitingForNetwork" => Self::WaitingForNetwork,
            "connectionStateConnectingToProxy" => Self::ConnectingToProxy,
            "connectionStateConnecting" => Self::Connecting,
            "connectionStateUpdating" => Self::Updating,
            "connectionStateReady" => Self::Ready,
            // 类型名会进日志与 JSON：只认标识符字符
            other => Self::Unknown {
                td_type: crate::logline::safe_td_type(other).to_owned(),
            },
        }
    }
}

/// 三条线的原值（`status()` 返回它和合成后的 [`UserStatus`]）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lines {
    pub core: CoreState,
    pub auth: AuthStep,
    pub net: NetState,
}

impl Lines {
    /// 起步：内核在起、等参数、还没连上
    pub fn starting() -> Self {
        Self {
            core: CoreState::Starting,
            auth: AuthStep::WaitParameters,
            net: NetState::Connecting,
        }
    }

    pub fn user_status(&self) -> UserStatus {
        user_status(self.core, self.auth, &self.net)
    }
}

/// 三条线 → 用户状态。按 `errors.md` 第一节第 3 小节的表从上往下，先中先出；每种组合恰好落一行
pub fn user_status(core: CoreState, auth: AuthStep, net: &NetState) -> UserStatus {
    use AuthStep as A;
    use CoreState as C;
    if core == C::Faulted {
        return UserStatus::Fault;
    }
    if matches!(core, C::Closing | C::Closed) || matches!(auth, A::Closing | A::Closed) {
        return UserStatus::Stopping;
    }
    if core == C::Starting || auth == A::WaitParameters {
        return UserStatus::Connecting;
    }
    match auth {
        A::Unsupported => return UserStatus::UnsupportedStep,
        A::WaitPhone | A::LoggingOut => return UserStatus::SignedOut,
        A::WaitCode
        | A::WaitEmail
        | A::WaitEmailCode
        | A::WaitPassword
        | A::WaitRegistration
        | A::WaitOtherDevice => return UserStatus::SigningIn,
        A::Ready | A::WaitParameters | A::Closing | A::Closed => {}
    }
    match net {
        NetState::WaitingForNetwork => UserStatus::Offline,
        NetState::Connecting | NetState::ConnectingToProxy | NetState::Unknown { .. } => {
            UserStatus::Connecting
        }
        NetState::Updating => UserStatus::Syncing,
        // 走到这里：内核 running、授权 ready（其余授权值都已在上面返回）
        NetState::Ready => UserStatus::Online,
    }
}
