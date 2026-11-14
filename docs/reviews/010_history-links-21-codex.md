# 评审 010 · #21 history / links、剧透遮挡统一补审（codex · gpt-6-astra · read-only）

> 对象：PR #56 合并范围 `df6882b..3a9c0cb`（`atg-cli` history / chats / serve / session，`atg-core` speech / tree::mask / tree::text / json / fake / td）。#32 补审第 4 块。**最新一轮：修复分支第 2 轮（2026-11-13）· 总评「通过」· 已达停止线（P1 #70 经 PR #76 修复；P2 → #71）**。
> 累计处理：P1×2、P2×1 逐条回当前源码核实：属实有效 P1×1、属实降 P2×1、P2 属实 1，驳回 0。

## 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-b18/codex-work/history-21-codex-1.md`（不入库）。diff 类型，effort high。行号指当前文件。
本轮 P0/P1：2（核实后有效 1）· 停止线：未满足 · 留作 issue 的 P2：#2（降级）、#3 → #71

- **P1｜json.rs:32 / speech.rs:700 链接文字里部分是剧透时，读法仍读出可见那段（网址前半段）**（第 1 轮 #1）
  - 核实：属实。`tree::mask`（`mask.rs:108`）对「文字里套着剧透的 `link` / `url`」整个换成空剧透；读法 `span` 对 `Link` / `Url` 照常遍历子节点，只把其中的剧透读成「隐藏内容」。`Url` 只把末段标剧透时，`brief` / `full`（文字输出、转发同源）读出网址前半段，而 `data` 与 `links` 整条不给。违反 `content-tree.md:240` 与拍板 008 H2'「chats、speech 同规则」。现有快照（`snapshots.rs:280`）只测了链接文字全部在剧透里的情形。
  - 处理：报大总管派修。建议读法对 `Link` / `Url` 先用 `text::contains_spoiler` 判，命中即整条读「隐藏内容」（与 `mask` 同一判据），补「链接部分文字为剧透」的跨输出（data / brief / full / links）测试。
- **P1→P2｜history.rs:287 已读位置之后整页都是自己发的，落点停在自己的消息上**（第 1 轮 #2）
  - 核实：属实。`skip == ids.len()` 时既不删也不往后补，返回自己发的那条且 `landing:"first_unread"`。触发要求未读 > 0 而已读位置之后先有满满一页自己发的消息（发消息通常会把已读推到自己那条），默认页长下罕见；`-n 1` 可稳定复现。降为 P2。
  - 处理：转 issue，补边界测试。
- **P2｜history.rs:272 反方向「还有更多」未验证**（第 1 轮 #3）
  - 核实：属实（`--before` 非空即 `more_newer:true`，`--after` 直接 `more:true`；游标那条被删后可能给出通向空页的游标）。
  - 处理：转 issue。

### 引用抽查

- codex 列的「核实属实」：4 条（mask.rs:109、speech.rs:700、history.rs:849 测试页长、snapshots.rs:280）；无「不符」。

### 是否再审

有有效 P1 → 修完后开第 2 轮，提示词附本记录。


### 修复分支第 1、2 轮（2026-11-13，与 009 A/C 合跑，PR #76 `fix/68-70-speech`）· 总评「通过」×2

原文：`_tmp/20261113-b18/codex-work/speech-fix-codex-1.md`、`-2.md`（不入库）。对象 `main...HEAD`（worktree `atg-wt/fix`，提交 `397ca79`），名 `speech-fix`，上一轮原文与本记录作依据。effort high。
两轮均「未发现 P0–P2 问题」· 停止线：**满足**（脚本 `stop-line:` 判定）。

- #68（置顶、投票隐藏内容的按键提示）：codex 两轮均判已修——`speech.rs:561` 置顶上传隐藏标记、`:578` 合并投票题目标记，顶层只加一次提示；快照 `hidden_everywhere` 覆盖两类节点与 reveal；规约补 M17b、M17c。手下回源码抽核属实。
- #70（链接部分剧透）：`tree/text.rs:560` `link_hidden` 成为 mask / brief / full / links_marked 唯一判据；测试 `partly_hidden_links_agree_across_outputs`。手下抽核属实。
- 无法核实 1：CLI `links` 最终序列化不在本轮对象内（共用判据与标记测试已核）。

是否再审：**停**。剩余 P2 见 #66、#69、#71。
