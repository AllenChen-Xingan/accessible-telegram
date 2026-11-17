# 按键表 v0 · 界面动作 · 保留键

> 2026-11-14 · AI 起草 · 状态：**草稿 v0**（三份调研已并进，2026-11-14）。⏳ = 要实测、要人听，或在等组件重评
> 机读正本（移交后）：按键表 `keymap`、保留键表 `reserved`、动作枚举 `GuiAction`（`docs/plan/012_gui-ssot.md` 第二节第 7–9 行）。帮助页、菜单、`atg-gui --schema` 的 `keys` / `actions` 段、读法里的按键提示都从它们生成

## 一、十一条规矩

1. **同一个操作在所有界面用同一个键**（`.42cog/real.md`「使用者现实」：快捷键要少而一致）。单测查：一个动作在所有非 `global` 视图里绑的键必须相同；**另外最多再有一个 `global` 的 Cmd 键**（如 `next_unread_chat` = 列表里 `n` + 任何焦点 `cmd-shift-u`）——这是 B19-4「单键或 Cmd + 键」的标准组合，不算两套
2. **同一个视图里一个键只做一件事**，单测查
3. **不撞保留键**（第四节）。确实要撞的，表里写明理由和出处，单测只放过写了理由的
4. **单字母键只在列表、阅读视图这类「不收文字」的视图里绑**；输入框、搜索框、登录框里一律不绑单字母（字母要能打出来）
5. **每个由人发起的能力类动作至少有一个键。** agent 在 macOS 层只有「按按钮、设焦点、发按键」三种操作（`a11y-tree.md` 第三节表注），没有键的动作 agent 就做不到。例外只有两类、在第三节「类」一列标明：由列表或事件**自动触发**的（`load_older`）和**入口策略**类（`mark_read_on_open` 等）
6. **读法里提到的键从表里取**：`SpeechCtx.listen_key`（「按 M 收听」）、`expand_key`（「按 D 展开」）（`src/atg-core/src/speech.rs:37-40`）由界面按表查出后传入，不写字面量；元素的 `aria_keyshortcuts`（`gpui/src/elements/div.rs:1398`）也照表设（macOS 层目前不送，`upstream-patches.md` P4）
7. **取向像 Slack（人定 B19-4，`docs/plan/013_decisions-b19-gui.md`）：首选单键（只在列表获焦、不在输入框、不在组字时生效）或 Cmd + 键**；Ctrl+Option（VO 键）组合允许用，但不作首选。VoiceOver 快速导航由人用本应用时自己关掉（人定 B19-3），按键设计**不为它绕路**；帮助页第一句写使用前提「使用本应用时请关闭 VoiceOver 快速导航（VO+Q、VO+Shift+Q）」。每个能力类动作另有一个**菜单栏项**——不是为快速导航，是给语音控制、切换控制和 agent 一条不靠按键的路（补自定义动作缺位，`upstream-patches.md` P1）
8. **输入法正在组字（有标记文本）时一律不截键**：回车先判有没有标记文本再决定发不发（中文输入法回车上屏的经典事故，`findings-a11y.md` 第三节第 5 小节）；列表获焦时截方向键前也先判
9. **全局键（`view = global`）一律带 Cmd，且不在文本编辑键族里。** 编辑键族以 AppKit `StandardKeyBinding.dict` 的解析结果为准，由脚本生成、不手抄（清单 K9）。**唯一例外是 `tab` / `shift-tab`**：区块间移动，系统惯例（`findings-a11y.md` 第三节第 2 小节，exp003b-v2 实测）；它们同时豁免「带 Cmd」和规矩 1 的「全局键必须是 Cmd 键」两条，单测白名单只放这两个。输入框获焦时全局键也生效，撞了编辑键人就没法按段移动、扩选（清单 K9；无障碍调研核：Opt+Shift+↑/↓ 是按段扩选）
10. **不 Cmd / Ctrl 双绑**：一个动作不同时绑 Cmd+X 和 Ctrl+X——Ctrl 那份会抢走输入框里的 Emacs 式编辑键（Quill 踩过，`findings-components.md` 第三、九节）
11. **处理不了的键要放行**（GPUI 里 `propagate`）：动作处理函数默认会把键吃掉，碰到不该自己管的情况（比如输入框为空时的回车）要交还，不然输入框、读屏收不到（同上）

