# 004_gui · GPUI 应用架构与组件先例（gcomp）

> 2026-11-14 · 手下 gcomp · 只读源码、未编译。
> 材料（详见 `sources.md` 的 `## gcomp`）：Zed `resources/zed`（79fe6a6，2026-10-23）与 `resources/zed-20261114`（15d136b）；gpui-component / gpui-kit `resources/gpui-component-20261114`（4379eeb）；同类先例 Quill `resources/quill-20261114`（d4a18b7）。四份 `resources/` 都在 MAIN 工作区，只读、不进库。
> 路径写法：`zed:` = `resources/zed/crates/`（新版行号不同时另注 `zed-new:`）；`gc:` = `resources/gpui-component-20261114/crates/`；`quill:` = `resources/quill-20261114/`。
> 节的顺序：第二节（组件）、第四节（按键）放在最前，因为它们是正本方案等着用的两条；其余按交代里的题号 1、3、5–8 排，第九到十一节是附加的（Quill 摘要、建议、要人定的事）。
> 简称：「001」= `docs/plan/001_arch-stack.md`（R1–R14、「第二节第 N 小节」都指它）；「旧核查」= `docs/research/001_stack/findings-gpui.md`；M3.x / M4.x = `docs/plan/002_roadmap.md` 第六、七节的块；T01–T10 = `specs/tasks.md`；F/N/B19-x = 人定代号（`docs/plan/003`、`005`、`013_decisions-*.md`）；exp0xx = `docs/experiments/`；gssot / ggpui / ga11y / gcomp = 本批四个调研手下（分工见同目录 `README.md`）；common.md = MAIN `_tmp/20261114-b19/common.md`；codex 043 = 本文的跨谱系评审记录 `docs/reviews/043_*`。
> 术语：先看 001 的「术语」一节（crate、active descendant、Live 节点、reducer……）。本文另用到的：**fork** = 把别人的仓复制一份自己改；**rev** = 锁定到某一次提交；**`[patch]`** = Cargo 里「把某个依赖整体换成我指定的那份」，替换品必须**同名、版本号满足原要求**，否则 Cargo 不用它；**vendor** = 把依赖的源码拷进自己仓里；**传递闭包** = 一个包连同它依赖的依赖……全部算上；**TextRun** = 无障碍树里「一段文字」的节点，读屏靠它知道字数、光标、逐字内容；**IME 组字 / marked range** = 中文输入法还没上屏的那段拼音；**prepaint / paint** = GPUI 每帧的布局后、绘制两步；**gpui-pre** = 别人把 Zed 某一次提交里的 GPUI 改名发到 crates.io 的快照。
> 可信度：**读源码** / **文档说** / **推断** / **实测**（本批未实测）。
> **状态：交代八条全部已写；按人定 B19-5 重评第 2 条；人已定 H1（B19-10）、H2（B19-11）；自审一轮已修（P1 六条清零）；codex 043 六轮，第 5、6 轮连续无 P0/P1，满足停止线，剩余 P2 已改（`docs/reviews/043_gui-components-codex.md`）。**

## 〇、结论先行

1. **组件库（按 B19-5 重评）：建议「Zed `ui` crate + 自写缺口、缺口尽量补进上游」**，与「补丁打在 Zed 主干 fork、一个底座」同仓同 rev，最顺。`ui` 单独依赖的代价不大：它**已和 Zed 设置体系解耦**（上游提交「ui: Make UI independent from settings crate」#52578；`zed-new:theme/src/theme_settings_provider.rs` 让应用自己提供字体、字号、密度），传递闭包只拖进 19 个 Zed 内部 crate，其中新增的 GPL 部分 7 个约 3.9 万行（`ui`、`theme`、`component`、`icons`、`menu`、`syntax_theme`、`ui_macros`），其余是我们本来就编的 gpui 一家；**不拖 settings、workspace、editor**（脚本算闭包，见第二节第 1 小节，读源码）。它缺三样：**能独立用的文本输入框**（`ui_input` 要 Zed 编辑器注入，编辑器闭包 97 个 crate、约 75 万行；Zed 主干里唯一有输入框无障碍的是 `settings_ui` 给编辑器套的一层包装，见第三节）、**对话框的无障碍**（`Modal` 无 Dialog 角色、无焦点陷阱）、**消息列表与播报**。gpui-kit（Apache-2.0）组件更全、输入框无障碍最好，但钉 crates.io 快照 `gpui-pre =0.3.12`，**与 Zed `ui` 不能组成同一棵组件树**（会是两套 gpui，类型不能互传；Cargo 本身不禁止两套包共存，链接冲突未查——这是工程取舍，不是构建禁令，推断），补丁也要写两遍。**人已定选甲**（B19-10，第十一节 H1）；改许可证本身另开 issue，不归本文
2. **gpui-component 的无障碍比预期深，但恰好在聊天主路径上最弱**：输入框、对话框、菜单、按钮、开关、标签页做得好；消息列表 `MessageScroller` 只有一个 `Role::Log`、消息与气泡没标注、全库没有播报接口、列表不用 active descendant（读源码，见第二节表）
3. **gpui-kit 做得好、我们用得上的四样**（怎么拿——照思路重写 / 带署名移植 / 推给上游——见第十一节 H2）：① 输入框的 TextRun 合成子节点 + 文本选区（让 VoiceOver 能逐字读、知道光标在哪）；② **不依赖 AccessKit 激活的无头测试观察器**——在 prepaint 时自己建一个 accesskit 节点、调元素的 `write_a11y_info` 读回属性，能在 `TestAppContext` 里断言角色和名称，正好补上游测试平台没有无障碍树的缺口（旧核查第 5 条；001 R6）；③ 焦点陷阱与关闭后焦点还原；④ `aria_disabled` 补丁与 macOS 命中测试转发
4. **按键表可以只写一份**：GPUI 的动作能**按名字构造**（`App::build_action(name, params)`，`zed:gpui/src/app.rs:2793`），按键绑定能**从字符串加载**（`KeyBinding::load`，`zed:gpui/src/keymap/binding.rs:51`），还能**反查**某动作当前的按键（`Window::bindings_for_action`，`zed:gpui/src/window.rs:6801`）。所以一份数据表（动作名 · 按键 · 上下文 · 读给人的说明）可以同时生成：绑定、帮助页、`aria_keyshortcuts` 文字、CLI 的键位清单。Zed 自己的 `settings/keymap_file.rs` 就是这么做的（GPL，只借思路）（读源码）
5. **变高消息列表用 GPUI 自带的 `list` + `ListState`**：`ListAlignment::Bottom`（聊天式）、`FollowMode::Tail`（贴底跟随）、`splice` 往前插历史时保持滚动锚点、`remeasure_items`，以及 **`splice_focusable`：带焦点的项滚出可见区后仍被渲染**（`zed:gpui/src/elements/list.rs:150-155, 490-540, 605-660`）。Zed 的 AI 对话视图就这么用（`zed:agent_ui/src/conversation_view.rs:1590-1610`）（读源码）
6. **「每批有上限、主动让出主线程」有两种现成先例**：Zed 终端按条数 + 时间（4 ms 或 100 条成批，再 `yield_now()`，`zed:terminal/src/terminal.rs:1425-1480`）；Quill 按时间预算（每轮最多 8 ms，`quill:src/ui/notifications/mod.rs:43, 433`）。建议时间预算为主、8 ms 起步（读源码；数字原型里量）
7. **同类先例 Quill**（GPUI 写的 Telegram 桌面客户端，MIT，约 115 万行 Rust）：绑 tdjson、单收线程 + 序号 + 单 reducer、关闭先 close 等 Closed——与我们 001 的设计几乎一致；但它是**视觉复刻官方客户端**路线，VoiceOver 写明是「待 Mac 上手动验证」，没有声称达到无障碍门槛（`quill:DECISIONS.md:23, 71`，文档说）。架构、性能、打包可借鉴，无障碍不能当标杆；它踩过的十个坑已分散写进各节。摘要见第九节，资料索引 `quill-index.md`
8. **系统通知（M4.4）GPUI 已内置**：`cx.show_system_notification` / `on_system_notification_response` / `set_app_identity`，底层是 `UNUserNotificationCenter`；**必须从 `.app` 包启动**，`cargo run` 下会被跳过（`zed:gpui_macos/src/system_notifications.rs:117-160`，读源码）
9. **B19-4 的单键在 GPUI 里走得通**：**正在组字时任何键都先给输入法**；不组字时，Cmd + 键**先**走按键匹配（没匹配上且不是菜单快捷键时，仍可能再交给输入法）；单个可打印键只在「有输入处理器、它要求输入法优先、且中日韩输入法开着」时才先给输入法，列表获焦（没有输入处理器）时直接进按键表；不带 Ctrl / Fn / Cmd 的无字符键（方向键、Esc、PageUp/Down、Home/End、**Shift+Tab**）总是先给输入法（001 R3 的根源）；Tab、回车带控制字符，不组字时先进按键表；**空格算可打印键**，与字母同规则（`zed-new:gpui_macos/src/window.rs:2875-2920`；`zed-new:gpui_macos/src/events.rs:367-381`，读源码）。见第四节第 4 小节
10. **测试**：GPUI 有 `TestAppContext` / `VisualTestContext`（模拟按键、派发动作、`run_until_parked`），还有能截图的 `HeadlessAppContext::capture_screenshot`（`zed:gpui/src/app/visual_test_context.rs:225-335`，`zed:gpui/src/app/headless_app_context.rs:164-168`）。但上游测试平台仍没有无障碍树，所以要么照 gpui-kit 的思路自写观察器，要么真窗口导树（读源码）

