# 评审 061 · #137 修 22 条 rustdoc 警告，CI 加 cargo doc -D warnings（codex · gpt-6-astra · read-only · diff 类）

> 对象：`2a17a44...HEAD`（检查点 49fe0b8、4cd4804）。本分支起点 2a17a44 在 #163（拆 td.rs）的分支上，这样数链接时看到的是拆完的 td。
> - 22 条警告：
>   - atg-core 16 条：公开文档链到私有项
>   - atg-core 2 条：`td.rs` 里的 `TdBackend`，不开 `tdlib-api` 时断
>   - atg-audio 1 条：链到私有项
>   - atg-audio 3 条：`decode` 既是函数又是模块，同名歧义
> - 修法：私有项和 `TdBackend` 改成反引号；`decode` 加 `()`，仍是链接。只动文档注释
> - `ci.yml` 加两步 `RUSTDOCFLAGS: -D warnings` 的 `cargo doc --no-deps`：一步 `--workspace`，一步 `-p atg-core --features tdlib-api,testkit`
>
> 依据文件：改动前两遍 `cargo doc` 的完整输出、`ci.yml`、`scripts/check-file-size.py`（门槛文件清单）。effort medium。
> **隐私扫描**：密钥类命中 7 条，全是 `ConfirmToken` 类型标注（`token: ConfirmToken,`，没有值），大总管逐条核过（按共同规矩 15），加 `--force-secrets` 放行。位置：`gate/check.rs:119`、`:150`，`send/file.rs:303`、`:348`，`send/react.rs:376`、`:385`、`:429`。
> **状态：第 2 轮（2026-11-17）· 总评「通过」· 连续两轮无 P0/P1 → 满足停止线，收敛。留作 issue 的 P2：无。**
> 之前：
> - **多视角自审 1 轮**（49fe0b8）：P0/P1 零
>   - P2×2：① CI 第二步没开 `testkit`，已改；② atg-cli 的 `tdlib` 特性下的代码 CI 查不到，另开 #164
>   - P3×6，改了 5 条（4cd4804）
> - **全套**（49fe0b8）：两遍 `RUSTDOCFLAGS=-D warnings cargo doc` 零警告，fmt、两种 clippy、`cargo test --workspace`（713 过、0 败、5 忽略）、check-no-debug、check-file-size、check-sources 全过；4cd4804 上重跑了开 `testkit` 的那一遍 doc，零警告

## 第 1 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-137-codex/d137-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- 无发现。codex 核实了这几点：
  - 两份日志的 22 条全对上了修改，没有多改的行
  - `decode()` 指向经根模块重导出的函数
  - `TdBackend` 那两处说明文字的意思没变
  - `--workspace --no-deps` 覆盖了 atg-audio、atg-core、atg-cli；第二步开了 `tdlib-api,testkit`
  - `ci.yml` 不在 `GATE_FILES` 里
- 无法核实 1 条：改后的运行结果只有文字记录，以 CI 为准

### 是否再审

再审一轮，确认是否连续第 2 轮无 P0/P1。

## 第 2 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-137-codex/d137-codex-2.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。隐私扫描同第 1 轮（同样 7 条 `ConfirmToken` 类型标注，按共同规矩 15 放行）。
本轮 P0/P1：0 · 停止线：**满足**（连续第 2 轮，脚本的 stop-line 行同此） · 留作 issue 的 P2：无

- 无发现。codex 复核了上一轮的结论，并补核了这几点：
  - 22 条对应 21 行改动（有一行里有两条）
  - 补丁新增的行与当前文件一致
- 无法核实 1 条：同第 1 轮

### 是否再审

不再审，按停止线收敛。
