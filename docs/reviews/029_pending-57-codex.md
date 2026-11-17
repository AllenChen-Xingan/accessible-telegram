# 评审 029 · #57 待发消息列表与按会话撤销（atg-cli pending / pending cancel、atg-core delete）+ #109 + #88（codex · gpt-6-astra · read-only）

> 对象：diff `main...HEAD`（分支 `feat/57-pending`，提交 `373decb`，WIP）；依据 `specs/contracts/actions.md`、`cli.md`。**最新一轮：第 6 轮（2026-11-17，身份 `pending-57-b20` 第 4 轮）· 总评「通过」· 连续第 2 轮无 P0/P1，满足停止线，收敛**。
> 之前：atg-review 自审 4 轮，每轮各有 P1、均已修；「撤销之后报下落」这一处同类 P1 第三次出现时按共同规矩第 12 条停下报大总管，**人定 A（2026-11-14）：就此收住**，不再自审。大总管转达口径：此后 P1 若又落在撤销下落时序上，降级记 P2、写明理由。
> 第 1 轮由大总管代跑（隐私扫描把 6 处 `token: ConfirmToken` 类型标注认成密钥赋值，按共同规矩第 13 条经人确认后加 `--force-secrets`）。
> 第 3 轮起本机 `main` 陈旧（停在 81f6195，检出在别的工作副本），三点范围改用 `origin/main...HEAD`，脚本身份换名 `pending-57-b20`（大总管同意），停止线从这一轮重新数。第 3 轮手下自跑，`--force-secrets` 经大总管逐条核实放行（共同规矩第 15 条：命中全是 `token: ConfirmToken`）。
> 累计处理：P1×6、P2×4 逐条回源码核实，属实 10、驳回 0；P2×1 开 issue #109（已在第 3 轮前一并修掉，`Closes #109`）。

## 第 6 轮（2026-11-17）· 总评「通过」

原文：`_tmp/codex-029/pending-57-b20-codex-4.md`（不入库）。对象 `origin/main...HEAD`（提交 `768485b`，与第 5 轮同）。模型 gpt-6-astra · read-only · medium，60 秒。放行同第 5 轮（同一批 17 处，大总管逐条核实后准）。
本轮 P0/P1：0 条 · P2：0 条 · 停止线：本轮与第 5 轮都无 P0/P1，**满足，停**；留作 issue 的 P2：无
- 引用抽查：「属实」8 条；「无法核实」1 条（TDLib 源码不在获准补读范围）

## 第 5 轮（2026-11-17）· 总评「通过」

原文：`_tmp/codex-029/pending-57-b20-codex-3.md`（不入库）。对象 `origin/main...HEAD`（提交 `768485b`，含第 4 轮修复）。模型 gpt-6-astra · read-only · medium，70 秒。放行同第 4 轮（17 处命中同一类误报，大总管逐条核实后准）。focus 写进共同规矩第 16 条定级口径（同进程构造时序竞态最高 P2）。
上一轮处理的验证：关闭后 `note` 不写盘、`begin` 拒、`settle` 拦关闭与继承标记、`load` 复位并补写，codex 抽查均属实；前几轮修过的点未见回退。
本轮 P0/P1：0 条 · P2：0 条 · 停止线：连续第 1 轮无 P0/P1
- 引用抽查：「属实」8 条；「无法核实」1 条（TDLib 源码不在获准补读范围）

### 是否再审

再审一轮确认（新身份第 4 轮，`--previous` 带本轮输出）。

## 第 4 轮（2026-11-17）· 总评「修改」

原文：`_tmp/codex-029/pending-57-b20-codex-2.md`（不入库）。对象 `origin/main...HEAD`（提交 `edcd11f`，含第 3 轮修复）。模型 gpt-6-astra · read-only · medium，101 秒。`--force-secrets`：14 处命中全是 `token: ConfirmToken` 或提到它的句子，大总管逐条核实后准（共同规矩第 15 条）。
上一轮处理的验证：#1 继承标记、#2 放锁前收尾、#3 撤销断线报未知，codex 抽查均属实；登记只留每个后端一处也核过。
本轮 P0/P1：1 条 · 停止线：未满足

