# 评审 032 · #9 表情回应（`react` / `reactions`，合 main 后直发）（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main`（= 分叉点 719d5d1，评审时 main 没前进，两点与三点等价）对工作区的 diff；新文件 `src/atg-core/src/send/react/tests/replace.rs` 未跟踪，作依据附上。重点写在 `_tmp/reviews-032/focus.md`（不入库）。
> **最新一轮：第 4 轮（2026-11-17）· 总评「修改」· 待第 5 轮**（第 2 轮起范围改三点、name 换 `react-9-3dot`，停止线从第 2 轮重数）。
> 累计：提出 8 条（P1×3、P2×5），属实 6 条、部分属实 1 条，交人裁决 1 条（第 2 轮 #4，与第 1 轮 #3 同一条；定 A → #128）。
> 之前：多视角自审 5 轮（atg-review；第 4 轮 P1「普通群信缓存」按人定 A 修；第 5 轮 P0/P1 零）。机器验收：`cargo test -p atg-core -p atg-cli` 486 过 0 败，clippy -D warnings 两种（tdlib-api、`--features atg-cli/tdlib`）0。
> 隐私扫描：密钥类命中 9 条，全是参数类型标注 `token: ConfirmToken`（同 023）；人定 2026-11-16 长期放行这一类，大总管逐条核实后准加 `--force-secrets`。chat-content 只提示 11 条，都是测试夹具里编的 TDLib JSON。

## 第 1 轮（2026-11-16）· 总评「修改」

原文：`_tmp/reviews-032/react-9-codex-1.md`（不入库）。effort medium，耗时 75 秒（`result: P0×0 P1×1 P2×2 P3×0`）。行号指评审时的版本。
本轮 P0/P1：1 条 · 停止线：未满足 · 留作 issue 的 P2：无

- **P1｜src/atg-core/src/send/react.rs:259 交出时不复核回应状态，旧计划能绕过默认拒绝**（第 1 轮 #1）
  - 核实：属实。`react()` 只验凭据、写留痕、交出，没有再判会不会挤掉。先后准备 👍、❤️ 两份不带 `replace` 的计划（当时都没点过），再依次开凭据交出，第二份会挤掉第一份，留痕还记 0。凭据 60 秒只限凭据，不限计划年龄。
  - 处理：`ReactPlan` 存准备时的 `replace`、`synced`；`react()` 验凭据后对没给 `replace` 的加回应按同样条件复核（`recheck`，与准备共用 `would_replace` / `refuse_replace`）。同一后端同一条消息同时只许一个回应在「复核 → 交出 → 等应答」之间（`InFlight`，键 = 闸地址 + 消息），第二个直接报 `reaction_state_unknown`，不排队。补测试 `stale_plan_is_rechecked_before_submit`、`concurrent_react_on_same_message_is_refused`。契约 `actions.md` 的 `react` 行写明「交出前复核」。
- **P2｜specs/contracts/actions.md:152 契约仍要求正式服务器给「AI 拿不到的东西」，与全部直发冲突**（第 1 轮 #2）
  - 核实：属实（#113 删 Touch ID 时的残留，不是本 PR 引入；但在本 PR 改到的文件里）。第 152 行理由段、第 296 行 A4、第 299 行 D1 仍是「一律 Touch ID」的说法；`cli.md` 的 `delete`、`resend` 行与 `args.rs` 的 `resend` 帮助还写「确认后」。
  - 处理：第 4 小节理由段标为「历史，#113 前」并改写结论；A4、D1 在原文前加「已废止：#113 起一律直发」；`cli.md` 两行改为「过闸（直发，不确认）后」，`args.rs` 帮助同改。
- **P2｜src/atg-cli/src/react.rs:298 结尾统计句在 CLI 里拼，没经过树和 speech**（第 1 轮 #3）
  - 核实：部分属实。`architecture-rules.md` 第 3 条确实不许 CLI 拼统计句。存量里 `history` / `links` 的结尾统计也是同样拼法，按 `docs/plan/010_complexity-budget.md` 随 R6 一起改成树节点。
  - 处理：句子挪进 `speech::reaction::page_summary`，CLI 只传数；不另起树节点，和其余命令一起留给 R6。

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：5 条（只有私聊、加密聊天信缓存；交出不复核；`TOKEN_TTL` 60 秒、`ConfirmMode` 只剩两种；转发时闸在执行方；留痕 `reaction` 只写 `message_id`、`may_replace`）
- 无法核实：TDLib `MessagesManager.cpp` 的推送范围（上游源码不在获准阅读范围）。多视角自审第 4、5 轮已回源码核过（`need_poll_dialog_message_reactions`）；测试由本地实跑补上。

