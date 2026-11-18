//! `td`：真实后端 `TdBackend`——起 TDLib、收线程、发请求、关闭流程。
//!
//! 只有这个模块许直接用 tdlib-rs；它的类型不许渗出去（`docs/plan/001_arch-stack.md` 第一节第 1、3、4 小节）。
//! TDLib 对象绝不用 `{:?}` 整个打印（拍板 005 补丁 ③），由 crate 根的 lint 行与 `scripts/check-no-debug.sh` 拦。
//! 代码都挂在 `tdlib-api` 特性下：不开特性时这个模块是空的，`cargo build` / CI 默认不碰 TDLib。
//!
//! 薄壳：收线程、序号、三线合成、监督都在 [`crate::link`]（与假后端共用）；映射在 [`crate::tdjson::classify`]
//! ——这里把 tdlib-rs 的 `Update` 原样序列化回 TDLib JSON 交过去，所以映射只有一份、夹具测过。
//!
//! 起步顺序（`config-privacy.md`）：校验档案路径 → 拿档案锁 → 读 `.env.local` → 从钥匙串取钥匙 → **建客户端之前**
//! 用同步 `execute` 把 TDLib 日志降到 0、写到 `~/Library/Logs/atg/tdlib-<档案>.log`（级别 1 也会漏正文，
//! `MessageContent.cpp:7696`）→ 放「运行中」标记 → 建客户端、起收线程 → `setTdlibParameters`（加密钥匙、
//! `use_test_dc` 只从档案标记来）→ 显式设 `online = false`。
//! 客户端建好之后 `start` 再失败（如钥匙不对、网络错）：客户端还在，调用方照常调 `shutdown` 收尾；例外是收线程起不来，
//! `start` 自己交一次 `close`、清运行标记、退回没起（#62），调用方不用再收尾。
//!
//! tdlib-rs 的 `receive()` 是全进程共用的，内部 `OBSERVER` 是全局单例，所以一个进程只许一个 `TdBackend`。

#[cfg(feature = "tdlib-api")]
pub use real::{TdBackend, TdConfig, tdlib_version};

#[cfg(feature = "tdlib-api")]
mod boot;
// 交出口的请求 JSON → tdlib-rs 参数
#[cfg(feature = "tdlib-api")]
mod requests;
// 纯类型：不开特性时只有单测用它
#[cfg(any(feature = "tdlib-api", test))]
#[cfg_attr(not(feature = "tdlib-api"), allow(dead_code))]
mod lifecycle;

#[cfg(feature = "tdlib-api")]
mod real;

#[cfg(test)]
mod tests;
