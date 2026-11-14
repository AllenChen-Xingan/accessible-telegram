# 评审 009 · #19 speech / #20 读法规约 / #22 本机接口补审（codex · gpt-6-astra · read-only）

> #32 补审第 3 块，拆三个对象分别跑：
> - A · #19 speech v0：PR #47 合并范围 `e625e4d..0d8ad57`（diff）
> - B · #22 本机接口 serve 与转发：PR #46 合并范围 `0d8ad57..01c86d4`（diff）
> - C · #20 读法规约 v0：`specs/style/`（doc，依据 `docs/plan/007_decisions-b15.md`、`specs/content-tree.md`、`src/atg-core/src/speech.rs`）
>
> **最新一轮**：2026-11-13 · **A、B、C 均已达停止线**——B 两轮无 P0/P1；A、C 的 P1（#68）经 PR #76 修复，修复分支两轮「通过」。剩余 P2：#66、#67、#69。

## A · #19 speech

### 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-b18/codex-work/speech-19-codex-1.md`（不入库）。diff 类型，effort high。行号指当前文件。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：#1–#3 → #66

- **P2｜atg-cli/src/speech.rs:108 `parse_tz` 对非 ASCII 输入会 panic**（第 1 轮 #1）
  - 核实：属实。`--tz '+中1'`：`rest` 为 4 字节，`split_at(2)` 切进汉字内部而 panic，先于数字校验；`cli.md:61` 要求报 `usage`。
  - 处理：转 issue。建议先校验全 ASCII 再切，补退出码 2 的测试。
- **P2｜atg-core/src/speech.rs:547/:618 置顶消息、投票题目里的隐藏内容不给按键提示**（第 1 轮 #2）
  - 核实：属实。置顶取 `content(c, reveal).0`、投票取 `spans(question, reveal).0`，丢掉「有隐藏」标记，整条末尾不出「按 D 展开」；`content-tree.md:256`（V13 人定）要求整条有任一处遮挡就在末尾说一次。投票处的注释写了「不算进整条的按键提示」，属有意偏离但未见拍板依据。
  - 处理：转 issue（若投票那处确属有意，由人确认后补进读法规约）。
- **P2｜atg-core/src/speech.rs:771 键帽表情（`1️⃣`）不能按 V12 合并**（第 1 轮 #3）
  - 核实：属实。`emoji_clusters` 以首字符是否图形符号判断，数字 `1` 不是，于是 `FE0F`、`20E3` 各被当成普通字符，连续键帽不会合并成「3 个 1️⃣」；`:756` 注释称键帽算同一个表情，与实现不符。
  - 处理：转 issue，补带 / 不带变体选择符的回归用例。

引用抽查：核实属实 4、不符 1（即 #3，已计入）。

是否再审：本轮无 P0/P1，连续第 1 轮 → 再跑一轮确认（排在 B、C 之后）。


### 修复分支第 1、2 轮（2026-11-13，PR #76 `fix/68-70-speech`，与 010 合跑；A、C 共用）· 总评「通过」×2

原文：`_tmp/20261113-b18/codex-work/speech-fix-codex-1.md`、`-2.md`（不入库）。对象 `main...HEAD`（worktree `atg-wt/fix`，提交 `397ca79`），名 `speech-fix`，上一轮原文与本记录作依据。effort high。
两轮均「未发现 P0–P2 问题」· 停止线：**满足**（脚本 `stop-line:` 判定）。

- #68（置顶、投票隐藏内容的按键提示）：codex 两轮均判已修——`speech.rs:561` 置顶上传隐藏标记、`:578` 合并投票题目标记，顶层只加一次提示；快照 `hidden_everywhere` 覆盖两类节点与 reveal；规约补 M17b、M17c。手下回源码抽核属实。
- #70（链接部分剧透）：`tree/text.rs:560` `link_hidden` 成为 mask / brief / full / links_marked 唯一判据；测试 `partly_hidden_links_agree_across_outputs`。手下抽核属实。
- 无法核实 1：CLI `links` 最终序列化不在本轮对象内（共用判据与标记测试已核）。

是否再审：**停**。剩余 P2 见 #66、#69、#71。

## B · #22 本机接口

### 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-b18/codex-work/ipc-22-codex-1.md`（不入库）。diff 类型，effort high。行号指当前文件。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：#1–#3 → #67

- **P2｜atg-cli/src/main.rs:222 探锁与直跑之间 serve 抢先拿锁，直跑报 `profile_busy` 而不改走转发**（第 1 轮 #1）
  - 核实：属实（`try_forward` 连不上 → 探锁空 → 直跑；其间 serve 起来拿锁监听，`session::chats` 返回 `ProfileBusy` 直接成结尾）。窗口窄、重跑即可。
  - 处理：转 issue。建议直跑因锁冲突失败时，对可转发命令再试一次连接。
- **P2｜atg-core/src/ipc.rs:316 「请求一行最多 5 秒」只限单次读，不限整帧**（第 1 轮 #2）
  - 核实：属实。`set_read_timeout` 作用于每次底层读，`read_until` 可反复读；对端每 5 秒内送一点就能一直占着单线程执行方。对端只能是本用户进程（`0600` + `getpeereid`），按威胁模型属健壮性。`:35` 注释与实现不符。发起方 90 秒同理。
  - 处理：转 issue。建议用单调时钟记整帧截止时间，每次读只给剩余预算。