### 是否再审

要再审：本轮有 P1，改完开第 2 轮，提示词附本记录。

## 第 2 轮（2026-11-16）· 总评「修改」

范围从两点 `origin/main` 改成三点 `origin/main...HEAD`：第 1 轮后 main 前进到 0538ffd，两点范围会把 main 的新提交显示成本分支的回退（同 023 的教训）；第 1 轮的改动已由大总管提交（eb38f0a），工作区干净。改了范围，脚本要求换 name（`react-9-3dot`），停止线从这一轮重新数。原文：`_tmp/reviews-032/react-9-3dot-codex-1.md`（不入库）。effort medium，95 秒（`result: P0×0 P1×2 P2×3 P3×0`）。
上一轮处理的验证：codex 抽查确认第 1 轮 #2 的三处 Touch ID 说法已标历史 / 废止。
本轮 P0/P1：2 条 · 停止线：未满足（连续第 1 轮也没满足）· 留作 issue 的 P2：#4 → #128

- **P1｜src/atg-core/src/send/react.rs:252 复核沿用准备时的 `synced`**（第 2 轮 #1）
  - 核实：属实。准备时补发完了，交出前断线重连、补发还没完，私聊仍凭旧的 `synced=true` 信缓存。
  - 处理：`recheck` 里「补发完了」= 准备时完了 **且** 现在连接线仍是 `ready`（`wait_net(0)`）。补测试 `reconnect_after_prepare_is_unknown`。
- **P1｜src/atg-core/src/send/react.rs:278 到点放弃等待后解锁，请求可能还在跑**（第 2 轮 #2）
  - 核实：属实。`react_within` 到点丢掉 future，`InFlight` 随之解锁，但 TDLib 那边的请求没撤回（`td.rs` 的等待线程照旧等）；`serve` 接着收同一条消息的下一次请求就放行了。
  - 处理：`InFlight` 交出前置 `armed`，等到应答再清；带着 `armed` 被丢掉的记成「放弃」，本进程里这条消息之后一律报 `reaction_state_unknown`，`replace` 才放行（人已接受可能挤掉）。补测试 `abandoned_submit_keeps_message_locked`。
- **P2｜src/atg-core/src/send/react.rs:247 `replace` 跳过复核，留痕可能记 0**（第 2 轮 #3）
  - 核实：属实。
  - 处理：`replace` 只免拒绝、不免重算；交出前新算的结果拿来留痕（`recheck` 回刷新后的计划）。补测试 `replace_plan_logs_fresh_count`。到点那一行（`submit_within` 记「结果未知」）仍用准备时的数，记在这里。
- **P2｜src/atg-cli/src/react.rs:290 统计没入树**（第 2 轮 #4，即第 1 轮 #3 的后半）
  - 核实：规则属实；第二次提出。
  - 处理：**交人裁决**（大总管转人）→ 大总管定 A（2026-11-16，人可推翻）：留到 R6，开 #128 记下 `reactions` 也要迁。我方理由：`history` / `links` / `chats` 的结尾统计同样在 CLI 组 JSON，`docs/plan/010_complexity-budget.md` 排在 R6 统一设计统计节点；现在单为 `reactions` 起节点会与 R6 分叉。选项 A 留 R6 并开 issue（建议）/ B 现在加节点。
- **P2｜specs/contracts/cli.md:43 `send` 行仍写「再按闸要求确认」**（第 2 轮 #5）
  - 核实：属实（#113 残留）。
  - 处理：改为「过闸直发（不确认）」；全文再搜「确认后 / 按闸要求」已无现行要求。

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：4 条
- 无法核实：无

### 是否再审

要再审：本轮有 P1，改完开第 3 轮。

## 第 3 轮（2026-11-17）· 总评「修改」

