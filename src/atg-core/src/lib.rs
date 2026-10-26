//! `atg-core`：无障碍 Telegram 的能力层。
//!
//! 已有（内容树原型，不接 TDLib、不依赖 GPUI）：
//! - [`tree`]：内容树（AST）——一切要读、要打印的东西的唯一数据形状
//! - [`speech`]：读法渲染器（`brief`）
//! - [`json`]：CLI 的 JSON 行渲染器
//! - [`tdjson`]：临时输入——TDLib JSON 夹具 → 树
//!
//! 骨架占位（模块名与架构方案第一节第 2 小节对齐，后续块填）：
//! [`td`] · [`model`] · [`store`] · [`notify`] · [`backend`]
//!
//! 架构见 `docs/plan/001_arch-stack.md`，树的规约见 `specs/content-tree.md`。

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
pub mod json;
pub mod model;
pub mod notify;
pub mod speech;
pub mod store;
pub mod td;
pub mod tdjson;
pub mod tree;