- **P2｜atg-core/src/ipc.rs:328 半帧后断开当成空连接；缺 `v` 或类型错误报成版本不符**（第 1 轮 #3）
  - 核实：属实。末尾无换行且未超长 → `Closed`，serve 静默结束，不回 `ipc_bad_request`；`serve.rs:275` 只要 `v` 不等于当前版本（含缺失、非数字）都报 `ipc_version_mismatch`。
  - 处理：转 issue。

引用抽查：核实属实 4、不符 1（即 #2 的注释，已计入）。

是否再审：本轮无 P0/P1，连续第 1 轮 → 再跑一轮确认。

### 第 2 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-b18/codex-work/ipc-22-codex-2.md`（不入库）。带第 1 轮原文与处理记录（`resp-ipc-22-1.md`）。effort high。
本轮 P0/P1：0 · 停止线：**满足**（第 1、2 轮均无 P0/P1）· 留作 issue 的 P2：#67

- 上一轮三条复核：codex 确认均未修、与处理记录一致，「无升级依据」；本轮三条 P2 即上一轮三条重述，无新增。
- 引用抽查：核实属实 5，无不符。

是否再审：**停**。剩余 P2 已在 #67。

## C · #20 读法规约

### 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-b18/codex-work/style-20-codex-1.md`（不入库）。doc 类型，effort medium。
本轮 P0/P1：1（与 A 第 1 轮 #2 同一处）· 停止线：未满足 · 留作 issue 的 P2：#2–#5 → #69

- **P1｜message.md:193 隐藏提示规则没覆盖投票题目、置顶引用；实现漏提示**（第 1 轮 #1）
  - 核实：属实，与 A 第 1 轮 #2 是同一处代码（`speech.rs:547`、`:618` 丢掉隐藏标记）。A 那轮 codex 定 P2，这轮定 P1；置顶那处没有任何「有意」说明，确实违反 V13（人定）→ **按 P1 处理**，#66 第 ② 条随之升 P1。投票那处代码注释写有意不加，待人定（大总管已攒）。
  - 处理：报大总管派修（并入 #66）；规约补这两类节点的正反例。A（#19）的第 2 轮等此条修完再跑。
- **P2｜message.md:640 三份规约的「现状 / 缺口」大量过期**（第 1 轮 #2）
  - 核实：属实。如缺口第 6 条称 `SpeechCtx` 没有展开键，实际 `speech.rs:35` 已有 `expand_key`；置顶内容、`spoiler`+`sensitive` 去重等也已实现。
  - 处理：转 issue（集中更新现状与缺口）。
- **P2｜README.md:46 夹具默认值没写 `reveal`、`expand_key`、`offset_at`**（第 1 轮 #3）
  - 核实：属实。处理：转 issue。
- **P2｜message.md:443 早晨钟点正例写「07:00」，实现读「7:00」**（第 1 轮 #4）
  - 核实：属实（`speech.rs:808` `format!("{h}:{m:02}")`，小时不补零；`service.md:113`「08:00」同）。要定哪边为准。
  - 处理：转 issue。
- **P2｜message.md:197 剧透后接问号的例子与实现不符**（第 1 轮 #5）
  - 核实：属实（按代码推：`span` 遇剧透无条件写「，隐藏内容，」，后接「？」得「隐藏内容，？」；未实跑）。
  - 处理：转 issue。

引用抽查：核实属实 4、不符 3（均已归入 #2）、无法核实 1（`chat-item.md:123` 引 TDLib 源码行号，不在允许读取范围）。

是否再审：有 P1 → 修完后开第 2 轮。


### 修复分支第 1、2 轮（2026-11-13，PR #76 `fix/68-70-speech`，与 010 合跑；A、C 共用）· 总评「通过」×2

原文：`_tmp/20261113-b18/codex-work/speech-fix-codex-1.md`、`-2.md`（不入库）。对象 `main...HEAD`（worktree `atg-wt/fix`，提交 `397ca79`），名 `speech-fix`，上一轮原文与本记录作依据。effort high。
两轮均「未发现 P0–P2 问题」· 停止线：**满足**（脚本 `stop-line:` 判定）。

- #68（置顶、投票隐藏内容的按键提示）：codex 两轮均判已修——`speech.rs:561` 置顶上传隐藏标记、`:578` 合并投票题目标记，顶层只加一次提示；快照 `hidden_everywhere` 覆盖两类节点与 reveal；规约补 M17b、M17c。手下回源码抽核属实。
- #70（链接部分剧透）：`tree/text.rs:560` `link_hidden` 成为 mask / brief / full / links_marked 唯一判据；测试 `partly_hidden_links_agree_across_outputs`。手下抽核属实。
- 无法核实 1：CLI `links` 最终序列化不在本轮对象内（共用判据与标记测试已核）。

是否再审：**停**。剩余 P2 见 #66、#69、#71。
