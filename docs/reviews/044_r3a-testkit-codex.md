# 评审 044 · R3a 测试共用夹具 testkit 第一刀（#60 第二段）（codex · gpt-6-astra · read-only · diff 类）

> 对象：新 `src/atg-core/src/testkit.rs`（`temp_dir`、`profile`）、`lib.rs` 的条件 `mod`、两个 `Cargo.toml`（`testkit` 特性与 dev-dependency）、`Cargo.lock`；4 个测试文件改用它（core `tests/config.rs`、`tests/lifecycle.rs`，cli `history/tests.rs`、`session/tests.rs`）。范围是三点 `origin/main...HEAD`。重点写在 `_tmp/20261114-r3a-codex/focus.md`（不入库）。
> **状态：第 1 轮（2026-11-14）· 总评「修改」· P2×2 属实，已改。第 2 轮待审。**
> 之前：多视角自审 1 轮（atg-review），P0/P1 零。P2×6 都是「说的比做的多、前后对不上」，已改：进度写实，已换 4 处、另 11 处各在哪条线手上，010 的 S11 份数改成实查的 15 份；单元测试里要写 `crate::testkit`（自身 dev-dependency 会让测试二进制里出两份类型）；写明怎么核正式构建不带它；`lib.rs` 的注释与模块索引；现状页的设计改成实际情况。机器验收：fmt、clippy 两种、`test --workspace` 全绿；`cargo tree -p atg-cli -e normal` 下 atg-core 不带任何特性；`Cargo.lock` 只多一条 atg-core → atg-core。
> 隐私扫描：第一次干跑被拦 1 条密钥类命中，是现状页里引用的 `ConfirmToken` 类型标注（R1a 那条误报，人已确认过）；现状页那句已改写，扫描干净。只提示的 8 条是测试夹具里编出来的 TDLib JSON。

## 第 1 轮（2026-11-14）· 总评「修改」

原文：`_tmp/20261114-r3a-codex/r3a-codex-1.md`（不入库）。effort medium（`result: P0×0 P1×0 P2×2 P3×0`）。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无（都已改）

- **P2｜src/atg-core/src/testkit.rs:39：删残留失败后仍可能交出非空的旧目录**（第 1 轮 #1）
  - 核实：属实。`let _ = remove_dir_all` 把所有错误都吞了。进程号会被重用，可能撞上上一次留下的同名目录；里面要是有删不掉的子目录，删除失败，接着 `create_dir_all` 对已存在的根目录照样成功，交出去的就是一个还留着旧东西的目录，违反「新的空临时目录」的约定。
  - 处理：清理挪进 `fresh(&Path)`，只放过 `NotFound`，其余删除错误一律报错。补 3 条单元测试：同一个 tag 两次拿到不同的空目录；残留会被清掉；删不掉的残留（子目录权限 000）报错、不被重用。以 root 运行时跳过这一条，因为 root 不受目录权限限制。
- **P2｜src/atg-core/src/testkit.rs:21：说 `atg-test-phone` 是文件路径不用换，依据不实**（第 1 轮 #2）
  - 核实：属实。`tests/config.rs:248` 建的是目录（249 行 `create_dir_all`），250 行才在它下面拼文件路径，而且名字只带进程号。
  - 处理：说明改正，把它登记进还没换的清单（下一刀一起换），本刀的代码范围不扩大。

### 引用抽查

- 「不符」1 条：即上面第 2 条，已改。
- 「核实属实」5 条：条件编译与 lint；atg-cli 只在 dev-dependencies 开特性；自身 dev-dependency 的集成测试没有混用两份类型；lifecycle 仍建名为 `test` 的测试档案；进程号 + 原子计数器的唯一性。
- 无法核实：工作区级的特性传播（根 `Cargo.toml` 与其他成员不在可读范围）。由本地 `cargo tree -p atg-cli -e normal` 实查补上：atg-core 不带特性。

### 是否再审

再审一轮：本轮有改动，要确认连续第 2 轮无 P0/P1。
