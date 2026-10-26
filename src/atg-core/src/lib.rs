//! `atg-core`：无障碍 Telegram 的能力层。
//!
//! 内容树、数据与映射（不依赖 GPUI）：
//! - [`tree`]：内容树（AST）——一切要读、要打印的东西的唯一数据形状
//! - [`speech`]：读法渲染器（`brief`）
//! - [`json`]：CLI 的 JSON 行渲染器
//! - [`model`]：会话、消息、用户等数据类型与事件 `CoreEvent`
//! - [`store`]：reducer——吃事件出新状态（排序、墓碑、按字段合并、代次、换 id）
//! - [`project`]：投影——model + store → 内容树 `Doc`
//! - [`tdjson`]：TDLib JSON → `CoreEvent` 的映射（夹具与真实后端共用）
//!
//! 接 TDLib（第一刀第 3 块）：
//! - [`backend`]：`Backend` 特征（意图级动作，本块只有生命周期一组）
//! - [`td`]：真实后端（`tdlib-api` / `tdlib` 特性）· [`fake`]：假后端
//! - [`link`]：收线程（带序号）、监督与故障广播、事件流——两份后端共用
//! - [`pump`]：消费方把事件成批喂 reducer、重拉、重发应答
//! - [`error`]：`CoreError` 与 `map_td_error` · [`status`]：三条状态线与合成
//! - [`config`]：`.env.local`、档案、锁 · [`keystore`]：本地库钥匙（钥匙串）· [`secret`] · [`logline`]：只记形状的日志
//!
//! 骨架占位（后续块填）：[`notify`]
//!
//! 架构见 `docs/plan/001_arch-stack.md`，树的规约见 `specs/content-tree.md`。
//!
//! ## 注释里的代号
//!
//! - **第一刀第 N 块**：第一个原型（`atg-cli` 登录 + 列会话 + 读法）拆成的第 N 件事，见 `docs/plan/001_arch-stack.md`
//!   第六节；路线图编号 M1.1 等见 `docs/plan/002_roadmap.md`
//! - **拍板 / 人定**：人做过的决定，记在 `docs/plan/00N_decisions-*.md`。「拍板 001-8」= `001_arch-stack.md` 第七节
//!   第 8 条；「005 N2」「006 甲5」「005 补丁 ③」= 对应决定记录表里那一行；「R7」等读法条目见 `specs/content-tree.md`
//! - **L0 / L1 / L2**：动作对外的影响等级（`specs/contracts/actions.md` 第一节第 2 小节）。L0 只读或只影响本机；
//!   L1 别人能看到痕迹但不发内容（已读、在线、正在输入）；L2 发内容或改别人看得到的东西（发送、删除……），
//!   要过**发送闸**（只有拿到人的确认凭据才发）且**绝不自动重试**
//! - **特征**（Rust trait，如 `Backend`）≠ **特性**（Cargo feature，如 `tdlib`）
//! - **夹具**（fixture）：手写的假 TDLib 数据，测试用；**快照测试**：输出存成文件，以后每次都与它比
//! - **窗口**：当前打开的会话里已加载的那一段消息（`store::Window`）；**凭据**（`HistoryTicket`）与**代次**：
//!   发历史请求时记下的版本，旧请求的应答不会盖掉新的
//! - **测试服务器**（test DC，DC = Telegram 的数据中心）：Telegram 给开发者的独立服务器，账号与正式服务器完全分开

// 拍板 005 补丁 ③（`docs/plan/005_decisions-b3.md`）：TDLib 对象（及由它映射来的含内容的类型）不许用 `{:?}` 整个打印。
// 非测试代码：这行拦 print! / write! 系的 Debug、`dbg!`，以及 unwrap / expect（失败时用 Debug 打出错误值）；
// format! / panic! / assert_eq! / 日志宏 clippy 管不到，由 `scripts/check-no-debug.sh` 拦（CI 跑）。
// 测试里的夹具是假数据，放行。
#![cfg_attr(
    not(test),
    deny(
        clippy::use_debug,
        clippy::dbg_macro,
        clippy::unwrap_used,
        clippy::expect_used
    )
)]

pub mod backend;
pub mod config;
pub mod error;
pub mod fake;
pub mod json;
pub mod keystore;
pub mod link;
pub mod logline;
pub mod model;
pub mod notify;
pub mod project;
pub mod pump;
// 本 crate 的单元测试里这些类型派生完整 Debug，脱敏实现只在非测试构建里
#[cfg(not(test))]
mod redact;
pub mod secret;
pub mod speech;
pub mod status;
pub mod store;
pub mod td;
pub mod tdjson;
pub mod tree;
