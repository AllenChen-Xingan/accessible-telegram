# 评审 053 · #60 R7 第五刀：拆 cli `history.rs`（codex · gpt-6-astra · read-only · diff 类）

> 对象：`2309789...HEAD`（检查点 20db939、a852ba3）。`src/atg-cli/src/history.rs` 把 links 一节挪进 `history/links.rs`，把读法与渲染挪进 `history/render.rs`；基线；010 表格；交接页。依据文件：两个新文件、原文件（取自 2309789）、`history/tests.rs`。effort medium。
> **状态：第 2 轮（2026-11-17）· 总评「修改」· 连续两轮无 P0/P1 → 满足停止线，收敛。第 2 轮的 P2 已当场改掉，留作 issue 的 P2：无。**
> 之前：多视角自审 1 轮（atg-review），P0/P1 零。P2 两条：交接页标题过时，已改；计划说「只给子模块用的导入跟着挪走」，改为故意留在根上（`history/tests.rs` 经 `use super::*` 用到 `Format`、`SpeechCtx`，挪走就得改测试），理由写进根头注释和交接页。P3 已收尾。
> 编译检查（本机）：第一次跑默认那种 clippy 红了一处：`pub use render::UNREAD_BELOW` 只有测试用，正式构建报「没用的导入」。改成 `#[cfg(test)] use` 后复跑：fmt、两种 clippy、`cargo test --workspace`（618 过、0 败、5 忽略，快照没变）、check-no-debug 全过。

## 第 1 轮（2026-11-17）· 总评「修改」

原文：`_tmp/20261117-r7history-codex/r7hi-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×1 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- **P2｜docs/plan/010:35：写「pub use 原样导出 8 个公开名字」，实际是 7 个；`render.rs` 写成 130 行，实际 131**（第 1 轮 #1）
  - 核实：属实。`UNREAD_BELOW` 改成只在测试时导入以后，就不算对外导出了
  - 处理：010 改成 7 个公开导出，另写明 `UNREAD_BELOW` 只在测试时导入；行数改成 131；交接页同步（检查点 a852ba3）
- 无法核实 1 条：正式构建里别处还用不用 `UNREAD_BELOW`。作者核过：全仓只有 `history/render.rs` 和 `history/tests.rs` 用它；有别处在用的话，编译和默认那种 clippy 都会报错，本机两者都过了

### 是否再审

再审一轮：本轮有 P2，已改。

## 第 2 轮（2026-11-17）· 总评「修改」

原文：`_tmp/20261117-r7history-codex/r7hi-codex-2.md`（不入库）。`result: P0×0 P1×0 P2×1 P3×0`。
本轮 P0/P1：0 · 停止线：**满足**（连续第 2 轮） · 留作 issue 的 P2：无（已当场改）

- **P2｜state/handoff/arch.md:252、257：交接页计划段还写着 `pub use render::{UNREAD_BELOW, …}` 和「8 个公开名字」**（第 2 轮 #1）
  - 核实：属实，上一轮只改了 010，交接页计划段没跟上
  - 处理：计划段改成定稿写法，并注明为什么和计划不同
- 无法核实 1 条，同上

### 是否再审

不再审：连续两轮无 P0/P1，按停止线收敛。这一轮的 P2 只涉及交接页文字，已改。