- **P1｜`send_limits.rs:270` 关闭后的写盘入口没封全**（第 4 轮 #1）
  - 核实：属实，第 3 轮 #2 修得不完整。`closed(wait)` 到点还有在途的交出线程时，它晚到的受限 / 限流经 `note` 照样写 `flood-until` / `account-limited`、写失败还补写 `send-pending`——锁已放，可能写到新进程的文件上（还可能拿较短的期限盖掉新进程的）；关闭后并发的 `begin` 也照写标记，而收尾已被封住，留下删不掉的标记
  - 处理：`note` 在关闭后只记内存、不写盘（下次 `load` 补写）；`begin` 关闭后拒（`internal{not_started}`）、不写标记；到点仍在途的，标记留着兜底。单测 `nothing_is_written_after_close`（晚到限流不写盘、标记留着；关闭后 `begin` 拒且不写；再 `load` 解封并补写）
- 引用抽查：「属实」5 条；「无法核实」1 条（TDLib 源码不在获准补读范围）

### 是否再审

本轮有 P1，改完再审（新身份第 3 轮）。

## 第 3 轮（2026-11-16）· 总评「修改」

原文：`_tmp/codex-029/pending-57-b20-codex-1.md`（不入库）。对象 `origin/main...HEAD`（提交 `3bdc42b`：#57 合 main 收尾 + #109 + #88）。模型 gpt-6-astra · read-only · medium，99 秒。
同时跑了 atg-review 自审（#88 #109，`git diff bbe2f20 3bdc42b`）：P0×1（同本轮 #1）、P2×9、P3×10，合并处理，要点记在下面。
本轮 P0/P1：2 条 · 停止线：未满足（新身份第 1 轮）

- **P1｜`send_limits.rs:252` 正常关闭会误删上个进程留下的标记**（第 3 轮 #1；自审 P0 同）
  - 核实：属实，#88 引入的回退。`load` 见到旧标记只设 `unreadable`；#88 新开了不经 `begin` 的两条路（`closed()`、收线程 `observe → outcome`）走到 `settle`，而 `settle` 不看是不是继承来的。跑一条只读命令再正常关闭，或续发的成功事件一到，就把它删了，下一个进程照常发
  - 处理：`State.inherited`（`load` 时标记已在），`settle` 一律不删继承来的；单测 `inherited_mark_survives_a_clean_close`（两次正常启动后仍在、仍拒）
- **P1｜`td.rs:816` 正常关闭不等交出线程、放锁后还碰标记**（第 3 轮 #2；自审 P2「关闭顺序反了」同）
  - 核实：属实。`detached` 线程可在 `closed()` 之后才 `await_outcome`，留下永远等不到的 id；且 `closed()` 在放档案锁之后，可能删到新进程的标记
  - 处理：`closed(wait)` 先等在途归零（`Condvar`，期限为关闭剩余时间），再清待回、收尾，之后置 `closed`：不再登记、不再删文件；td 与 fake 都改为放锁之前调；到点还在途的，标记留着。单测 `close_waits_for_the_submit_thread_then_keeps_hands_off`
- **P2｜`delete.rs:203` 撤销交出时断线仍报成失败**（第 3 轮 #3）
  - 核实：属实。#109 只修了超时
  - 处理：`delete::cancel` 交出出错改走 `send::submit_failed`（断线 / 网络 / 应答转不回 → `send_outcome_unknown`、留痕记未知）；CLI 换撤销说明；单测 `cancel_disconnected_after_submit_is_outcome_unknown`；契约 cli / errors 同步
- 自审另有属实、本轮照修的：
  - 「要等成败的是哪几条」登记了两份（通用交出口按 `Answer`、真实后端交出线程按 tdlib 类型）→ 去掉通用那份，每个后端在拿到应答处登记一次（td 交出线程、fake 的 `submit`），删 `Sealed::is_preview`
  - 缺测试：成败先于应答到、删除事件只认永久非缓存 → 补 `outcome_before_the_answer_is_not_waited_for_again`、`only_permanent_deletions_settle_an_outcome`
  - 文档：写明「正常关闭」指什么、断网时标记可留很久（待发最长 24 小时）；「Q1 默认 b」改为「暂定、待人定、题面在 issue #88」；errors.md 写明撤销交出 10 秒、`temp_message_id` 恒为 0 的原因；模块文档「两个文件」改「三个」；`early` 为什么存在、删除事件字段含义补注释
  - `send_limits.rs` 加测试后 635 行，单测挪进 `send_limits/tests.rs`
- 自审余下 P3 未改（写进现状页）：`jam` 在单测与集成测试各一份且依赖 `write_atomic` 临时文件名；`cancel_unknown` 的留痕没测（测试服务器走测试豁免、`after_outcome` 不记）；「守卫」「交出」等自造词无定义
- 引用抽查：「属实」4 条；「无法核实」1 条（TDLib 源码不在获准补读范围）

### 是否再审

本轮有 P1，改完再审（新身份第 2 轮）；要连续两轮无 P0/P1 才停。

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
