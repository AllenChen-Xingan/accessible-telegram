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
- 2026-11-17 大总管：三题进待定清单，先按默认做（不做 `--copy`、留痕记来源会话 id、一次最多 20 条）；等 #135 合后开工

## #10 开工记录（2026-11-17，工作副本 atg-wt/msg2-10，分支 feat/10-forward）
- CLI 命令文件名 `src/atg-cli/src/forwarding.rs`：`forward.rs` 已是本机接口转发；`specs/README.md` 命名规约只管档案，代码按命令名起，撞名时照 `send/resending.rs` 的先例用动名词（大总管同意）
- 核心放 `send/forward.rs`（`Submitted` 只有 send 模块能造）；共享文件只新增：`send.rs`（`pub mod forward;`、`SealedKind::Forward`）、`gate.rs`（`log_source` 默认方法）、`gate/check.rs`（`from_chat_id` 字段两处）、`error.rs`（`NotForwardable`、`ForwardTooMany`），都经大总管批
- 「能不能转」照 #8 的 `message_caps` 写（不另起 props.rs、不动 td.rs 的 getMessageProperties）。**#8 合进 main、合给我之后要补**：`model.rs` 的 `MessageCaps` 加 `can_forward`；`td/requests.rs` 的 `caps()` 加 `can_forward: p.can_be_forwarded`；`fake/caps.rs` 的 `Cap` 加 `Forward`、`caps()` 加 `can_forward: on(Cap::Forward)`。在那之前 core 编不过，不领名额
- 2026-11-17 大总管：论坛群不带话题落进 General（不拒、计划行写明）进待定清单，默认照现在走；人要统一成拒再改。#10 代码与契约已写完，等 #8 合进 main、合给我后补 MessageCaps.can_forward 三处再编译
- 2026-11-17 fix 的 codex 048 改了 --again 接法：fix 的 PR 进 main、合给我后，`send/forward.rs` 的 `forward()` 改用 `verify_again` / `before_submit_again`（原 verify / before_submit 变成不带 again 的包装）。回归靠 core 的 `same_batch_twice_is_held_unless_again` 与 CLI 的 `same_batch_again_is_held_then_released`：没改对它们会红
- 2026-11-17 自审第 2 轮 P1（防重复按整批指纹，换子集能把结果未知的那条再转一遍）：大总管选 A——闸支持一张计划多个指纹（Gated::intents 默认 vec![intent]，check.rs 查重复与收尾逐个），forward 每条一个指纹。**等 #143 进 main、合给我后再动闸**；A 落地后整个 #10 PR 属一级评审，PR 描述单列「闸的改动」「只有闸能发清单（加 forward_messages）」两节
- 2026-11-17 #10 收敛：自审 6 轮（改闸后第 5、6 轮无 P0/P1）；codex 033 三轮（第 2、3 轮无 P0/P1），记录 `docs/reviews/033_forward-10-codex.md`；test 622 过、clippy 两种 0。下一步：大总管开 PR（闸改动 + 「只有闸能发」清单属一级评审，PR 描述单列两节），之后接 #12 发文件 / 图片（codex 034）

## #12 发文件 / 图片 · 设计计划（只读阶段，2026-11-17；等 #10 的 PR 合后从 main 新开分支，codex 034）

**结论**：照 `send` 的骨架做「发一个本机文件」，L2 直发过闸；比发文字多三件要紧的事——**交出的是快照副本**（防交出后文件被换）、**哪些路径能发**（防 AI 被诱导把本机密钥发出去，要人定）、**上传要等得久**（30 秒不够）。

### TDLib
- `sendMessage` + `inputMessageDocument{document: inputDocument{document: inputFileLocal{path}, thumbnail: null, disable_content_type_detection: false}, caption}`（`td_api.tl:6700,6426,340`）；图片另有 `inputMessagePhoto`（`:6716`，要 `width`/`height`，有 `self_destruct_type`、`has_spoiler`）
- 交出后 TDLib **异步读文件、上传**，进度走 `updateFile`（`:11596`，`remote.uploaded_size` / `is_uploading_completed`）；成败照旧 `updateMessageSendSucceeded` / `Failed`
- 说明文字上限 `getOption("message_caption_length_max")`（普通号 1024 字）

