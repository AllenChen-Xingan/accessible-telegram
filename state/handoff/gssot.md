# 现状页 · gssot（图形界面的唯一真相源）

## b20 接手（2026-11-16）

- **手上**：`_tmp/20261116-b20/gssot.md`（MAIN）——① 按 Touch ID 删除改 GUI 规约；② 待人拍板卡整理
- **做到哪**：**本条线已收尾（2026-11-17）**。① 规约改完（直接发送、不弹框；L2 发送流程收成 `specs/gui/focus.md` 第六节）；② 卡 G9 G11 G15 G17 G18 人已定（013 B20-1～B20-6），G16 按默认；自审 atg-review 11 轮（第 10、11 轮无 P0/P1）；codex 040 五轮（第 4、5 轮无 P0/P1），记录 `docs/reviews/040_gui-ssot-codex.md` + 索引；无剩余 P2
- **下一步**：等大总管提交（7b9e8cf 之后的改动 + 040 记录）；人拍板后起写代码的手下（M3.1 骨架 + `--dump-tree`），不归我
- **要人定的事**：`send_again`（输入框 Cmd+回车，重复提交被拦后仍要发）这个键是 AI 草案，下次问人带上（大总管已攒）
- **别的线要做的（已登记 012 第八节，大多已开 issue）**：#77 闸查重竞态 · #115 契约第 5 小节 GUI 标题直发模式 / M4.6 过确认 · #118 朗读省 id · #119 core 两段式发送入口（含晚到上报、早到暂存、查询期限、删除时限、重发复核待定）· #120 speech 截断 · **第 13 条 logline debug 级入口（未开 issue）**
- **不动**：.42cog/intent.md、exp009、exp010、042/043、别人的现状页

### b20 流水

