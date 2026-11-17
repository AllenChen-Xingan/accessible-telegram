# 评审 031 · #8 B 段：编辑自己发的一条纯文字消息（atg-cli edit、atg-core edit_prepare / edit_text）（codex · gpt-6-astra · read-only）

> 对象：diff `2309789..bde20a0`（第 1 轮）、`2309789..HEAD`（第 2 轮起，HEAD = `c28d1d1`）（分支 `feat/8-edit`；main 2309789 = A 段 #140 合入之后）；依据 `specs/contracts/actions.md`、`cli.md`、`errors.md`、`config-privacy.md`。**最新一轮：第 3 轮（2026-11-17，身份 `edit-8b-r` 第 2 轮）· 总评「通过」· 连续两轮无 P0/P1，满足停止线，收敛**。
> 第 2 轮起换身份：脚本身份按 target 字符串认对象，第 1 轮写死的范围一变即被拒；不改身份文件，换名 `edit-8b-r`、target 用不变的 `2309789..HEAD`（大总管同意），停止线从这一轮重新数。
> 之前：atg-review 自审 1 轮（共同规矩第 17 条：按提交范围 `2309789..1fca933` 取 diff），P0/P1 无；P2×1（链接预览按默认重新生成）与 P3 三条开 #147，其余 P3 九条改在 `bde20a0`。
> `--force-secrets`：6 处命中全是 `token: ConfirmToken` 类型标注或提到它的句子，大总管逐条核实后准（共同规矩第 15 条）。
> focus：`_tmp/codex-031/focus-1.md`（不入库），含共同规矩第 16 条定级口径。

## 第 3 轮（2026-11-17，`edit-8b-r` 第 2 轮）· 总评「通过」

原文：`_tmp/codex-031/edit-8b-r-codex-2.md`（不入库）。模型 gpt-6-astra · read-only · medium，39 秒。对象同第 2 轮（无改动）。放行同第 2 轮（同一批 9 处）。
本轮 P0/P1：0 条 · P2：0 条 · 停止线：本轮与第 2 轮都无 P0/P1，**满足，停**；留作 issue 的 P2：无
- 引用抽查：「属实」7 条；「无法核实」2 条（TDLib 源码、`QUERY_TIMEOUT` 定义不在补读范围）

## 第 2 轮（2026-11-17，`edit-8b-r` 第 1 轮）· 总评「通过」

原文：`_tmp/codex-031/edit-8b-r-codex-1.md`（不入库）。模型 gpt-6-astra · read-only · medium，43 秒。对象 `2309789..HEAD`（`c28d1d1`，含第 1 轮修复）。`--force-secrets`：9 处命中（第 1 轮那 6 处 + 本记录第 5 行提到它的句子，记录与补丁里各一份），大总管逐条核实后准。
上一轮处理的验证：实测三处断言、交出报错留痕带 `final_id`、测试按结果行断言，codex 抽查均属实。
本轮 P0/P1：0 条 · P2：0 条 · 停止线：连续第 1 轮无 P0/P1
- 引用抽查：「属实」7 条；「无法核实」2 条（同上）

## 第 1 轮（2026-11-17）· 总评「修改」

原文：`_tmp/codex-031/edit-8b-codex-1.md`（不入库）。模型 gpt-6-astra · read-only · medium，51 秒。
本轮 P0/P1：0 条 · P2：2 条 · 停止线：连续第 1 轮无 P0/P1

- **P2｜`edit/exp_real.rs:83` 实测全部失败仍可通过**（第 1 轮 #1）
  - 核实：属实。每轮核对失败只记 `good=false`、前置发送没成就跳过，最后成功 0 轮也返回成功
  - 处理：前置发送没成直接判失败；每轮断言改成；结尾断言成功轮数等于 `ROUNDS`
- **P2｜`edit.rs:197` 交出报错时留痕不记改的是哪条**（第 1 轮 #2）
  - 核实：属实。共用的 `send::submit_failed` 在失败与结果未知两支都给 `after_outcome` 传 `None`；成功与 CLI 超时两支传了 `Some(id)`，最需要追查的断线反而没有
  - 处理（大总管准动 `send.rs`）：加 `send::submit_failed_for(…, target)`，原 `submit_failed` 改成包装、传 `None`，别的调用处不动；`edit_text` 改调它传 `Some(plan.id)`。测试 `every_outcome_is_logged_with_the_message_id`（成功、失败、断线三行留痕都带 `final_id`、不含正文）；契约 actions.md `edit_text` 行写明留痕的 `final_id`
- 引用抽查：「属实」5 条；「无法核实」2 条（TDLib 源码、`QUERY_TIMEOUT` 定义不在补读范围）

### 是否再审

两条 P2 已改，再审一轮确认（`--previous` 带本轮输出）。第 2 轮起见上。