## 二、表的格式（一行一条绑定）

| 字段 | 类型 | 说明 |
|---|---|---|
| `action` | `GuiAction` | 第三节的一个变体 |
| `keys` | 键串（GPUI 的写法，如 `cmd-down`、`shift-tab`、`d`） | 一条绑定可有几个等价键；⏳ 写法以 `findings-components.md` 核的 GPUI 版本为准 |
| `view` | `global` · `chat_list` · `message_list` · `search_results` · `link_list` · `composer` · `search` · `reading` · `dialog` · `login` · `help` | 作用范围。**这一行是视图取值的正本**：`UiState.region`、焦点规则表的「区」都用这一套（去掉 `global`）。`global` 在所有视图生效（规矩 9 管它的写法） |
| `help` | `TextId` | 帮助页里这一条的说明，取自文案表 |
| `when` | `list_focused`（不收文字的视图获焦、不在组字）· `always`（任何焦点，含输入框）· `text_only`（只在输入框获焦时） | 生效条件。单字母 / 可打印单键一律 `list_focused`（第一节第 7、8 条）；「列表获焦」指焦点在**不收文字的视图**：`NavList` 系列或阅读视图（规矩 4）。表里只写这三个取值之一；生成 GPUI 绑定时由生成器换成上下文谓词（如 `list_focused` → `(NavList || ReadingView) && !TextInput`，`findings-components.md` 第四节第 4 小节），谓词不手写进表 |
| `bypass_ime` | 真 / 假 | **无字符的键**——方向键、Esc、PageUp / PageDown、Home / End、Shift+Tab——GPUI 总是先交给输入法（`gpui_macos/src/events.rs:381`；001 R3），要 `KeyMonitor` 截，标「真」。Tab、回车在 macOS 上带字符（`\t`、`\n`，`events.rs:371-378`），不在组字时先走按键匹配，标「假」；Cmd + 键从不经过输入法；单个可打印键只在有获焦输入框且开着中日韩输入法时才先给输入法（组件调研更正，`findings-components.md` 第四节；读源码）。⏳ 001 第二节第 1 小节「Tab / Shift+Tab 任何时候都截」：对 Tab 可能多余、对 Shift+Tab 仍需，待实测复核 |
| `state` | 空，或子状态 | 只在某个子状态下生效，如 `playing`（播放中，读 `atg-audio` 播放器的状态）、`auth_code`（登录的验证码这一步，读 core 的 `auth_state`）。子状态不进 `UiState`，从各自的正本现读 |
| `reserved_ok` | 空，或一句理由 + 出处 | 撞保留键时必填 |
| `source` | 出处 | 实验编号、拍板号或「草案」 |

## 三、界面动作 `GuiAction` v0

对外等级一列的 L0 / L1 / L1 外部 / L2 见 `specs/contracts/actions.md` 第一节第 2 小节（L2 = 要过发送闸；闸直接开 `direct_mode`（测试服务器收藏夹为 `test_exempt`）凭据，界面不弹确认框，人定 2026-11-16 删除 Touch ID，#113；`actions.md` 第一节第 5 小节）。

「类」：**能力** = 要 `atg-core` 做事，必须点名下面一列（`check-gui-sources.py` 比对它在 `specs/contracts/actions.md` / `src/atg-cli/src/args.rs` 里真有）；**界面** = 只动 `UiState`；**入口策略** = core 有这个能力、CLI 刻意不调（003 F1、003 A2，`actions.md` 第一节第 2 小节），不需要按键，由事件触发。