---

## 二、组件库：有什么、无障碍做到哪、能不能用

### 1. 三家对比

| | Zed `ui` crate | gpui-component / gpui-kit | 自写 |
|---|---|---|---|
| 许可证 | **GPL-3.0-or-later**（`zed:ui/Cargo.toml:6`；`menu`、`theme`、`settings`、`editor`、`picker`、`ui_input` 同为 GPL）；人定 B19-5 已允许项目改 GPL | Apache-2.0（`gc:base/Cargo.toml:4`、`gc:component/Cargo.toml:4`、`gc:kit/Cargo.toml:7`） | 随项目 |
| 绑定的 gpui | 同仓 git（与我们现在一致） | crates.io `gpui-pre =0.3.12`，CI 卡死钉版（`resources/gpui-component-20261114/Cargo.toml:62-80`）；gpui-pre 是 Zed 某 commit 的每周快照、改名重发 | 我们选 |
| 体量 | 闭包新增 7 个 GPL crate 约 3.9 万行；不拖 settings / workspace / editor（第 ① 条） | base 15 万行、component 11.7 万行（含测试）；单是输入框 `element.rs` 超 6500 行 | 只写要的 |
| 无障碍 | 15 处 `.role()`/`aria_role()`，集中在按钮、菜单、开关、下拉、树项、列表项（`zed-new:ui/src/components/`） | 123 处 `.role(`、66 处 `aria_label`、5 处 `a11y_synthetic_children`；输入框最好 | 按我们的规约 |
| 播报 | 无 | 无（搜不到 Live / announce） | 001 第二节第 2 小节已有设计 |

**判断（按 B19-5 重评，推断）**：「开源已有的就用，缺的补进上游」（`docs/plan/013_decisions-b19-gui.md:27-30`）落到三家上，结论如下：

**① Zed `ui` crate 能不能单独依赖**——能，代价可控（读源码）：

- 传递闭包（只算 Zed 工作区内的 crate；默认模式按 macOS 目标、不含可选依赖，**上界模式连可选、构建、全 target 依赖都算**：可选特性全开时的静态差集是比 `gpui` + `gpui_platform` 普通构建多 9 个（同样 7 个 GPL + 2 个 gpui 可选特性带的 Apache 小包），`ui` 的完整上界闭包里也没有 settings / editor / workspace；本项目实际特性与平台，接进工程时用 `cargo tree` 再确认；脚本与输出入库在 `docs/experiments/exp010-ui-closure/`，做法与局限见那里的 README；特性未精确解析、外部依赖未展开，行数是量级）：19 个内部 crate、约 14.8 万行；其中 gpui 一家（gpui、gpui_macros、sum_tree、scheduler、collections、refineable……）约 10.9 万行，是我们本来就要编的；**新增 GPL 的 7 个约 3.9 万行**：`ui` 31642、`theme` 5957、`component` 530、`icons` 360、`syntax_theme` 345、`ui_macros` 239、`menu` 37。外部直接依赖 62 个
- **不拖设置体系**：`settings` 只在 `ui` 的开发依赖里（`zed-new:ui/Cargo.toml` dev-dependencies）；主题读设置走一个特征 `ThemeSettingsProvider`，应用自己实现字体、字号、界面密度五个方法，`theme::set_theme_settings_provider` 注册（`zed-new:theme/src/theme_settings_provider.rs:1-40`）；`theme::init(LoadThemes::JustBase, cx)` 只装内置基础主题、不读 JSON、不碰设置（`zed-new:theme/src/theme.rs:109-140`）
- 要自备的：图标 SVG（`icons` 只给路径 `icons/<名>.svg`，`zed-new:icons/src/icons.rs:314-316`，文件在 Zed 的 `assets` crate 里，同为 GPL。**不从 `resources/` 拷文件**（铁律 3、B19-11）；可选：依赖同 rev 的 Zed `assets` crate 拿图标（要看它的闭包，exp010 可算），或自备开源图标，或干脆不用图标按钮）；一个 `AssetSource`
- 升级代价：`ui` 与 gpui 同仓同 rev，**我们锁 Zed fork 的哪个 rev，`ui` 就跟着哪个**，没有第二个底座；`ui` 的接口随 Zed 改，升级时一起改（与 gpui 本身同一笔账）
- 编译：多 3.9 万行，比 gpui 本身（约 9.5 万行 + 平台层）小得多（推断，没实测；exp010 可量）

**② 哪些能直接用、缺的怎么补**（读源码；评级见第 2、4 小节）：

| 要的东西 | Zed `ui` | gpui-kit | 缺口补到哪（上游 PR 候选） |
|---|---|---|---|
| 按钮 / 图标按钮 | ✅ 好（默认 Button 角色、标签、快捷键、展开、切换） | ✅ 好 | — |
| 右键菜单 / 下拉菜单 | ✅ 好（MenuItem、active descendant、快捷键） | ✅ 好 | — |
| 开关 / 复选框 / 标签页 / 提示 | ✅（Switch、Tab 有；Tooltip 无关联） | ✅ | 可选：Tooltip 关联到触发器 → Zed `ui` |
| 列表项 | ✅ ListItem 可设角色、标签、选中、active descendant | 中 | — |
| **文本输入**（登录、消息输入、搜索、阅读视图） | ❌ `ui_input` 要注入 Zed 编辑器（闭包 97 crate / 约 75 万行，`zed-new:ui_input/src/ui_input.rs:16-45`，`zed-new:editor/src/editor.rs:421`），且编辑器无 TextInput 角色 | ✅ 好（TextRun + 选区 + 密码角色） | 自写一个轻量输入元素（`EntityInputHandler` + TextRun + 选区 + 角色）；**做好后提给 Zed**：作为 `gpui` 的 `examples/input.rs` 无障碍版，或 `ui` 里的轻量输入组件（是否收由上游定） |
| 对话框（确认发送、删除） | ⚠ `Modal` 无 Dialog 角色、无焦点陷阱（`zed-new:ui/src/components/modal.rs` 搜不到 role） | ✅ 好 | `Modal` 加 Dialog 角色、名称、焦点陷阱与还原 → Zed `ui` |
| 通知 / 提示条 | ⚠ 无标注 | 中（Alert 角色、无 live） | 走我们的播报器；通用的 live 接口 → Zed `gpui`（ggpui 核） |
| 消息列表 `NavList` | ❌ | ❌（MessageScroller 差） | 自写；通用部分（active descendant + 先滚后选）若成型可提给 `ui` 的 List |
| 播报器 | ❌ | ❌ | 自写；「Live 节点 / 直发公告」接口 → Zed `gpui` |
| 禁用状态 `aria_disabled` | ❌（GPUI 无此接口） | 用包装元素补 | 直接补进 Zed `gpui` 的 div（一行级别的接口），比包装元素干净 |
| 无头无障碍测试观察器 | ❌ | ✅（`gc:base/src/observe.rs`） | 自写；或提给 Zed `gpui` 的测试平台（让测试平台能建树，ggpui 核可行性） |