### 核心（atg-core，新文件 `send/file.rs`）
- `prepare_file(b, chat, path, caption, reply_to, origin) -> FilePlan`：路径规则（人定 A，见下）→ `stat`（普通文件、大小上限 50 MB，人定）→ **复制一份快照**到档案目录 `outbox/`（0600，文件名随机，不带原名以外的信息）并算 SHA-256 → 计划带「原文件名、大小、类型、SHA-256 前 8 位」给人看；请求里的路径是快照的
- 摘要绑：请求（含快照路径）+ 文件 SHA-256 + 说明文字；交出前重算快照的 SHA-256，变了拒（`content_mismatch`）。快照在等到结果（成功 / 失败）后删；结果未知的留着（TDLib 下次启动续发要读它），过 24 小时清
- 防重复：进；指纹 = 目标 + 文件 SHA-256 + 说明 + 回复（`sending_id` 记 0），**不含路径**（同一文件换个地方再发也拦）
- 等结果：不是固定 30 秒，而是「有进度就续」——`updateFile` 的已上传量在涨就接着等，**无进度 60 秒**到点按结果未知；总上限按大小算（人定默认）
- 留痕：内容类型 `document` / `photo`、文件大小；**不记路径、不记文件名、不记说明文字**
- 话题群规则、测试服务器只许收藏夹、`--redact` 先拒：同 `send`

### CLI
- `atg-cli send-file CHAT_ID PATH [--caption T | --caption-file P] [--reply-to MSG_ID] [--dry-run] [--again]`（图片按扩展名自动判断，没有 `--photo`）（或做成 `send --file PATH`）；标准错误按 10% 报上传进度（`kind:"progress"` 进标准输出要先进契约）
- 经本机接口代跑：发起方把**文件内容**带过去还是只带路径？只带路径的话执行方读的是发起方给的路径（同 `--text-file` 的先例：发起方读成内容再带过去）。大文件走管道不合适——v0 档案被占时报 `profile_busy`、不转（同 `download`），待定

### 要改的契约
- `actions.md`：`send_file` 从第二批留位挪进主表；第 5 小节写明文件指纹不含路径；第 6 小节加 `FilePlan`；「只有闸能发」清单不变（仍是 `send_message`）
- `cli.md` 加一行；`errors.md` 加 `why`：`file_not_allowed`（路径不许发）、`file_too_large`、`not_a_file`、`caption_too_long`；`config-privacy.md`：档案目录加 `outbox/`，写清何时删

### 测试（先写会红的）
- 路径规则：许可目录外、密钥类文件名（`.env*`、`id_rsa*`、`*.pem`、档案目录本身、TDLib 数据目录）一律拒
- 准备后改了原文件 → 交出的是快照，内容照旧；快照被改 → `content_mismatch`
- 同一文件换路径再发被拦、`--again` 放行；失败的删快照、结果未知的留
- 上传有进度不超时、无进度 60 秒按结果未知
- 真机只发收藏夹、一个几 KB 的编造文本文件

### 人定（2026-11-17，大总管转达，选择题）
1. **哪些文件能发 → A**：只许「下载」「桌面」「文稿」（`~/Downloads`、`~/Desktop`、`~/Documents`，按解析符号链接后的真实路径判）+ 密钥类文件名黑名单（`.env*`、`id_rsa*` / `id_ecdsa*` / `id_ed25519*`、`*.pem` `*.key` `*.p12` `*.keystore`、`*.session`、`credentials*`、`.netrc`，以及 atg 档案目录、TDLib 数据目录里的任何东西）。不许 → `invalid_input`，`why:"file_not_allowed"`（说明里写许可的目录）
2. **大小上限 → A**：50 MB，超了 `why:"file_too_large"`
3. **图片 → B（按扩展名自动判断）**：`jpg` `jpeg` `png` `webp` 自动按图片发（`inputMessagePhoto`，Telegram 会压缩、对方直接看到图），其余按文件发（`inputMessageDocument`，不压缩）。按图片发不了的自动退回按文件发、计划行写明：TDLib 规定图片 ≤ 10 MB、宽 + 高 ≤ 10000、长宽比 ≤ 20（`td_api.tl:6429`）——大小在准备时判；宽高从文件头读（PNG / JPEG / WebP 头，读不出给 0，TDLib 说「可能被服务器替换」，待实测）。`gif` `heic` 等 v0 一律按文件发。计划行与文字模式都说清「按图片发（会被压缩）」还是「按文件发」
4. **等多久 → 默认**：有进度就续，无进度 60 秒算结果未知，总上限 10 分钟
- 另：删除 / 编辑「不放开改别人的」人已定（与本块无关，记一笔）

