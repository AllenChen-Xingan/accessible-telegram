# 评审 054 · #60 R7 第六刀：拆 cli `watch.rs`（codex · gpt-6-astra · read-only · diff 类）

> 对象：`1f161b3...HEAD`（检查点 5936af4、ce53d52）。`src/atg-cli/src/watch.rs` 的「出口与订阅者」整节挪进新文件 `watch/hub.rs`；基线；010 表格与路线表；交接页。依据文件：`hub.rs`、原文件（取自 1f161b3）、`watch/end.rs`，第 2 轮另加 `watch/tests.rs`。effort medium。
> **状态：第 2 轮（2026-11-17）· 总评「通过」· 连续两轮无 P0/P1 → 满足停止线，收敛。留作 issue 的 P2：无。**
> 之前：多视角自审 1 轮（atg-review，按提交范围），P0/P1 零。P2 两条已改：路线表 R7 补上第四到六刀和 cli 的两个文件；第 105 行 `result_line` 的位置改成 `watch/hub.rs`。P3 已收尾：解释 `pub(super)` 和 tdlib 特性、行号注明按哪个版本、`hub.rs` 头注释改准并补词汇、`fail_unless_stopped` 的注释补上 `super::`。
> 编译检查（本机）：第一次跑默认那种 clippy 红了一处：`pub use hub::StdoutLines` 在默认构建里没人用（只有开 tdlib 特性的 `session.rs` 用）。改成单独一行加 `#[cfg_attr(not(feature = "tdlib"), allow(unused_imports))]` 后复跑：fmt、两种 clippy、`cargo test --workspace`（618 过、0 败、5 忽略）、check-no-debug 全过。之后只改了注释和文档。

## 第 1 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-r7watch-codex/r7wa-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- 无发现
- 无法核实 1 条：其他私有成员有没有模块外在用（`watch/tests.rs` 不在可读范围）。作者说明：要是有模块外在用，编译就过不去，本机编译和 618 个测试都过了。第 2 轮把 `watch/tests.rs` 加进了依据文件

### 是否再审

再审一轮，确认是否连续第 2 轮无 P0/P1。

## 第 2 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-r7watch-codex/r7wa-codex-2.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：**满足**（连续第 2 轮） · 留作 issue 的 P2：无

- 无发现。无法核实 1 条：测试结果只有作者报告（以 CI 为准）

### 是否再审

不再审，按停止线收敛。