范围同第 2 轮（三点，name `react-9-3dot`，检查点 ec2a075）。原文：`_tmp/reviews-032/react-9-3dot-codex-2.md`（不入库）。effort medium，77 秒（`result: P0×0 P1×2 P2×1 P3×0`）。
上一轮处理的验证：codex 抽查确认 `send` 行已改为直发、统计入树已记为留 R6（#128）。
本轮 P0/P1：2 条（#2 按人定降为 P2）· 停止线：未满足 · 留作 issue 的 P2：#2 → #131
⚠️ 同一类（「挤掉原回应」防线的并发 / 时序）第三次出 P1，已报大总管定威胁模型。

- **P1｜src/atg-core/src/send/react.rs:315 「放弃」槽位会被提前清掉**（第 3 轮 #1）
  - 核实：属实。① 槽位只按消息为键，带 `replace` 的后一次收尾 `retain` 掉了同消息全部槽位，包括前一次的「放弃」；② 交出口报连接断了时 `armed` 已先清零，按结果未知报的这次没留成「放弃」。第 2 轮的测试还把错误行为固化了（抽查属实）。
  - 处理：槽位带序号，收尾只动自己那一个；`armed` 等结果确定才清，`submit_failed` 回 `send_outcome_unknown` 的留成「放弃」。改测试 `abandoned_submit_keeps_message_locked`，补 `disconnected_submit_keeps_message_locked`。
- **P1｜src/atg-core/src/send/react.rs:252 查 ready 早于异步读取**（第 3 轮 #2）
  - 核实：属实（时间窗是两次读取之间，毫秒级）。
  - 处理：**降为 P2，不修，开 issue**。人定 A（2026-11-17，大总管转达）：同一进程内只有靠构造时序才出现的竞态（读取期间断线重连、同条消息并发交出等）最高算 P2，开 issue，不进停止线；确定性路径（旧计划、断线后提交、超时记账）照 P1。正修（连接线世代计数）记在 #131。
- **P2｜src/atg-core/src/send/react.rs:395 到点那一行仍记准备时的数**（第 3 轮 #3）
  - 核实：属实。
  - 处理：`react_within` 不再借 `submit_within`，自己管超时：复核后的计划交出前放进共享槽，到点时已交出就按它记「结果未知」（拿不准为 `null`）、报 `send_outcome_unknown`；还没交出报 `network`。到点路径没有测试（假后端的表情回应交出不会卡住），记在这里。

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：6 条
- 无法核实：无

### 是否再审

要再审：#1、#3 改完开第 4 轮。

## 第 4 轮（2026-11-17）· 总评「修改」

范围同上（检查点 461477b）。原文：`_tmp/reviews-032/react-9-3dot-codex-3.md`（不入库）。effort medium，83 秒（`result: P0×0 P1×1 P2×1 P3×0`）。
上一轮处理的验证：codex 抽查确认第 3 轮 #1（按序号收尾、断线留「放弃」、测试已改）与 #3（到点按复核后的计划记账）都已修；#2 已记为降级开 issue，不重复报。
本轮 P0/P1：1 条 · 停止线：未满足 · 留作 issue 的 P2：无

- **P1｜src/atg-core/src/send/react.rs:378 有放弃了的请求时，`--replace` 留痕仍记确定值**（第 4 轮 #1）
  - 核实：属实（确定性路径：A 到点放弃 → B 带 `replace` 放行；私聊、补发完、缓存为空时 B 记 `may_replace:0`，A 却可能随后点上）。
  - 处理：`InFlight` 拿到时记下同条消息上是否已有槽位（`pending`），传给 `recheck`，有就按拿不准算（`replaces_known=false`，交出行、结果行都记 `null`）。补测试 `replace_after_abandoned_logs_null`。
- **P2｜specs/contracts/actions.md:249 「为什么」第 3 条仍以「须人确认」为依据**（第 4 轮 #2）
  - 核实：属实。但这句引的是 `.42cog/intent.md` 第 16 行原文（「发消息等对外动作，无论走哪条路都须人确认」），intent 本身还没随 #113 改。
  - 处理：契约第 3 条改为现行的「过闸」依据，注明 intent 原句与 #113 冲突、待人改；intent 是人的正本，不由我改，已报大总管。

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：7 条
- 无法核实：无

### 是否再审

要再审：本轮有 P1，改完开第 5 轮。