**③ 和「一个底座」对齐**：Zed `ui` 与 gpui 同仓同 rev，补丁、`ui` 的修改、上游 PR 全在同一个 Zed fork 上，**这点确实利好**。gpui-kit 与 Zed `ui` 不能直接组成同一棵组件树（两套 gpui 类型不互通；Cargo 层面能否共存未查，不作为理由，人定 B19-10 是工程取舍）；它做得好的东西怎么拿，见下条。

**④ gpui-kit 的好代码怎么拿（人已定，B19-11）**：先把输入框无障碍、无头观察器这类能力提给 Zed 上游，我们用上游的；不收或来不及就照思路自己写。`CLAUDE.md` 铁律 3（`resources/` 不 copy-paste）不动——Apache-2.0 法律上能移植，但本项目不走这条（`docs/plan/013_decisions-b19-gui.md:52-55`）

**补丁共存**（common.md 第 15 条：GPUI 有缺口就自己打补丁、锁 git rev、再提回上游）：

| 路线 | 补丁打在哪 | 能不能用 `[patch]` | 上游 PR | 每次升级 |
|---|---|---|---|---|
| **甲 · 不用 gpui-kit**（建议；可同时用 Zed `ui`） | Zed 主干的 fork，分支 `atg/<缺口>` | 直接 `gpui = { git, rev }`，不用 `[patch]` 改名 | 同一份提交直接提 | 换 rev，补丁 rebase 到新主干 |
| 乙 · 用 gpui-kit + 补 gpui-pre | gpui-pre 0.3.12 那份快照（= Zed 某个旧 commit，描述里写着 `d2a4fb6`，`sources.md` gcomp 节） | `[patch.crates-io] gpui-pre = { git 或 path }`，替换品的包名必须叫 `gpui-pre`、版本必须满足 `=0.3.12`；**直接指 Zed 主干不行**：主干 gpui 版本是 0.2.2（`zed-new:gpui/Cargo.toml` version 行）、包名是 `gpui`，Cargo 会报「patch 未被使用」（推断，依据 Cargo `[patch]` 规则）；gpui-kit 同时依赖 `gpui-pre-platform`、`-macros`、`-sum-tree` 等多个快照包（`resources/gpui-component-20261114/Cargo.toml:69-84`），补到哪个就要替换哪个 | 快照上的补丁要在主干上另做一遍再提 | gpui-kit 每升一个小版本就换一次快照（0.8.0 → 0.3.9，0.9.1 → 0.3.12，`gc` 仓 `release-notes.md:10, 329`），补丁要重新搬 |

- **先例证明乙走得通**：Quill 正是这么做的——`[patch.crates-io]` 把 `gpui-base`、`gpui-component`、`gpui-pre-macos`、`gpui-pre-linux`、`gpui-pre-windows` 等换成仓内 `third_party/` 的本地副本并打了补丁（空闲 CPU、表情渲染等），每个补丁带一份决定记录（`quill:Cargo.toml:376-395`；`quill:third_party/`）。它的 `docs/decisions/gpui-kit-0-7-3.md` 到 `gpui-kit-0-9-1.md` 五份、`gpui-pre-0-3-12.md` 一份，说明每次升级都是一件单独的活（读源码 + 文档说）
- **accesskit / accesskit_macos 的补丁**两条路线一样：都是 crates.io 包，`[patch.crates-io]` 指向 AccessKit fork 即可，不受 gpui 底座影响（推断）
- **代价对比**（推断）：乙多出来的是「两个底座」——自用补丁在快照上、上游 PR 在主干上，同一个缺口要写两遍、测两遍；而且我们升 GPUI 的时机被 gpui-kit 的发版节奏绑住。甲只有一个底座，组件用 Zed `ui`、缺口自写（见上面 ①②）
- **结论**：路线甲（不依赖 gpui-kit）与「先用 Zed `ui`」是同一条路：补丁、`ui`、上游 PR 都在一个 Zed fork 上

### 2. gpui-component 逐个组件（读源码；子代理审计，主会话抽查过输入框角色、`Role::Log`、观察器机制三处）

| 组件 | 角色 | 名称 | 状态 | 键盘 | 评级 | 出处 |
|---|---|---|---|---|---|---|
| List | List / ListItem | 行可设 | selected、位置/总数 | 上下、确认、取消；**无 active descendant** | 中 | `gc:component/src/list/list.rs:517-520, 713-716, 743-746` |
| VirtualList | 无 | 无 | 无 | 无 | 无 | `gc:base/src/virtual_list.rs:1-12` |
| MessageScroller | 视口 `Log` | 无 | **无 live** | 跳到底部按钮 | 差 | `gc:component/src/message_scroller.rs:418` |
| Message / Bubble | 默认不进树 | 无接口 | 无 | 无 | 差 / 无 | `gc:component/src/message.rs:110-128`；`bubble.rs` |
| 单行输入 | TextInput / PasswordInput / EmailInput / PhoneNumberInput… | 有 | 值、TextRun、选区；密码不露值 | 完整编辑 + 输入法；**不接 SetTextSelection** | 好 | `gc:component/src/input/input.rs:50-75`；`gc:base/src/input/base/a11y_text.rs` |
| 多行输入 / Editor | MultilineTextInput | 有 | 每行一个 TextRun | 完整编辑 | 好 | 同上 |
| 验证码输入 OtpInput | 无 | 无 | 无 | 按键 | 差 | `gc:base/src/otp_input.rs:196-206` |
| Dialog / AlertDialog | Dialog / AlertDialog | 有 | — | 焦点陷阱、Esc、**关闭后焦点还原** | 好 | `gc:base/src/dialog.rs:570-576`；`gc:component/src/root.rs:394-444` |
| Sheet | 无 | 无 | — | 焦点陷阱、Esc | 中 | `gc:base/src/sheet.rs:132-134` |
| 菜单 / 右键菜单 | Menu / MenuItem / MenuBar | 有 | selected、toggled | 方向键、焦点还原 | 好 | `gc:component/src/menu/menu_item.rs:108-112` |
| Toast / 通知 | Alert | 内容 | 无 live | 关闭按钮 | 中 | `gc:base/src/toast.rs:655` |
| Button | Button（可覆盖） | 有 | — | Enter、Space、AX 点击、tab_index | 好 | `gc:base/src/button.rs:287-291` |
| Checkbox / Switch | CheckBox / Switch | 有 | toggled | 焦点、Space | 好 | `gc:base/src/checkbox.rs:380-390`；`switch.rs:367-381` |
| Tabs | Tab / TabList | 有 | selected、位置/总数、方向 | 方向键 | 好 | `gc:base/src/tabs.rs:178-193, 470-502` |
| Select | ComboBox（触发器） | 有 | expanded、value | 弹层无 active descendant | 中 | `gc:base/src/select.rs:48-51, 211-230` |
| Tooltip | Tooltip | 内容 | 不关联触发器 | 无 | 中 | `gc:base/src/tooltip.rs:30` |

