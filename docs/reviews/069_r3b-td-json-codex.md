# 评审 069 · #60 R3b 第一刀：造 TDLib JSON 的测试助手收进 testkit::td_json（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main...HEAD`，HEAD = 9cbaf79，含检查点 e0de0a2、自审修正 9cbaf79。**只动测试**。
> - 新模块 core `testkit/td_json.rs`：`user`、`new_chat`、`new_private_chat`、`new_private_chat_notify_default`、`text`、`message`、`with`，带两条自测
> - 7 个测试文件改用它：core `send/file`、`send/forward`、`send/tests`，cli `forwarding`、`send_file`、`history`、`watch`。测试体和测试数据都不变
> - `testkit.rs` 加 `pub mod td_json;`，头注释和 `Cargo.toml` 的特性说明跟着改
>
> **状态：第 2 轮（2026-11-24）· 总评「通过」· 第 1、2 轮连续零发现 → 满足停止线，收敛。留作 issue 的 P2：无。**
> effort medium。依据：`fixtures/names.txt`、core `Cargo.toml`。
> 隐私扫描：干净。32 条提示是测试里的假号码、TDLib 消息 JSON 形状，按规矩在测试路径里只提示，不拦。
> 之前：
> - **多视角自审 1 轮**（e0de0a2）：P0/P1 零，P2×3，P3×8。改了：模块从 `td` 改名为 `td_json`（避免和 `crate::td` 撞名）、写准几处文档与用途说明、别名加注释、自测改用 `names.txt` 里的名字、现状页写准（9cbaf79）
> - **全量检查**（9cbaf79）：clippy、`cargo test --workspace`（746 过、0 败、5 忽略）、两遍 `cargo doc -D warnings`、fmt、check-no-debug、check-file-size（超线 0）、check-sources 都过。第一遍开 testkit 特性的 doc 有一条断链：给 `pub mod` 写的 `///` 和子文件的 `//!` 合并了，已改成 `//`

## 第 1 轮（2026-11-24）· 总评「通过」

原文：MAIN 下 `_tmp/20261124-b21/r3b-codex/r3b-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- 无发现
- 抽查属实 7 条：
  - 两种通知设置都保留，没有统一
  - `send/tests` 的 `msg` 经 `with` 加 `topic_id`（含显式 null），其余字段逐字段一致
  - `forwarding` 的 `chat_id`、`sender` 仍是 `FRIEND`，日期不变
  - `with` 的覆盖与「不是对象就忽略」和原来一致
  - `history` 只换了 `user`、`text`，场景专有的 `msg` 保留，测试体没改
  - testkit 只在 dev 依赖里开；core 写 `crate::testkit`、cli 写 `atg_core::testkit`
  - 自测名字在 `names.txt` 第 3 行；新模块没有 `unwrap`、`expect`、`{:?}`
- 无法核实 1 条：746 过的运行结果没有日志可读，codex 这一轮没编译，以上面「全量检查」为准

### 是否再审

本轮无 P0/P1，是连续第 1 轮；再审一轮确认。

## 第 2 轮（2026-11-24）· 总评「通过」

原文：MAIN 下 `_tmp/20261124-b21/r3b-codex/r3b-codex-2.md`（不入库）。HEAD 仍是 9cbaf79：第 1 轮没有要改的，本记录是唯一的新文件。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：满足（第 1、2 轮连续无 P0/P1）· 留作 issue 的 P2：无

- 无发现。codex 复核了上一轮的结论，「字段等价、调用路径对、测试体没改」仍然成立；自审时说改了的几处（改名、别名注释、假名字、模块导出）也都落实了
- 抽查属实 8 条（同第 1 轮，另加 `history` / `watch` 换入的 `user`、`text` 逐字段一致，以及 `testkit.rs` 导出处用普通注释）
- 无法核实 2 条：
  - cli 的依赖声明、库里的 cfg 门控不在可读范围内，没法独立确认整张依赖图里 testkit 只在测试时打开。check-sources 的 testkit 规则已经全过
  - 746 过的运行结果，同第 1 轮

### 是否再审

不再审：连续两轮无 P0/P1，收敛。
