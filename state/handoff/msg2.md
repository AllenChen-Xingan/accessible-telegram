# 现状页 · 手下 msg2（批次 b18：收发消息其余功能）

## b20 接手（2026-11-16）

- 交代：MAIN `_tmp/20261116-b20/msg2.md`；大总管 `allenchen-37`
- 现状：#9 代码基本写完（自审 4 轮），停在第 4 轮 P1（普通群信缓存判「挤掉」）未修；工作副本 29 改 + 11 新，未提交，无 codex
- 合 main：22 个文件两边都改；`gate.rs`、`send.rs` 两边各自拆过子模块（main #103 拆成 gate/check·display·tests、send/outcome·resending·text·tests；我拆成 gate/log·visible、send/json·react），冲突最重；serve.rs、fake.rs、td.rs、四份契约次之
- Touch ID 删除影响：`ConfirmMode::unconfirmed`、`needs_confirm`、`NEEDS_CONFIRM`、`mode_for(target, confirm)`、留痕 `confirm` 字段整套作废——直发是唯一方式，react 直接走直发留痕；要人定的 B 选项作废，只剩 A / C
- 下一步：等大总管合 main → 删 unconfirmed 那套 → 修第 4 轮 P1 → 测试 → 自审 → codex 032
- 2026-11-16 合 main 冲突已解（大总管 WIP 93f626f + merge，未提交）：
  - core：send.rs、gate.rs 取 main 版再加 react 钩子（`SealedKind::React`、`Answer::Done`、`pub mod react`、`Gated::log_target` 默认 None）；`gate/check.rs` 只在 DirectLogLine 加 `target` 字段（大总管定不挪）；删 `gate/log.rs`、`gate/visible.rs`（用 main 的 display.rs）、`send/json.rs`（`react_plan_json` 挪进 react.rs）
  - unconfirmed 整套删；react 开 direct_mode、不进防重复
  - 第 4 轮 P1 按 A（人定）：只有私聊、加密聊天信本机，普通群也问服务器；计划加 `replaces_known`，留痕 `replaced` 拿不准记 null；补测试 `basic_group_asks_the_server_who_reacted`、`unknown_state_with_replace_logs_null`
  - CLI：session.rs 去掉 banner/l0_run，`reactions` 也走 l2_run；react 文案改「直接点」
  - 契约：actions / cli / errors / config-privacy / content-tree 冲突合并 + 去 unconfirmed / B18-2 说法；行数基线合并（check-file-size 0 报错）
  - ⚠️ 要大总管同意：`send/text.rs` 两处、`send/resending.rs` 一处 match 加 `| Answer::Done`（不加编译不过）
  - 未编译，等名额

交代：MAIN `_tmp/20261113-b18/msg2.md`。顺序 #9 → #10 → #12 → #13 → #11 → #7，一个 issue 一个 PR。

## 手上：#9 表情回应（分支 feat/9-react）

### 做了什么（2026-11-14）
- **闸**（`atg-core/src/gate.rs`）：`ConfirmMode` 加 `unconfirmed`（B18-2：人定不确认的动作种类）；`Gated::needs_confirm()`（默认真）；`mode_for(target, confirm)`；`unconfirmed` 与 `direct_mode` 一样写 `direct-send.log`（行里加 `confirm` 字段），但不进防重复、不算 `direct_sent`
- **动作**（`send/react.rs` 新）：`prepare_react` / `react` / `react_within`，计划 `ReactPlan`；请求 `addMessageReaction`（`is_big=false`、`update_recent_reactions=false`）/ `removeMessageReaction`；v0 只收 emoji，付费星星不做
- **只读**：`Backend::added_reactions`（`getMessageAddedReactions`），真后端 `td/real/reactions.rs`、假后端 `fake/submit.rs`
- **树 / 读法**：顶层节点 `Doc::Reactor`（`tree/reaction.rs`），读法「林小雨，👍，19:33」（`speech/reaction.rs`，AI 草稿待人实听）；`project::reactor`
- **CLI**：`react CHAT MSG EMOJI [--remove] [--dry-run]`、`reactions CHAT MSG [--emoji] [-n] [--cursor]`（`atg-cli/src/react.rs`），都能经本机接口转发
- **错误**：`invalid_input.why` 加 `reaction_invalid`、`reaction_not_chosen`
- **契约**：actions / cli / errors / config-privacy / content-tree 已同步（各文件修订记录 2026-11-14）
- **拆文件守行数线**（超线文件净增 ≤ 0）：gate→`gate/visible.rs`；send→`send/json.rs`；td→`td/real/submit.rs`；fake→`fake/submit.rs`；tdjson→`tdjson/react.rs`；speech→`speech/reaction.rs`；tree/mod→`tree/reaction.rs`；serve→`serve/conn.rs`