| 动作 | 类 | core 动作 / Store 查询 · CLI 子命令 | 对外等级 |
|---|---|---|---|
| `focus_next` · `focus_prev` | 界面 | — | — |
| `list_up` · `list_down` · `list_home` · `list_end` · `list_page_up` · `list_page_down` | 界面 | —（只动选中锚点；靠近已加载的边缘时由列表触发 `history` 加载更早的，那一下算能力，见下行） | — |
| `load_older` | 能力 · **自动触发**（列表靠近已加载的边缘时，不绑键；规矩 5 例外） | `history` · `atg-cli history --before` | L0 |
| `open_chat` | 能力 | `chat_open` + `history`（003 F2 落点）· `atg-cli history` | L0 |
| `mark_read_on_open`（进会话自动） | 入口策略 | `mark_read`（003 F1：GUI 进会话标到最新一条）· CLI 不调 | L1 |
| `set_online`（界面开着自动） | 入口策略 | `set_online`（003 A2）· CLI 不调 | L1 |
| `typing`（输入框打字自动） | 入口策略 | `typing`（003 A2）· CLI 不调 | L1 |
| `back`（回上一层） | 界面 | — | — |
| `jump_first_unread` | 能力 | 投影的落点（003 F2 的「第一条未读」，由 store 的已读位置算）· `atg-cli history CHAT_ID`（不给 `--before` / `--after` 时落在第一条未读，008 H1） | L0 |
| `next_unread_chat` | 能力 | Store 查询 `next_unread_chat`（`src/atg-core/src/store/mod.rs:431`）· `atg-cli next-unread` | L0 |
| `listen_new`（按 M 收听） | 能力 | `notify` 的待播汇总 · `atg-cli watch`（同一份判定） | L0 |
| `read_detail`（D） | 能力 | `speech::detail` + `replied_message_get` · `atg-cli show`。**一键合一（B19-6）**，按「这条现在展开没有」分两种：**没展开**（不在 `UiState.revealed` 里）= 说一遍详情（`announce.md` 来源 `detail`），这条有隐藏内容（剧透、18+ 遮挡）时同时展开——加进 `revealed`，名称随之出原文；**已展开** = 收起（移出 `revealed`），不说话。没有隐藏内容的消息永远不进 `revealed`，所以每按一次都再说一遍详情。焦点不动（`focus.md` `detail_read`）。想反复看就按回车开阅读视图 | L0 |
| （没有单独的 `reveal_hidden`） | — | 展开隐藏内容并进 `read_detail`（B19-6）；`SpeechCtx.expand_key` = `read_detail` 的键 | — |
| `open_reading_view` | 界面 | `speech::full` · `atg-cli history --full` | — |
| `list_links` | 能力 | `message_links` · `atg-cli links`；打开一个 `NavList`（视图 `link_list`），每项是一个链接 | L0 |
| `open_link` | 能力 | `open_link` · `atg-cli open --link`（在 `link_list` 里对选中的链接） | L1 外部 |
| `open_attachment` | 能力 | `file_download` + `open_file` · `atg-cli download` / `open --file` | L1 外部 |
| `play_pause` · `seek_back` · `seek_forward` · `speed_up` · `speed_down` | 能力 | `atg-audio` 播放 · `atg-cli play`（标准输入的 `pause` / `resume` / `seek` / `speed`） | L0；播放即标已听是入口策略（L1，`mark_listened`） |
| `mute_chat` | 能力 | `mute` · `atg-cli mute` | L0 |
| `focus_composer` · `reply` | 界面 | 只设焦点和回复目标 | — |
| `insert_newline` | 界面 | 输入框里换行（文本编辑本身，不经 core） | — |
| `send` | 能力 | `send_text_prepare` → 过闸 → `send_text`，不弹框；返回成功读一遍计划（`announce.md` `send_plan`）、返回错误播 `send_refused`；按下到返回之间在途，回车、Cmd+回车被吃掉（`focus.md` `send_submitted`） · `atg-cli send` | L2 |
| `send_again` | 能力 | 同 `send`，但这一次绕过重复提交拦截（对应 `atg-cli send --again`，契约「只这一次」）。**只在**上一次 `send_refused_now` 的原因是 `duplicate_recent`、且它报的上次结果是「已发出」（上次是「未知 / 已提交」不武装：那条可能已经发出）、之后草稿与回复目标一个字没改、键盘焦点没离开这个输入框（切走应用再回来不算离开；VO 光标拖走键盘焦点算离开）、且不在在途态时有效（`UiState.send_again_armed`）；任一条件不成立即失效，失效时这个键不截 | L2 |
| `resend` | 能力 | `resend_prepare` → 过闸 → `resend`；返回成功读一遍计划（`send_plan`）、返回错误播 `send_refused`，不弹框 · `atg-cli resend` | L2 |
| `delete_for_all` | 能力 | `delete_prepare` → 过闸 → `delete`；返回成功读一遍计划（`send_plan`）、返回错误播 `send_refused`，不弹框 · `atg-cli delete` | L2 |
| `search_messages` · `search_chats` | 能力 | 打开搜索框（`UiState.search.mode` = 消息 / 会话）并随输入查：`search_messages`（当前会话里）/ `search_chats` · `atg-cli search messages` / `search chats` | L0 |
| `search_enter_results` · `search_back_to_input` | 界面 | 卡 G17 人选 A：搜索框里 ↓ 把焦点移进结果列表（`NavList`，选中第一项）；结果列表里 Esc 或打可打印字符回到搜索框（字符接着输进去）。**不借用 `focus_next`**（那是 Tab 一圈） | — |
| `open_search_hit` | 能力 | 消息命中：`chat_open` + `history` 落在那条（003 F2：搜索结果进来落在那条）· `atg-cli history CHAT_ID --after MSG_ID`；会话命中：同 `open_chat` | L0 |
| `confirm` · `cancel` | 界面 | 确认框（只有打开链接 / 附件）的两个按钮；确认后才调 `open_link` / `open_file` | — |
| `login_submit` · `login_resend_code` | 能力 | `auth_submit_*` · `auth_resend_code` · `atg-cli auth …` | L0 |
| `show_help` | 界面 | — | — |
| `copy_text` | 界面 | `tree::text::plain_text`（系统剪贴板只在本机） | — |

