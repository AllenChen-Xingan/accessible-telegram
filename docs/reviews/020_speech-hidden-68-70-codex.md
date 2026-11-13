# 评审 020 · #68 置顶 / 投票隐藏内容补按键提示 + #70 链接部分剧透整条遮（codex · gpt-6-astra · read-only）

> 对象：`git diff main`（worktree `atg-wt/fix`，分支 `fix/68-70-speech`），另附未跟踪的 `src/atg-core/src/speech/tests.rs`、`src/atg-core/src/tree/text/tests.rs` 作依据。**最新一轮：第 2 轮（2026-11-13）· 总评「修改」· 已达停止线（连续两轮无 P0/P1）**。
> 累计处理：P0×0、P1×0、P2×3、P3×0 逐条回源码核实，属实 3、驳回 0、无法核实 0；3 条都已修，没有留作 issue 的。

## 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/reviews/speech-hidden-68-70-codex-1.md`（不入库）。effort medium，耗时 42 秒。行号指评审时的版本。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- **P2｜src/atg-core/tests/snapshots.rs:416 新增断言验证不了文字链接「整条遮」**（第 1 轮 #1）
  - 核实：属实。3007 链接文字「地图暗号」，只遮剧透段、读成「地图，隐藏内容」时，完整文字、地址、域名三项断言都过；`hidden_everywhere` 快照也只有 brief，没有 full
  - 处理：`partly_hidden_links_agree_across_outputs` 加一条：brief / detail / full 里不许出现链接文字的任何两个相邻字
- **P2｜src/atg-core/src/speech.rs:695 「隐藏内容，？」去逗号是对最终字符串替换，会改用户原文**（第 1 轮 #2）
  - 核实：属实。原文本来写着「隐藏内容，？」也会被改；投票题目在 full 层经 `attachment` → `spans` 同样受影响
  - 处理：`say_hidden` 改放占位符 `MASK`（与 full 层同一个），收尾时只对「占位符，？」去逗号，再把占位符换成「隐藏内容」；原文里的占位符字符先剔掉（同 `full_span`）。补单测 `question_mark_after_hidden_only_for_masks`（遮住的去逗号、原文不动、展开照读）

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：6 条（含两份挪出的测试文件与原行内测试逐行一致）
- 无法核实：无（codex 未编译；修后 `cargo test -p atg-core` 216 过，clippy、fmt、check-no-debug 过）

### 是否再审

本轮无 P0/P1，只连续 1 轮 → 再审一轮确认。

## 第 2 轮（2026-11-13）· 总评「修改」

原文：`_tmp/reviews/speech-hidden-68-70-codex-2.md`（不入库）。effort medium，耗时 45 秒。本轮另审 `scripts/check-no-debug.sh` 新改动（被 `#[cfg(test)] mod X;` 声明的测试文件不扫，大总管定 A）与 `specs/content-tree.md` 三处同步。
本轮 P0/P1：0 条 · 停止线：本轮与上一轮都无 P0/P1 → **满足** · 留作 issue 的 P2：无（唯一一条已修）

上一轮处理的验证：第 1 轮 #1、#2 codex 核实已修复。

- **P2｜scripts/check-no-debug.sh:145 声明丢了所在作用域，可能误把生产文件当测试文件**（第 2 轮 #1，codex 标「构造绕过」）
  - 核实：属实。内联 `mod worker { #[cfg(test)] mod helper; }` 会被算成同目录 `helper.rs`，若顶层另有生产的 `mod helper;`，那份文件整份免扫。仓内没有这种写法
  - 处理：虽按威胁模型可留 issue，但修法小，直接修：只认文件顶层（代码态花括号层数为 0）的声明，内联模块里的照扫；补自测 `nest/helper.rs`（要报）。变异验证：去掉顶层限制后自测报「nest/helper.rs 该报没报」

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：6 条
- 无法核实：macOS bash 3.2 下能否跑——作者实跑 `/bin/bash`（3.2.57）与 bash 5 都过，exit 0

### 是否再审

连续两轮无 P0/P1 → 停。