### 开工
等 #154 合进 main 后从 main 新开分支（大总管开工作副本）；动 `send.rs`、`gate/**` 之前先报大总管。
- 2026-11-17 大总管：#12 计划照此做；`--as-file` 先不加（人选 B），进待定清单下次问人；开工等 #154 合

## #12 发文件 · 开工（2026-11-17，工作副本 msg2-12，分支 feat/12-send-file，codex 034）
- 大总管许：send.rs 只加 `pub mod file;`；gate 只加留痕字段（`Gated::log_file` 默认 None、`DirectLogLine.file{kind,size}`），拦截逻辑不动；td/fake 改动集中
- 基线放宽（大总管定 A）：td.rs 797→802、fake.rs 1014→1021，单独提交，arch 拆 fake/td 时收回；PR 说明单列「基线放宽」
- 写好（未编译）：core `send/file.rs` + `send/file/local.rs`（许发路径、快照、图片头）+ `send/file/tests.rs`；`backend.rs` `Upload` + `upload_progress`（默认 None）；`td/real/upload.rs`、`fake/upload.rs`；`tdjson/file.rs::upload`（`tdjson::file` 改 pub）；`error.rs` 4 个 why；`gate/tests/file_log.rs`；`config::home` 改 pub(crate)；CLI `send_file.rs` + tests、`args.rs` / `main.rs` / `tests/cli.rs` 登记
- 契约：actions（表、第 5、6 小节、留痕）、cli（`send-file` 行）、errors（4 个 why、profile_busy 场景）、config-privacy（`outbox/`、留痕、不代跑）
- 与计划不同处：读不出图片宽高的退回按文件发（`photo_unreadable`），不按图片发 0×0；v0 不出上传进度行
- 下一步：等名额 → test + clippy 两种 → 自审 → codex 034 → 真机只发收藏夹
- 2026-11-17 提交：72aaa84 基线放宽、dace3f8 #12 第一版（大总管提交）
- 自审第 1 轮（384da95...HEAD）：P0/P1 无；P2×4 P3×11 全修。先红后绿（edges.rs 4 条红：硬链接、外部路径先报不许、本机检查先于查会话、失败能重发留快照）。改法：`local::allowed` 打开时 O_NOFOLLOW + dev/ino 比对 + nlink==1，之后只从句柄读；失败 can_retry 留快照；契约补结果未知下一步、24 小时与收藏夹例外、进度怎么算、photo_fallback 含义、sha256_8 核对法、UTF-16 码元；黑名单正本在代码
- 验证：676 过 0 败；clippy 两种 0；fmt / no-debug / file-size 过
- 下一步：大总管提交检查点 → 自审第 2 轮
- 自审第 2 轮（检查点 702c92a 后）：P0/P1 无 → 连续两轮无 P0/P1，自审收敛。P2×4 P3×9：CLI 入口先查会话（先红后绿，加 `file::precheck`）、文案与契约补写已修；resend 带文件不按进度等 → #158（契约引用）
- 验证：677 过 0 败；clippy 两种 0；no-debug / file-size 过
- 下一步：大总管提交 → codex 034（384da95...HEAD）；PR 前合 main（#156 拆 tdjson，`tdjson.rs` 的 `pub mod file;` 会冲突），合时报大总管
- 合 main 注意：#156（tdjson.rs `pub mod file;`）、#157 fe8d1ec（gate/tests.rs 我加的 `mod file_log;` 挨着它换的 temp_dir）可能冲突；合后申请名额编译验证
- codex 034 第 1 轮：P2×3 全属实已改（句柄路径再判许可、意图摘要不装媒体结构、等进度用绝对截止）；679 过；记录 docs/reviews/034_send-file-12-codex.md
- codex 034 第 2 轮：P2×2 属实已改（关类型识别、查询限时 + 判未知前看事件）；大总管已合 main 9434d39；680 过；下一步第 3 轮 → PR（说明放 _tmp/20261116-b20/，两节：闸逐行、基线放宽）
- codex 034 第 3 轮（三点范围，确认轮）：无 P0/P1，P2×2 → issue；PR 说明 MAIN _tmp/20261116-b20/pr-12-send-file.md；下一步大总管提交记录、开 PR；真机只发收藏夹等排
## #13 投票与按钮 · 设计计划（只读阶段，2026-11-17；工作副本 msg2-13，分支 feat/13-poll-buttons，从 main fe8d1ec；codex 035）

