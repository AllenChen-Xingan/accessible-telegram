//! `backend`：`Backend` 特征——只暴露本项目要的意图级操作，真实（TDLib，`td` 模块，`tdlib` 特性）与
//! 假数据（[`crate::fake::FakeBackend`]）各一份实现。
//!
//! 规约正本：`specs/contracts/actions.md` 第一节第 3 小节。现有两组，都是 L0：
//! - **生命周期**（第一刀第 3 块）：`start` · `subscribe` · `status` · `shutdown`
//! - **登录**（第 4 块，T01）：`auth_prompt` · `auth_submit_*` · `auth_resend_code` · `auth_register`（只测试档案）·
//!   `logout`。输入一律是 [`Secret`]（规约里号码、邮箱写的是 `String`；这里更严：`Debug` 打不出、用完清零）。
//!   每个动作先查当前授权步骤，不收这项输入 → `auth_wrong_state`，不发给 TDLib。**提交成功只表示 TDLib
//!   收下了**；下一步是什么看下一条授权状态更新（[`Backend::wait_auth_update`]）
//!
//! 会话（#18）、发送等动作后续块加在这里，两份实现一起加。
//!
//! 线程：`start` 是异步的（等 `setTdlibParameters` 的应答），可放在任何执行器上；`shutdown` 阻塞到
//! `authorizationStateClosed` 或超时（退出路径上用，GUI 也在退出时调）。

use crate::auth::AuthPrompt;
use crate::config::Profile;
use crate::error::CoreError;
use crate::link::EventStream;
use crate::secret::Secret;
use crate::status::{AuthStep, Lines};
use crate::tree::UserStatus;
use std::time::Duration;

/// 关闭时等 `authorizationStateClosed` 的默认上限（`001_arch-stack.md` 第一节第 4 小节：超时就记日志后退出）
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

/// `status()` 的返回：合成后的用户状态 + 三条线原值（`errors.md` 第一节第 3 小节）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusReport {
    pub status: UserStatus,
    pub lines: Lines,
    /// 上次没有正常关闭（`start` 时发现「运行中」标记还在）：界面提示「上次没有正常关闭」
    pub previous_unclean: bool,
}

// 只给本项目自己的两个入口用，不跨线程传 future；不加 `Send` 约束（GUI 的前台执行器不要求）
#[allow(async_fn_in_trait)]
pub trait Backend {
    /// 起 TDLib 与收线程；读配置与钥匙串。档案被占 → `profile_busy`
    async fn start(&self, profile: &Profile) -> Result<(), CoreError>;

    /// 交出事件流（`CoreEvent` 带递增 `seq`）。一个后端只许一个订阅者，第二次 → `internal`
    fn subscribe(&self) -> Result<EventStream, CoreError>;

    fn status(&self) -> StatusReport;

    /// 当前授权步骤（`auth_state` 的步骤名部分）
    fn auth_state(&self) -> AuthStep {
        self.status().lines.auth
    }

    /// 当前授权步骤 + 给人看的提示（`auth_state` 全量）。提示**只许给终端 / 登录界面**，不进 JSON
    fn auth_prompt(&self) -> AuthPrompt;

    /// 等到授权线满足 `pred`（或内核故障 / 超时 → `None`）
    fn wait_auth(&self, timeout: Duration, pred: &dyn Fn(AuthStep) -> bool) -> Option<AuthStep>;

    /// 到现在为止收到过几条授权状态更新。提交之前记下它，提交成功后用 [`Backend::wait_auth_update`] 等下一条：
    /// 同一步重新下发时（换号重交、重发验证码之后还是 `wait_code`）只比步骤名是等不到的
    fn auth_generation(&self) -> u64;

    /// 等第 `after` 条之后的下一条授权状态更新（或内核故障 / 超时 → `None`），返回那时的步骤
    fn wait_auth_update(&self, timeout: Duration, after: u64) -> Option<AuthStep>;

    /// 手机号（`setAuthenticationPhoneNumber`）。本地先校验形状（`auth_phone_invalid`）与档案（测试档案只收
    /// `99966` 开头，正式档案不收 → `auth_phone_not_for_profile`）
    async fn auth_submit_phone(&self, phone: &Secret) -> Result<(), CoreError>;

    /// 验证码（`checkAuthenticationCode`）。错 → `auth_code_invalid`，过期 → `auth_code_expired`，步骤不变
    async fn auth_submit_code(&self, code: &Secret) -> Result<(), CoreError>;

    /// 换一种方式重发验证码（`resendAuthenticationCode`）
    async fn auth_resend_code(&self) -> Result<(), CoreError>;

    /// 登录邮箱（`setAuthenticationEmailAddress`）
    async fn auth_submit_email(&self, address: &Secret) -> Result<(), CoreError>;

    /// 邮箱里收到的验证码（`checkAuthenticationEmailCode`）
    async fn auth_submit_email_code(&self, code: &Secret) -> Result<(), CoreError>;

    /// 二步验证密码（`checkAuthenticationPassword`）。错 → `auth_password_invalid`
    async fn auth_submit_password(&self, password: &Secret) -> Result<(), CoreError>;

    /// 注册（`registerUser`）：**只在测试档案上**（测试服务器会定期清空账号，`testing.md` 第一节第 5 小节）；
    /// 正式档案 → `forbidden`。名字写死（[`crate::auth::TEST_REGISTER_FIRST_NAME`]）
    async fn auth_register(&self) -> Result<(), CoreError>;

    /// 登出本机（`logOut`）。调用方须先让人当场确认（`actions.md` `logout`）。之后 TDLib 自己关掉客户端
    /// （`logging_out` → `closing` → `closed`），本地库被清空；调用方照常 `shutdown`
    async fn logout(&self) -> Result<(), CoreError>;

    /// 发 `close`，等到 `authorizationStateClosed`（最多 `timeout`）。超时 → `internal`，故障 → `disconnected`；
    /// 两种都留着「运行中」标记，下次启动提示
    fn shutdown(&self, timeout: Duration) -> Result<(), CoreError>;
}

/// 一个后端只许一个订阅者：第二次取流时报的错
pub(crate) fn already_subscribed() -> CoreError {
    CoreError::Internal {
        what: "already_subscribed",
    }
}
