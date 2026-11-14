# 评审 026 · #59 会话租约共用模块（lease.rs）与 Hub / serve / watch 迁移（codex · gpt-6-astra · read-only）

> 对象：`git diff origin/main...HEAD`（`feat/59-lease`）：`src/atg-cli/src/lease.rs`（新）、`watch.rs`、`serve.rs`、`watch/tests/hub.rs`（新）、`serve/tests/lifecycle.rs`、`specs/contracts/config-privacy.md`、`docs/experiments/exp008-local-stt/design.md`、`scripts/file-size-baseline.json`；依据 design.md 第二节第 3 条、config-privacy.md 第一节第 2 小节、arch 2026-11-14 批文（计数内部带锁、方法吃 `&self`、放没要过的不回关但 `debug_assert`）。**最新一轮：第 1 轮（2026-11-14）· 总评「修改」（仅 P2）**。
> 之前：atg-review 自审一轮（三视角），P0/P1 无，P2×4 全改（`fail_last` 前提写死、`lease::open` 两步前提写明、后端层测试、serve 端到端测试），P3 大部分改。
> 累计处理：P2×2 逐条核实，属实 2、驳回 0。

## 第 1 轮（2026-11-14）· 总评「修改」（仅 P2）

原文：`_tmp/20261113-media/reviews/lease-codex-1.md`（不入库）。effort medium，37 秒。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮无 P0/P1

- **P2｜读方断开与后端故障同时到时仍会关会话**（第 1 轮 #1）
  - 核实：属实。直跑 `watch_loop` 迁移后在循环里 `reap` 完立刻 `lease::close`，下一行才判 `Faulted`；改前直跑只在 `Ended::Stopped` 才关，这是本 PR 新引入的。`serve` 循环改前就是这个顺序（既有）
  - 处理：「后端故障时不关」判在 `lease::close` 一处（后端已故障就一个都不关），两处循环都受管；补 `watch/tests/hub.rs` `no_close_after_backend_fault`
- **P2｜serve 端到端测试验不到「先走的不关」**（第 1 轮 #2）
  - 核实：属实。第一个退出后立刻停第二个，两条连接可能在同一次 `reap` 里一起摘掉；最终 (1, 1) 排除不了「先走的就关、最后的没关」
  - 处理：加同步点——反复挂一个不开会话的探针，直到执行方日志报「现有 2 个」（= 第二个 + 探针，即第一个已摘掉），此时断言关闭 0 次；再停第二个，断言恰好一次。复验时抓到探针连挂太紧（执行方还没摘上一个探针就接下一个，数总多一），重试间留 50 毫秒一拍、期限 10 秒，连跑 5 次全过
- 引用抽查：「属实」6 条（open 的前提、开失败 undo、hub 测试计数、失败重试、行数基线、契约与设计）；「不符」1 条（`lease.rs` 「后端故障时调用方不调它」——即 #1，已改）

### 是否再审

再审一轮（连续第 1 轮无 P0/P1），带本轮原文与本记录。
