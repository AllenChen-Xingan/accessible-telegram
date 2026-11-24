# 评审 066 · #60 R8 小零件归一：lock、时钟与时区、原子写各留一份（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main...HEAD`（第 1 轮 HEAD = a7755f4：检查点 44250a7、自审修正 a7755f4）。
> - `lock<T>` 原来 5 份，收成 `atg_core::lock`（`pub(crate)`）
> - 「现在」与系统时区：新模块 `atg_core::clock`（`now_s`、`now_ms`、`local_offset_at`）。不放 `tree`，因为 `tree` 只放数据（大总管批，守则 4）
>   - `gate::unix_now` 留着名字，只是转调
>   - cli 的 `watch::now_ms` 改成重新导出
>   - cli 的 `out::now_rfc3339_ms` 改从 `clock::now_ms` 取时间
> - `write_atomic` 从 `gate` 挪到 `config`（`pub(crate)`），删掉非原子的 `write_private`；`mark_running` 改成原子写（行为变化，另有两处碰不到的溢出边界变化，见第 1 轮 #1）
> - `docs/plan/010` 状态更新
>
> effort medium。隐私扫描：干净。
> **状态：第 2 轮（2026-11-24）· 总评「修改」· 第 1、2 轮连续无 P0/P1 → 满足停止线，收敛。第 2 轮的 P2×2 都是注释措辞，已当场改掉，不留 issue。**
> 之前：
> - **多视角自审 1 轮**（44250a7）：P0/P1 零，P2×3，P3×12。改了：时钟改放 `clock`、010 不再误用 ✅、可见性收紧、注释写准、导入挪到文件顶部（a7755f4）。不改 1 条：`write_atomic` 失败时删临时文件（会改 gate、send_limits 写盘的行为，测试也靠它；大总管定不开 issue，注释写准即可）
> - **全量检查**（a7755f4）：两种 clippy、`cargo test --workspace`（718 过、0 败、5 忽略）、两遍 `cargo doc -D warnings`、fmt、check-no-debug、check-file-size（超线 0）、check-sources 全过

## 第 1 轮（2026-11-24）· 总评「修改」

原文：MAIN 下 `_tmp/20261124-b21/r8-codex/r8-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×2 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- **P2 #1 属实，改成记录在案**：溢出时的边界值和原来不完全一样，有两处。
  - 原 cli `speech::now` 的秒溢出时取 0，现在取 `i64::MAX`（计划 `plan-r8-arch.md` 里写了，大总管批过）
  - `out::now_rfc3339_ms` 在总毫秒数溢出以后，毫秒位固定为 807
  - 秒要到约 2920 亿年、毫秒要到约 2.9 亿年才溢出，实际碰不到，不为它们加可以注入时间的测试，在 `clock.rs` 的注释里记一笔。PR 里说的「只有 mark_running 改了行为」，补一句「另有两处碰不到的溢出边界」
- **P2 #2 属实，已改注释**：`mark_running` 那段注释说「两种写法读出来一样」，这只在写成功以后成立。
  - 原先没有 `running` 时，写到一半崩溃：原来 `running` 可能已经建好了，下次 `previous_unclean` 为真；现在 `running` 还没出现，下次为假（原先已有 `running` 时，改名前失败不碰它，照旧为真）
  - 作者核过调用顺序：`td/real/start.rs` 里 `mark_running` 在 `create_client` 之前，假后端也在登记运行之前。这时 TDLib 还没开库，什么都没跑，没有「上次没正常关」可提示，所以为假才对
  - 注释已改成分「写成功」「写到一半」两种情况说明，并写明这个调用顺序的依据
- 无法核实 2 条：
  - `gate/check.rs` 经 `use super::*` 取导入的情况：编译结果见上「全量检查」
  - 010 里 serve.rs 的行数：涉及历史版本，以 c48ff54 为准
- 抽查属实 5 条：
  - `lock` 和删掉的 5 份一致
  - gate 父模块里的导入都保留了，闸的判断逻辑没动
  - `is_fresh` 不允许 `running` 或 `running.tmp<pid>`，#97 处理竞态的那段还在
  - `mark_clean` 只删 `running`
  - `clock` 在 1970 年以前取 0；不溢出时，按毫秒整除、取余，和原来的秒数加 `subsec_millis` 一致
- 只改了注释，没重新编译（fmt 检查过）

### 是否再审

本轮无 P0/P1，是连续第 1 轮；再审一轮确认。

## 第 2 轮（2026-11-24）· 总评「修改」

原文：MAIN 下 `_tmp/20261124-b21/r8-codex/r8-codex-2.md`（不入库）。HEAD = 9c36417。`result: P0×0 P1×0 P2×2 P3×0`。
本轮 P0/P1：0 · 停止线：满足（第 1、2 轮连续无 P0/P1）· 留作 issue 的 P2：无（两条已改）

- **P2 #1 属实，已改**：上一轮记溢出边界时说法前后不一，`clock.rs` 写「不算行为变化」，本记录文件头又写「唯一一处行为变化」。统一成「除 `mark_running` 外，另有两处极端溢出边界变化」，`clock.rs` 和本记录文件头都改了
- **P2 #2 属实，已改**：上一轮改注释时，漏掉了「原先没有 `running`」这个前提。原先已有 `running`（上次没正常关）时，改名前失败不会碰它，`previous_unclean` 照旧为真，异常状态保留。`config.rs` 的注释分成两种情况写，本记录第 1 轮 #2 也同步改了
- 无法核实 2 条：`td/real/start.rs` 的启动顺序（作者核过，`mark_running` 在 `create_client` 之前）、010 里 serve.rs 的历史行数
- 抽查属实 6 条：`lock` 一致；gate 父模块里的导入都在、闸逻辑没动；`out.rs` 不溢出时等价；`is_fresh`；`mark_clean`；假后端里 `mark_running` 在登记运行之前
- 改动只在注释和本记录里，没重新编译（fmt 检查过）

### 是否再审

不再审：连续两轮无 P0/P1，收敛。本轮两条 P2 都是注释措辞，已改。
