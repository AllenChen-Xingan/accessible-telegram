# 评审 057 · #60 R7 第七刀：拆 core `tdjson.rs`，只挪不改（codex · gpt-6-astra · read-only · diff 类）

> 对象：`384da95...1e34e79`（检查点 05f731d、1e34e79）。`src/atg-core/src/tdjson.rs` 的「消息」整节和两张常量表 `OTHER_SERVICE`、`OTHER_CONTENT` 挪进新文件 `src/atg-core/src/tdjson/message.rs`。依据文件：
> - `message.rs`、`tdjson.rs`
> - 挪动前的原文件
> - `--color-moved` 比对结果
> - `react.rs`、`file.rs`
> - `docs/plan/010`
>
> effort medium。
> **状态：第 1 轮（2026-11-17）· 总评「通过」· 连续第 1 轮无 P0/P1，再审一轮。**
> 之前：
> - **挪动证据**：`git diff --color-moved=plain 384da95...05f731d -- src/` 共挪动 670 行。没被识别为挪动的新增行只有这些：
>   - 根上的 `mod` / `use` / `pub use`，以及一行注释
>   - 新文件的头注释和 `use super::*`
>   - `body`、`ephemeral`、`sender` 三处 `pub(super)`
>
>   删除的只有分节线和那三个函数原来的私有签名。
> - **多视角自审第 1 轮**（05f731d）：P1×2，其实是同一件事——工作副本里的交接页是旧版本，盖掉了 main 上 R7 前六刀、R3a 第二刀、#153 的记录。已改回 main 的全文，再接上本节。P2、P3 也已改：
>   - content-tree 的位置指针
>   - 010 的说明
>   - 根头注释补上文件分工
> - **多视角自审第 2 轮**（1e34e79）：P0/P1 为零。P2×1：010 的 R7 行写着「本 PR」，与全文其他地方的用法冲突。P3×5 是注释和文档的补充。这些都已改完，随本记录一起提交。
> - **全套**：fmt、两种 clippy（`-D warnings`）、`cargo test --workspace`（680 过、0 败、5 忽略）、check-no-debug、check-file-size 全过。基线里只剩 `fake.rs`、`td.rs`。

## 第 1 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-r7tdjson-codex/r7tj-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- 无发现。codex 逐字比对后确认：挪过去的代码除获准的三处可见性调整外完全一致；对外路径都还在（`message`、`formatted`、`classify`、`event`、`forum_topic`、`chat_object`、`failed_info`、`added_reactions`、`reaction_type`）；`react.rs` 的 `super::sender` 经根上那行 `use` 拿得到。
- 无法核实 1 条：680 过只有交接页的文字记录，codex 只读、不跑测试。以 CI 为准。

### 是否再审

再审一轮，确认是否连续第 2 轮无 P0/P1。第 2 轮的对象包括自审第 2 轮的文档修改，要等它和本记录一起提交以后再跑。
