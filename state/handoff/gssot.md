# 现状页 · gssot（图形界面的唯一真相源）

## b20 接手（2026-11-16）

- **手上**：`_tmp/20261116-b20/gssot.md`（MAIN）——① 按 Touch ID 删除改 GUI 规约（specs/gui/**、012、decision.md：确认步骤 → 发送前读一遍、直接发、不弹框）；② 把待人拍板的卡（G9 G11 G15 G16 G17）整理成能直接问人的样子
- **做到哪**：大总管已提交 304e9ae 并合 main（53d4f8c）。① Touch ID 删除已改完（7 个文件：README、keymap、a11y-tree、focus、announce、checklist、012；新增播报来源 `send_plan`、焦点触发 `send_submitted` / `delete_submitted` / `resend_submitted`；确认框只剩打开链接 / 附件）② 五张卡已整理成「问人」一段（decision.md 第一节；G11 新增一问：删除不弹框后改 Cmd+Shift+Delete）
- **下一步**：自审到连续两轮无 P0/P1 → codex 040（范围 `53d4f8c` 起）→ 报大总管提交；`send_again` 键（Cmd+回车）待下次问人
- **不动**：.42cog/intent.md 改动、docs/plan/013、exp009（ggpui）、exp010（gcomp）、docs/reviews/042/043、别人的现状页
- **要人定的事**：`docs/research/004_gui/decision.md` 第一节 G9 G11 G15 G16 G17（已报大总管去问）
- **报给大总管的规约冲突（别人的文件，只报不改）**：`actions.md` 第一节第 5 小节仍写「GUI 窗口标题前加『直发模式』，启动时播报『直发模式已开启，发送不再确认』」——直发成唯一方式后这句没意义，GUI 规约没照写；`002_roadmap.md` M4.6 仍写「过确认（同 M4.2）」，GUI 规约按 D1 / #113 写成不弹框

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