子代理的完整报告没有入库，要点已全部并入本表；要细节按表里的路径回源码。

### 3. 值得照思路自写的辅助件（读源码）

- **TextRun + 选区**（`gc:base/src/input/base/a11y_text.rs`，约 370 行）：只在辅助技术激活、值可暴露时才生成 `Role::TextRun` 合成子节点；每个子节点带 `character_lengths`、`word_starts`，单行还带字符位置和宽度；父节点设 `set_text_selection`。一个子节点最多 255 个字符，按字形簇切。多行每行一个子节点。GPUI 自己的文档也给了同一套做法的示意（`zed:gpui/src/_accessibility.rs:223-270`）。accesskit_macos 只有节点「支持文本范围」时才实现选区、字数、按行取文本这些 NSAccessibility 方法（`~/.cargo/registry/src/*/accesskit_macos-0.26.3/src/node.rs:341, 733-1010, 1234-1239`）——**没有 TextRun 子节点，VoiceOver 在输入框里就读不出光标位置和逐字内容**
- **无头测试观察器**（`gc:base/src/observe.rs:20-35, 171-172, 477-575`；查询 `gc:kit/src/test/query.rs`）：元素外包一层，只在 `test-support` 特性下生效；prepaint 时 `accesskit::Node::new(role)` 再调内层的 `write_a11y_info` 读回角色、名称、描述、值、选中、展开、禁用、是否可聚焦；paint 时补可见性和焦点；存进按窗口 + 全局元素 id 索引的注册表。查询有 `find_by_label`（精确、有歧义就 panic）、`find_all_by_role`、`within`（按 id 作用域）。用例见 `gc:kit/tests/integration/query_helpers.rs`。**局限**：只看得到挂了它的元素；看不到合成子节点、父子结构、active descendant；不验证 macOS 适配层，也不验证真播报
- **`aria_disabled` 补丁**（`gc:base/src/a11y_disabled.rs:1-50`）：GPUI 没有禁用状态的设置方法（ggpui 的 `findings-gpui.md` 可对照），它用一个不加布局节点的包装元素在节点上写 disabled
- **macOS 命中测试转发**（`gc:base/src/macos_accessibility.rs:16-38`，在 `gc:base/src/root.rs:176-177` 安装）：运行时给 NSWindow 的类加 `accessibilityHitTest:`，转给 contentView。源码没写原因；推断是窗口默认的命中测试停在窗口层，VoiceOver 触控板探查、辅助功能检查器指点拿不到具体控件。**值得 exp010 实测装与不装的差别**
- **焦点陷阱 + 焦点还原**（`gc:base/src/focus_trap.rs:118-121`；`gc:component/src/root.rs:394-444`）：全局一个陷阱管理器让 Tab 在对话框里循环；打开前记下原焦点、关闭后还原。对应 001 第二节第 4 小节「关掉对话框后焦点落在哪」

### 4. Zed `ui` crate 的标注习惯（人定 B19-10 直接依赖它；这里记它已经做对的地方）

- 按钮基类统一默认 `Role::Button`，可覆盖角色，标签、快捷键、描述、值、展开、切换一路透传（`zed:ui/src/components/button/button_like.rs:601-641, 775-790`）
- 右键菜单：菜单项 `MenuItem` / `MenuItemCheckBox`、带 `aria_checked`、**选中项用 active descendant**、`aria_keyshortcuts` 填从按键表反查到的快捷键（`zed:ui/src/components/context_menu.rs:1500-1502, 1588-1590, 1925-1934, 2313`）——这正是「按键表一份、处处引用」的现成样子
- 展开按钮直接用「Collapse / Expand」做名称 + `aria_expanded`（`zed:ui/src/components/disclosure.rs:103-104`）
- Zed 的编辑器本体（`editor` crate）**没有** `TextInput` 角色（两版都搜不到）；但 Zed 主干的设置界面给单行编辑器**包了一层无障碍**：外层元素 `Role::TextInput`，合成 `TextRun` 子节点带文字与选区，聚焦期间冻结 `aria_value` 免得每按一键重读全文（`zed-new:settings_ui/src/components/input_field.rs:226, 232, 241, 363-380, 504, 559`；第 241 行把算出的 TextRun 闭包挂到 `a11y_synthetic_children`，设置页搜索框同样挂接，`zed-new:settings_ui/src/settings_ui.rs:3027-3035`，GPL，读源码；ggpui 的 `findings-gpui.md` 也登记了它）。这是我们**同一个 Zed 底座里**现成的样本，自写输入框照它和 GPUI 文档的示意写；它绑的是 `Editor`，不能脱离编辑器直接用

---

## 四、动作与按键系统（给 gssot 的按键表正本用）

### 1. 机制（读源码）

- **声明动作**：`actions!(命名空间, [A, B, ...])` 为每个名字生成一个单元结构体并实现 `Action`（`zed:gpui/src/action.rs:24-40`）；带参数的动作用 `#[derive(Action)]` + serde
- **动作有名字、可按名字构造**：全局登记表 `by_name`；`App::build_action(name, params)`（`zed:gpui/src/app.rs:2793`，实现 `action.rs:351-369`）；`all_action_names()` 列出全部（`app.rs:2803`）；还带文档串、废弃别名
- **绑定**：`KeyBinding::new("cmd-shift-p", 动作, Some("上下文谓词"))` 静态写死；`KeyBinding::load(字符串, Box<dyn Action>, 谓词, ...)` 从数据加载（`zed:gpui/src/keymap/binding.rs:30-100`）；`cx.bind_keys([...])` 装入、`cx.clear_key_bindings()` 清空（`app.rs:2741-2753`）。多键序列用空格分隔（`binding.rs:79-80`），有挂起超时
- **上下文**：元素上 `.key_context("MessageList")`；谓词支持 `A && !B`、`A > B`（父子）、`key == value`（`zed:gpui/src/keymap/context.rs:247-330`）。同一按键在不同上下文绑不同动作，按焦点路径从深到浅匹配
- **处理**：元素上 `.on_action(|动作, 窗口, cx| ...)`、`.capture_action(...)`（`zed:gpui/src/elements/div.rs:459-482`）；动作沿焦点路径冒泡。也可 `window.dispatch_action(...)` 代码派发——**测试和 CLI 驱动界面都能走这条**
- **反查**：`window.bindings_for_action(&动作)`、`highest_precedence_binding_for_action`、`keystroke_text_for`（`zed:gpui/src/window.rs:5618, 6801-6860`）——可拿来填 `aria_keyshortcuts`、提示语和帮助页
- **解绑**：`NoAction` / `Unbind`（`zed:gpui/src/action.rs:447-455`）

### 2. 一份按键表生成一切（推断，依据上面的接口）

建议正本是一份数据文件（格式由 gssot 定），每行：**动作名 · 默认按键 · 上下文 · 一句读给人的说明 · 对应 CLI 子命令（若有）**。由它生成 / 比对：

