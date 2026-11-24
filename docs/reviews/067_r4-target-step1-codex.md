# 评审 067 · #60 R4 第一步：会话参数与 saved 解析收进 history 一份（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main...HEAD`（第 1 轮 HEAD = e354eae：检查点 8c76222、自审修正 e354eae）。
> - cli `history.rs` 新增 `CHAT_USAGE`、`chat_arg`、`resolve_chat`
> - `saved` 改成问后端 `me()`，按查询类 10 秒；`history`、`links`、`voice`（`download`、`play`、`transcribe`）、`watch`（含 `serve` 代跑）改用它
> - `watch::resolve` 里自己那份「确认会话存在」删掉，改调 `history::load_chat`
> - `history::resolve` 留一行转调给 `react`、`reactions`
> - 删掉没人用的 `ME_TIMEOUT`
> - **对外错误码有变化**：`saved` 换不出收藏夹 id 时，原来报 `internal`（`me_unknown`，等 5 秒），现在照 `me()` 的错报：到点 `network`、内核故障 `disconnected`、没登录 `auth_required`。`errors.md` 删掉 `me_unknown`，`cli.md`「最多等多久」与两份修订记录写明
>
> 分两步做，避开 msg2 的 13b；两个 `target.rs`、`ChatRef` 挪进 core、send 那一族、`-n` 常量都在第二步。effort medium。
> 依据（`--context`）：`react.rs`、`serve.rs`、`download.rs`、`play.rs`、`transcribe.rs`、`send.rs`、`deadlines.rs`、core `fake.rs`，让 codex 自己追「哪条命令的错误码变了」。隐私扫描：干净（11 条提示都是「提示」这个词，不是隐私）。
> **focus**（大总管要求）：① 契约和代码对不对得上；② 有没有哪条命令的错误码变了却没写进清单。
> 之前：
> - **多视角自审 1 轮**（8c76222）：P1×2，都是同一件事：`errors.md` 码表里其实登记着 `me_unknown`，子串搜索被 `send_outcome_unknown` 淹掉了，修订记录还写错了。已改。P2×7、P3×5 大多改了（e354eae）
> - **全量检查**（e354eae）：clippy、`cargo test --workspace`（719 过、0 败、5 忽略）、doc、fmt、no-debug、file-size（超线 0）、sources 全过
> - 已知遗留：「到点 network、内核故障 disconnected」还没有测试。假后端的 `me()` 没有「不应答」的开关，和 #175 合在一起，第二步补

## 第 1 轮（2026-11-24）· 总评「修改」

原文：MAIN 下 `_tmp/20261124-b21/r4-codex/r4a-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×1 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- **P2 #1 属实，已改**：现状页「第一步只碰」写的还是批过的旧单子，里面有两个 `target.rs` 和 core 的 `lib.rs`，和实际补丁不符。已改成实际做的（共用助手暂放 `history.rs`）和第二步的完整内容
- focus 两件都核过，没有发现问题：
  - ① `errors.md` 的 `internal` 行已删掉 `me_unknown`；`cli.md` 第 72 行写明 `saved` 按 10 秒；`deadlines.rs` 到点时按内核故障与否报 `disconnected` 或 `network`；假后端没登记 me 时回 401，测试钉死为 `auth_required`
  - ② 受影响清单里有 `reactions`、`serve` 代跑的 `watch`、三个语音命令；send 那一族只作为原有行为来对照，没有误列进受影响清单
- 抽查属实 7 条，另核过：`react`、`reactions` 在转调之后紧接着调 `load_chat`（里面会 drain），没有后续逻辑依赖删掉的那次 drain；`watch::resolve` 改调 `load_chat` 后，和原来逐项一致
- 无法核实 1 条：`download`、`play`、`transcribe` 到 `voice::chat` 的完整调用链（`session.rs` 不在准读范围）。三条入口都经 `voice::query` 和 `session` 走到 `voice::chat`，其中调的就是 `history::resolve_chat`，编译和测试都过了

### 是否再审

本轮无 P0/P1，是连续第 1 轮；再审一轮确认。
