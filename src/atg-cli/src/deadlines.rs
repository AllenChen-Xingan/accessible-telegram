//! 期限（#63 #73 #78）：CLI 各处「最多等多久」只在这里定数，别处不写数字字面量；对外的说法在
//! `cli.md`「最多等多久」。每个常量一行说明：等什么 · 为什么是这个数 · 出处。内核机制的期限留在 atg-core 原处
//! （`backend::SHUTDOWN_TIMEOUT`、`ipc::REQUEST_TIMEOUT` / `WRITE_TIMEOUT`）；
//! 发送结果等多久是发送动作本身的规则，GUI 也用：`atg_core::send::RESULT_TIMEOUT`。
//!
//! 等一个后端请求经下面两个函数之一（超时换成 `network` / `disconnected` 只在 [`stalled`] 一处）：
//! - [`within`]：期限 **且看 Ctrl+C**（停止标志）。按下就丢下等待、报 `interrupted`。用在装了信号处理、
//!   约定 Ctrl+C 报 `interrupted` 的路径：登录这组（提交也算——丢下无妨，结果以 TDLib 的授权线为准，
//!   下次 `auth state` 照实报）、`play` 的取消息 / 下载。
//! - [`bounded`]：**只看期限**。用在三处：收尾路径；Ctrl+C 约定是照常出 `result` 的 `watch` / `serve`；
//!   发出去就改服务器状态、事后又没地方查回来的请求（`play --mark-listened` 的标已听）。没装信号处理的
//!   `chats` / `history` / `send` / `download` 也用它：Ctrl+C 照默认直接结束进程。
//!
//! 「看一眼 Ctrl+C」的间隔有两份：通用等待每片 100 毫秒（`atg_core::link` 的 `STOP_POLL`），等授权线每片
//! [`AUTH_POLL`]（授权线另有条件变量，醒得早，片可以长些）。

use crate::out::Fail;
use atg_core::backend::Backend;
use atg_core::error::CoreError;
use atg_core::link::{Until, block_on_timeout, block_on_until};
use atg_core::status::CoreState;
use std::future::Future;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

/// `chats` 等授权线离开「等参数」· TDLib 起来要一会儿 · `cli.md` `chats`
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
/// 等补发完（`chats` / `history`）· 未读位置要等补发完才准 · `cli.md`：超时 10 秒
pub const SYNC_TIMEOUT: Duration = Duration::from_secs(10);
/// `chats` 整段加载 · 断网时 `loadChats` 可能一直不应答 · `cli.md` `chats`
pub const LOAD_TIMEOUT: Duration = Duration::from_secs(10);
/// 一次查询类请求（取会话、开关会话、取一条消息、一页历史、`me`、发送前的计划、标已听）· 同 [`LOAD_TIMEOUT`] 取值 · 大总管定 2026-11-13、#78
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(10);
/// 下载一个文件（语音等，`download` / `play`）· 大文件慢网也要下得完；到点报 `network`，Ctrl+C 照样停 · 大总管定 2026-11-13、#78，人可推翻
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
/// 起后端（开本地库、交 TDLib 参数）· 本地操作，正常一两秒；大库或慢盘留足 · #78 新加，人可推翻。
/// 到点也报 `network`（[`stalled`]）：本地卡住与网络卡住对人的处置相同（稍后重试），不另开码
pub const START_TIMEOUT: Duration = Duration::from_secs(30);
/// 登录每一步：提交与随后等授权线（TDLib 的授权状态 `AuthStep`，`errors.md` 第一节第 3 小节）离开这一步共用 · 要连服务器 · #63
pub const STEP_TIMEOUT: Duration = Duration::from_secs(30);
/// 等授权线时每片最多等多久再看一眼 Ctrl+C · 200 毫秒人感觉不到 · #63
pub const AUTH_POLL: Duration = Duration::from_millis(200);
/// 取一个论坛话题的设置（`watch` / `serve`）· 取不到就按 `topic_unknown` 不提示，宁可不吵 · 大总管定 2026-11-13、#73
pub const TOPIC_TIMEOUT: Duration = Duration::from_secs(5);
/// `history` / `watch` 的 `saved` 等事件流里 TDLib 推来的 `my_id` · 本地就有，5 秒足够 · 等不到报 `me_unknown`（`errors.md`）。
/// `send` 的 `saved` 不等它，直接问 `me()`（闸用的也是它），按 [`QUERY_TIMEOUT`] 等、到点报 `network`
pub const ME_TIMEOUT: Duration = Duration::from_secs(5);
/// 转发时两帧之间：一条 `chats` 最长约 30 秒（等授权 + 加载 + 等补发各 10 秒），执行方还可能先在处理别人的请求；
/// `send` 出 `plan` 之前要找会话、取会话、做计划、取被回复的那条（各 10 秒，约 40 秒），出完之后要等人按
/// Touch ID（最多 60 秒）再等结果（最多 30 秒），中间都没有帧——留足 · `config-privacy.md` 本机接口
pub const FRAME_TIMEOUT: Duration = Duration::from_secs(150);

/// 按期限等一个后端请求。外层 `Err` 是没等到：超时 → [`stalled`]，叫停 → `interrupted`；
/// 内层是后端自己的应答（调用方可能要分辨哪些错让人重输）
pub fn within<B: Backend, T>(
    b: &B,
    fut: impl Future<Output = Result<T, CoreError>>,
    deadline: Instant,
    stop: &AtomicBool,
) -> Result<Result<T, CoreError>, Fail> {
    match block_on_until(fut, deadline, stop) {
        Until::Done(r) => Ok(r),
        Until::TimedOut => Err(Fail::Core(stalled(b))),
        Until::Stopped => Err(Fail::Interrupted),
    }
}

/// 同 [`within`]，但不看 Ctrl+C（用在哪见模块头）。超时与后端的错合成一个 `CoreError`
pub fn bounded<B: Backend, T>(
    b: &B,
    fut: impl Future<Output = Result<T, CoreError>>,
    timeout: Duration,
) -> Result<T, CoreError> {
    block_on_timeout(fut, timeout).unwrap_or_else(|| Err(stalled(b)))
}

/// 期限内等不到：内核故障 → `disconnected`；否则当网络问题（`network`）
pub fn stalled<B: Backend>(b: &B) -> CoreError {
    if b.status().lines.core == CoreState::Faulted {
        CoreError::Disconnected
    } else {
        CoreError::Network { td: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 提示语里写着的秒数跟着 FRAME_TIMEOUT（改了期限，提示语不能还说旧数）
    #[test]
    fn executor_silent_text_follows_frame_timeout() {
        let want = format!("{} 秒", FRAME_TIMEOUT.as_secs());
        assert!(crate::forward::EXECUTOR_SILENT_ZH.contains(&want));
    }
}