1. 启动时：逐行 `build_action(名字)` + `KeyBinding::load` + `bind_keys`。名字拼错、按键串解析失败 → 启动即报错（`build_action` 返回 `NotFound`，`load` 返回 `InvalidKeystrokeError`）
2. 单测：表里每个动作名都在 `all_action_names()` 里；代码里每个 `actions!` 声明的动作都在表里（双向比对，进 `scripts/check-sources.py` 那类正本比对）；同一上下文同一按键不出现两次
3. 界面：按钮、菜单项的 `aria_keyshortcuts` 和提示语，用 `bindings_for_action` 现查，不手写
4. 帮助页 / CLI：`atg-cli keys`（若要）打印同一张表
5. 先例：Zed 用 JSON keymap 文件 + `KeymapFile::load` 做同样的事（`zed:settings/src/keymap_file.rs:204-268`，GPL，只借思路）；gpui-kit 没有配置层，各组件 `init(cx)` 里写死 `bind_keys`（197 处），应用在其后再绑覆盖（`gc:component/src/lib.rs:131-152`）

### 3. 跟我们已知的坑对照

- **VoiceOver 组合键可能到不了应用**（exp004 结论 4）：Control+Option 组合人定**允许但非首选**（B19-4，`docs/plan/013_decisions-b19-gui.md:24-25`）；表里每用一个这类组合，都要实测它能不能到达应用
- **输入法先吃方向键**（001 R3，`zed:gpui_macos/src/window.rs:2633-2770`，老版行号；新版见下一小节）：GPUI 的按键匹配在输入法之后，所以表里绑在列表上下文的 ↑/↓，中文输入法开着时可能收不到。exp003b-v2 的 AppKit 监视器绕法仍要保留；表里给这类键标一列「需绕过输入法」
- **`aria_keyshortcuts` 还没传到 VoiceOver**（`.42cog/real.md`「技术栈现实」；ggpui 核新版）：照填，导出的树里有，agent 读得到；读给人听的那份放提示语
- **同一操作同一键**（`.42cog/real.md`「使用者现实」）：会话列表和消息列表共用 `NavList` 上下文，绑定只写一次

### 4. 对照人定 B19-4（首选单键或 Cmd + 键，`docs/plan/013_decisions-b19-gui.md:22-25`）

读 `zed-new:gpui_macos/src/window.rs:2875-2920`（按键先给谁）得出三类键的走向（读源码；「输入框不获焦时没有输入处理器」一句为推断，依据 `with_input_handler` 取的是窗口当前登记的那个，`window.rs:4002-4014`）：

| 键 | 先给输入法吗 | 对 B19-4 的意思 |
|---|---|---|
| **正在组字时的任何键**（`is_composing` 是独立的「或」分支，不看修饰键） | **先给输入法**；输入法不要才回到按键匹配 | 组字中按 Cmd + 键、回车、方向键都先归输入法——测试要覆盖这种状态 |
| **Cmd + 键**（不组字时） | **不先给**（带 platform 修饰的跳过「可打印键」与「无字符键」两条路，先走按键匹配；匹配不上、又不是菜单快捷键时，后段仍会交 `inputContext`） | 最稳，不组字时任何焦点下都到得了按键表 |
| **单个可打印键**（字母、数字、标点、空格；不组字时） | 三条都满足才先给输入法：有输入处理器、处理器的「输入法优先」返回真（`ElementInputHandler` 取视图的 `accepts_text_input`，`zed-new:gpui/src/input.rs:297-300`；平台默认假，`platform.rs:2555-2562`）、中日韩输入法开着；没有获焦的输入框就直接进按键匹配 | **列表获焦时单键可用，中文输入法开着也行**；输入框获焦时单键归打字——正好就是 B19-4 写的「不在输入框 / 不在组字时生效」，用 `key_context` 谓词 `NavList && !TextInput` 一类表达 |
| **Tab、回车**（macOS 给它们带了字符 `\t`、`\n`，`zed-new:gpui_macos/src/events.rs:371-378`） | 不组字时**不先给**（控制字符不算可打印键，也不进「无字符键」那条路），先走按键匹配；组字时先给输入法 | 按键表可直接绑，不必绕输入法；回车在组字时归输入法选字——这正是 M4.1「回车时查组字」的场景 |
| **没有字符的键**：方向键、Esc、PageUp/PageDown、Home/End、**Shift+Tab**（`events.rs:381` 只给键名、不给字符），**且不带 Ctrl / Fn / Cmd** | **总是先给输入法**，不看有没有输入框；带了 Ctrl / Fn / Cmd 的（如 Cmd+↑）不组字时不给 | 这就是 001 R3：中文输入法可能吞方向键。exp003b-v2 的 AppKit 监视器仍要保留；表里给这类键标「需绕过输入法」。**注意 Tab 与 Shift+Tab 走两条路**：001 第二节第 1 小节「Tab / Shift+Tab 任何时候都截」，对 Tab 可能多余、对 Shift+Tab 仍需要，待实测复核 |
| Ctrl / Ctrl-Option（VO 键）组合 | 不组字时不给（组字时照第一行先给输入法） | 能用但非首选；VO 组合本身被 VoiceOver 截走、到不了应用（exp004 结论 4） |

按键表因此建议多两列：**生效条件**（列表获焦 / 任何时候 / 仅输入框）和**是否需绕过输入法**。VoiceOver「快速导航」开着时会截方向键和单字母——人定 B19-3：使用时由人自己关掉，按键不为它绕路（`docs/plan/013_decisions-b19-gui.md:17-20`）

---

## 一、应用骨架

- **入口**：`Application::new().run(|cx| { 各模块 init(cx)；绑键；cx.open_window(...) })`；Zed 与 gpui-kit 都是「各模块一个 `init(cx)`，入口逐个调」（`gc:component/src/lib.rs:131-152`）。用 Zed `ui` 时多两步：`theme::init(LoadThemes::JustBase, cx)` + `theme::set_theme_settings_provider(..)`（第二节第 1 小节 ①）（读源码）
- **状态**：`cx.new(|cx| ...)` 建 `Entity`；视图实现 `Render`；`cx.notify()` 标脏；`cx.observe` / `cx.subscribe` + `EventEmitter` 订阅（`zed:gpui/src/app/context.rs:64-221, 780`）。001 的「Store 实体唯一一份状态」= 一个 `Entity<Store>`，各视图 `observe` 它
- **别让一次 notify 整窗重排**：Quill 一个大视图挂全部状态，后来拆出若干小「切片」实体、用 `Entity::cached` 嵌入，没变就复用上一帧；动画单独一层（`quill:src/ui/app_slice.rs:1-33`，读源码）。**但我们不照这么做**：上游 issue #66690 + ggpui 实测 exp009——从缓存画出的视图不再推无障碍节点，5 次里 4 次把控件从树里抹掉；ggpui 建议 `atg-gui` 在上游修好前**禁用 `.cached(..)`**（`findings-gpui.md` 第 4 节末条）。拆小实体照样做（每个实体自己 notify），只是不用 `cached`。**坑**：macOS 上一旦有辅助技术碰过窗口，GPUI 开始建无障碍树就不再停，而且会让缓存视图失效、每帧全量重画——Quill 为此给 gpui 打了「缓存视图复用它那一块树」的补丁，修前 CPU 多 5–10 个百分点（`quill:docs/decisions/a11y-cached-views.md:13-70`；`quill:third_party/gpui-pre/QUILL-CHANGES.md:265`，文档说）。**我们开着 VoiceOver 是常态，这条直接相关**：要么不靠 `cached` 省性能，要么这个补丁也是我们的上游 PR 候选（报 ggpui 核主干现状）
- **后台与主线程**：`cx.spawn(async move |this, cx| ...)` 在主线程跑异步任务；`cx.background_spawn` 去线程池；`background_executor().timer(..)` 定时（`zed:gpui/src/executor.rs:103-183, 354`）
- **每批有上限**，两种先例（读源码）：
  - 按条数 + 时间：Zed 终端——第一条立刻处理 → 4 ms 窗口或满 100 条成批 → `entity.update` 一次 → `yield_now()`（`zed:terminal/src/terminal.rs:1425-1480`）
  - 按时间预算：Quill 每个 tick 非阻塞取通道，满 **8 ms**（前台账号；后台账号 4 ms）就停，余下留到下一轮；收线程有数据时主动唤醒（`quill:src/ui/notifications/mod.rs:43, 63-66, 433`；`quill:src/ui/background_accounts/mod.rs:61`）
  - 建议：**时间预算为主（8 ms 起步）、条数为辅**，突发（重连、首次同步）时更稳；原型里量（推断）
