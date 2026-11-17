# 评审 033 · #10 转发（`forward`，直发；含闸支持多指纹）（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main...HEAD`（三点；main 已合进分支，含 #140 #143）。重点写在 `_tmp/reviews-033/focus.md`（不入库）。
> **最新一轮：第 2 轮（2026-11-17）· 总评「修改」· 无 P0/P1，待第 3 轮确认**。
> 之前：多视角自审 6 轮（atg-review）。第 1、2 轮各有 P1（交出后个别 null 的说法、回序列化失败压成 None；防重复按整批指纹、换子集能绕过 → 人定方案 A，闸支持多指纹，合 #143 后做）；第 3、4 轮无 P0/P1；改闸后重数，第 5、6 轮无 P0/P1，收敛。机器验收：`cargo test -p atg-core -p atg-cli` 618 过 0 败，clippy -D warnings 两种（tdlib-api、`--features atg-cli/tdlib --all-targets`）0。
> 隐私扫描：密钥类命中 8 条，全是参数类型标注 `token: ConfirmToken`（人定 2026-11-16 长期放行这一类），大总管逐条核实后准加 `--force-secrets`。
> 闸的改动（`Gated::intents`、`GuardStore::claim_all` / `settle_all` / `hit_any`、`worst_hit`、`after_item` 按条改指纹）与「只有闸能发」清单加 `forward_messages` 属一级评审，人本人看。

## 第 1 轮（2026-11-17）· 总评「修改」

原文：`_tmp/reviews-033/forward-10-codex-1.md`（不入库）。effort medium，58 秒（`result: P0×0 P1×1 P2×3 P3×0`）。行号指评审时的版本。
本轮 P0/P1：1 条 · 停止线：未满足 · 留作 issue 的 P2：1 条（#4）
注：重点文件是改闸前写的，仍说「指纹 = 请求本身（整批）」「#143 进 main 后再改 verify_again」；本轮发现都与此无关。

- **P1｜src/atg-cli/src/args.rs:359 可变长 `MSG_ID` 开着 `allow_hyphen_values`，吞掉后置选项**（第 1 轮 #1）
  - 核实：属实。`forward 777 5 --to 888` 里 `--to` 被当成 id 解析而报错；流程测试直接造参数，没走命令行解析。
  - 处理：`MSG_ID` 去掉 `allow_hyphen_values`（消息 id 都是正数；`FROM_CHAT`、`--to` 是单值，照旧收负数）。补测试 `trailing_options_are_not_swallowed_as_ids`（按契约顺序带 `--to`、`--dry-run`、`--again`，另测负数目标）。`resend` 的 `MSG_ID`（args.rs:382）是同一写法，存量，报大总管开 issue。
- **P2｜src/atg-cli/src/forwarding.rs:160 交出这一步就结果未知时没有逐条 `items`**（第 1 轮 #2）
  - 核实：属实。断线、到点、应答认不出走 `?`，只出通用错误。
  - 处理：接住 `send_outcome_unknown`，按计划里的来源 id 每条列 `unknown`（`temp_message_id` 为 0），结尾句同样点名。补测试 `unknown_at_hand_over_still_lists_every_item`；cli.md 写明。
- **P2｜src/atg-cli/src/forward.rs:302 代跑断开时取不到 `forward` 的目标会话**（第 1 轮 #3）
  - 核实：属实。只读 `params["chat"]`，`ForwardParams` 的目标在 `to`，`chat_id` 恒为 `null`。
  - 处理：`forward` 取 `to`，别的发送类仍取 `chat`。
- **P2｜src/atg-cli/src/forwarding.rs:258 结尾统计句在 CLI 里拼，没经过树**（第 1 轮 #4）
  - 核实：规则属实（`architecture-rules.md` 第 3 条）。与 #9 的同一条（032 第 1 轮 #3、第 2 轮 #4）同类，那条人定 A → #128（结尾统计入树等 R6 一起做）。
  - 处理：不在本块改，并入 #128。

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：5 条
- 无法核实：1 条（未编译、未跑测试：只读审查）

### 是否再审

要再审：#1 改完开第 2 轮（重点文件先改成改闸后的样子）。

## 第 2 轮（2026-11-17）· 总评「修改」

原文：`_tmp/reviews-033/forward-10-codex-2.md`（不入库）。重点文件已改成改闸后的样子。隐私扫描密钥类 11 条，全是 `token: ConfirmToken` 及记录里引用它的那句，大总管逐条核过准加 `--force-secrets`。
上一轮处理的验证：codex 抽查确认 #1（`MSG_ID` 不吞后置选项，测试走 clap 解析）、#3（`forward` 目标取 `to`）已修；#4 按题面并入 #128，不重复报。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮无 P0/P1 · 留作 issue 的 P2：无（两条都改了）

- **P2｜src/atg-cli/src/forward.rs:305 本机代跑途中断开时，`forward` 的错误行没有逐条 `items`、没有来源会话**（第 2 轮 #1）
  - 核实：属实。上一轮 #2 只改了执行方那头（`forwarding.rs`），发起方补的断线错误行仍是通用的。
  - 处理：`forwarding::gone_extra` 按请求里的 id（去重、从小到大）每条列 `unknown`、带 `from_chat_id`，`message_zh` 点名这几条；`out::emit_send_unknown` 加附加字段参数。补测试 `executor_gone_lists_every_item_as_unknown`。
- **P2｜src/atg-core/src/gate.rs:248 `worst_hit` 漏看同一指纹上较早的「结果未知」**（第 2 轮 #2）
  - 核实：属实（拦截本身不受影响，只是 `last_outcome` 报成「已发出」）。
  - 处理：`worst_hit` 改看全部还算数的匹配记录，不只每个指纹最新的那条。补测试 `worst_hit_sees_older_unknown_under_newer_success`。

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：5 条
- 无法核实：1 条（只读，未编译）

### 是否再审

要再审：第 3 轮确认（连续第 2 轮）。