### 自审（atg-review 第 1 轮，2026-11-14）
- P1 ×1（已修）：加回应会被 TDLib 按上限挤掉我原先点的回应（不确认就没人看得到）→ 默认拒 `reaction_would_replace`，`--replace` 放行、`plan.replaces` 列出；假后端按「非会员 1 个」模拟替换；补测试（core + cli 各一条）
- P2 ×8：已修 emoji 写法提示（报错 + cli.md）、组合 emoji 不再转义显示（`check_emoji` 改为只放行 U+FE0F / U+200D）、`confirm:unconfirmed` 含义、cli kind 表加 `reactor`/`ReactPlan`、actions 第 5/6 小节与第 4 小节理由段、gate 模块文档
- P3 ×10：已修帮助文字去代号、`reactions` 借 l2_run 加注释、第 7 条清单补函数、backend submit 注释、计划行挪进 `send/json.rs`、`NEEDS_CONFIRM` 单一来源、「最多等多久」补两条命令、content-tree 索引/类型名/读法、config-privacy 字段、现状页
- 下一步：重跑测试 → 第 2 轮自审 → codex 032

### 自审第 2 轮（2026-11-14）
- P1 ×2（已修）：`send/json.rs` 缺 `ReactPlan` 引入（编译断）；留痕不记哪条消息、挤掉几个 → 加 `target{message_id, replaced}`，日志代码拆进 `gate/log.rs`
- P2/P3 已修：`reactions` 走 `l0_run` 不出直发提示；`react` 加 `--redact` 先拒；`check_react` 拦 remove+replace；措辞统一「可能挤掉」；契约补全
- 修后验证：test 460 过；clippy 0；`--features atg-cli/tdlib` check + clippy 通过

### 自审第 3 轮（2026-11-14）
- P1 ×1（已修，未编译）：防挤掉的判断读本机缓存，可能旧 → CLI 先等补发完；超级群 / 频道另向服务器取「谁点了」；拿不准就拒 `reaction_state_unknown`。⚠️「挤掉原回应」这一块第 2 次出 P1（第 3 次就停下报大总管）
- P2 已修：内核报错不写死 CLI 参数（CLI 补 `hint_zh`）；提示改指 `history --detail`（`show` 还没做）；`next_cursor` 只在 `more` 时有
- P3 留着：读法测试放在 send 而不在 speech；`L2Body` 名字；付费回应读「星星」；plan / result / 留痕「加还是去」三种说法

### 自审第 4 轮（2026-11-14）——⛔ 按第 12 条停下
- 第 3 轮修复验证：test 462 过；clippy（tdlib-api 与 `--features atg-cli/tdlib`）0 警告
- **P1 ×1（未修，停下报大总管）**：普通群被当成「回应变化 TDLib 照推」，直接信缓存。TDLib 源码 `MessagesManager.cpp:6333-6345` 只把私聊、私密聊天算作不用轮询，普通群和频道一样靠查看时轮询。所以普通群里人刚点的 ❤️ 可能不在缓存里，AI 点 👍 不会被拦，留痕还记 0。这是「挤掉原回应」这一块的**第 3 次** P1（第 1 轮没拦 → 第 3 轮缓存旧 → 第 4 轮普通群判错）
- 同轮 P2 ×3（未动）：react 超时报的是「消息已交出……」那句，没提回应；`hint_zh` 没登记进 errors.md，且和 `--again` 的拼接写法不一致；拿不准时硬点，留痕记 `replaced:0`，和「确知 0」分不出
- 同轮 P3 ×7（未动）：措辞、变更记录顺序、`--help` 漏写 state_unknown、转发断开的话、契约签名缺 `synced` / 返回 ConfirmMode、模块文档流程旧

### 验证
- `cargo check -p atg-core --tests --features tdlib-api`、`cargo check -p atg-cli --tests --features atg-core/tdlib-api`：通过
- 测试（自审修前）：`cargo test -p atg-core -p atg-cli` 457 过 0 败；clippy 0 警告
- `--features tdlib`：本工作区没编 TDLib，下次用主仓 `_build/20261025-171733/tdlib`（大总管给的）
- 行数棘轮：0 报错；已按大总管授权跑 `--tighten`（基线 15→13 个文件）

### 下一步
1. 名额到 → 重跑测试 + clippy + `--features atg-cli/tdlib` 检查
2. 第 2 轮自审 → codex 032
3. 真点回应只在收藏夹、先报大总管等「可以」

### 要人定 / 报大总管
- 题面：表情回应「不确认」的前提是「可撤销、留痕可查」，可「加回应可能挤掉人原来点的回应」这一块防线已连续三轮出 P1，本地很难确知我已点了哪些
- 选项：
  - A（建议）照审查给的法子补：只有私聊、私密聊天信缓存，普通群也向服务器取名单；拿不准时留痕记 `replaced:null`（与 0 区分）。代价：普通群、超级群、频道里，服务器不给名单就一律要 `--replace`，多拒一些
  - B 改规则：加回应在拿不准时改走 Touch ID（只有去掉回应、拿得准的加回应才免确认）。代价：要改 B18-2 的适用范围，人定
  - C 接受风险：不拦挤掉，只在留痕里记「可能挤掉、拿不准」。代价：B18-2「可撤销」的前提在会员 / 上限场景下不成立
- 默认：等回复前不动代码
