# 评审 050 · #60 R7 第二刀：拆 `speech.rs`（codex · gpt-6-astra · read-only · diff 类）

> 对象：`266f5f3...HEAD`（检查点 ff44a49）：`src/atg-core/src/speech.rs` 把系统消息挪进新文件 `speech/service.rs`，把全文逐段渲染和正文与附件的读法接进已有的 `speech/content.rs`；`scripts/file-size-baseline.json`；`docs/plan/010` 表格；`specs/content-tree.md` 代码地图。依据文件：`service.rs`、两个改动前的原文件（取自 266f5f3）、`speech/tests.rs`。effort medium。
> **状态：第 2 轮（2026-11-17）· 总评「通过」· 连续两轮无 P0/P1 → 满足停止线，收敛。留作 issue 的 P2：无。**
> 之前：多视角自审 1 轮（atg-review）。P1×1 已改：新写的 `use` 行没按 rustfmt 排，CI 的 fmt 会红。P2×1 已改：全文层（`MASK`、`full_spans`、`full_span`）原来还留在根里，`content.rs` 得反过来从根里引用，所以一起挪进 `content.rs`，仍是纯挪。P3 已收尾：010 表格写准、代码地图补上 `service.rs`、头注释指回模块根、测试专用的导入加了一句说明。
> 编译检查（本机）：fmt、两种 clippy、`cargo test --workspace`（600 过、0 败、5 忽略；读法快照没变）、check-no-debug、cargo doc 全过。

## 第 1 轮（2026-11-17）· 总评「修改」

原文：`_tmp/20261117-r7speech-codex/r7s-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×1 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- **P2｜state/handoff/arch.md:200：交接页的进度写的还是中间版本**（第 1 轮 #1）。写的是 468 / 89 / 361 行，也没提多挪的全文层
  - 核实：属实。那是自审改动之前的数
  - 处理：改成最终版（394 / 93 / 438），写明全文层也挪了、为什么挪
- codex 逐字符核过：挪过去的段落除了 `pub(super)` 前缀，函数体、常量、文档注释、段内空白都一致；`deny(clippy::wildcard_enum_match_arm)` 仍然管得到两个子模块；`content` 函数和 `content` 模块不在同一个命名空间，显式导入和本地定义优先于 glob，调用目标没被换掉；`merge_emoji` 经根里 `cfg(test)` 那行导入，测试照样能用；010 和代码地图与实现对得上
- 无法核实 1 条：600 过、快照不变（codex 只读，以 CI 为准）

### 是否再审

再审一轮：本轮有 P2，已改。

## 第 2 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-r7speech-codex/r7s-codex-2.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：**满足**（连续第 2 轮） · 留作 issue 的 P2：无

- 无发现，上一轮那条已改对
- 无法核实 1 条：运行结果没附日志（同上，以 CI 为准）

### 是否再审

不再审，按停止线收敛。