**结论**：建议拆成两个 PR——**13a 投票**（读全 + 投票 / 撤票）、**13b 按钮**（读出 + 按下）。两边都有对外动作（L2，直发不确认，同 #113），都要在「只有闸能发」清单加函数（一级评审）。读的部分（L0）先做、不动闸。

### 现状
- 树里投票只有题目（`Attachment::Poll { question }`，`tree/mod.rs:276`），选项、票数、我投了哪个都没有；消息上的按钮（`message.reply_markup`）完全没读

### TDLib（`src/tdlib-rs/tdlib-rs/tl/api.tl`）
- 投票：`messagePoll`（:5796）→ `poll`（:731：选项、总票数、匿名、多选、能否改票 `allows_revoting`、测验 `pollTypeQuiz`、已结束、`vote_restriction_reason`）；`pollOption`（:475：文字、票数、百分比、`is_chosen`）。投票 `setPollAnswer(chat, msg, option_ids: [int32])`（:13890，0 起的位置；空表 = 撤票）。变化走 `updatePoll`（:12059）
- 按钮：`replyMarkupInlineKeyboard`（:4314，消息下面的按钮）/ `replyMarkupShowKeyboard`（:4309，机器人换掉输入框的键盘）。内联按钮 11 种（:4230–4266）：网址、登录网址、网页应用、回调、带密码的回调、游戏、切换内联、购买、用户、复制文字、禁用。键盘按钮 9 种（:4167–4215）：文字、发手机号、发位置、发起投票、选用户、选会话、网页应用……
- 按回调按钮：`getCallbackQueryAnswer(chat, msg, callbackQueryPayloadData{data})`（:14098），机器人回一句 `callbackQueryAnswer{text, show_alert, url}`（:8384）；机器人 502 = 没在时限内回

### 动作与对外等级（建议）
| 动作 | 等级 | TDLib | 说明 |
|---|---|---|---|
| 读投票全貌（选项、票数、我选了哪个、匿名 / 多选 / 测验 / 已结束 / 能否改票） | L0 | 已有 `getMessage` | 进树与读法（「投票：午饭吃什么，3 个选项，共 12 票，你选了第 2 项」） |
| `vote`（投一票 / 多选几项） | **L2** | `setPollAnswer` | 非匿名投票别人看得到你投了谁；匿名也改票数。直发、留痕；**测验投了不能改**（计划行写明）；已结束、不能投（`vote_restriction_reason`）的准备时拒 |
| `vote --retract`（撤票） | **L2** | `setPollAnswer` 空表 | 只有 `allows_revoting` 且非测验才许 |
| 读按钮（每行每个：文字、种类、网址按钮的网址） | L0 | 已有 `getMessage`（`reply_markup`） | 进树（新节点或消息的子节点）与读法（「按钮 3 个：确认、取消、打开网页」） |
| `press`（按回调按钮） | **L2** | `getCallbackQueryAnswer` | 机器人收到、可能做任何事（验证入群、确认操作……）；直发、留痕；结尾带机器人回的那句 |
| `press`（键盘上的文字按钮） | **L2** | 就是发那句文字 | 走现成的 `send_text`（同一道闸、同防重复），不新开交出口 |
| 网址按钮 / 复制文字按钮 / 用户按钮 | L0 | — | 只读出网址、文字、用户 id，**不自动打开**（打开是 `open`，另一件 issue） |
| 其余（登录网址、网页应用、购买、游戏、切换内联、带密码的回调；键盘上发手机号 / 位置 / 选用户 / 选会话 / 发起投票 / 网页应用） | 拒 | — | v0 一律 `invalid_input`（`why:"button_not_supported"`）：要么交出个人信息、要么要付钱或密码、要么要开网页 |

