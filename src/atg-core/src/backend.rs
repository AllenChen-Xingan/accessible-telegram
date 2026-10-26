//! `backend`：`Backend` 特征——只暴露本项目要的意图级操作，真实（TDLib，`td` 模块，`tdlib` 特性）与
//! 假数据（[`crate::fake::FakeBackend`]）各一份实现。
//!
//! 规约正本：`specs/contracts/actions.md` 第一节第 3 小节。本块（第一刀第 3 块）只做**生命周期**一组：
//! `start` · `subscribe` · `status` · `shutdown`，外加读授权线的 `auth_state`。登录（#17）、会话（#18）、发送
//! 等动作后续块加在这里，两份实现一起加。所有动作都是 L0。
//!
//! 线程：`start` 是异步的（等 `setTdlibParameters` 的应答），可放在任何执行器上；`shutdown` 阻塞到
//! `authorizationStateClosed` 或超时（退出路径上用，GUI 也在退出时调）。

use crate::config::Profile;
use crate::error::CoreError;
use crate::link::EventStream;
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

    /// 当前授权步骤（`auth_state` 的步骤名部分；提示语归登录块 #17）
    fn auth_state(&self) -> AuthStep {
        self.status().lines.auth
    }

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