## 四、保留键（不许绑，或绑之前写理由）

| 键 | 谁占着 | 出处 / 可信度 |
|---|---|---|
| **Control + Option**（VO 键）+ VoiceOver 自己的命令键（方向、空格、Shift+↓/↑、U、J、F、A、Cmd+H/J/L/T/G/X……）；设成 VO 键的 Caps Lock；「只按 Option 当 VO」开着时的 Option | VoiceOver | VO 命令被 VoiceOver 拦下、到不了应用（exp004 结论 4，实测）；清单文档说（`findings-a11y.md` 第三节第 1 小节）。**人定 B19-4：Ctrl+Option 组合允许用、不作首选**——只能用 VO 不占的那些，且每条要实测能到应用（⏳ 未测：VO 会不会把它不认识的 VO 组合也吞掉） |
| **单独按 Control** | VoiceOver：停止朗读。使用者用它打断长内容（`.42cog/real.md`「使用者现实」） | 文档说 + 使用者习惯 |
| Cmd+Tab、Cmd+Space、Cmd+Q / W / H / M / , / Z / X / C / V / A / G / \` / ?、Cmd+F5、Opt+Cmd+F5 | 系统与编辑标准（切应用、聚焦搜索、退出、关窗、隐藏、最小化、设置、撤销 / 剪切 / 复制 / 粘贴 / 全选、开关 VoiceOver、辅助功能快捷面板） | 文档说（`findings-a11y.md` 第三节第 1 小节）。Cmd+F 是标准「查找」，本项目照标准用作搜索，不算撞 |
| Ctrl+Space、Caps Lock | 输入法切换 | 文档说 |
| Ctrl+F1…F8 · Fn / 🌐 组合 · Tab+H（全键盘访问开着时） | 键盘导航、表情面板、全键盘访问帮助 | 文档说 |
| 不带修饰键的方向键、Home / End / PageUp / PageDown | 先交给输入法（`gpui_macos/src/window.rs:2633-2770`）；只在列表有焦点时由 `KeyMonitor` 截下（exp003b-v2） | 读源码 + 实测（U.S. 输入法）；双拼未测（`001` 第五节 R3） |
| 单字母键（单键快速导航开着时，VO+Q 切换）；单独方向键（方向键快速导航开着时，VO+Shift+Q 切换） | VoiceOver 吃掉：b 按钮、h 标题、l 链接、t 表格、1–6 标题级别……；方向键 | 文档说（`findings-a11y.md` 第三节第 1 小节）。**人定 B19-3：用本应用时人自己关掉快速导航，不算冲突**；帮助页写明前提 |

**和 Zed `ui` 撞不撞**（B19-10 用它当底座）：`Z:ui/src` 里只有 `components/context_menu.rs:3413` 一处 `bind_keys`（`down` 绑在 `menu` 上下文，在测试里），没有别的全局键；Zed 的快捷键都在应用自己的 keymap 文件里，我们不加载它，所以本表的键不和 Zed `ui` 撞（读源码，2026-11-14）。用到 Zed `ui` 的菜单类组件时，它要的 `menu::SelectNext` 等动作由本表绑。

## 五、初版条目

> 2026-11-16 之前标「草案」的行经卡 G11 人选 B 照用，只改了删除键（`cmd-shift-backspace`）；之后新加的草案行（如 `send_again`）还没经人看；实用后要改再开卡。

本表只写 `view` · `keys` · `action` · `source`，其余字段按下面的规则推出，生成器照推、单测照核；推出来不对的行在「出处」里写明覆盖值：`when` = `global` → `always`；`composer` / `search` / `login` 视图里的键 → `text_only`；`dialog` 视图里的键 → `always`（对话框不是列表，焦点在按钮上）；其余（列表、阅读视图）→ `list_focused`。**生成的谓词 = 视图自己的上下文（如 `Dialog`、`MessageList`）与 `when` 条件相与**，所以 `always` 只表示「这个视图里任何焦点都生效」，不越出视图。`bypass_ime` **按单个键推**（一行里几个键可以不同，生成器逐键存）：不带 Cmd 的无字符键（方向、Esc、Home / End、PageUp / PageDown、Shift+Tab）→ 真；带 Cmd 的（如 `cmd-up`）和带字符的（Tab、回车、字母）→ 假。`help` = 文案表里与动作同名的那条。`state` 写在视图格的括号里（如「`message_list`（`playing`）」）。

| 视图 | 键 | 动作 | 出处 |
|---|---|---|---|
| global | `tab` · `shift-tab` | `focus_next` · `focus_prev` | 已测：exp003b-v2 `selftest-tab` 45 步 |
| 列表类（`chat_list` `message_list` `search_results` `link_list` `help`） | `up` · `down` · `home` · `end` · `pageup` · `pagedown` · `cmd-up` · `cmd-down` | `list_*`（`cmd-up` = `home`、`cmd-down` = `end`） | 已测：exp003 1137 步、exp003b-v2 |
| `chat_list` | `enter` | `open_chat` | 草案 |
| `chat_list` · `message_list` | `n` | `next_unread_chat`（T09；到底时说「没有更多未读」，003 A3） | 草案（B19-4 首选单键） |
| global | `cmd-shift-u` | `next_unread_chat`（任何焦点下，含输入框） | 草案。原草案 Slack 的 `alt-shift-down` 撤掉：Opt+Shift+↓ 是按段扩选（规矩 9、清单 K9）。也不用 `cmd-j`：Slack 的 Cmd-J 是「本会话第一条未读」（`findings-a11y.md` 第四节表），照 Slack 留给下一行；Cocoa 应用「编辑 › 查找 › 跳到所选内容」的 Cmd-J 是菜单项，本应用的菜单栏由动作表生成、不带这一项，所以不撞。「上一个未读」先不绑：core / CLI 没有（`docs/plan/012_gui-ssot.md` 第九节） |
| global | `cmd-j` | `jump_first_unread`（当前会话的第一条未读；没有未读就最新一条） | 草案；Slack 同键同义（`findings-a11y.md` 第四节表） |
| `chat_list` · `message_list` | `m` | `listen_new` | 001 拍板 3（「按 m 主动收听」）；exp004 |
| `chat_list` | `shift-m` | `mute_chat`（M3.3「一个键静音」） | 草案；和 `m` 分开是因为 `m` 已给收听 |
| `message_list` | `d` | `read_detail`（读详情，有隐藏内容同时展开；再按收起） | 003 R1「按 D 展开」；001 拍板 3 B「按 D 听详情」；**B19-6 人定合一** |
| `message_list` | `enter` | `open_reading_view` | 草案（M3.5） |
| `message_list` | `r` | `reply` | 草案 |
| `message_list` | `i` | `focus_composer`（T04「一个快捷键进输入框」） | 草案 |
| global | `cmd-l` | `focus_composer`（任何焦点下进当前会话的输入框，清单 F16） | 草案 |
| `message_list` | `o` | `open_attachment` | 草案（M3.7） |
| `message_list` | `space` | `play_pause` | 草案（M3.8） |
| `message_list`（`playing`） | `left` · `right` · `[` · `]` | `seek_back` · `seek_forward` · `speed_down` · `speed_up` | 草案 ⏳ ←→ 不在播放时留给什么 |
| `message_list` | `cmd-shift-backspace` | `delete_for_all`（读一遍删哪条、给所有人，直接删，不弹框，#113） | 卡 G11 人选 B（2026-11-16）：不再有确认框，改用比访达「移到废纸篓」多一个 Shift 的键，防顺手误按。`backspace` 就是 Mac 键盘上印着 Delete 的那个键（不是向前删除 fn+Delete）；M4.6 |
| `message_list` | `shift-r` | `resend`（只对发送失败的那条；读一遍、直接重发，不弹框） | 草案 |
| `message_list` · `reading` | `escape` | `back` | 草案（M3.5「Esc 回原处」） |
| `reading` · `message_list` | `l` | `list_links` | 草案（M3.5） |
| `link_list` | `enter` | `open_link`（先过 `open_link` 的确认，`actions.md`） | 草案 |
| `link_list` | `escape` | `back`（回阅读视图或消息列表） | 草案 |
| `reading` | `cmd-shift-c` | `copy_text`（复制全文；`cmd-c` 留给复制选中的那段） | 草案 |
| `composer` | `enter` · `shift-enter` | `send` · `insert_newline` | T04；组字时回车不截（规矩 8）；在途态里回车吃掉、不发（`focus.md` `send_submitted`） |
| `composer` | `cmd-enter` | `send_again`（Cmd+回车，回车即 Mac 键盘的 Return 键；有效条件见动作表 `send_again`，无效时不截、交给输入框，输入框对它没有动作） | 草案（AI 起，自审 b20 第 2 轮；契约 `send --again`） |
| `composer` | `escape` | `back`（回消息列表，草稿保留） | 草案 |
| global | `cmd-f` | `search_messages`（在当前会话里搜消息；焦点进搜索框） | 草案（T08）；macOS 标准「查找」 |
| global | `cmd-k` | `search_chats`（搜会话、快速切换） | 草案（T08）；Slack 同键 |
| `search` | `down` | `search_enter_results`（有结果时；没结果放行） | 清单 K14；卡 G17 |
| `search` | `escape` | `back`（关搜索，回打开前的元素） | 清单 K14 |
| `search_results` | `enter` | `open_search_hit` | 清单 K14；M4.5 |
| `search_results` | `escape` | `search_back_to_input` | 清单 K14；卡 G17 |
| `help` | `escape` | `back` | 草案 |
| global | `cmd-/` | `show_help` | 草案；Slack 同键 |
| `dialog` | `escape` | `cancel` | 草案；确认要在按钮上按（Space / 回车按当前焦点的按钮），见 `focus.md` |
| `login` | `enter` | `login_submit` | T01 |
| `login`（`auth_code`） | `cmd-r` | `login_resend_code` | 草案 |

单字母键不再要求另配修饰键入口（B19-3、B19-4）；菜单栏项随动作表生成（第六节）。

## 六、生成物

- **GPUI 按键绑定**：启动时逐行 `App::build_action(名字)` + `KeyBinding::load(键串, 动作, 上下文谓词)` + `cx.bind_keys`（`zed:gpui/src/app.rs:2793`、`keymap/binding.rs:51`；`findings-components.md` 第四节）。名字拼错、键串解析不了，启动即报错
- **双向比对**（单测，在 `atg-gui` 里，要编 GPUI）：表里每个动作名都在 `all_action_names()` 里；代码里 `actions!` 声明的每个动作都在表里
- **`aria_keyshortcuts` 和提示语里的键**：用 `window.bindings_for_action` 现查，不手写（Zed 的右键菜单就这么做，`zed:ui/src/components/context_menu.rs:2313`）
- **菜单栏**：每个能力类动作一项，标出键（第一节第 7 条）
- **帮助页**：第一项是使用前提（关快速导航，B19-3）；其余是一个 `NavList`，每项读「<键的读法>：<help 文案>」，按视图分组
- **自描述的 `keys` 段**：`[{action, keys, view, when, state, help}]`，给 agent
- **读法按键**：`listen_key` = 表里 `listen_new` 的键；`expand_key` = `read_detail` 的键（B19-6）
- **本文第五节**：移交后由表生成，金样比对
