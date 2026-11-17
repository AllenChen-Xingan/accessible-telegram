# 评审 012 · #53 watch 实时收消息与提醒（notify v0）补审（codex · gpt-6-astra · read-only）

> 对象：PR #58 合并范围 `75c6ea8..9d2e459`（`atg-cli` watch / serve / forward / signals，`atg-core` notify / model / store / tdjson / link / ipc）。#32 补审第 6 块；另按大总管要求专门核 #65（serve 单线程阻塞写 watch 订阅者）。**最新一轮：修复后第 3 轮（2026-11-13，分支 fix/63-bounded-waits）· 总评「通过」· 已达停止线（P1 #73 已修；P2 → #65、#74、#99）**。
> 累计处理：P1×3、P2×1 逐条回当前源码核实：属实有效 P1×1、属实降 P2×2、P2 属实 1，驳回 0。

## 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-b18/codex-work/watch-53-codex-1.md`（不入库）。diff 类型，effort high。行号指当前文件。
本轮 P0/P1：3（核实后有效 1）· 停止线：未满足 · 留作 issue 的 P2：#1、#2（降级）→ #65，#4 → #74

- **P1｜watch.rs:843 `fetch_topics` 无期限地 `block_on(forum_topic_get)`，卡住整个消费循环**（第 1 轮 #3）
  - 核实：属实。`link::guard` 只在内核故障 / 关闭时失败；收到新论坛话题的消息后紧接着断网，`getForumTopic` 等网络恢复才应答。期间直跑 `watch` 不处理 Ctrl+C（违反 `cli.md:128`），serve 的其他请求、所有订阅者一起停住，时长取决于断网多久——比 #65 的 60 秒更糟。与 #63（login 请求无超时）同一类。
  - 处理：报大总管派修。建议 `link::block_on_timeout` 加短期限（取不到只记错误码，现有分支已有），或改成异步待办；期间照常检查停止标志。
- **P1→P2｜serve.rs:400 #65：单线程执行方阻塞写订阅者**（第 1 轮 #1）
  - 核实：调用链属实：`serve` 主循环 → `Feed::drain` → `Hub::on_event`（`watch.rs:330`）→ `ConnOut::line`（`serve.rs:400`）→ `Conn::send` → `write_all`，套接字阻塞、写超时 60 秒（`ipc.rs:37`、`:175`），全在执行方主线程。
  - 但对 #65 原题「`watch | less` 停住」：发起方（`forward.rs:135`）有独立收帧线程，把帧读进**无界** channel，套接字不会写满，**执行方不会被卡**；被卡的是发起方主线程（堵在 stdout），代价是发起方内存随积压增长（见下条）。执行方真正被拖住的情形是发起方整个进程被挂起（Ctrl+Z / SIGSTOP）或收帧线程停读：每个这样的订阅者拖一次最多 60 秒后被摘掉，最多 8 个订阅者 → 最坏约 8 分钟；读得慢但一直在读的对端可使每帧都在 60 秒内写完，反复拖延，`config-privacy.md:49`「最多拖住执行方 60 秒」措辞不准。
  - 「一个读得慢的订阅者最多拖住执行方 60 秒，别的订阅者和请求跟着等」是契约明文写的 v0 取舍（大总管授权 2026-10-26），故降为 P2；修法（每订阅者有界发送队列 + 写线程、整帧截止时间）留 #65。
- **P1→P2｜forward.rs:135 `watch | less` 停读时发起方无界积压**（第 1 轮 #2）
  - 核实：属实（`std::sync::mpsc::channel()` 无界；主线程堵在 stdout 时到不了 `:185` 的停止检查，首次 Ctrl+C 无效，`less` 退出后才因写失败退出）。积压速度等于消息到达速度，正常账号下增长慢，不会很快耗尽内存，降为 P2，并入 #65。
- **P2｜watch.rs:207 「正在输入」「重连汇总」提醒直接拼中文，不走内容树**（第 1 轮 #4）
  - 核实：属实（`TYPING_ZH`、`missed_zh` 直接写进 `speech.brief`）；违反 `content-tree.md:21`「新消息提醒、连接状态……都先变成一棵树」及 #60 架构约束。
  - 处理：转 issue，建议抄送 arch 线。

### 引用抽查

- codex 列的「核实属实」：4 条（ipc.rs:37/175、watch.rs:330、td.rs:487 只 openChat、watch.rs:217 走 json::value）；「不符」1 条（`config-privacy.md:49`「最多 60 秒」，已计入 #1）。

### 是否再审

有有效 P1 → 修完后开第 2 轮，提示词附本记录。

### 修复后第 1–3 轮（2026-11-13，与 008 的 #63 部分合跑，审分支 `fix/63-bounded-waits` 的 `main...HEAD`）· 「修改」→「通过」×2

原文：`_tmp/20261113-b18/codex-work/fix-63-codex-1.md`–`-3.md`（不入库）。名 `fix-63`，详细条目与处理见 008 同名一节。停止线：**满足**（第 2、3 轮无 P0/P1）。
- #73（第 1 轮 P1）：已修。`watch/topics.rs:16` 每个论坛话题最多等 5 秒且片间看停止标志，serve 共用此实现；测试 `hung_forum_topic_gives_up_after_deadline`、`stop_while_forum_topic_hangs_returns_at_once`。手下抽核属实。
- 同类 #78：`session.rs:390` 起后端 30 秒、`serve.rs:375` `chat_open` 10 秒；codex 所查 CLI 请求调用点未见遗留裸 `block_on`（`td.rs:177` 发送交出线程有意无期限，不占 serve 主线程）。#79：`spoiler_hidden_unless_reveal` 改用不与时间戳相撞的文字，已修；`watch/tests.rs:408` 仍有一处 100 毫秒旧延时（搬移，不新报）。
- 本块相关的新 P2：`serve.rs:375` `chat_open` 迟到成功漏配 `chat_close`（见 008）→ #99。

是否再审：**停**。剩余 P2 见 #65、#74、#99。