- **重绘分级**：Quill 给每条更新打分——不重绘 / 只重绘会话列表（前台 ≤ 400 ms 一次）/ 稍后整窗（≤ 1 s）/ 立刻；前台空闲 CPU 从 6–8% 降到约 2%，静止时停 display link 到 0%（`quill:docs/decisions/codex-idle-cpu.md:55-70, 211-216`；`quill:src/state/redraw.rs`，常量读源码、数字文档说）。我们的 reducer 输出「哪块变了」时可以照这个分级，但**当前打开会话的新消息必须立刻出帧**，否则 Live 节点和树会滞后（001 R2）
- **收线程**：Quill 与我们同判断——`td_receive` 全进程共享，只能一个收线程，JSON 在收线程上解析（`quill:src/telegram/hub.rs:1-21`；`quill:docs/decisions/codex-ingest-budget.md:9`）；关闭不在主线程上同步等 close（`quill:docs/decisions/tdlib-close-off-ui-thread.md`，文档说）
- 小坑：大结构体按值放在视图里，Windows 1 MB 主线程栈溢出（`quill:docs/decisions/main-thread-stack.md`）——macOS 主线程栈 8 MB，问题小，但别把大数组按值放进 `Entity`（推断）

## 三、文本输入

- **GPUI 底座**：视图实现 `EntityInputHandler`（含输入法的 `marked_text_range`、`replace_and_mark_text_in_range`），paint 时 `window.handle_input(ElementInputHandler::new(..))`；程序改了文字或选区要调 `notify_text_input_changed` / `notify_text_selection_changed`（`zed:gpui/src/input.rs:1-40`）。官方示例 `zed:gpui/examples/input.rs`（920 行，单行，**没有任何无障碍标注**）（读源码）
- **各家**：Zed 的输入框是整个编辑器（`ui_input` 靠注入，闭包约 75 万行，且无 TextInput 角色）；gpui-kit 的输入框最好（TextRun + 选区 + 按内容选角色 + 密码不露值 + 输入法），但不接读屏发来的「移动光标 / 替换选中文字」动作（`SetTextSelection`、`ReplaceSelectedText`）；Quill 用 gpui-kit 的多行输入（第二节第 2 小节；读源码）
- **建议**：自写一个轻量输入元素，四件：① `EntityInputHandler`；② 角色按用途 `TextInput` / `MultilineTextInput` / `PasswordInput` / `PhoneNumberInput` / `SearchInput`；③ TextRun 合成子节点 + 选区；④ 接 AccessKit 的 `SetTextSelection`（gpui-kit 也缺，可一并作上游 PR）。照 B19-11 先试着提给 Zed 上游（第二节第 1 小节 ② 表）；样本是同底座的 `settings_ui` 包装（上一条）
- **回车与输入法**：回车事件不一定带「正在组字」标志，要在回车那一刻自己查 `marked_text_range`，组字中一律不发；「是否发送」写成纯函数单测（`quill:src/composer/send.rs:29-37`；`quill:DECISIONS.md:20-22`）。Ctrl+Enter 模式下组件可能已先插了换行，发送时要去掉（`quill:src/composer/send.rs:39-49`）
- **方向键 / Cocoa 文本键**：动作处理默认吃掉按键——全局绑了方向键，输入框就收不到；处理不了要 `cx.propagate()`（`quill:docs/decisions/codex-composer-keys.md`）。每个快捷键同时绑 Cmd 和 Ctrl 会在 macOS 上抢掉 Ctrl+K、Ctrl+F 这类 Cocoa 文本键（`quill:docs/decisions/native-macos-shortcuts.md`）
- **长文本**：单行超长（约 100 KB）时每按一次键 260 ms，Quill 改成增量换行、超 16 KB 不做字形排版（`quill:docs/decisions/long-text.md`，文档说）。阅读视图（M3.5）要注意
- **登录（M3.2）**：手机号 `PhoneNumberInput`；验证码用普通单行输入（gpui-kit 的 OtpInput 无标注、Zed 无）；二步密码 `PasswordInput`（推断）

## 五、变高列表

- **元素**：`uniform_list` 要求等高（R10）；变高用 `list` + `ListState`：`ListAlignment::Bottom`、`FollowMode::Tail`、`splice` 往前插历史时锚点自动后移、`remeasure_items`、`scroll_to_reveal_item`（`zed:gpui/src/elements/list.rs:150-155, 490-540, 600-700`）。gpui-kit 的 `MessageScroller` 与 Quill 的会话视图都建在它上面（`gc:component/src/message_scroller.rs`；`quill:src/ui/history_state.rs:97`）（读源码）
- **屏外项不进树**：`list` 只把可见项画进去（001 R10，codex 已核）；Quill 也一样
- **active descendant 与虚拟化**：选中项必须正在被渲染才进树。**先滚、确认目标项进了树、再设选中**——但「怎么滚」要分两种（ggpui 实测 exp009，`findings-gpui.md` 第 4 节）：**逐项 ↑/↓** 用 `scroll_to_reveal_item(ix)` + overdraw（约 200 px）；**跳跃**（PageUp/PageDown、Home/End、落到第一条未读、从通知跳到某条）用 `scroll_to(ListOffset)`——往下跳到没量过的项时 `scroll_to_reveal_item` 根本不滚，目标不进树、active descendant 落空（9 步错 8 步）；不用 `measure_all()`（长会话首帧全排版）。exp003 的「同一帧生效」是 `uniform_list` 等高列表的结论，`list` 不能照搬（001 第二节第 1 小节「跨屏」）。`splice_focusable`（带焦点的项滚出可见区后仍被渲染）是给「每项自己拿焦点」的设计用的，可作备选
- **Quill 的读屏焦点模型**（照官方桌面客户端移植，`quill:src/ui/history_a11y.rs`，469 行，读源码）：焦点进列表时优先回原来那条，否则未读分隔条后第一条，否则最后一条；↑/↓、PageUp/PageDown、Home/End；当前行用 active descendant 暴露；**只有确实检测到读屏在跑时才改按键含义**（`history_a11y.rs:149-165`）。与我们 001 的 `NavList` 几乎同构，可作对照，F2 落点（第一条未读）一致
- **落点与滚动恢复**：有未读就让未读分隔条贴顶，否则停底；每次绘制后量偏差再补滚。「按会话恢复上次滚动位置」做了又撤——总把人带离新的未读（`quill:docs/decisions/unread-open-landing.md`；`sweep-scroll-restore.md`，文档说）。与 F2 一致：**不做按会话恢复位置**
- **历史窗口只留连续一段**：把搜索命中那条孤立插进历史，会造成看不见的空洞（`quill:docs/decisions/codex-history-window.md`）——M4.5「回车跳回原消息上下文」要整段重拉，不能只插一条
- **行缓存**：内容与外框（发送者、已读）分开缓存，2000 条压测 CPU 36.7% → 26.3%；提前三屏预载，滚动绘制 p99 约 4 ms（`quill:docs/decisions/codex-history-rows-cache.md`、`history-preload.md`，文档说）
- 贴底跟随的坑：内容比窗格高时会挡住第一行（`quill:DECISIONS.md:63`）；ggpui 登记了未合的 Zed PR #65740（list 自动滚动时无障碍节点回滚），跟进
- **焦点监听在一帧末尾跑，这时 `cx.notify()` 不再排下一帧**：焦点改动要推到下一帧（`quill:docs/decisions/accessibility-and-dock-menu.md`「A bug the smoke run caught」，文档说）。exp003 的「同一帧生效」要在正式代码里复核