- 2026-11-16 · 读 common、交代、旧现状页；只读看 git status / log / diff；报到大总管
- 2026-11-16 · 大总管提交 304e9ae、合 main 53d4f8c
- 2026-11-16 · 改 Touch ID 相关 7 个文件（脚本 scratchpad `edits.py`，逐处唯一匹配替换）；research 下 findings-*.md 里的 Touch ID 是调研存档，不改
- 2026-11-16 · atg-review 第 1 轮已起（run wf_aff83e1a-80f，范围 `git diff HEAD`——起时未提交）；大总管随后提交 835ef2d，**以后各轮与 codex 一律用范围 `53d4f8c...835ef2d`（及其后的修复）**
- 2026-11-16 · 大总管：冲突 ③a ③b 并进 issue #115（codex 线改，不归我）；五张卡大总管去问人
- 2026-11-16 · atg-review 第 1 轮：P1×2（同一件：契约第 5 小节 GUI 启动提示规约没交代）、P2×9、P3×8。修：README 写明该条与 M4.6「过确认」待 #115、GUI 先不做；术语「直发」改「直接发送（direct_mode）」，单说「直发」只指直发公告；补术语 发送闸 / 凭据 / 交出 / 撤回占位；send_plan 时机写清（读与发同时、不等读完）；focus 删键名副本、M4.6 出处注明；清单修订表恢复两条旧行、追加新行；F8 / A18 覆盖删除、重发；keymap resend / delete 行写法统一；L2 术语补 test_exempt；G11 措辞与 B19-3/4 出处。新开卡 G18（读一遍：边念边发还是念完再发、长文、念不念编号）
- 2026-11-16 · 人拍板（大总管转）：G9 A · G11 B · G15 B · G17 A；G16 未问按默认。落：decision.md 各卡标「人选」、013 加 B20-1～B20-4；keymap 删除改 `cmd-shift-backspace`、草案行照用；a11y-tree 消息项 `role_description`「消息」、导出格式加 `roledesc`、清单 S30
- 2026-11-16 · atg-review 第 2 轮（wf_7b0c1fc8-ac3）：P1×1——闸当场拒发（`duplicate_recent` 等）时草稿被清、无播报、无放行键。修：`focus.md` 闸收下才清空，新触发 `send_refused_now`（草稿与回复目标保留）；`announce.md` 新来源 `send_refused`，`send_plan` 改为收下时才播；`keymap.md` 新动作 `send_again` + 输入框 `cmd-enter`（AI 草案，对应 `send --again`）；清单 A18 补夹具。P2/P3 顺手修：「删除直发」改「直接发送」、roledesc 只在应用设了时写、013 体例（原话待大总管补、G16 编 B20-5）、decision 文件头两套归档规矩写明、012 卡号 G1–G18、backspace = Mac 的 Delete 键、send_plan 一处写全 G18 默认、出处排序、K14 人选、keymap 逐行重复标注去掉、T04 去 F7
- 2026-11-16 · atg-review 第 3 轮（wf_748ec976-be7）：P1×2——① `cmd-enter`「放行」两义、`send_again` 有效条件没写；② 按回车到闸收下之间没有在途锁，连按回车可能发两条。修：focus `send_submitted` 加在途态（回车 / Cmd+回车 吃掉）与「收下」定义；`send_refused_now` 覆盖发送、删除、重发与全部错误码；keymap `send_again` 有效条件（只这一次、草稿未改、未离开输入框，失效即不截）、resend / delete 行时机统一；A18 夹具按正式服务器 direct_mode、加连按两次断言；README test_exempt 补 N7；覆盖表 S30→T03、F7→T06；013 B20-5 写明不是人定
- 2026-11-16 · **报大总管（契约 / atg-core，不归我；记 012 第八节第 8 条）**：契约没要求闸对同一会话「查重复指纹」与「写已提交」串行、原子；GUI 侧在途态兜住了界面连按，但 CLI 并发、GUI 与 CLI 同时发仍可能两条都过
- 2026-11-16 · atg-review 第 4 轮（wf_0c8daf86-76a）：P1×2（同根）——「收下」定义含 TDLib 应答，交出后断网 / 超时会被当成拒发、草稿保留、`send_again` 可放行 → 发两条。修：分界改为**交出**（交给 TDLib 那一刻，不论 direct_mode / test_exempt）；交出前错 → `send_refused_now`，交出后一切结局 → `send_failed` / 结果未知、草稿不回来、`send_again` 不生效；在途态输入框只读、优先于 `send_again`；UiState 加 `in_flight`、`send_again_armed`；012 第八节加第 8 条（core 交出时通知界面）、第 9 条（闸串行化）；「留痕」一词改「调试日志」以别于直发留痕；A18 补正反两面与交出后失败；G18 ① B 代价补「要重写交出分界」
- 2026-11-16 · atg-review 第 5 轮（wf_889c6709-ab7）：P1×2——① 「交出时刻」与契约 30 秒（含等交出应答）对不上，复核卡住时在途态无出口；② TDLib 对 sendMessage 当场回错误时草稿已被清、无失败项可重发 → 丢字。修：**界面分界改按 core 调用的三种返回**——拿到待发消息（清空、读计划）/ 返回错误（`send_refused_now`，含 TDLib 当场错误，草稿保留）/ 30 秒结果未知（新触发 `send_unknown_now`，草稿保留、`send_again` 不武装、提示先别重发）；不再需要 core 交出时通知（删 012 第八节原第 8 条）；`send_again` 只在上次结果「已发出」时武装；`in_flight` 改会话 × 动作、只有发送锁输入框；树导出加 `readonly`、输入框清空例外于值冻结；清单 A18 拆出 A27、A28；README「发送留痕」术语、交出只给指针；012 第八节第 9 条登记「断网恢复后仍会发出」偏离契约
- 2026-11-16 · atg-review 第 6 轮已起（wf_9826b9f6-bf7；起时 G18 未定，结论按新内容核）
- 2026-11-16 · atg-review 第 6 轮（wf_9826b9f6-bf7）：P1×1——拿到临时 id 后 30 秒无成败 / 断线（`send_outcome_unknown` 的 timeout / disconnected）没有行、没有播报。修：`send_failed` 行覆盖之后的全部结果未知、播「可能已经发出，先别重发」；`send_unknown_now` 限定「拿到待发消息之前」；写明 prepare → send_text 两步自动接连；删除按 Ack + `messages_deleted`、重发逐条，不另造 30 秒；`send_again` 按 `last_outcome = succeeded`；GPUI 层状态加 `readonly`（macOS 属性 ⏳ 实测）；A18 标题注「边念边发」；A28 两种结果未知夹具；清单新增行；012 表格补列尾
- 2026-11-16 · atg-review 第 7 轮已起（wf_fe298bc8-7a5）
- 2026-11-16 · atg-review 第 7 轮（wf_fe298bc8-7a5）：P1×2（同根）——规约里「拿到临时 id 后由 core 报结果未知」没有落点：core 只有阻塞的 `send_and_wait`，`wait` / `finish` 谁调没写，不 finish 则闸记录永停 `submitted`、24 小时拦死。修：focus 写明交出段（prepare → 闸 → `send_text` 带交出限时）+ 后台收尾段（`wait` + `finish`），界面不计时不写闸；012 第八节第 11 条登记 core 两段式入口（M4.1 定）、第 12 条登记 `speech` 截断；删除 Ack 后不另播、无结果上限；重发同分两段、逐条；`send_failed` 两种文案；10 秒不抄数；「摘要」改「节选」；长正文字数按字素、往前截；A29；013 B20-6 补人选行
- 2026-11-16 · **再报大总管**：core 两段式发送入口（012 第八节第 11 条）、speech 截断（第 12 条）
- 2026-11-16 · 大总管开 issue #119（core 两段式发送入口）、#120（speech 截断），M4.1 前由 core 做；012 第八节第 11、12 条已引
- 2026-11-16 · atg-review 第 8 轮：P1×1——时限写成交出 30 秒 + 收尾再 30 秒，契约与 core 是一个共用时限。修：focus / 012 第 11 条 / A28 改为「一个从开始交出起算的时限，收尾段只用剩余」，不抄数；交出段统一为「准备 → 闸 → send_text」；README 补术语（临时 id · 待发消息 · 应答、交出段 · 收尾段 · 在途态、Ack）；明确失败按 `can_resend` 提示；013 B20-6 补 #120；清单 / keymap 去重复
- 2026-11-16 · atg-review 第 9 轮：P1×1——报「未知」后同一条的晚到结果（GUI 常开必遇）没有行；晚到失败不提示重发、闸按未知拦 24 小时。修：**发送流程收成 `focus.md` 第六节**（两段、时限、在途态、交出段三结局表、收尾段结局表含「晚到」、删除、重发 not_resent），规则表各行只引第六节；012 第 11 条补晚到上报与闸改记录（#119）；`send_ok` 晚到成功文案；A30；README 术语修（临时 id 为 0 两种来由、交出 = send_text 这一步、应答可为错误）
- 2026-11-16 · atg-review 第 10 轮已起
- 2026-11-16 · atg-review 第 10 轮（wf_bbece4d7-735）：**无 P0/P1**；P2×7、P3×10。修了实质 P2：重发「交出时就未知」只算整次、单条 None 归第 7 条（新来源 `resend_skipped`）；晚到按临时 id 认、`UiState.unknown_pending`，临时 id 为 0 的不念晚到；删除交出暂无时限 → 按发送时限结束在途、说「可能稍后生效」（AI 定）；`why` 字段说明；规则表只留指针；`selected_message_removed` 排除待发消息；012 第 11 条补查询期限、删除时限；步骤名统一契约名
- 2026-11-16 · atg-review 第 11 轮已起（若无 P0/P1 即到停止线）
- 2026-11-16 · atg-review 第 11 轮（wf_619f54dd-8cf）：**无 P0/P1** → 连续两轮（10、11）无 P0/P1，**自审到停止线**。顺手修 P2：重发全部 None 归「当场没成」只播 `resend_skipped`；删除没回应进结局表（新来源 `delete_pending`）；选中的待发消息被 `pending_deleted` 拿掉时原位留占位；A29 / A30 与修订表对齐
- 2026-11-16 · codex 040 第 1 轮已起（`_tmp/reviews-040/`，doc 类，对象 specs/gui + 012 + decision.md + 013，依据 actions / errors / cli / 002）。隐私扫描误报 `send/text.rs:273,298`（`token: ConfirmToken,` 参数名）→ 没用 `--force-secrets`，改 `--exclude` 掉这一个文件（send.rs 文件头、outcome.rs、resending.rs 仍在）
- 2026-11-16 · 大总管核过 `send/text.rs:273,298` 均为 `token: ConfirmToken,`，按 common 第 15 条回「准」→ 第 2 轮起去掉 `--exclude`、加 `--force-secrets`，text.rs 放回依据（记录里写明授权）
- 2026-11-16 · codex 040 第 1 轮：P1×3、P2×1，全部属实已改——① 失败换 id 也迁移锚点（成功 / 失败 / 重发交出统一原地改）；② 交出段来源补被挡 / 后台路由（拒发、删除没回应、不能重发走系统通知）；③ 提交快照 + 在途锁回复目标 + 返回不拉回焦点；④ 在途挡在动作入口（按钮、菜单、AX Press），发送按钮在途禁用。记录 `docs/reviews/040_gui-ssot-codex.md` 已写第 1 轮
- 2026-11-16 · codex 040 第 2 轮已起（`--force-secrets`，命中 3 处均为 `token: ConfirmToken` 及记录里提它的句子，common 第 15 条范围内）
- 2026-11-16 · codex 040 第 2 轮：P1×2、P2×1，全部属实已改——换 id 迁移正本落 012 第三节第 2 小节（列全持 MsgId 的字段）；成功后复核回复 / 话题（check_kept，不一致说「已发送，但…被改了」）；一切异步结局不移动焦点与选中。第 3 轮已起
- 2026-11-17 · codex 040 第 3 轮：P1×1（迁移漏 `last_focus`）、P2×2（复核只适用文字发送；删除 / 重发读法漏目标会话），全部属实已改；第 4 轮已起
- 2026-11-17 · codex 040 第 4 轮：**无 P0/P1**（连续第 1 轮）；P2×2 属实已改（早到成败的顺序 → focus 第六节第 5 条 + 012 第 11 条；A30 收尾段逐条判）。第 5 轮确认已起
- 2026-11-17 · codex 040 第 5 轮：**无 P0/P1 → 满足停止线**（第 4、5 轮）；P2×2 属实直接改（调试日志落点照隐私契约 `~/Library/Logs/atg/`、debug 级登记 012 第八节第 13 条；卡 G2 写定 `--exclude atg-gui` + `-p atg-gui` 并核选包）。记录定稿、索引加 040 一行。**已收尾**
- 2026-11-16 · 大总管：晚到上报已补进 #119；同意——第 10 轮若还冒发送流程（focus.md 第六节）的 P1 就停自审，该节标「待 #119 接口定后复审」，P2 列给大总管开 issue，其余交 codex 040
- 2026-11-16 · 大总管：检查点提交 7b9e8cf（2–5 轮 + G18）；朗读省 id 的契约改动开 issue #118 → 012 第八节第 10 条、013 B20-6 引 #118
- 2026-11-16 · 大总管：已提交 588a6d7；G18 人选 ① 边念边发 ② 念开头加字数 ③ 不念编号；013 写「人在大总管给的选择题里选 X」；契约缺口即 issue #77；`send_again` 攒着下次问人。落：decision.md G18 标人选；013 原话行全填、加 **B20-6**（大总管说 B20-5，但 B20-5 已给 G16，故用 B20-6，已报）；announce `send_plan` 不念编号、超 60 字念前 60 字加「共 N 字」（60 是 AI 数，⏳ 试听）；012 第八节第 8 条引 #77、加第 10 条（契约第 4 小节含 id，GUI 朗读省 id）
- 2026-11-16 · 重写 decision.md 第一节：五张卡各一段「问人」+ 选项 · 代价 · 建议 · 默认；细目与出处挪到节末；修掉 G11 旧文里与 G17 矛盾的「焦点留在搜索框，↑↓ 移高亮」

