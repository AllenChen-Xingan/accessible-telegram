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
  - 合并提交 b2d0e85（大总管）；三行 `| Answer::Done` 经准加上
  - 验证（2026-11-16）：check 过；test 483 过 0 败；clippy -D warnings 0（tdlib-api 与 --features atg-cli/tdlib）
- 自审第 5 轮（2026-11-16）：P0 0 · P1 0 · P2 7 · P3 12
  - 已修 P2：选 A 出处；ReactPlan 定义补 replaces_known；留痕字段改名 `reaction{message_id, may_replace}`；errors.md hint_zh 指明两种；CLI 计划句用 `speech::reaction::label`（删 `emoji_of`）；补测试（服务器失败 / 一页没取全 / 频道 / 服务器全但没 synced；fake `_added_fail`）
  - 已修 P3：加密聊天统一、补发结束指针、去掉判法写进契约、changelog 顺序、注释残留
  - 未修 P2（不在本线）：`src/atg-cli/README.md:21` Touch ID 旧说法，已报大总管
  - 拆 `send/react/tests/replace.rs`（行数线）
  - 修后验证：test 486 过 0 败；clippy 0（tdlib-api 与 atg-cli/tdlib）
  - codex 032：第 1 轮 P1×1 P2×2 已修（eb38f0a）；第 2 轮（三点范围 name react-9-3dot，停止线重数）P1×2 P2×3：4 条已修，#4 统计入树大总管定 A → #128
  - 下一步：名额（排 fix → msg1 → 我）→ test + clippy → codex 第 3 轮

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

## 2026-11-17 #9 codex 032 收敛
- 上面「要人定」那题已定 A（#128）；威胁模型另定：构造时序竞态最高 P2（#131）
- codex 032 共 6 轮：第 5 轮仅 P2×1（契约删确认残留，已改），第 6 轮零发现，连续两轮无 P0/P1，满足停止线；无剩余 P2
- 合 main（64918b8，含 #117 #125 #133）后复验：test 533 过 0 败；clippy 两种 0（带 tdlib 的示例 `live_wait_phone.rs` 是 main 的遗留，PR #134 修，合进来后再合一次）
- 下一步：大总管开 #9 的 PR；之后接 #10 转发（codex 033）

## #10 转发 · 设计计划（只读阶段，2026-11-17；等 #135 合后从 main 新开分支）

**结论**：照 `resend` 的骨架做，一条命令一张凭据转发同一来源会话的一条或几条，进重复提交拦截；要动 `send/**`（新文件 `send/forward.rs`、`SealedKind`/`Answer` 加一种）和 `gate/**`（只是 `Gated` 实现，不改闸），开工前报大总管。

### TDLib
- `forwardMessages chat_id topic_id from_chat_id message_ids options send_copy remove_caption = Messages`（`td_api.tl:13180`）：id 须严格递增、一次最多 100；不能转的那条返回 `null`；转进私密聊天、或本地消息时 `send_copy` 恒为真
- 能不能转看 `getMessageProperties.can_be_forwarded`（复制看 `can_be_copied` / `can_be_copied_to_secret_chat`）；开了「禁止转发」（`has_protected_content`）的会话里都是假
- 转出的每条都是新的待发消息：临时 id → `updateMessageSendSucceeded` / `Failed`，等法同 `resend`（逐条，30 秒）

### 内核（atg-core）
- `prepare_forward(b, to: ChatId, topic: Option<..>, from: ChatId, ids: &[i64], copy: bool, origin) -> ForwardPlan`
  - 目标走现成的 `chat_target`，测试服务器只许收藏夹（同 `send`）；论坛群不给话题就落「General」，计划里写明
  - 去重、排序，超 100 → `invalid_input`（`why:"too_many"`）
  - 逐条：在不在（`not_found`）、`can_be_forwarded`（复制时 `can_be_copied`，目标是私密聊天看 `_to_secret_chat`），不行 → `invalid_input`（`why:"not_forwardable"`，点名哪条）
  - 计划带每条的简短读法（同 `history`，隐藏内容照样遮）与原发送人——人事后看 `plan` 就知道转了什么、转给谁
- 摘要绑：请求 JSON + 每条的内容指纹（交出前重取一次比对，变了（被编辑、被删）→ `send_refused(content_mismatch)`，同 `resend` codex 016 第 2 轮 #2 的做法）
- `forward(b, plan, token)`：验凭据 → 复核 → `before_submit` → `SealedKind::Forward{request}` → `Answer::Resent` 那种 `Vec<Option<Message>>`（考虑改名成通用的 `Answer::Many`，或另加 `Answer::Forwarded`）→ 逐条 `wait` / `finish`
- `forward_and_wait`：同 `resend_and_wait`，CLI 与以后的 GUI 共用
- **重复提交拦截**：要进（同一批转两次对方会收到两份，撤不回）。指纹 = 目标 + 话题 + 来源会话 + id 列表 + `copy`；`--again` 放行。这一点和 `react`、`resend` 都不一样
- 留痕：`direct-send.log` 的内容类型记 `forward`、条数；不记来源会话名与内容（来源 `chat_id` 记不记待定，见下）
- 假后端：`do_forward`（按 `can_be_forwarded` 字段决定能不能转、给出临时 id、可配置让某条失败）

### CLI
- `atg-cli forward FROM_CHAT MSG_ID... --to CHAT [--topic TOPIC_ID] [--copy] [--dry-run] [--again]`：先一行 `plan`，直发；逐条等结果，结尾 `result` / `error` 的 `items` 照 `resend`（成功 / 失败 / 结果未知 / 不能转）；结果未知退出码 10、不许重转。`CHAT` 可写 `saved`。`--redact` 先报 `not_implemented`
- 经本机接口转发（`config-privacy.md` 能转的命令加一条）

### 要改的契约
- `actions.md`：`forward` 从「第二批留位」挪进主表（`forward_prepare` · `forward`），第 5 小节「重复提交拦截」写 `forward` 进；第 6 小节加 `ForwardPlan`
- `cli.md`：加 `forward` 一行；`errors.md`：`why` 加 `too_many`、`not_forwardable`；`config-privacy.md`：能转的命令加 `forward`

### 测试（先写会红的）
- 测试服务器转给非收藏夹被拒；禁止转发的会话被拒；超 100 被拒；id 乱序去重后按序
- 准备后来源消息被编辑 → `content_mismatch`；同一批 10 分钟内再转被拦、`--again` 放行
- 一条转成功一条失败 → `items` 各自下落、退出码按 `resend` 规则；到点 → `send_outcome_unknown`
- 真机只转到收藏夹（来源也用收藏夹里的消息），先报大总管

### 要人定的事（建议 + 默认，转大总管）
1. **`--copy`（不带原发送人的复制）v0 做不做**：A 做（多一个开关，复制会隐去原作者）· B 先只做普通转发（建议，默认 B；复制以后再加）
2. **留痕记不记来源会话 id**：A 记（事后能查转了哪个会话的东西，但多一处会话关系痕迹）· B 不记、只记条数（建议 A：留痕本来就记目标 `chat_id`，来源同一级，不涉内容）。默认 A
3. **一次最多几条**：TDLib 上限 100。A 照 100 · B 先限 20（建议 B：误转一大批撤不回，防重复只拦同一批）。默认 B
