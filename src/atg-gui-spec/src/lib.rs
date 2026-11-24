//! `atg-gui-spec`：界面自己的表，和「界面状态 → 无障碍树」的纯函数投影。**不依赖 GPUI**（卡 G1，
//! `docs/research/004_gui/decision.md` 第三节），所以 CI 不编界面也能生成、比对、快照。
//!
//! 正本在哪、其余怎么跟上：`docs/plan/012_gui-ssot.md`；产出该长什么样：`specs/gui/`。
//!
//! - [`aid`]：稳定标识（AXIdentifier）的唯一写法，界面代码只经 [`aid::aid`] 拿
//! - [`a11y`]：无障碍树契约表——每类元素的角色、名称从哪来、标识种类、能不能操作
//! - [`text`]：固定文案表 `TextId` → 中文（不带数据的控件名；带数据的句子归 `atg_core::speech`）
//! - [`project`]：界面输入 + [`project::UiState`] → [`project::A11yNode`] 树，并按 `specs/gui/a11y-tree.md`
//!   第四节的 GPUI 层格式渲染成文本
//! - [`schema`]：给 agent 的界面自描述（`gui-schema` 程序输出、金样比对、`GUI_SCHEMA_V` + 内容哈希）
//!
//! 现在只收了 M3.1 骨架用到的那几类元素（窗口、区块、会话列表 / 项、消息列表 / 项、状态栏）；
//! 按键、焦点、播报、视觉令牌几张表随用到它们的块（M3.3 起）再进。

// 与 atg-core 同一行（拍板 005 补丁 ③ `docs/plan/005_decisions-b3.md`，`scripts/check-no-debug.sh` 检查每个 crate 根都带着）
#![cfg_attr(
    not(test),
    deny(
        clippy::use_debug,
        clippy::dbg_macro,
        clippy::unwrap_used,
        clippy::expect_used
    )
)]

pub mod a11y;
pub mod aid;
pub mod project;
pub mod schema;
pub mod text;
