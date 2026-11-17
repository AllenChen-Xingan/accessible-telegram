# 评审 055 · #60 R3a 第二刀：测试里的临时目录助手换成 testkit（codex · gpt-6-astra · read-only · diff 类）

> 对象：`8a98ab3...HEAD`（检查点 b48f01f、af73c9d）。12 处测试临时目录助手换成 `atg_core::testkit`（core `send/tests.rs`、`send/react/tests.rs`、`send_limits/tests.rs`、`tests/config.rs` 的 `test-phone`；cli `chats`、`login`、`send`、`react`、`pending`、`voice`、`voice/tests/transcribe.rs`、`watch` 的测试）；故意不换的几处加注释；`testkit.rs` 头注释更新进度。依据文件：`testkit.rs`、`config.rs`，第 2 轮另加 `atg-audio/Cargo.toml`。effort medium。
> **状态：第 2 轮（2026-11-17）· 总评「通过」· 连续两轮无 P0/P1 → 满足停止线，收敛。留作 issue 的 P2：无。**
> 之前：
> - 计划里的 core `tests/send_limits.rs` 改了又改回：它要用同一个 tag 再算出同一个路径，模拟下一个进程打开同一个档案；换成 testkit 测试就坏
> - 多视角自审 1 轮（atg-review，按提交范围）：P0/P1/P2 零。P3 已改：`ipc.rs` 不换的理由原来写成「路径会变长」，实际是要 0o700 的目录并断言它；`serve/tests.rs` 的理由写准；testkit 头注释解释 R3a / R3b；`tests/config.rs` 和 cli `chats` 的写法与同类处统一
> 编译检查（本机）：fmt、两种 clippy、`cargo test --workspace`（626 过、0 败、5 忽略；改过的每个测试文件 `#[test]` 数前后相等）、check-no-debug，自审修改后又复跑一次，全过。

## 第 1 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-r3a2-codex/r3a2-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- 无发现
- 无法核实 2 条：
  - atg-audio 不依赖 atg-core：依赖清单不在可读范围，第 2 轮加进来了。`src/atg-audio/Cargo.toml` 的描述就写着「不依赖 atg-core」，依赖表里也没有它
  - 626 过：以 CI 为准

### 是否再审

再审一轮，确认是否连续第 2 轮无 P0/P1。

## 第 2 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-r3a2-codex/r3a2-codex-2.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：**满足**（连续第 2 轮） · 留作 issue 的 P2：无

- 无发现。无法核实 1 条：测试结果只有作者报告（以 CI 为准）

### 是否再审

不再审，按停止线收敛。
