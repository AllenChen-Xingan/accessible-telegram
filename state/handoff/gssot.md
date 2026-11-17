# 现状页 · gssot（图形界面的唯一真相源）

## b20 接手（2026-11-16）

- **手上**：`_tmp/20261116-b20/gssot.md`（MAIN）——① 按 Touch ID 删除改 GUI 规约（specs/gui/**、012、decision.md：确认步骤 → 发送前读一遍、直接发、不弹框）；② 把待人拍板的卡（G9 G11 G15 G16 G17）整理成能直接问人的样子
- **做到哪**：已报到大总管（旧进度、未提交改动归属、合 main 预计冲突只在 docs/reviews/README.md 且应能自动合）；等大总管合 main
- **下一步**：合完后改约 40 处确认相关文字 → 整理卡 → atg-review → codex 040
- **不动**：.42cog/intent.md 改动、docs/plan/013、exp009（ggpui）、exp010（gcomp）、docs/reviews/042/043、别人的现状页
- **要人定的事**：见 decision.md（整理中）

### b20 流水

- 2026-11-16 · 读 common、交代、旧现状页；只读看 git status / log / diff；报到大总管

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
