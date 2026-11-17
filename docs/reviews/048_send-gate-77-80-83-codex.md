# 评审 048 · #77 #80 #83 发送闸组：查重与登记一步做完、交出前到点报 network、重发留痕按条记（codex · gpt-6-astra · read-only）

> 对象：`git diff origin/main...HEAD`（worktree `atg-wt/fix-gate`，分支 `fix/77-80-83`），依据 `specs/contracts/actions.md`、`errors.md`、`cli.md`、`config-privacy.md`、`testing.md`。**最新一轮：第 1 轮（2026-11-17）· 总评「修改」· 未达停止线**。
> 累计处理：P0×0、P1×1、P2×1、P3×0 逐条回源码核实，属实 2、驳回 0、无法核实 0；2 条已修。

开审前：先写会红的测试并实测——临时撤掉三处修法，7 条新测试 5 红 2 绿（2 绿是「交出后到点仍结果未知」的回归测试，修前修后都该过），改动放回后 `git diff` 与存档补丁逐字节一致。多视角自审（atg-review，3 个视角）：无 P0/P1；P2×7（多为重复）、P3×9 全改（`--again` 表只记该记的、查重判定合一、留痕标记与 CLI 同名 `not_resent`、`handed` 注释、契约 `send_text` / `resend` 两行与 `cli.md` `resend` 行补 #80、`errors.md` `network` 行、`testing.md` 留痕行数、变更记录来源统一）。

送审前隐私扫描命中 13 处 key-assignment，全是 `token: ConfirmToken` 类型标注或提它的句子；按 b20 共同规矩第 15 条报大总管逐条核实回「准」后加 `--force-secrets`。

## 第 1 轮（2026-11-17）· 总评「修改」

原文：`_tmp/reviews/048-send-gate-codex-1.md`（不入库）。effort medium，模型 gpt-6-astra，只读沙箱。对象到 530f2a5。
本轮 P0/P1：1 条 · 停止线：未满足 · 留作 issue 的 P2：无

- **P1｜src/atg-core/src/gate/check.rs:192 `--again` 放行会被另一张凭据拿走**（第 1 轮 #1）
  - 核实：属实。放行按计划摘要记在闸里的一张共享表：同一份计划的普通凭据 A 验过、停在复核，带 `--again` 的 B 再验、复核失败没交出，表里留着 true；别的同内容发送这时已登记，A 恢复后取走 B 的放行，发出第二条。反过来也会误清 B 的放行。自审时已把它定成「后验为准」写进契约，但那只是把缺陷写成了规则
  - 处理：去掉共享表。`verify_again` 交回 (确认方式, 这张凭据带没带 `--again`)，文字发送一路带到 `before_submit_again`；`verify` / `before_submit` 签名不变（删除、回应、重发、转发不进防重复，照旧用，等于不带）。契约「放行」条改成「绑在开出的那张凭据上，别的凭据拿不走、清不掉」。测试按 codex 说的交错补上（A 普通先验、B 带 again 再验未交出、别的先交出 → A 照拦）
- **P2｜src/atg-core/src/gate/check.rs:324 分步重发（resend → wait → finish）的结果行不带 `old_id`**（第 1 轮 #2）
  - 核实：属实。按条记只在 `resend_and_wait` 里做了；公开的分步接口走 `finish` → `after_outcome`，结果行不带 `old_id`，不能重发的那条更没有行
  - 处理：`Submitted` 带上重发前的旧 id，`finish` 见到就按条记（`after_item`）；不能重发（应答 null）的那条在 `resend` 交出时当场记一行 `not_resent`。`resend_and_wait` 改走 `finish`，两条路径只此一处。补分步两条重发的测试（每个旧 id 各两行）

### 引用抽查

- codex 列的「核实属实」：4 条（`claim` 先查后记、调用处持 facts 锁；留痕失败时把刚登记的改失败；verify 与 before_submit 之间有 await；新集成测试只传一个旧 id）
- 「无法核实」1 条：`block_on_timeout` 只在 await 看期限、tdlib-rs 首次 poll 同步交出——依据源码不在许可范围。作者核过：`link/block.rs` 的 `block_on_timeout` 每次 poll 返回 Pending 才比期限；同步交出见 `td.rs` 注释（tdlib-rs 1.4.0 `lib.rs:61-68`）
- codex 未编译、未跑测试

### 是否再审

P1 属实已修 → 编译复跑后开第 2 轮，带本轮原文与本记录。