---

- **手上**：`_tmp/20261114-b19/gssot.md`（MAIN）——界面里每样东西的正本在哪、其余怎么生成或比对
- **做到哪**：第二段主体完成——三份调研已并进 012 与 specs/gui；收了 checklist.md；出了 decision.md（G1–G15，含 Quill 一节）；补丁账 upstream-patches.md 与 ggpui 对齐成一套编号（P1–P15）
- **下一步**：等 gcomp 按 B19-5 重评 + 第九节 Quill → 补组件清单与 G9 / G10；然后 atg-review 自审（范围：012 + specs/gui + 004 README/decision）→ codex 040
- **要人定的事**：见文末（第二段收进 `docs/research/004_gui/decision.md`）

## 流水（时间 · 做了什么 · 路径或结果）

- 2026-11-14 · 读交代、共同规矩；报到大总管（已发）
- 2026-11-14 · 读 skill `single-source-truth`、`complexity-budget`
- 2026-11-14 · 读底子：`.42cog/intent.md` `real.md` `cog.md`、`docs/plan/001`（全）、`002` 第六七节、`003` `005`–`009` `011`、`specs/tasks.md` `architecture-rules.md` `content-tree.md`（纲要 + 第三节）、`contracts/actions.md`（纲要 + 动作表 + 事件表）、`testing.md` 第一节第 3–6 小节、`errors.md` 第一节第 4–5 小节、`cli.md` 第一节第 6 小节、exp001 / exp003（含 b、b-v2）/ exp004 README、MAIN `state/handoff/exp003b.md`
- 2026-11-14 · 读源码（只读）：`src/atg-core/src/speech.rs:29-44`（`SpeechCtx.listen_key` / `expand_key` 由调用方注入）、`notify.rs:1-57`（策略在 core）、`tree/mod.rs:28-66`（`SCHEMA_V`、`MsgId`、`Doc`）；GPUI `resources/zed/crates/gpui/src/elements/div.rs:1365-1375`（`.accessibility_id()` → `author_id`）；accesskit_macos 0.26.3 `node.rs:501-504`（`author_id` → `accessibilityIdentifier`，即 macOS 的 AXIdentifier）——**agent 能读到的稳定标识有现成的出口**
- 2026-11-14 · 查 CI：`ci.yml` 在 macos-latest 上跑 `cargo clippy --workspace` / `cargo test --workspace`，工作区成员是 `src/atg-*` 通配——**`src/atg-gui` 一建好，CI 就要编 GPUI**（已写进 012 第五节，列为待人定 G2）