### 闸（一级评审，动前报大总管）
- `SealedKind` 加 `Vote`、`Callback` 两种；「只有闸能发」清单加 `set_poll_answer`、`get_callback_query_answer`
- 防重复：投票**不进**（同样的票投两次结果一样，同 `react`）；按回调按钮**进**（按两次机器人收两次，可能做两次事；10 分钟，`--again` 放行）——要人定（题 3）
- 落点：交出口分支加在 `td/real/submit.rs`、`fake/submit.rs`（不动 `td.rs`、`fake.rs` 本体）；读按钮要 `model.rs` / `tdjson` / `tree` / `speech` 加字段，`tdjson` 已被 #156 拆过，按新结构放新文件。不碰 #12 改过的文件

### CLI（建议）
- `poll CHAT MSG`（读全，L0，可并进 `history --detail`）· `vote CHAT MSG OPTION... [--retract] [--dry-run]`（OPTION 用 1 起的序号，计划行写出选项文字）
- `buttons CHAT MSG`（列出，L0）· `press CHAT MSG ROW COL [--dry-run] [--again]`（或按序号；计划行写出按钮文字、种类、发给哪个机器人）

### 要人定（经大总管）
1. **拆不拆**：A 拆 13a 投票 / 13b 按钮两个 PR（建议，各自小、各自一级评审）；B 一个 PR
2. **按回调按钮直发吗**：A 同一切 L2 直发、不确认（建议，按 D1 / #113）；B 回调按钮另要确认（但确认已删，等于另起一套）；C 只许按按钮文字在白名单里的（如「我不是机器人」）
3. **按回调按钮进不进防重复**：A 进，10 分钟内同一条消息同一个按钮再按拦下，`--again` 放行（建议）；B 不进
4. **键盘文字按钮**：A `press` 帮你把那句文字发出去（走 `send_text`，建议）；B 不做，让人自己 `send --text`
5. **哪些按钮种类先做**：A 只做回调 + 键盘文字，网址 / 复制文字 / 用户只读出，其余拒（建议）；B 网址按钮也直接打开（要 `open`，那是另一件 issue）
6. **测验**：A 照样许投、计划行写明「测验投了不能改、投后会公布正确答案」（建议）；B 测验不许投
- 2026-11-17 大总管：只读那半先开工（投票全貌 + 按钮 reply_markup 读进树与读法，不碰闸、不外发）；题 1 = A（拆 13a / 13b）、题 5 = A（回调 + 键盘文字，网址只读）大总管定；题 2/3/4/6 问人中
- 2026-11-17 **人定（经大总管）全按 A**：1 拆 13a 投票 / 13b 按钮（大总管定）· 2 按回调按钮同一切 L2 直发，计划行写明哪个按钮、哪个机器人 · 3 按回调按钮进防重复（10 分钟内同一条消息同一个按钮再按拦下，`--again` 放行），投票不进 · 4 键盘文字按钮由 `press` 走现成的 `send_text`（闸、防重复、留痕照旧）· 5 先做回调 + 键盘文字，网址 / 复制文字 / 用户只读出，其余拒（大总管定）· 6 测验许投，计划行写明「投了不能改、投后会公布答案」
- 2026-11-17 大总管：读法里题目已以标点结尾就不再加逗号（「投票，几点出发？2 个选项」），补快照
- 2026-11-17 #13 只读那半写完、验过：投票读全 + 按钮读出（树 `tree/interactive.rs`、映射 `tdjson/interactive.rs`、读法 `speech/interactive.rs`，`model::Message.buttons`，受限清按钮，mask 遮选项剧透）；读法题目以标点结尾不接逗号 / 分号；新夹具 `interactive.json`（含无标点题目一例）；快照：basic brief / detail / full / json、hidden_everywhere 变（逐行看过，全是投票补选项带来的），新 `interactive_layers`；649 过 0 败，clippy 两种 0，fmt / no-debug / file-size 过
- 没接：`updateMessageEdited.reply_markup`（机器人改按钮）、`updateChatReplyMarkup`——要动 CoreEvent / store / watch，13a / 13b 前报大总管。（更正 2026-11-17 第 5 轮：票数变化对用户账号走 `updateMessageContent`，早已接上；`updatePoll` 只给机器人，不用接）
- 2026-11-17 #13 只读 自审第 1 轮：P1×3（去重）全修——`results_visible`（看不到结果不读票数）、`option_order` 排序 + `index`、名称撤回守第九节（大总管认可；「名称说有按钮」默认不说，攒清单）；P2 修：说明文字进正文、`blocked` 原因（开放集合）、测验正确答案与解析、全文 18+ 遮挡统一、有临时内容不挂原按钮、按钮说法、已结束不说能改、附件说法以标点结尾不接逗号、契约补 `updateChatReplyMarkup`；P3 小事留着。649 过 0 败，clippy 两种 0；对 main 的快照差异：basic detail / full / json、hidden_everywhere 各一处（投票补字段），名称（brief）与 main 一致；新 interactive_layers
- 2026-11-17 #13 只读 自审第 2 轮：P1×1（看不到结果不许诺「投了就看得到」：可能是结束后才公布）已修；P2/P3：按钮详情去括号、全文说明紧跟题目、已结束测验不说能不能改、契约写明「第 N 项」↔ index、membership_required 带 chat_id、标点表一份、契约补类型名 / 遮挡范围 / 网址例外 / 13a13b。649 过，clippy 两种 0；本轮只 interactive_layers 快照变（加两例）。下一步：大总管提交 → 第 3 轮（第 1、2 轮都有 P1，停止线未满足）
- 2026-11-17 #13 只读 自审第 3 轮：无 P0/P1（连续第 1 轮）；P2 修：第九节详情旧句、名称带说明写明、带密码回调说明是账号两步验证密码；P3：拼接规则并成一个 glue、「不进树」清单补全、测验不看 revotable 写明依据。遗留（大总管记，13a 补）：「先加入指定的会话」不读会话名。682 过，clippy 两种 0，快照无变化。下一步：大总管提交 → 第 4 轮 → 无 P0/P1 就 codex 035
- 2026-11-17 #13 只读 自审第 4 轮：P1×1（测验一律「投了不能改」不对：TDLib 改票只看 has_revoting_disabled_，`PollManager.cpp:905,1240-1262`）已修，照 revotable 读；人定 6 措辞「投了不能改」要改，大总管问人中；P2 补 request_managed_bot；P3 契约多处补写。未编译，等名额（arch 用着）。停止线从头算：下一轮第 5 轮
- 2026-11-17 人改定人定 6：计划行照投票实际设置写能不能改，测验另写「投后会公布答案」；契约已改（actions.md 原条标注 + 新增一行「人改定」，content-tree.md 指过去）
- 2026-11-17 #13 只读 自审第 5 轮：P1×1（契约说票数要再取才更新、13a 要接 updatePoll——错：updatePoll 只给机器人，用户账号走 updateMessageContent，早已实时更新；TDLib api.tl:12058、PollManager.cpp:484-499 核实）已改契约 + 加快照测试；P2 KeyboardKind 加 unknown 兜底（kind 改成带 type 判别）；P3 契约多处。未编译
- 2026-11-17 #13 只读 自审第 6 轮（加一个视角逐条核 TDLib 说法：约 30 条属实，2 处小错——tl:4293→4292、历史修订记录加「已更正」，另补测验答案「自己发起的也有」）：多视角无 P0/P1（第 5 轮之后连续第 1 轮）；P2 修：quiz 换开放集合 kind（硬规矩 2）、「谁投了」守卫真正写进 poll() 注释并加快照测试；P3 修：13b 归属、入口写全、标点规则只留一条通则、Keyboard 注释、KeyboardKind derive 体例、basic.json 重复键、测试注释行号。未编译
