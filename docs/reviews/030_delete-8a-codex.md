# 评审 030 · #8 A 段：给所有人删除自己发的消息（atg-cli delete、atg-core delete_prepare / delete）（codex · gpt-6-astra · read-only）

> 对象：diff `030f91b`（main，PR #134 合入点）到工作区（分支 `feat/8-edit-delete`，检查点提交 `7c6641a` + 自审修正）；依据 `specs/contracts/actions.md`、`cli.md`、`errors.md`。**最新一轮：第 2 轮（2026-11-17）· 总评「修改」· 连续第 2 轮无 P0/P1，满足停止线，收敛**；余 P2×1 开 issue。
> 之前：atg-review 自审 1 轮，P1×1（TDLib 对还没发出的消息说「能给所有人删」，`MessagesManager.cpp:8352-8354`，代码只听它 → 会用 `revoke=true` 删待发消息；假后端编了一条真 TDLib 没有的规则掩盖了它）已修：核心自己拦 `forbidden why:not_sent`，假后端照真 TDLib。P2 中「交出前复查到内容变了，三个 L2 动作的码不统一」与 P3「结果未知时 delete 的 `temp_message_id` 恒为 0」由大总管合开 issue。
> `--force-secrets`：7 处命中全是 `token: ConfirmToken` 类型标注或提到它的句子，大总管逐条核实后准（共同规矩第 15 条）。
> focus：`_tmp/codex-030/focus-1.md`（不入库），含共同规矩第 16 条定级口径（同进程构造时序竞态最高 P2）。

## 第 2 轮（2026-11-17）· 总评「修改」

原文：`_tmp/codex-030/delete-8a-codex-2.md`（不入库）。模型 gpt-6-astra · read-only · medium，62 秒。带 `--previous` 第 1 轮输出与本记录。`--force-secrets`：8 处命中（第 1 轮那 7 处的新行号 + 本记录第 5 行提到它的句子），大总管逐条核实后准。
上一轮处理的验证：改说明已能拒、复查比整份 `DeleteItem`、`mark_opened` 只抹已听 / 已看，codex 抽查属实。
本轮 P0/P1：0 条 · P2：1 条 · 停止线：本轮与第 1 轮都无 P0/P1，**满足，停**

- **P2｜`delete.rs:267` 换图仍靠 `edit_date` 认**（第 2 轮 #1）
  - 核实：属实但窄。图片节点不带文件身份，已编辑过的消息若在同一秒内再换一次图、说明不变，`edit_date` 不变，复查认不出。codex 按时序规则列 P2
  - 处理：**开 issue**（不进停止线）：图片等附件节点挂上文件身份（`FileRef`，M3.7 要做）后，复查自然比得出来
- 引用抽查：「属实」5 条；「不符」1 条——`model.rs:105` `MessageCaps` 注释仍说「还在待发的不行」，与真 TDLib 不符，已改（还没发出的 TDLib 说行，`delete` 自己拦）；「无法核实」2 条（TDLib 源码、`gate` 内部不在补读范围）

## 第 1 轮（2026-11-17）· 总评「修改」

原文：`_tmp/codex-030/delete-8a-codex-1.md`（不入库）。模型 gpt-6-astra · read-only · medium，59 秒。
本轮 P0/P1：0 条 · P2：1 条 · 停止线：连续第 1 轮无 P0/P1

- **P2｜`delete.rs:252` 附件消息被编辑后仍能通过复查并给所有人删除**（第 1 轮 #1）
  - 核实：属实。`DeleteItem` 只存 id 与可确认正文，附件的正文恒为 `None`，复查只比这两样；改说明、换图都比不出来，违反「准备后被编辑则整批不删」。codex 按时序规则列 P2
  - 处理：`DeleteItem` 另存准备时的整份内容（已听 / 已看先抹平：对方听了语音不算编辑）与 `edit_date`（图片节点不带文件，只换图不改说明时靠它认），复查一并比，变了 `invalid_input why:changed`。测试 `attachment_changed_after_prepare_is_refused`（改说明、换图两种）、`voice_listened_meanwhile_still_deletes`
- 引用抽查：「属实」5 条；「无法核实」2 条（TDLib 源码路径不在获准补读范围；`gate` 内部实现不在补读范围）

### 是否再审

本轮的 P2 已改，再审一轮确认（`--previous` 带本轮输出）。
