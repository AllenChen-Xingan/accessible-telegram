# 评审 032 · #9 表情回应（`react` / `reactions`，合 main 后直发）（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main`（= 分叉点 719d5d1，评审时 main 没前进，两点与三点等价）对工作区的 diff；新文件 `src/atg-core/src/send/react/tests/replace.rs` 未跟踪，作依据附上。重点写在 `_tmp/reviews-032/focus.md`（不入库）。
> **最新一轮：第 1 轮（2026-11-16）· 总评「修改」· 待第 2 轮**。
> 累计：提出 3 条（P1×1、P2×2），属实 2 条、部分属实 1 条，驳回 0。
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
