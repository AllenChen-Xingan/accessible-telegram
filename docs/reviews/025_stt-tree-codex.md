# 评审 025 · #59 转写进树（store 转写表、凭据与裁决、投影、读法、遮挡）（codex · gpt-6-astra · read-only）

> 对象：`src/atg-core/src/store/{ticket.rs,transcripts.rs}`、`src/atg-core/src/tree/{media.rs,top.rs}`、`src/atg-core/src/speech/content.rs`、`src/atg-core/tests/transcripts.rs`；已跟踪文件的改动（`store/mod.rs`、`tree/mod.rs`、`mask.rs`、`speech.rs`、`project.rs`、`redact.rs`、`tests/privacy.rs`、快照）以差异文件作依据传入。依据 `docs/experiments/exp008-local-stt/design.md` 第二节第 3、4 条、`specs/content-tree.md`、`specs/architecture-rules.md`。**最新一轮：第 2 轮（2026-11-13）· 总评「通过」· 满足停止线（第 1、2 轮连续无 P0/P1），不再审**。
> 为什么逐文件审（doc 类型）：开审时新文件还没进 git；大总管随后把 3b 提交到 `feat/59-transcript`，定为各轮保持 doc 类型、同一对象（停止线不重数）。
> 之前：atg-review 自审一轮（三视角），P0/P1 无，P2×8 全改（含结构性一条：表键从稳定键改成现在的 id，见 design.md）。
> 已定事项：凭据 = 消息 id + 语音文件 id（人定 2026-11-13）；转写挂消息的 `transcript`（大总管定）。
> 累计处理：P2×3 及「不符」1 条逐条核实，属实 4、驳回 0。

## 第 2 轮（2026-11-13）· 总评「通过」· 停

原文：`_tmp/20261113-media/reviews/stt-tree-codex-2.md`（不入库）。effort medium。对象同第 1 轮（已跟踪文件的差异按 `feat/59-stt-base...HEAD` 重新落文件）。
本轮 P0/P1：0 条 · 停止线：**满足**（第 1、2 轮连续无 P0/P1）；剩余 P2：无

- 无 P0–P2
- 引用抽查：「属实」9 条（含第 1 轮四项处理都已落实、两个顶格文件净减行数）；「无法核实」2 条（`table.rs`、`redact.rs` 不在可读范围，同第 1 轮）

## 第 1 轮（2026-11-13）· 总评「修改」（仅 P2）

原文：`_tmp/20261113-media/reviews/stt-tree-codex-1.md`（不入库）。effort medium。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮无 P0/P1

- **P2｜tests/transcripts.rs 换 id 测试没测到已有转写的挪键**（第 1 轮 #1）
  - 核实：属实（两条换 id 用例都是换 id 之后才提交，整理时把待挪的直接丢掉也照样过）
  - 处理：新用例 `committed_then_renamed_moves_key`：先提交再换 id（失败换 id、再成功换 id，连换两次），断言新 id 上原文还在、旧 id 上没了；换 id 时文件变了就删
- **P2｜「v 不加一」的断言是自己和自己比**（第 1 轮 #2）
  - 核实：属实
  - 处理：改成断言固定值 `1`
- **P2｜18+ 遮挡测试没断言整个字段消失、没覆盖会话项的最后一条**（第 1 轮 #3）
  - 核实：属实
  - 处理：断言 `data` 里没有 `transcript` 键、整行不含 `transcript`；新用例 `chat_item_last_hides_sensitive_transcript`（遮着时无、展开了有）
- **不符｜store/mod.rs:258 字段注释仍写「稳定键 → 转写」**
  - 核实：属实（自审改了表键，这句漏改）
  - 处理：改成「消息现在的 id → 转写」

### 引用抽查

- 「属实」6 条；「不符」1 条（见上）；「无法核实」2 条（`table.rs`、`redact.rs` 宏实现不在本轮可读范围；`redact.rs` 的登记在差异里，暗号测试 `tests/privacy.rs` 佐证）

### 是否再审

无 P0/P1；改完、领名额复验后开第 2 轮确认（停止线要连续两轮）。
