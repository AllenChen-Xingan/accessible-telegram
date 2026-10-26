//! `td`：起 TDLib、收线程（带序号）、发请求、关闭流程、收线程监督与故障广播。
//!
//! **骨架占位**，第一刀第 3 块（M1.1）填（路线图编号见 `docs/plan/002_roadmap.md`）。只有这个模块许直接用 tdlib-rs；它的类型不许渗出去
//! （`docs/plan/001_arch-stack.md` 第一节第 1、3、4 小节）。TDLib 对象绝不用 `{:?}` 整个打印
//! （拍板 005 补丁 ③，`docs/plan/005_decisions-b3.md`），由 crate 根的 lint 行与 `scripts/check-no-debug.sh` 拦。