- 2026-11-14 · 写 `docs/plan/012_gui-ssot.md`（正本总表 18 行、门槛清单、散落 10 条、G1–G8、等调研空位、要改别人的文件 6 条）
- 2026-11-14 · 写 `specs/gui/README.md` `a11y-tree.md` `keymap.md` `focus.md` `announce.md`（v0 草稿）
- 2026-11-14 · 写 `docs/research/004_gui/README.md`（决策点 G1–G11）
- 2026-11-14 · 收到大总管转告：gcomp 发现 Quill（GPUI 写的 Telegram 客户端），取舍卡要列「借鉴哪些 / 不借鉴哪些」→ G9

## 要人定的事（草稿，第二段收进 decision.md）

正本：`docs/research/004_gui/decision.md`（人要定：G10 G5 G6 G12 G13 G14 G15 G11 G9；AI 已定可推翻：G1 G2 G3 G4 G7 G8）。

## 流水（续）

- 2026-11-14 · 收到 ggpui 抄送（CustomAction / CustomRotor 都走不通，次要操作以快捷键为正本）；收到大总管更正：本批大总管是 `telegram-accessibility-98`，common.md 第 15 条人定「GPUI 缺口直接自己打补丁再提交上游」
- 2026-11-14 · 第二段开始：读 `findings-gpui.md`（第 1、2 节）、`findings-a11y.md`（第一至三、五、六节）
- 2026-11-14 · 写 `specs/gui/upstream-patches.md`（补丁账格式 + 候选 P1–P9）；012 加第 19 行「上游补丁清单」、G12（intent 措辞改写，待人定）
- 2026-11-14 · 并进调研：`a11y-tree.md`（根节点无标题、窗口名走 NSWindow 标题、地标、输入框 / 密码 / 搜索 / 确认框角色、同一字段一处设、名称以可见文字开头、获焦无副作用）；`keymap.md`（单字母另有修饰键入口 + 菜单项、组字不截键、保留键补全、Slack 式 Opt+Shift+↓；「上一个未读」CLI 没有 → 先不绑）；`focus.md`（确认框焦点落取消、关框回触发处、获焦无副作用、公告错开焦点移动）；`announce.md`（入口带类别、同类只留最新、流式中间态不播、发送失败高优先级、详略设置）
- 2026-11-14 · 读 findings-components（第一至八、十节初稿）、findings-gpui 第 3–8 节、checklist-candidate
- 2026-11-14 · 收大总管转人定 B19-2～B19-5（`docs/plan/013_decisions-b19-gui.md`）：改 keymap（取向像 Slack；快速导航人自己关、不为它绕路；修饰键副本不强制、菜单栏项按「agent / 语音控制」理由保留）、focus（B19-2）、组件清单（先用现成开源组件）
- 2026-11-14 · 补丁编号与 ggpui 对齐：P10 焦点落窗口 #66691 · P11 缓存视图 #66690 · P12 选区通知 · P13 #66730 · P14 命中测试 · P15 转子（实验）；已回 ggpui
- 2026-11-14 · 写 `specs/gui/checklist.md`（候选收进，改了 K1 K7 K13 K16 A7 A14 S6 S15 S16 S17 G7）；写 `docs/research/004_gui/decision.md`；012 第六节改为指向 decision.md（不留副本）
- 2026-11-14 · 大总管回：不等 gcomp，先自审。atg-review 已起（run wf_75205a08-b7b；范围 specs/gui/**（组件清单除外）+ 012 + decision.md G1–G8、G11–G15）
- 2026-11-14 · atg-review 第 1 轮（3 视角）：P1 × 8、P2 × 19、P3 × 11。全部 P1 已修：① 编号空间——README 加「编号空间」「源码路径写法」「术语」三节，规约里引别处编号一律带前缀（003 F1、清单 F6、卡 G5、P10）；② ③ 清单按 ga11y 最新候选重收（编号、前缀、判法与候选一致，调整表重写）；④ 同 ①；⑤ keymap 规矩 5 加例外类、`open_link` / `login_resend_code` 补键、删掉没有动作的 `alt-down/up`、加 `insert_newline`；⑥ 加规矩 9（全局键带 Cmd、不占文本编辑键族），撤 `alt-shift-down` 改 `n` + `cmd-j`；⑦ 012 的 G2 状态与取舍卡一致；⑧ D 键详情的交付写死：`announce.md` 来源 `detail`（直发、高档）+ `focus.md` `detail_read`（焦点不动）+ 再按收起只动 `revealed`。P2 大部分顺手修（视图枚举正本、八张表、补丁表格式、焦点/路由一行一个、档位对值表、例子、过时 ⏳、会话名移出规约）
- 2026-11-14 · 收大总管转人定 B19-6～B19-12 → decision.md 重写（已定的标「已定 → 013」，卡片补齐格式）；规约照改：D 合一、未读前缀、详略开关 MVP、组件清单按 Zed ui + 自写（B19-10/11）；收 gcomp 两条更正（Tab/回车带字符不先给输入法；输入框照 `settings_ui/.../input_field.rs` 写）
- 2026-11-14 · 读 findings-components 第九节（Quill）→ decision.md G9 重写（借鉴 10 条、不借鉴 5 条）；Quill 的坑落进规约：keymap 规矩 10（不 Cmd/Ctrl 双绑）、11（处理不了的键放行）；focus 规矩 6（焦点改动推到下一帧）、7（重开会话一律落 F2 落点，不恢复滚动）
- 2026-11-14 · Cmd-J 核：Slack 的 Cmd-J = 本会话第一条未读 → 本项目照用（新动作 `jump_first_unread`，CLI 对应 `history` 默认落点）；下一个未读会话的全局键改 `cmd-shift-u`。Cocoa「跳到所选内容」Cmd-J 是菜单项，我们菜单由动作表生成、不带它；Zed `ui` 只有一处测试里的 `bind_keys`（`context_menu.rs:3413`），不撞（读源码）
- 2026-11-14 · 收 ga11y 更新的候选（A7/A22/A23/A26、K18/K19）→ 清单改用脚本重收（scratchpad `gen_checklist.py`：只填〔待定〕、四条整格改写 S23 K12 K13 F16，其余逐字照候选），以后候选再变直接重跑
- 2026-11-14 · atg-review 第 2 轮：P1 × 6（实为 3 类，三视角重复）——① keymap 规矩 9 与 global `tab` 矛盾 → 规矩 9 写明 Tab / Shift+Tab 唯一例外，清单 K9 同改；② 012 第五节第 1 条句尾残留「待人定 G2」→ 删；③ 清单与候选不一致（审的是重收前的快照）→ 按 ga11y 最新候选最后一次重收，此后正本只改 `specs/gui/checklist.md`。P2/P3 顺手修：规矩 1 写明「非全局一套 + 最多一个全局 Cmd 键」；`when` 限可打印单键、`list_focused` 含阅读视图；搜索一路补键与焦点规则（`open_search_hit`、search ↓/Esc、`search_opened/closed`）；help Esc；UiState 加 `return_to` 栈和两个锚点；`revealed` → 每条 `SpeechCtx.reveal` 的换算；D 键「没展开 / 已展开」两态写清、叠读风险 ⏳；出声时机常数 `FOCUS_SETTLE_MS` 500 / `SEARCH_COUNT_DEBOUNCE_MS` 400 收进 announce 第四节一处（012 白名单同步）；012 能力动作比对也认 core 公开函数；012 组件与清单的过时说法；a11y-tree 加链接列表、帮助页两行，例子落点改到第一条未读；编号空间补 008、调研 003；P8、P9 引用修正
- 2026-11-14 · 发现别人的文件过时（只报不改）：`docs/plan/013_decisions-b19-gui.md` B19-1「落到哪」仍写 intent 改写「待人定」，B19-9 已改
- 2026-11-14 · 加 focus.md 第三节「外部无障碍动作」（大总管转 ggpui）：外部 Focus 暂按 GPUI 默认；调试日志记时间、动作名、目标标识，不记内容；新卡 G16（真碰到再问）
- 2026-11-14 · atg-review 第 3 轮：P1 × 4（搜索焦点模型三处互斥 ×3；macOS 层导出格式缺字段）。修：搜索统一成 combobox（焦点留搜索框、↑↓ 移高亮、`search_results` 视图删掉；`cmd-f` 搜消息 / `cmd-k` 搜会话；会话命中标识 `chat-hit:`）；macOS 层格式补子角色、值、可聚焦、禁用、必填、模态、动作、linked，frame 现场比不进快照。P2/P3 修：确认框推广到 5 种（`confirm_opened`）、`nothing_here` 来源、`peer_typing` 改名、UiState 加 search / links / help_anchor / last_focus / 跨运行的 last_chat、Tab 一圈去掉搜索、日期分隔 Heading 行、`state` 字段与第五节字段推导规则、生成器改 `atg-gui-spec` 自带（不编 GPUI）、组件清单 TreeDump 与无头观察器分开、批大小改 8 ms 时间预算、KeyMonitor Shift+Tab 任何时候截、Zed ui rev 正本位置、术语补全、G9 补三条、卡按号排
- 2026-11-14 · 收 ga11y 第 7 轮：F8 验收改走假后端、正式服务器只留可选人判；F8 / A18 / AG9 补 005 N7；K9 编辑键族以 StandardKeyBinding.dict 脚本解析为准；AG4 未读数在名称里；S22 出处；AG7 因果
- 2026-11-14 · 大总管问「开链接 / 附件确认有没有依据」→ 有：`actions.md:25`、`:97`，`cli.md:35`；保留，不开 G17（大总管同意）。S24 照 003 读法原则改（回复、转发、已编辑、回应只在详情）
- 2026-11-14 · atg-review 第 4 轮：P1 × 9（合并后 5 件）——① combobox 在 GPUI 上做不出（active descendant 只认子孙、且顶替焦点，`Z:gpui/src/window/a11y.rs:258-271`）→ 开卡 G17，规约按默认 A 写（↓ 进结果 `NavList`、Esc / 打字回搜索框），恢复 `search_results` 视图；② KeyMonitor 与搜索框 ↑↓ / Esc 矛盾 → 收文字视图里只截表里明文绑的键、组字放行；③ macOS 导出格式缺两处焦点、通知记录、选区 → 补；④ dialog 的 `when` 推不出 → 推导加 dialog 档，谓词 = 视图上下文 ∧ when；⑤ 确认之后的焦点只有发送 → `confirmed` 按种类、加 `error_dialog_opened`。P2：bypass_ime 逐键推、UiState 字符串例外白名单、日志标识拆种类 + 数字、跳跃用 `scroll_to(ListOffset)`（exp009）、规矩 1 指向外部动作、卡号 G1–G17、子状态来源、术语补 combobox / ListBox / controls
- 2026-11-14 · 收 ggpui 滚动更正（P5 退路同步）、ga11y S24 / S25（以 `specs/style/message.md` 为准、「未播放」进详情）
