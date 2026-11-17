# 评审 058 · #60 R3a 第三刀：剩下的测试临时目录助手换成 testkit（codex · gpt-6-astra · read-only · diff 类）

> 对象：`384da95...HEAD`（检查点 d776ea1、c0e5152）。8 个测试文件里自写的临时目录助手换成 `testkit::temp_dir`：
> - core：`gate/tests.rs`、`delete/tests.rs`、`edit/tests.rs`、`send/forward/tests.rs`
> - cli：`delete/tests.rs`、`edit/tests.rs`、`forwarding/tests.rs`、`tests/cli.rs` 的 speech 那一处
>
> 外加 `testkit.rs` 头注释、010 R3 行、交接页。依据文件：`testkit.rs`、`config.rs`。effort medium。
> **状态：第 2 轮（2026-11-17）· 总评「通过」· 连续两轮无 P0/P1 → 满足停止线，收敛。留作 issue 的 P2：无。**
> 之前：
> - **多视角自审 1 轮**（d776ea1）：P0/P1/P2 为零。P3×8 已改（c0e5152）：
>   - gate 的本地助手补注释，说明为什么留着它
>   - 010 的 R3 行补上第二刀、第三刀
>   - 交接页补上两处行为差别（`Profile::create` 现在拿到的是已建好的空目录；speech 那处每次都是全新的空目录），以及几处说明
> - **全套**：fmt、两种 clippy（`-D warnings`）、`cargo test --workspace`（680 过、0 败、5 忽略）、check-no-debug、check-sources、check-file-size 全过；8 个测试文件的 `#[test]` 个数改前改后一样
> - 故意不换：`tests/cli.rs` 的 `send_usage_paths`，等 #12

## 第 1 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-r3a3-codex/r3a3-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- 无发现。codex 核实了这几点：
  - `Profile::create` 判断的是 `root/profiles/<name>` 在不在，根目录事先建好不影响
  - 没有哪处依赖「同一路径能再算出来」
  - core 单元测试都写 `crate::testkit`，cli 都写 `atg_core::testkit`
  - 删掉的 `AtomicU32`、`Ordering` 在这些文件里已没有别处在用
  - `send_usage_paths` 保留原样
  - 清单、010、交接页与实际改动一致
- 无法核实 1 条：680 过只有文字记录，codex 只读、不跑测试，以 CI 为准

### 是否再审

再审一轮，确认是否连续第 2 轮无 P0/P1。

## 第 2 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-r3a3-codex/r3a3-codex-2.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：**满足**（连续第 2 轮，脚本的 stop-line 行同此） · 留作 issue 的 P2：无

- 无发现。codex 复核了上一轮的结论，并确认本记录提到的修改都已落地。
- 无法核实 1 条：同第 1 轮（测试结果只有文字记录，以 CI 为准）。

### 是否再审

不再审，按停止线收敛。