## 六、测试

- **GPUI 自带**：`#[gpui::test]` + `TestAppContext` / `VisualTestContext`：`simulate_keystrokes`、`simulate_input`、`dispatch_action`、`run_until_parked`（`zed:gpui/src/app/visual_test_context.rs:152-335`；示例 `zed:gpui/examples/testing.rs`）；截图 `HeadlessAppContext::capture_screenshot`（`zed:gpui/src/app/headless_app_context.rs:164-168`）。我们不以像素为准，截图用处有限（读源码）
- **无障碍断言**：上游测试平台没有树。两条路：① 照 gpui-kit 观察器思路自写（第二节第 3 小节）；② Quill 的做法——给 gpui 打补丁「强制建树 + 进程内读快照」，测试直接对树快照断言，强制建树每帧多约 1 ms（release）（`quill:third_party/gpui-pre/QUILL-CHANGES.md:134`；`quill:docs/decisions/agent-access.md:375-460`，文档说）。② 正好也是 agent 友好那把尺子要的「AI 读树」，**是上游 PR 的好候选**（与 ggpui、gssot 对齐）
- **回放测试**：Quill 32 个 `tests/replay_*.rs` 把 JSON 序列喂给 reducer，不要界面（`quill:tests/replay_common.rs`，读源码）——我们 `atg-core` 已是这种做法
- **真窗口冒烟**：Quill 的 `scripts/macos-accessibility-smoke.sh` 用系统 AX 接口遍历窗口、查无名控件、按按钮、改输入框值；`macos-voiceover-smoke.sh` 读 VoiceOver 最后一句——**它们说后者从没在开着 VoiceOver 的机器上跑通过**（`quill:docs/decisions/accessibility-and-dock-menu.md`「Not verified」）。对应我们 001 第二节第 3 小节的两层导树 + exp003 的 `selftest-tab`
- **不稳定测试的教训**：组件动画用墙钟，测试时钟推不动 → 测试里关动效；全局原子量让测试互相污染 → 测试下改线程局部；**界面测试从没进 CI，红了很久没人发现**（`quill:docs/decisions/flaky-ui-tests.md`、`codex-ui-tests-green.md`，文档说）
- **按键追踪开关**：环境变量打开后打印每次按键解析成什么、命中哪个动作（`quill:src/ui/keybindings/mod.rs:684-700`）——值得照做，调「输入法吞键」「VO 截键」时用。**但只记动作名、走了哪条路（组字 / 可打印 / 无字符 / 按键匹配）和修饰键，不记字符**：输入框获焦时打的是手机号、验证码、二步密码和消息正文，记下来就违反 `.42cog/real.md`「隐私」与 001 第四节第 1 小节「日志里不出现聊天内容」。要配一条测试：开着追踪往输入框打一串假号码，断言日志里没有这串字符

## 七、macOS 打包、签名、通知

- **通知**：GPUI 内置 `cx.show_system_notification` / `dismiss_system_notification` / `on_system_notification_response`（返回 tag 与按钮 id，可据此打开会话）/ `set_app_identity`；底层 `UNUserNotificationCenter`，首次发通知时申请授权（`zed:gpui_macos/src/system_notifications.rs:117-175`；`zed:gpui/examples/system_notifications.rs`）。**不在 `.app` 里运行时该接口会让进程中止**，上游与 Quill 都先查 bundle id、为空就关通知（同文件 `:117-128`；`quill:third_party/gpui-pre-macos/src/system_notifications.rs:181-186`）（读源码）
- **打包**：Zed 用自家 fork 的 `cargo bundle` 出 `.app`（`resources/zed/script/bundle-mac:112`），元数据在 crate 的 `[package.metadata.bundle-*]`（`zed:zed/Cargo.toml:286-299`）；缺证书时退回 ad-hoc（`bundle-mac:231`）
- **签名与钥匙串（重要）**：Quill 起初 ad-hoc 签名，每个版本 cdhash 不同、钥匙串分区跟着变，**每次更新都弹钥匙串密码框**；改用固定 Apple Development 身份后分区变成 `teamid:` 就不弹了（`quill:docs/decisions/macos-release-signing.md`，文档说）。与我们 exp005 的结论、`scripts/dev-sign.sh:4-6` 完全一致——**打成 `.app` 后签名身份与标识沿用 `atg.dev`**，否则读本地库密钥会弹框
- **公证的反作用**：公证过的包在重启后会被 XProtect 按大小重扫，Quill 108 MB 主程序冷启动多 0.7–0.9 s；只签名不公证只扫一次（`quill:docs/decisions/launch-first-load-check.md:20-60`，文档说）。我们自用、不分发，**不公证**即可
- **点通知后窗口冻住**：停了 display link 后没重启（`quill:docs/decisions/notification-click-repaint.md`）——我们若照 Quill 做空闲停帧，要记得这条
- **最小路径**（推断）：手写 `Info.plist`（bundle id、名称）+ `X.app/Contents/{MacOS,Resources}`，可执行文件放进去，用同一身份、同一标识 `atg.dev` 签整个包——**现有 `scripts/dev-sign.sh` 只收普通文件**（`:20` 用 `-f` 检查，传 `.app` 目录会直接退出），要先扩展它支持应用包，或另写一步 `codesign -s <同一身份> -i atg.dev --force X.app`；不必引入 cargo-bundle

## 八、性能

- **本项目已有**：新消息到树 50 项中位 21 ms、500 项 48 ms，几乎全在 GPUI 渲染；冷启动大头在开窗和第一次建树约 0.28 s（exp002，debug）
- **上游**：只有厂商口径（120 fps、打字延迟 < 10 ms），冷启动无可信第三方数字（`sources.md` C10）
- **Quill 实测**（M1 Pro，release，均为文档说）：

| 指标 | 数值 | 出处（`quill:docs/decisions/`） |
|---|---|---|
| 冷启动到窗口上屏 | 1.36 s → 约 1.0 s → 约 0.72–0.77 s | `launch-window-first.md:95`、`launch-first-load-check.md:5` |
| 热启动到上屏 | 约 330–362 ms；空 Metal 窗口下限约 225 ms | `launch-warm-waits.md:5-8` |
| 滚动 | 绘制 p99 约 4 ms，输入延迟 p99 约 13.6 ms | `history-preload.md:12-18` |
| 强制建无障碍树 | 每帧约 +1 ms（2.0 → 3.3 ms） | `agent-access.md:418-421` |
| 空闲 CPU | 6–8% → 前台约 2%，静止 0% | `codex-idle-cpu.md` |
| 切会话 | **没有实测数字** | — |

- **冷启动手法**（同上目录 `launch-*.md`）：进 `main` 就在工作线程提前开 TDLib，首帧就能出会话列表；字体列表后台预取（约 100 ms）；键盘布局首帧后再读（约 87 ms）；避开系统写作工具框架的加载（约 280 ms）；首帧文本排版缓存。001 第一节第 4 小节「开窗和起 TDLib 并行」与此一致
- **对照 exp002**：我们 debug 冷启动 0.28 s 是「到第一帧 + 建树」的量法，Quill 1 s 级是 release、整个大应用；量法不同，不能直接比。**切会话没人量过**——这是收敛方向「切会话快」的空白，M3.4 要自己量（推断）

## 九、同类先例 Quill（摘要；索引见 `quill-index.md`）

