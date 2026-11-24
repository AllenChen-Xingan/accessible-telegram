# 评审 070 · #60 R4 第二步 2a：其余调用方接上 chat_arg / resolve_chat（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main...HEAD`（第 1 轮 HEAD = d5abe39：检查点 35229f2、合 main f088145、自审修正 d5abe39）。
> - cli 的 `edit`、`pending`、`delete`、`forwarding`、`send_file`、`vote`，以及 `edit/exp_real`、`pending/exp_real`：改用 `history::chat_arg` 与 `history::resolve_chat`（`saved` 问 `me()`），不再经过 `send::resolve`、`send::chat_ref`
> - `vote.rs` 删掉本地的 `chat_ref`，原来经 `history::resolve` 转调，现在直接调 `resolve_chat`
> - `delete` 的另一句用法提示保留
> - **承诺行为不变**：`send::resolve` 本来就问 `me()`，期限相同
>
> **状态：第 2 轮（2026-11-24）· 总评「通过」· 第 1、2 轮连续零发现 → 满足停止线，收敛。留作 issue 的 P2：无。**
> 分两刀，避开 msg2 的 13b-2：`send.rs`、`react.rs`、两个 `target.rs`、`ChatRef` 挪进 core、`-n` 常量都在第二刀（2b）。effort medium。
> 依据（`--context`）：`send.rs`、`react.rs`，codex 用它们比对新旧两份实现。隐私扫描：干净。
> 之前：
> - **多视角自审 1 轮**（35229f2 + 合 main）：**三个视角里只回了两个**（第三个没交结果，不算它审过）；回了的两个只报 P3，共 6 条。改了 5 条（d5abe39）：调用写法统一、注释不再手抄调用方清单、批次代号补上出处、交接页记上实测结果。不改 1 条：`send.rs` 里两个函数的注释，因为 `send.rs` 正在 msg2 那边改，留到 2b
> - **全量检查**（35229f2，名额内跑）：fmt、两种 clippy、`cargo test --workspace`（770 过、0 败、5 忽略）、两遍 `cargo doc -D warnings`、check-no-debug、check-file-size（超线 0）、check-sources 都过；外加 `cargo clippy -p atg-cli --features tdlib --all-targets` 零警告，两个 `exp_real` 都编到了
> - d5abe39 上重新编译：clippy、test（772 过，多出的 2 条是合进来的 main 带的）、doc 都过

## 第 1 轮（2026-11-24）· 总评「通过」

原文：MAIN 下 `_tmp/20261124-b21/r4b-codex/r4b-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- 无发现
- 抽查属实 5 条：
  - 新旧两份解析的实现一致：期限、错误传播都一样
  - `CHAT_USAGE` 和原来的提示逐字相同
  - `vote` 之后紧接着就是 `load_chat`，没有丢掉什么副作用
  - `forwarding` 的解析和报错顺序不变
  - `delete` 的例外照旧
- 无法核实 2 条：
  - **全仓只剩计划内的调用方**：codex 的可读范围有限，作者用 `git grep -nw` 在 `src/` 下核过。`send::resolve`、`send::chat_ref`、`history::resolve`、`ChatRef::parse` 剩下的用法只在下面几处：
    - `send.rs`、`react.rs`（2b）
    - `delete.rs`、`watch.rs`（两处的提示本来就不同）
    - `history.rs` 自己（`chat_arg` 内部）和 `history/tests.rs`
  - 测试与 tdlib 特性 clippy 的结果：见上文「全量检查」

### 是否再审

本轮无 P0/P1，是连续第 1 轮；再审一轮确认。

## 第 2 轮（2026-11-24）· 总评「通过」

原文：MAIN 下 `_tmp/20261124-b21/r4b-codex/r4b-codex-2.md`（不入库）。HEAD 仍是 d5abe39：第 1 轮没有要改的，只多了这份记录。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：满足（第 1、2 轮连续无 P0/P1）· 留作 issue 的 P2：无

- 无发现。上一轮的 5 条「属实」又复核了一遍，结论不变；另外核了两个 `exp_real` 都已改用 `history::resolve_chat`
- 无法核实 2 条，同第 1 轮：全仓剩下哪些调用方（作者 grep 的结论见第 1 轮）、测试结果（见文件头）

### 是否再审

不再审：连续两轮无 P0/P1，收敛。
