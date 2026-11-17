# 评审 038 · R2a 挪出 `chats.rs`、`notify.rs` 的行内测试（#60 第二段）（codex · gpt-6-astra · read-only · diff 类）

> 对象：`src/atg-cli/src/chats.rs` + `chats/tests.rs`；`src/atg-core/src/notify.rs` + `notify/settings.rs` + `notify/tests.rs` + `notify/tests/{messages,sync,typing}.rs`。范围是三点 `origin/main...HEAD`。口径是只挪不改，公开名字原样导出。重点写在 `_tmp/20261114-r2a-codex/focus.md`（不入库）。
> **第 2 轮（2026-11-14）· 总评「通过」· 已达停止线（第 1、2 轮连续零发现）。本评审结束。** 累计提出 0 条。
> 之前：多视角自审 1 轮（atg-review），P0、P1、P2 都是零，P3×11，改了其中不费事的：`mute.rs` 改名 `settings.rs`（不叫 `effective.rs`，免得模块和函数同名，intra-doc 链接撞名）；生产子模块改用显式导入；测试按主题分成 messages、sync、typing 三份；头注释讲清术语和分工。机器验收：行多重集比对只多出 mod、pub use、use 和注释（import 有几行换了位置）；公开名字 notify 10、chats 9，前后一致；`#[test]` notify 24、chats 13，前后一致；fmt、no-debug、clippy 两种、`test --workspace` 全绿；基线 `--tighten` 15 → 13。
> 隐私扫描：干净。21 条只提示的命中都是测试夹具里编出来的 TDLib JSON（`会话{id}`、`第N条` 之类），不是真数据。

## 第 1 轮（2026-11-14）· 总评「通过」

原文：`_tmp/20261114-r2a-codex/r2a-codex-1.md`（不入库）。effort medium（`result: P0×0 P1×0 P2×0 P3×0`）。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- codex 逐个函数比对：chats 13 条、notify 24 条（messages 13 条、sync 8 条、typing 3 条），除了缩进没有任何变化。`summary`、`typing`、`topic_update`、`in_topic` 这几个助手都跟着用它们的测试走了，没有绑错成别的同名助手；`Effective`、`Layer`、`effective` 的公开路径保留；文档注释还挂在原来的项上。

### 引用抽查

- 「核实属实」7 条，「不符」0 条。
- 无法核实：编译、测试、rustdoc（只读沙箱）。由本地实跑补上。

### 是否再审

再审一轮，确认是否连续第 2 轮无 P0/P1。

## 第 2 轮（2026-11-14）· 总评「通过」

原文：`_tmp/20261114-r2a-codex/r2a-codex-2.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：**满足** · 留作 issue 的 P2：无

- 独立再比对一遍，结论同第 1 轮。「核实属实」7 条，「不符」0 条；无法核实的同第 1 轮（编译、测试，由本地实跑补上）。