- **是什么**：用 GPUI + gpui-kit 写的 Telegram 桌面客户端，MIT，约 115.7 万行 Rust，跨 macOS / Linux / Windows，目标是功能对标官方客户端（`quill:README.md`；`quill:STATUS.md`）
- **与我们一致的判断**：一个收线程 + 序号 + 单 reducer；关闭先 close 等 Closed；回放夹具测 reducer；数据库密钥放钥匙串；未读以服务端为准（`quill:DECISIONS.md:26-47`）
- **与我们相反的判断**：否决 tdlib-rs、直接绑 tdjson（理由：schema 落后、应答与更新分两路、未知类型把原始 JSON 打进日志——后两条正是我们 001 R8、R13 记过的坑，我们选了自维护补丁）（`quill:DECISIONS.md:30-32`）
- **无障碍的实际水平**（读源码 + 文档说）：底座扎实——`.aria_label` 555 处、`.role(` 433 处（以 Button、Group、Label 为主），有强制建树、缓存视图带树的 gpui 补丁，有照官方桌面客户端移植的逐条读法（`quill:src/message_a11y.rs`，461 行）和列表焦点模型；**但新消息不播报、没有通用 live 机制、VoiceOver 实际读什么从没核过**（`quill:STATUS.md:1172-1173` 未勾）。**不能当无障碍标杆，能当「GPUI 上做到哪一步、踩了哪些坑」的地图**
- **代价**：vendor 了 12 个 gpui 相关 crate、补丁记录约 2500 行；gpui-kit 两个月升约 6 次，每次重搬重合（`quill:third_party/*/QUILL-CHANGES.md`；`quill:docs/decisions/gpui-kit-0-*.md`）——这正是第十一节 H1 乙路线的代价实例
- **十个坑**（已分散写进上面各节）：建树不停且废缓存（一）· 动作默认吃键（三）· Cmd/Ctrl 双绑抢文本键（三）· 回车不带组字标志（三）· ad-hoc 签名每次弹钥匙串（七）· 不在 `.app` 里通知会中止（七）· 主线程同步等 close（一）· 按会话恢复滚动把人带离未读（五）· 测试被墙钟动画和全局量搞乱、界面测试不进 CI（六）· 焦点监听帧末 notify 不排帧（五）
- **按键表**：一张 `ShortcutRow { 说明, 分组, 绑定 }` 同时注册绑定和渲染快捷键参考页，但另拼进三组不上参考页的绑定，可改键又是第二张表——**表格化了，但不是唯一正本**（`quill:src/ui/keybindings/shortcut_rows.rs:6-32`；`mod.rs:99, 427-434, 712`，读源码）。我们按第四节第 2 小节做成真正的一份，并让「参考页、绑定、可改键」都从它出
- **改键**：只替换自己命名空间下的绑定、保存前先解析校验、固定键不许改、两个动作不许共用一键（`quill:src/ui/keybindings/mod.rs:712`；`quill:docs/decisions/parity-platform-custom-keybindings.md`）——若以后做改键，照这四条

---

## 十、对本项目的建议（逐条对到 M3.1–M4.6）

按人定 B19-10「Zed `ui` + 自写缺口」写。

| 块 | 建议 | 依据 |
|---|---|---|
| M3.1 骨架 + 树导出 | 一个 `Entity<Store>`，视图按块拆小实体（**不用 `.cached(..)`**，可机查）；收通道任务 8 ms 时间预算 + `yield_now`；按键表正本启动时加载并与 `actions!` 双向比对；自写无头观察器或推「强制建树」上游 PR，让「可操作元素有角色和名称」进 CI；`theme::init(JustBase)` + 自己的 `ThemeSettingsProvider` | 一、四、六、二 |
| M3.2 登录 | 自写轻量输入元素（`PhoneNumberInput` / `TextInput` / `PasswordInput` + TextRun + 选区）；按钮用 Zed `ui` 的 Button；出错原因写进输入框描述并播报 | 三、二 |
| M3.3 会话列表 | `NavList` 自写（ListItem 可用 Zed `ui` 的，带 active descendant）；等高可用 `uniform_list`，否则 `list`；跳下一个未读、静音进按键表 | 五、四 |
| M3.4 消息列表 | `list` + Bottom + Tail；逐项移动用 `scroll_to_reveal_item` + overdraw、跳跃（PageDown、End、第一条未读、通知跳转）用 `scroll_to(ListOffset)`，**目标项进了树再设** active descendant；不用 `Entity::cached`；落点「有未读落未读，否则落底」，**不按会话恢复位置**；焦点改动推到下一帧；**量切会话耗时**（没人量过） | 五、八 |
| M3.5 详情 + 阅读视图 | 阅读视图用只读多行输入角色 + 每行 TextRun；长文本注意排版耗时 | 三 |
| M3.6 状态播报 | 现成库都没有播报，按 001 第二节第 2 小节自写；通用 live 接口作 Zed `gpui` 上游 PR 候选 | 二 |
| M3.7 / M3.8 | 动作进按键表；`aria_keyshortcuts` 用 `bindings_for_action` 现查 | 四 |
| M4.1 输入框 | 多行输入 + 回车时查 `marked_text_range`，「是否发送」纯函数单测；处理不了的键 `propagate`；不 Cmd/Ctrl 双绑 | 三 |
| M4.2 确认步骤 | 对话框：Dialog 角色 + 名称 + 焦点陷阱 + 关闭后还原焦点；若用 Zed `ui` 的 `Modal`，先补这几样（上游 PR 候选） | 二 |
| M4.3 新消息提示 | 播报器自写；**详略开关「汇总 / 只计数 / 关」MVP 就做**（人定 B19-8，`013:40-42`），开关进按键表与设置，`notify` 策略按三档分支、假时钟单测各一组；重绘分级时「当前会话新消息」必须立刻出帧 | 一、四 |
| M4.4 后台通知 | 先做最小 `.app`，签名身份与标识沿用 `atg.dev`、不公证（`dev-sign.sh` 要先支持签目录）；再用 GPUI 内置系统通知，`on_system_notification_response` 的 tag 带 `(会话 id, 消息 id)` | 七 |
| M4.5 搜索 | 搜索框 `SearchInput` 角色（推断）；结果列表复用 `NavList`；跳回上下文时整段重拉历史，不孤立插一条 | 三、五 |
| M4.6 删除 | 走 M4.2 同一对话框 | — |

---

## 十一、要人定的事（两条都已定）

与同目录 `decision.md` 的对应：H1 ≈ G10「GPUI 跟哪条线」里的组件底座部分；H2 属 G9「借鉴 Quill / 别人的开源代码哪些」的同类问题。

**H1 · GUI 组件依赖谁 —— ✅ 人定甲（B19-10，`docs/plan/013_decisions-b19-gui.md:48-50`）：Zed `ui` + 自写缺口，不用 gpui-kit / gpui-pre**

- 当时的选项：甲 Zed `ui` + 自写缺口（与 Zed fork 同仓同 rev、一个底座；新增 7 个 GPL crate 约 3.9 万行）／乙 gpui-kit 整套（GPUI 换成 `gpui-pre =0.3.12` 快照，补丁写两遍、升级绑它的节奏）／乙′ 把 Zed fork 开在快照对应的提交上冒充 `gpui-pre`（未验证；人定不做实验）／丙 全部自写（与 B19-5 相悖）
- 随之要做：项目许可证改 GPL-3.0（B19-5，另开 issue，动手前再经人确认）

**H2 · 别人开源代码里的好东西怎么拿 —— ✅ 人定「先推上游，否则照思路重写」（B19-11，`013:52-55`）；铁律 3 不动**

- 当时的选项：A 照思路重写／B 带署名移植（需给铁律 3 开例外）／C 推给上游再用
