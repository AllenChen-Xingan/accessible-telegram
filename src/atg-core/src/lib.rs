//! `atg-core`：无障碍 Telegram 的能力层。
//!
//! 原型阶段只有四块（不接 TDLib、不依赖 GPUI）：
//! - [`tree`]：内容树（AST）——一切要读、要打印的东西的唯一数据形状
//! - [`speech`]：读法渲染器（`brief`）
//! - [`json`]：CLI 的 JSON 行渲染器
//! - [`tdjson`]：临时输入——TDLib JSON 夹具 → 树
//!
//! 架构见 `docs/plan/001_arch-stack.md`，树的规约见 `specs/content-tree.md`。

pub mod json;
pub mod speech;
pub mod tdjson;
pub mod tree;
