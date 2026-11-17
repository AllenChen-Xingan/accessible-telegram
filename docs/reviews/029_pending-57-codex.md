# 评审 029 · #57 待发消息列表与按会话撤销（atg-cli pending / pending cancel、atg-core delete）（codex · gpt-6-astra · read-only）

> 对象：diff `main...HEAD`（分支 `feat/57-pending`，提交 `373decb`，WIP）；依据 `specs/contracts/actions.md`、`cli.md`。**最新一轮：第 2 轮（2026-11-14）· 总评「修改」（仅 P2）· 连续第 1 轮无 P0/P1，待第 3 轮确认**。
> 之前：atg-review 自审 4 轮，每轮各有 P1、均已修；「撤销之后报下落」这一处同类 P1 第三次出现时按共同规矩第 12 条停下报大总管，**人定 A（2026-11-14）：就此收住**，不再自审。大总管转达口径：此后 P1 若又落在撤销下落时序上，降级记 P2、写明理由。
> 第 1 轮由大总管代跑（隐私扫描把 6 处 `token: ConfirmToken` 类型标注认成密钥赋值，按共同规矩第 13 条经人确认后加 `--force-secrets`）。
> 累计处理：P1×3、P2×3 逐条回源码核实，属实 6、驳回 0；P2×1 开 issue #109。

## 第 2 轮（2026-11-14）· 总评「修改」（仅 P2）

原文：`_tmp/codex-029/pending-57-codex-2.md`（不入库）。对象 `main...HEAD`（提交 `7702a6e`，含第 1 轮修复）。大总管代跑。
上一轮处理的验证：#1 漏待发与截断、#3 附件正文已修（codex 确认）；#2 正式消息缺失分支已收紧，但另有遗漏（本轮 #1）；#4 已延期到 #109，不再列。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮无 P0/P1 · 留作 issue 的 P2：无（两条都已修）

- **P2｜`pending.rs:346` 清缓存的删除仍可能被报成撤销成功**（第 2 轮 #1）
  - 核实：属实。`send::verdict` 不看 `is_permanent` / `from_cache`，任何删除都成 `PendingDeleted`，CLI 转成 `cancelled`；`fate` 把没换 id、不在了的报 `deleted`，而 store 清缓存也会拿掉活消息（`store/mod.rs` 删除处）
  - 处理：CLI 等下落时另记非永久删除（`note_soft`：`!is_permanent || from_cache`），这类报 `unknown`；`resolved` 里「不在了」改名 `gone`（被删或清缓存，分不清）；`send::verdict` 不动（对 `send` 来说清缓存与超时同归「结果未知」，结论不变）；单测补两种标志；契约 cli / actions 同步
  - 属撤销下落一处，codex 已按口径列 P2；修法便宜，照修
- **P2｜`store/mod.rs:620` 待发消息仍占普通配额，导致过量裁剪**（第 2 轮 #2）
  - 核实：属实。`excess` 按全部活消息数算、却只从非待发里删，60 条待发会把之后的普通消息立刻挤掉
  - 处理：`prune` 的超额改按能删的条数算（`prunable(…, None)`，只排除待发）；单测 `pending_do_not_eat_the_quota`（60 条待发 + 10 条普通，普通全留）；`store/mod.rs` 行数不增（`current_id` / `is_restricted` 两个查询迁到 `store/query.rs`）
- 引用抽查：「属实」4 条；「不符」1 条——`store/query.rs:39` 注释仍说待发可能被删旧，已改；「无法核实」2 条（同第 1 轮）

### 是否再审

本轮无 P0/P1，只连续 1 轮，再审一轮确认（第 3 轮脚本交大总管代跑）。

## 第 1 轮（2026-11-14）· 总评「修改」

原文：`_tmp/codex-029/pending-57-codex-1.md`（不入库）。effort medium。行号指评审时的版本。
本轮 P0/P1：3 条 · 停止线：未满足 · 留作 issue 的 P2：#4

- **P1｜`store/query.rs:42` 待发清单会漏条，撤销仍可能报全部成功**（第 1 轮 #1）
  - 核实：属实。`store/mod.rs` `prune` 只排除会话的最后一条，非当前会话超过 `keep_per_chat`（默认 50）条时会删掉待发消息；`pending.rs` 又对超过 100 条的静默 `take(100)`，之后报 `all_cancelled`
  - 处理：`store/table.rs` 加 `prunable`，删旧时跳过待发的（`prune` 那一行换成调用它，行数不变）；`StoreConfig` 文档补「待发的不删」；CLI 超过 100 条不截断，整批拒 `invalid_input(why:"too_many")`；单测 `tests/pending.rs` `pending_survive_pruning`（60 条待发一条不丢）；契约 cli / errors 同步
  - 这条不是「撤销下落时序」，不降级
- **P1｜`pending.rs:341` 没有删除事件也可能报 `sent_deleted_here`**（第 1 轮 #2）
  - 核实：属实。原判法是「store 里不是活消息」就当删过；被删旧（`prune` → `take`）的正式消息也会落进这一支；`fate` 同理把缺失判成 `deleted`
  - 处理：`cancel_outcome` 只认墓碑（删除事件 `is_permanent && !from_cache` 才留）为 `sent_deleted_here`，store 里干脆没有的报 `unknown`；`fate` 对换过 id、正式那条不在的不报（待发的已不参与删旧，没换 id 就不在 = 真被删了）；单测补「store 里没有 → unknown」；契约 cli 的 `sent_deleted_here` / `unknown` 说明同步
  - 属撤销下落一处，但修法只是收紧判据、不再推时序，成本低，照修，不按口径降级
- **P1｜`delete.rs:188` 非纯文字消息的正文没绑进凭据，复查也看不出变化**（第 1 轮 #3）
  - 核实：属实。`confirmable_text` 对带附件消息回 `None`，摘要只绑了 `null`，说明文字变了复查看不出
  - 处理：准备与复查（`load_pending`）遇到带附件的待发消息整批拒 `invalid_input(why:"not_text")`（同 `resend` 不让人确认没看到的内容的取向）；`error.rs` 加 `NotText` 与说明；单测 `delete/tests.rs` `non_text_pending_is_refused`；契约 actions / errors / cli 同步
- **P2｜`pending.rs:265` 撤销已交出但应答超时，被报成普通 `network` / `disconnected`**（第 1 轮 #4）
  - 核实：属实。`bounded(…, delete::cancel(…), QUERY_TIMEOUT)` 到点返回查询错误，真后端的删除在独立线程里照样可能生效；这条路径也跳过了 `cancel` 里的记账
  - 处理：按停止线开 issue #109 留后处理（参照 `send` 的「交出前失败 / 交出后未知」区分，后者报撤销结果未知并记 `Unknown`）

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：4 条
- 无法核实：闸的 60 秒、一次性、会话绑定（`gate.rs` 不在补读范围；本 PR 未改闸）；TDLib 续发与 `get_persistent_message_id` 的行号（源码在主仓 `resources/td/`，本工作区没有；手下 msg1 已逐行读过，见 `state/handoff/msg1.md`「已核的事实」）——留给实测

### 是否再审

有 P1，改完开第 2 轮（脚本交大总管代跑）。
