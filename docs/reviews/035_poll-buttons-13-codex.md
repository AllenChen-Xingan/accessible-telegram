# 评审 035 · #13 只读那半：投票读全、消息上的按钮读出（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main...HEAD`（三点；分支已合 main 27038c9）。重点写在 `_tmp/reviews-035/focus.md`（不入库）。
> **最新一轮：第 2 轮（2026-11-24）· 总评「通过」· 零发现**。第 1、2 轮连续无 P0/P1，满足停止线，收敛；剩余 P2：0 条。
> 之前：多视角自审 7 轮（atg-review），P1 出在第 1、2、4、5、6、7 轮（看不到结果读 0 票、没按 option_order 排、名称违背第九节、「投了才看得到」假许诺、测验能否改票看 revotable、updatePoll 只给机器人、投票类型改开放集合、「满一天」改说投不了），都已改；第 6、7 轮无 P0/P1。机器验收（合 main 0c81363 后）：`cargo test --workspace` 717 过 0 败（atg-core + atg-cli 684），clippy -D warnings 两种（不带 tdlib 全工作区、`--features atg-cli/tdlib --all-targets`）0，fmt、check-no-debug、check-file-size、check-sources 过。
> 隐私扫描：密钥类命中 2 条，是同一行 `src/atg-core/src/tdjson/interactive.rs:148`（TDLib 类型名 `inlineKeyboardButtonTypeCallbackWithPassword` 映射到 `ButtonAction::CallbackWithPassword`，扫描器把 `Password` + `=>` 当密钥赋值）与它在 diff 里的重复；不在 ConfirmToken 长期放行内，**大总管逐条核过（2026-11-24）准加 `--force-secrets`，只限这一处**。聊天内容类只提示：全在 `fixtures/speech/*.json` 与 `tests/lifecycle.rs`，是编造的 TDLib JSON（名字取自 `fixtures/names.txt`）。
> 不动闸、不往外发；不属一级评审。

## 第 1 轮（2026-11-24）· 总评「修改」

原文：`_tmp/reviews-035/poll-buttons-13-codex-1.md`（不入库）。effort medium，43 秒（`result: P0×0 P1×0 P2×2 P3×0`）。行号指评审时的版本。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮 · 留作 issue 的 P2：0 条

- **P2｜src/atg-core/src/speech/interactive.rs:46 投票全文误用名称层的 `spans()`，连续相同表情被合并**（第 1 轮 #2）
  - 核实：属实。`spans()` 末尾调 `merge_emoji()`（`content.rs:346`），按 V12 人定只改名称（`content.rs:400` 注释）；正文全文走 `full_spans()`，投票的题目、选项、解析却走了 `spans()`，「👍👍👍」读成「3 个👍」。
  - 处理：三处改用 `full_spans()`（改为 `pub(super)`）。先红后绿：`snapshots::poll_full_keeps_repeated_emoji`（全文保留原样、名称照旧合并）。
- **驳回｜src/atg-core/src/speech/interactive.rs:183 复制文字与用户按钮只读动作名称**（第 1 轮 #1）
  - 核实：依据不成立。codex 引的「网址 / 复制文字 / 用户只读出」（`specs/contracts/actions.md:402`，#13 人定）说的是 13b **按下**这几种按钮时只读出、不执行，不是要求读法念出复制的内容或用户编号；用户编号是数字，念出来对人无用。
  - 处理：不改代码。「复制按钮要不要连复制的内容一起读」属读法草稿，放进 PR 遗留「待人实听」（大总管认可，2026-11-24）。

### 引用抽查

- codex 列的「核实属实」：5 条（`poll` 的 `can_see_results` / `allows_revoting` / `option_order`、选项 `index` 排序前取原位置、表情合并只改名称、投票人测试、临时内容与受限消息不带按钮）
- 不符：0 条 · 无法核实：0 条

### 是否再审

要再审：#2 改完开第 2 轮（`--previous` 带本轮原文、`--response` 带本记录）。

## 第 2 轮（2026-11-24）· 总评「通过」

原文：`_tmp/reviews-035/poll-buttons-13-codex-2.md`（不入库）。范围 `origin/main...HEAD`（含检查点 73e182d），`--previous` 带第 1 轮原文、`--response` 带本记录。effort medium，37 秒（`result: P0×0 P1×0 P2×0 P3×0`）。隐私扫描同第 1 轮，只有那 2 条（大总管准的同一处）。
本轮 P0/P1：0 条 · 停止线：**满足**（第 1、2 轮连续无 P0/P1）· 留作 issue 的 P2：0 条

- 未发现 P0–P2。codex 确认第 1 轮 #2 已改到题目、选项、解析三处，新测试盖到题目与选项、名称照旧合并；第 1 轮 #1 无新证据，不重复报。

### 引用抽查

- 核实属实：6 条（`full_spans` 三处、`poll` 字段与 0 起编号、排序保留 `index` 与正确答案、遮挡题目解析选项、临时内容与受限不带按钮、投票人测试与回调不收 `data`）
- 不符：0 条 · 无法核实：1 条（本记录写的测试数与 clippy 结果是作者运行记录，codex 只读不跑）

### 是否再审

不再审：收敛。修后验证（73e182d 前，名额内）：`cargo test -p atg-core` 430 过 0 败，两种 clippy -D warnings 0，fmt、check-no-debug、check-file-size 过。
