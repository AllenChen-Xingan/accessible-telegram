# 核查结果 · GPUI 无障碍接口现状（2026-11-14，上游最新）

> 手下 ggpui · 批次 b19 · 状态：**第 1–8 节完成；已过一轮多视角自审（atg-review）并修完 P1；codex 041 未做**
> 材料：Zed 主干 `15d136b`（2026-11-14 19:33 UTC，部分克隆在 MAIN 的 `resources/zed-20261114/`）对比老版本 `79fe6a6`（2026-10-23，`resources/zed/`）；accesskit 0.24.1 · accesskit_consumer 0.38.0 · accesskit_macos 0.26.3（本机 cargo 缓存里的源码）；GitHub 上的提交与 PR 列表（只查公开仓库）。
> 路径前缀：`Z:` = `resources/zed-20261114/crates/`；`ak:` = `accesskit_macos-0.26.3/src/`；`akc:` = `accesskit_consumer-0.38.0/src/`。
> 可信度标记：**实测** · **读源码** · **文档说** · **推断**。第 1、2、8 节是读源码 / 读文档；第 3–7 节里标「实测」的来自 exp009（`docs/experiments/exp009-gpui-head/`，7 次进程内运行 R1–R7，运行表在其 README；没听 VoiceOver 实际说了什么）。第 2 节有几格标「实测」，来自 exp003（macOS 角色映射），不是 exp009。第 2 节「VoiceOver 读不读」一列是按 macOS 角色推的（推断），没人听过。
> 公告优先级是 NSAccessibilityPriorityLevel：10 低 · 50 中 · 90 高（越大越急）。codex 041 = 用另一家模型做的跨谱系评审，记录将落 `docs/reviews/041_*-codex.md`。
> 链路：GPUI 元素（`.role()` / `.aria_*()`）→ AccessKit 树（跨平台的无障碍数据）→ accesskit_macos（macOS 适配层）→ NSAccessibility（macOS 无障碍接口）→ VoiceOver（读屏，简称 VO）。「GPUI 能设、VO 拿不到」都是卡在适配层这一段。a11y = accessibility（无障碍）。MAIN = 主工作区 `…/5-App/telegram accessibility `（目录名末尾有空格）。
> 术语：**prepaint**（GPUI 每帧先排版再画，无障碍节点在这一步推进树）· **overdraw**（`list` 在可见区外多量的那一段像素）· **active descendant**（列表持焦点，用属性告诉读屏「当前是哪一项」）· **第一响应者 / contentView / GPUIView**（AppKit 里接键盘的视图 / 窗口的内容视图 / GPUI 自己的视图）· **TextRun**（AccessKit 表示一段文字的子节点）· **userInfo**（macOS 通知附带的字典）· **Live**（值变就播报的节点属性）· **出口**（指 `a11y_synthetic_children` 的 `parent_node()`：能直接改 AccessKit 节点）· **薄壳**（`atg-gui` 里的无障碍封装，见 `docs/plan/001_arch-stack.md` 第二节）· **名额**（全机一个的重编译许可）· **转子**（VoiceOver 的 rotor：按类别跳转的菜单）· **直发**（应用直接调 macOS 发公告，不经 AccessKit 节点）· **NodeId / TreeUpdate**（AccessKit 的节点编号 / 一帧的树更新）· **P<n>**（补丁正本 `specs/gui/upstream-patches.md` 的编号）。
> 代号：「001 拍板 N」「001 RN」= `docs/plan/001_arch-stack.md`（拍板在第七节，R 在第五节）；「001 的五条」「001 第 N 条」= `docs/research/001_stack/findings-gpui.md`· exp00N = `docs/experiments/exp00N-*/README.md` · T01–T10 = `specs/tasks.md` · M3/M4 = `docs/plan/002_roadmap.md` 第六、七节 · common 第 N 条 = MAIN `_tmp/20261114-b19/common.md` · B19-N = `docs/plan/013_decisions-b19-gui.md` · 现状页 = `state/handoff/ggpui.md` · gssot / ga11y / gcomp = 本批另外三个调研 / 设计手下

## 〇、结论先行

1. **三周来 GPUI 的无障碍接口一行没变。** `79fe6a6..15d136b` 共 273 个提交，`window/a11y.rs`、`window/a11y/debug.rs`、`_accessibility.rs`、`examples/a11y.rs` 逐字相同；`div.rs` / `element.rs` / `window.rs` / `list.rs` / `uniform_list.rs` 里凡含 a11y、aria、accesskit、role 的行也逐字相同，只是行号移了。accesskit 全家版本没动。（读源码 + `git log`）
2. **001 的五条结论全部仍成立**，只需把 `window.rs` 的行号换新（见第 1 节表）。
3. **新冒出来一个要防的坑**：上游未合并的 PR #65740 —— `list` 里的子项请求自动滚动时，`list` 会在同一帧重做一次 prepaint，无障碍节点被推两遍：**调试构建直接 panic「Duplicate a11y node id」，发布构建丢掉第二遍的节点、留下滚动前的位置**。**前提**：只有列表项在 prepaint 里调 `window.request_autoscroll`（Zed 的编辑器、markdown 元素会调）才触发；消息项只放普通 div + 文字就碰不到（第 4 节，exp009 7 次运行里从没出现）。（读 PR 描述与 diff）
4. **GPUI 的 `div` 只开放了一部分 AccessKit 属性**：名称、描述、值、占位、选中、展开、勾选、数值、层级、位置、行列、方向、快捷键、`accessibility_id`。**没有**：禁用、必填、忙碌、无效、模态、Live、labelled_by / described_by / controls、角色描述。这些要走 `a11y_synthetic_children(|b| b.parent_node().set_xxx())` 这个出口直接改 AccessKit 节点（exp001 用过，读源码确认仍在：`Z:gpui/src/window/a11y.rs:364`）。
5. **到了 macOS 那一层又掉一批**：`expanded`、`level`、`position_in_set` / `size_of_set`、`row_count`、`keyboard_shortcut`、`busy`、`invalid`、`labelled_by` 的关系本身、`described_by`，accesskit_macos 0.26.3 一概不往 NSAccessibility 送（`ak:` 里零引用）。**VoiceOver 拿得到的只有**：角色 / 子角色、标题（名称）、帮助（描述）、值、占位、选中、禁用、必填、模态、焦点、方向、数值上下限、标识符、`controls`（变成 AXLinkedUIElements）、文本范围那一套。详见第 2 节表。

6. **两个 11-13 刚开的上游 issue 直接打到我们**：#66691 应用级焦点元素报成 AXWindow（VO 切回应用落在窗口上；薄壳可先用 `class_addMethod` 绕）；#66690 `Entity::cached` 的视图从缓存画时丢无障碍节点（规则：`atg-gui` 不用 `.cached`）。（读 issue，第 4、6 节）
7. **播报**：仍无正式接口，出口那条路还在；但 accesskit_macos 0.27 把 Live 的触发字段从「值」改成「名称」，所以**名称和值设同一句**才能跨版本不哑。（读上游提交，第 3 节）
8. **测试**：外部 crate 仍激活不了测试平台的无障碍；GPUI 自己的测试能（`pub(crate)`）。给上游加一个 test-support 开关是小改动、高价值。（第 6、8 节 P6）
9. **补丁**：升级 AccessKit 补不上 macOS 缺口（main 的方法集合与 0.26.3 相同）；**AccessKit 不收外部 PR，要先开 issue 附 Chromium 对照**；Zed 收 bug 修复 PR，功能要先讨论。建议先做 P10（焦点落窗口）、P6（测试开关）。（第 8 节）
10. **版本**：crates.io 的 `gpui` 太旧，`gpui-pre` 是第三方快照；用 git + 锁 rev，`[patch.crates-io]` 表要随 rev 一起抄（已变过一次）。只编 gpui 的 debug 冷编译约 29 秒、2.1 GB（第一次量，待重测）；本机缺 Metal Toolchain，要开 `runtime_shaders` 或人装工具。（第 7 节）

## 1. 上游变了什么（问题 1）

**提交面**（`git log 79fe6a6..15d136b`，读源码）：

- 无障碍核心文件零改动：`Z:gpui/src/window/a11y.rs` 最后一次改动仍是 2026-09-23 的 #64365；`_accessibility.rs` 最后是 2026-06-17 的 #59429
- 动了的大文件都是无关重构：#66719（Div 的 Interactivity 移出行内）、#64864（去手写 IntoElement）、#66156（清理）、#64056（`list` 横向内边距）、#65288（`uniform_list` 长列表布局）——对 a11y 行逐行比对，内容不变
- `gpui_macos` 的改动（#66934 通知服务挪出 UI 线程、#65994 要求 macOS 12 + SDK 26、#65744 退格键等）不碰无障碍
- 测试平台 `Z:gpui/src/platform/test/` 只改了 `cfg` 和导入，**仍然没有任何 a11y 代码**（`grep` 零命中）
- accesskit 版本（`Cargo.lock`）：accesskit 0.24.1、consumer 0.38.0、macos 0.26.3 —— 与 10-23 相同
- 期间合并的、标题带无障碍字样的 PR 只有 UI 组件层的（如 #66937 图标切换按钮），不改 GPUI 接口

**上游正在进行的**（GitHub 开放 PR，文档说）：

| PR | 内容 | 对本项目 |
|---|---|---|
| #65740（10-17 开，未合，有一条评审意见）| `Window::transact` 失败时回滚无障碍节点；修 `list` 自动滚动重做 prepaint 时的重复节点 panic（关联 issue #65623） | **直接相关**，见第 4 节 |
| #66631 | Windows 端焦点 | 无关 |
| #57579（5 月开，未合）| macOS「全键盘控制」下的文本输入 | 可能相关，第 5 节再看 |

**001 五条逐条复核**（001 = `docs/research/001_stack/findings-gpui.md`）：

| 001 条目 | 今天 | 新出处（HEAD `15d136b`） |
|---|---|---|
| 1 列表项进树（角色 / 名称 / 选中 / 位置），前提 `.id()` + `.role()` | **成立** | `Z:gpui/src/elements/div.rs:1357,1379,1441,1510,1516`（行号没变）；推树 `Z:gpui/src/element.rs:435-449` |
| 2 代码移焦点 VoiceOver 跟随；`aria_active_descendant` | **成立** | `div.rs:1418`；`window/a11y.rs:258-272`（没变） |
| 3 虚拟化列表屏外项不进树；读屏的滚动动作 GPUI 不处理 | **成立** | `uniform_list.rs:475-492`；`list.rs:1301-1306`（只给要画的项 prepaint）；GPUI 只内置 Click / Focus / Blur，其余记日志 `Z:gpui/src/window.rs:7537-7573` |
| 4 没有 `aria_live`；可经 `a11y_synthetic_children` / `write_a11y_info` 设 Live | **成立** | `window/a11y.rs:364`（`parent_node()`）；`element.rs:121`（`write_a11y_info`）；`ak:event.rs:35-44` |
| 5 `debug_a11y_tree_json` 有；测试平台拿不到树 | **成立**（细节见第 6 节：GPUI 自己的 crate 内测试能造一个「已激活」的 a11y，外部 crate 不能） | `window.rs:7493`；`platform.rs:1484` 默认空实现；激活回调 `window.rs:1776-1790` |

另外两处 001 没提、但一直就有的公开接口：

- **`.accessibility_id("…")`**（`div.rs:1373`，#61926，2026-08 合并）：设一个**进程外可见**的稳定标识，macOS 上就是 `AXIdentifier`（`ak:node.rs:502-508`）。对「agent 只读树操作」这把尺子很要紧：agent 认元素可以不靠名称（名称会随读法变）。**建议薄壳里每个可操作元素都设**
- **`Window::on_a11y_action(node_id, action, f)`**（`window.rs:7502`）：按节点 id 挂动作监听，不必挂在 div 上。合成子节点（Live 节点、文本片段）要接动作时用它

## 2. 角色与属性全表（问题 2）

### 2.1 GPUI `div` 能设的（`Z:gpui/src/elements/div.rs`，写入点 `:3635-3707`）

| GPUI 方法 | AccessKit 字段 | macOS 层拿到什么（读源码） | VoiceOver 读不读（推断，没人听过） |
|---|---|---|---|
| `.role(Role)` `:1357` | role | 角色 + 子角色，见 2.3 | 读角色名 |
| `.accessibility_id(s)` `:1373` | author_id | `AXIdentifier` `ak:node.rs:502` | 不读（给 agent / 脚本） |
| `.aria_label(s)` `:1379` | label | `AXTitle` `ak:node.rs:510`（根窗口节点除外，`ak:node.rs:306-313`）| 读（名称） |
| `.aria_description(s)` `:1388` | description | `AXHelp` `ak:node.rs:519` | 停顿后读「提示」（与 VO 提示设置有关） |
| `.aria_keyshortcuts(s)` `:1398` | keyboard_shortcut | **没有**（`ak:` 零引用）| 不读 |
| `.aria_active_descendant()` `:1418` | 由 GPUI 写进 TreeUpdate 的 focus（`window/a11y.rs:258-272,513-528`）| 焦点元素变成那一项 | 读那一项（exp003 实测树层 + macOS 层） |
| `.aria_selected(b)` `:1441` | selected | `isAccessibilitySelected` `ak:node.rs:1031`；父为可选容器（ListBox、Tree、Grid、TabList…，`akc:node.rs:921-938`）时还进 `AXSelectedChildren` | 读「已选」 |
| `.aria_expanded(b)` `:1447` | expanded | **没有**（`ak:` 零引用）| **不读「展开 / 收起」** |
| `.aria_toggled(Toggled)` `:1453` | toggled | `AXValue` 布尔 + 按钮时子角色 `AXToggle`（`ak:node.rs:323-327,242`）| 读「已勾选」等 |
| `.aria_numeric_value / _step / min / max` `:1459-1492` | numeric_value 等 | `AXValue` / `AXMinValue` / `AXMaxValue` `ak:node.rs:583-598`；step 无 | 读数值 |
| `.aria_value(s)` `:1473` | value | `AXValue` 字符串 | 读值 |
| `.aria_placeholder(s)` `:1480` | placeholder | `AXPlaceholderValue` `ak:node.rs:528` | 空输入框时读 |
| `.aria_orientation(o)` `:1498` | orientation | `AXOrientation` `ak:node.rs:610` | 一般不读 |
| `.aria_level(n)` `:1504` | level | **没有** | 不读 |
| `.aria_position_in_set / size_of_set` `:1510,1516` | position_in_set / size_of_set | **没有**（exp003 实测：`accessibilityIndex` 等都不存在） | 不读 |
| `.aria_row_index / column_index / row_count / column_count` `:1522-1540` | 同名 | **没有**；表格的行列表来自子节点（`ak:node.rs:1080-1110`），不来自计数 | 只数得到已渲染的行 |
| `.on_a11y_action(Action, f)` `:1550` | 声明支持该动作 `:3705` | `AXActionNames` / `accessibilityPerformAction:` `ak:node.rs:1181-1206` | 给 VO 动作菜单 / 语音控制用 |
| `.on_click` / 可聚焦 | 自动加 Click / Focus 动作 `:3699-3703` | `AXPress` / 可设焦点 | — |
| `.tab_stop / .tab_index` `:824,833` | 不进树；管 GPUI 自己的 Tab 顺序 | — | — |
| `.a11y_synthetic_children(f)` `:1432` | 任意：子节点 + `parent_node()` 改本节点 | 看设了什么 | — |

### 2.2 `div` 没开放、要走 `parent_node()` 出口的

这些 AccessKit 有、macOS 适配层也用，但 GPUI 没给 `aria_*` 方法（`Z:gpui/src` 里 `set_disabled` / `set_live` / `set_modal` / `set_required` / `set_controls` / `set_labelled_by` 零命中，读源码）：

| 想要 | AccessKit 方法 | macOS 层 | 用途 |
|---|---|---|---|
| 禁用 | `set_disabled()` | `isAccessibilityEnabled = false`（`ak:node.rs:634-636`） | 发送按钮在输入为空时 |
| 必填 | `set_required()` | `isAccessibilityRequired`（`ak:node.rs:1024`） | 登录表单（T01） |
| 模态 | `set_modal()` | `isAccessibilityModal`（`ak:node.rs:1167`） | 确认发送对话框（M4.2） |
| Live | `set_live(Live::Polite/Assertive)` | 值变化时发公告，优先级 50 / 90（`ak:event.rs:35-44`） | 播报器（第 3 节） |
| controls | `push_controls(id)` | `AXLinkedUIElements`（`ak:node.rs:1135-1150`） | 搜索框 → 结果列表 |
| labelled_by | `push_labelled_by(id)` | 关系不送 macOS；但 consumer 拼名称时会读被引用节点的名称（`akc:node.rs:731-747`），所以**名称能借过来** | 输入框借旁边标签的字 |
| described_by | `push_described_by(id)` | 没有（`ak:` 零引用） | 不用 |
| 角色描述 | `set_role_description(s)` | `AXRoleDescription`（`ak:node.rs:490-500`） | 把「组」换成「消息」？要人听，见待定 |
| 盲文标签 | `set_braille_label(s)` | `AXBrailleLabel`（`ak:node.rs:1066-1077`） | 以后给盲文显示器缩写 |

⚠️ 注意（读源码）：`parent_node()` 拿到的是**本帧**已由 `write_a11y_info` 写过的节点，出口里再设同名字段会覆盖 `aria_*` 设的值；GPUI 不防重复设。薄壳里定一条：**同一字段只从一处设**。

### 2.3 角色怎么映射、VoiceOver 读成什么（`ak:node.rs:36-232` 角色、`:233-286` 子角色）

只列本项目可能用的（读源码；「VO 读法」一列是按 macOS 角色推的，**没开 VoiceOver 实测**，exp003 只实测了 List/ListItem/ListBox/ListBoxOption 这四个）：

| AccessKit 角色 | macOS 角色 / 子角色 | VO 大概读成 | 本项目用在 |
|---|---|---|---|
| `List` | AXList | 「列表」 | 会话列表、消息列表（001 拍板 1）——**实测** |
| `ListItem` | **AXGroup** | 「组」 | 列表项——**实测**（exp003）|
| `ListBox` / `ListBoxOption` | AXList / **AXStaticText** | 「列表」/「文本」，多报行数（只数可见） | 备选——**实测** |
| `Button` / 带 toggled | AXButton / AXToggle | 「按钮」/「切换按钮」 | 发送、静音 |
| `CheckBox` / `Switch` | AXCheckBox / AXSwitch | 「复选框」/「开关」 | 设置 |
| `TextInput` / `SearchInput` / `PasswordInput` | AXTextField / AXSearchField / AXSecureTextField | 「文本框」/「搜索框」/「安全文本框」 | 登录、搜索 |
| `MultilineTextInput` | AXTextArea | 「文本区域」 | 输入框（多行） |
| `Label` | AXStaticText | 读文字 | 标签 |
| `Heading` | `"Heading"`（字面字符串，不是 AX 常量）| 「标题」？未验证 | 分区标题 |
| `Status` | AXGroup / AXApplicationStatus | 「状态」？未验证 | 播报节点（exp001 用过） |
| `Log` | AXGroup / AXApplicationLog | 未验证 | 不建议用（读屏对 log 的默认行为不可控）|
| `Dialog` / `AlertDialog` | AXWindow / AXDialog | 「对话框」 | 确认步骤 |
| `Toolbar` | AXToolbar | 「工具栏」 | 顶栏 |
| `Tab` / `TabList` | AXRadioButton·AXTabButton / AXTabGroup | 「标签页」 | 以后 |
| `Main` / `Navigation` / `Search` / `Region` | AXGroup + `AXLandmark*` | 「主要」「导航」等地标，VO 转子可跳 | 会话区、消息区（#60397 在 Zed 里就是这么用的） |
| `Article` | AXGroup / AXDocumentArticle | 未验证 | 消息项的另一种候选 |
| `Image` | AXImage | 「图像」 | 图片附件 |
| `ProgressIndicator` / `Meter` | AXProgressIndicator / AXLevelIndicator | 「进度指示器」 | 下载、语音播放进度 |
| `Slider` | AXSlider | 「滑块」，可增减 | 播放进度、倍速 |
| `GenericContainer` / `ScrollView` / `Pane` / `Feed` / `TextRun` | AXUnknown | 被跳过 | 不要给要读的东西用 |
| `Window`（根） | AXGroup，**不带标题**（`ak:node.rs:306-313`：带标题 VO 会坏） | — | GPUI 根节点 |

### 2.4 动作（`on_a11y_action` 能接什么）

macOS 适配层只会发出这些动作请求（读源码，`ak:node.rs:639-1206`）：

| 读屏做了什么 | macOS 调用 | AccessKit 动作 | GPUI 默认 |
|---|---|---|---|
| VO+空格 按下 | `accessibilityPerformPress` | Click | 内置：在节点中心模拟一次鼠标点击（`window.rs:7538-7557`）——**元素必须有 bounds** |
| VO 光标落到可聚焦项（且「键盘焦点跟随 VO」开着）| `setAccessibilityFocused:` | Focus / Blur | 内置：移键盘焦点（`window.rs:7558-7567`）|
| 滑块 VO+↑/↓ | `accessibilityPerformIncrement/Decrement` | Increment / Decrement | 不处理，要自己接 |
| 改文本框的值 | `setAccessibilityValue:` | SetValue | 不处理（Zed 设置页自己接了，见第 5 节） |
| 选中某项 / 选取 | `setAccessibilitySelected:` / `accessibilityPerformPick` | Click——只对「可点、像列表项、可选」的节点发（`ak:node.rs:1042-1062,1113-1130`）| 同 Click |
| 改选区 / 光标 | `setAccessibilitySelectedTextRange:` | SetTextSelection（`ak:node.rs:1007-1022`）| 不处理 |
| 滚进可见区 | `accessibilityPerformAction:` 只认 `AXScrollToVisible` 一种（`ak:node.rs:1181-1206`）| ScrollIntoView | 不处理（exp003b 接过）|

macOS 层**没有**「显示菜单」（AXShowMenu）、「取消」、「确认」这类动作的转发：`AXActionNames` 只会列出 ScrollToVisible（`ak:node.rs:1181-1189`），按压走 `accessibilityPerformPress`。所以消息上的「回复 / 删除」等不能指望 VO 动作菜单，**一律走应用自己的快捷键**。
自己挂的 `on_a11y_action` 监听会先跑；匹配上就不走内置（`window.rs:7516-7533`）。

**自定义动作 / 自定义转子（ga11y 来问，2026-11-14 核，读源码）**：
- `CustomAction`：AccessKit 数据结构里有（`accesskit-0.24.1/src/lib.rs:300` 动作、`:761` 结构、`:2323` `set_custom_actions`），经 `parent_node()` 写得进去；但**没有任何适配器读它**——accesskit_macos 0.26.3 零引用，上游 main（0.27.2）的 macOS 适配层也零引用，Windows / AT-SPI 同样没有。VoiceOver 的「操作」转子里不会出现
- `CustomRotor`：AccessKit 没有这个概念
- 自补两条（推断，未实测）：① 转子挂在 `NSWindow` 上（AppKit 自己的对象，`setAccessibilityCustomRotors:`，不碰 AccessKit）；屏外项不在树里、委托又是同步的，所以只能返回已渲染项或列表容器，补不全虚拟化缺口。② 每项自定义动作要在运行时给 AccessKit 的 PlatformNode 类加 `accessibilityCustomActions`，用 `accessibilityIdentifier` 认项——能做但脆
- 建议：自定义动作给 AccessKit **开 issue**（AccessKit 不收外部 PR，第 8 节 P1；草稿 `upstream-drafts/accesskit-macos-custom-actions.md`）；转子列为薄壳实验候选（P15）；次要操作的正本仍是快捷键

---

## 3. 播报（问题 3）

**现状（读源码）**：

- GPUI 仍然**没有** live region 正式接口：`div` 没有 `aria_live`，`Z:gpui/src` 里 `set_live` 零命中；Zed 自己的代码里也一处 Live 都没用（`grep Live::` 零命中）。上游 issue / PR 里没找到相关计划（按 live region、aria-live、announce 搜，2026-11-14）
- 001 那条路**还在**：`a11y_synthetic_children` → `parent_node()`（`window/a11y.rs:364`）或 `push_child`（`:346`）拿到 AccessKit 节点，自己 `set_live` + 设文字；accesskit_macos 发 `NSAccessibilityAnnouncementRequestedNotification`（`ak:event.rs:35-44, 236, 299-305`）
- ⚠️ **AccessKit 升级会改播报的触发字段**（读上游提交 `e6ae72f`，#782，进了 accesskit_macos 0.27.0）：
  - 0.26.3（Zed 现用）：**值（value）**变了才播，播的是值
  - 0.27.0 起：**名称（label）**变了才播，播的是名称（`Role::Label` 例外，它的名称取自值，`akc:node.rs:727-729`）
  - 所以 exp001 那种「只设值」的 Status 节点，**Zed 一升级就哑了**。对策：**名称和值设成同一句**，两个版本都播；升级后改成只设名称
  - **实测（exp009，0.26.3，7/7 次）**：只设值 → 1 条公告；只设名称 → **0 条**；两者都设 → 1 条。实验每次换节点键，所以只测到 accesskit 的「新增节点」分支（`ak:event.rs:236`）；「同一节点值变化」分支（`:299-305`）和 0.27 那半边（只设名称会播）是读源码
- Zed 升级 accesskit 的时间点不可控（上游 accesskit 已到 0.25.2 / macos 0.27.2，Zed 还在 0.24.1 / 0.26.3）。我们锁 rev（第 7 节），升级时必须重跑播报自测

**与 exp001 / exp004 合起来，给播报器的推荐**（推断 + 实测证据）：

| 场景 | 出路 | 依据 |
|---|---|---|
| 新消息到达（前台）| 提示音 + 写进列表名 / 标题；停手后按估算时长说一句汇总；按 m 收听 | 001 拍板 3；exp004 模式 3、5、6（实测）；人试听：Polite 也会插话（exp001） |
| 明确的状态变化（已发送、断开 / 已连接、下载完成）| Live 节点（Status 角色，名称 = 值 = 同一句；重复句换节点键）| exp001 结论 1、3（实测）；本节 0.27 变化 |
| 低优先级、后台、被挡住 | 直发公告（主线程调 `NSAccessibilityPostNotificationWithUserInfo`，可用优先级 10）| exp001 结论 5、exp004 结论 1、5（实测）|
| 应用在后台的新消息 | 系统通知 | 001 拍板 4 |

封装要点（读源码）：

1. 播报节点要有 `.id()` + `.role(Role::Status)`，id 稳定；重复同一句时**换合成子节点的键**（`window/a11y.rs:335-344` 由父 id + 键哈希出 id），不往文字里塞零宽字符
2. 切换 Live 时先清空文字（exp001 结论 4）
3. 树导出里看不到 `live` 字段（exp001 末段；`window/a11y/debug.rs` 至今没加），薄壳的树导出要自己记一笔
4. 窗口被挡住时 GPUI 不出帧，Live 路径会滞后（exp004 风险）；需要「必达」的提示只走直发

## 4. 虚拟化列表（问题 4）

- **屏外项不进树：仍然如此**（第 1 节第 3 条）。上游没有任何「虚拟化列表给读屏报全量 / 懒加载」的 issue 或 PR（按 uniform_list accessibility、virtualized list screen reader 搜，2026-11-14）
- 相关的新 issue **#66730**（11-13 开）：`overflow_y_scroll` 容器里被裁掉的元素仍报「可见」，提议给完全在裁剪区外的节点设 `hidden`。和虚拟化列表无关（虚拟化列表压根不渲染屏外项），但提醒一件事：**普通滚动容器里所有子项都在树里**，VO 能走到看不见的地方，VO 发 ScrollToVisible，GPUI 又不处理（第 2.4 节）。薄壳里非虚拟化的滚动区要自己接 ScrollIntoView 或保持短
- **「容器持焦点 + active_descendant + 先滚后选」仍是最佳方案**。理由：GPUI 接口没变；exp003 / exp003b-v2 的实测（1137 步、45 步 0 失败）仍适用；上游无替代方案
- **变高消息用 `list` 的代价**（读源码）：
  - `ListState::scroll_to_reveal_item(ix)`（`list.rs:680`）按已量过的高度算滚动位置；没量过的项按估计值，第一次滚过去可能差一点，下一帧修正。
  - **实测更正（exp009 R1）**：不是「差一点」——**往下跳到没量过的项时根本不滚**：overdraw 为 0 时屏外项都没量过，`scroll_to_reveal_item` 算出的目标位置等于当前位置，于是目标项不渲染、不进树，active descendant 落空、焦点退回列表容器（9 步错 8 步；最后的树里始终只有第 1–5 项）。R2–R7 比了三种改法：`measure_all()` 9/9（R3）、`scroll_to(ListOffset)` 9/9（R4）、逐项 ↓ + overdraw 200 px 116/120 与 120/120（R5、R6；R5 错的 4 步目标项在树里，是焦点漂移，见下一条）；reveal 三次 R1、R2、R7 分别错 8/9、8/9、7/9。**结论：逐项 ↓ 用 `scroll_to_reveal_item` + overdraw；跳跃（PageDown、End、落到第一条未读、从通知跳到某条）用 `scroll_to(ListOffset)`；不用 `measure_all()`（首帧排版全部项，长会话不行）**。active descendant 本身没问题：目标项进了树就是它
  - ⚠️ **焦点漂移（exp009 运行表 R1–R7 里 3/7 次；原因未定，可信度：推断）**：代码只在动列表时，GPUI 焦点跑到输入框，之后又回来（R1 选 120 起、R2 选 299 起、R5 选 39→36）。这 3 次都没挂焦点记录。挂了记录的 R6、R7（VO 开着，`isVoiceOverEnabled = true`）各抓到 1 次外部「设焦点」，都在窗口刚出来时、都指向**列表**，两次都没漂移；R7 里 active descendant 落空 7 步，焦点也没被带走。发请求的进程在进程内拿不到
    - 解释：① VoiceOver「键盘焦点跟随 VO 光标」——VO 光标落到输入框时发 Focus，GPUI 内置处理移走键盘焦点（`window.rs:7558-7563`）；② 应用自己让焦点节点消失、VO 另找落点（薄壳缺陷）——**被 R7 削弱**；③ 人当时在操作电脑（如点了输入框；没记 Click 动作）
    - **结论：原因未定，不再追**。建议 `atg-gui` 调试日志记下每个外部 Focus / Click 动作及目标，GUI 开发中真碰到再查。逐次数据见 exp009 README「运行表」
  - **只有坐实是解释 ①，才需要定这条策略（现状页第 5 条，先别问人）**：A. 照常移（GPUI 默认；语音控制、切换控制这类辅助技术就是靠它操作界面，`.42cog/intent.md` 也要求不只服务读屏）· B. 只在 VoiceOver 开着时移（`NSWorkspace.isVoiceOverEnabled`），否则记日志不移——会挡住语音控制等别的辅助技术 · C. 一律记日志不移——最稳但等于关掉 AX 移焦点，违背「无障碍树就是 agent 的接口」。实现上都只是给可聚焦元素挂 `on_a11y_action(Focus)` 自己决定（自己的监听会先于内置处理，`window.rs:7516-7533`）。**建议 A**，另加：焦点被外部移走时不播报、不改选中项，自测运行时排除外部客户端。若是解释 ②：在薄壳里保证焦点节点每帧都在树里（选中项先进树再设 active descendant）
  - active descendant 只在目标节点已在树里、且焦点是它的祖先时才生效（`window/a11y.rs:269-271`），否则焦点退回容器
  - **GPUI 的 active descendant 其实是「换焦点」**（读源码）：帧末把 TreeUpdate 的 focus 直接换成那个节点（`window/a11y.rs:567-579`）。所以 macOS 层的无障碍焦点会落到选中项上，不是 ARIA 那种「焦点留在容器、属性指向选中项」。对列表这正是想要的；但**搜索框 + 结果列表（combobox）做不出「焦点留在输入框、高亮读得出」**：就算放开祖先检查，焦点也会离开输入框，打字回显没了。gssot 卡 G17 的三条路：A ↓ 把焦点移进结果列表（规约默认）；B 焦点留在输入框，结果用 ListBox + `aria_selected`，靠 accesskit_macos 的「选中变化」通知（`ak:event.rs:211`）——VO 焦点不在列表时念不念，要人试；C 高亮变化经播报器说一句。建议 A，B 列实验候选
  - **PR #65740 的坑**：`list` 的某项在 prepaint 里调 `window.request_autoscroll` 时，`list` 在同一帧重做 prepaint（`list.rs:1281-1370` 的 `transact` + 返回 `Err(ListOffset)`），无障碍节点推两遍：**调试构建 panic，发布构建丢节点、位置是滚动前的**。会调 `request_autoscroll` 的是 Zed 的编辑器和 markdown（`markdown/src/markdown.rs:2630`）。所以：**消息项只放普通 div + 文字，不嵌编辑器 / markdown 元素；滚动只用 `ListState` 自己的方法（逐项 ↓ 用 `scroll_to_reveal_item`，跳跃用 `scroll_to(ListOffset)`，见上一条；两者都不经 `request_autoscroll`）**，就碰不到这条；要嵌的话先等 #65740 合并或把它打进补丁（第 8 节）
  - `uniform_list` 每项等高，跨屏最稳（exp003 实测）；`list` 的先滚后选见上（exp009 实测）
- **别用 `Entity::cached` 包消息列表或其中的视图**：新 issue **#66690**（11-13 开，附可复现程序，读 issue）——从缓存画出的视图不再推无障碍节点，焦点退回整窗，直到那个视图自己被 notify。一个状态栏闪烁就能把列表从树里抹掉。**实测（exp009）**：只重画根视图后缓存视图里的按钮不在树里，7 次运行里 6 次复现；缓存视图自己被 notify 后才进树。跟帧时序有关，不是每次都掉。在上游修好前，`atg-gui` 里**禁用 `.cached(..)`**（可以写进 `specs/gui` 当规则，机器可查：`grep '\.cached('`）

## 5. 文本输入（问题 5）

- **GPUI 本体没有可用的文本输入元素**（读源码）。`Z:gpui/examples/input.rs`（912 行）是示例：实现 `EntityInputHandler`（输入法要的 marked text、`replace_and_mark_text_in_range` 等，`:288-372`），**一个 role / aria 都没设**。Zed 的 `Editor` 在 `crates/editor`，本身也不进无障碍树（`editor/src` 里只有调试悬浮窗按钮有 `aria_label`）
- **输入法（中文）这一面 GPUI 是支持的**：`ElementInputHandler`（`input.rs:609`）接到 macOS 的 NSTextInputClient，marked text / 候选窗走系统。候选窗是输入法自己的窗口，VoiceOver 读候选靠的是输入法进程，不靠我们（推断，要人试）
- **无障碍这一面有现成先例**：Zed 设置页的输入框 `Z:settings_ui/src/components/input_field.rs`（#59429、#60397）——外层 div 设 `Role::TextInput` + 名称 + 值 + 占位，`on_a11y_action(SetValue)`；`a11y_synthetic_children` 推若干 `Role::TextRun` 子节点（每段 ≤255 字，因为 `word_starts` 是 u8），设 `character_lengths`（每字 UTF-8 字节数）、`word_starts`、首尾相连，再在父节点设 `text_selection`（光标 / 选区）。另一个要学的细节：**获焦期间值冻结在获焦那一刻**，失焦再同步，因为读屏在获焦控件的值每变一次就把整段重读一遍（源码注释 `input_field.rs:366-369`，原注释讲的是通用读屏行为）
- **macOS 层能接住**（读源码）：有了 TextRun + 选区，accesskit_macos 给出 `accessibilityNumberOfCharacters`、`SelectedTextRange`、`RangeForLine`、`StringForRange`、`AttributedStringForRange`、`setAccessibilitySelectedTextRange:` 等（`ak:node.rs:731-1022`），并在选区变化时发 `AXSelectedTextChanged`（`ak:event.rs:289-296`）
- ⚠️ **但光标移动时 VO 读字 / 读词，这条没把握**（推断）：AppKit 原生文本框发 `AXSelectedTextChanged` 时带 userInfo（变化类型、方向、粒度），VoiceOver 靠它决定读一个字还是一个词；accesskit_macos 0.26.3 和上游 main 发的都是**不带 userInfo** 的通知（`ak:event.rs:289-296`；`grep TextStateChange` 零命中）。**实测（exp009）**：光标移 4 次 → 4 条 `AXSelectedTextChanged`，4 条都不带 userInfo。可能的结果：移动光标时 VO 不读或读整行。**必须人试**（exp009 只能证明通知发了）。不行的话这是 accesskit_macos 的缺口（第 8 节 P12）
- 中文的 `word_starts`：先例用 `char::is_alphanumeric` 切词，中文连续汉字会被当成一个词（推断）；VO 按词读时整句当一词。我们可以自己按标点 / 空格切，或先不设词边界，要人试
- **最小可行做法**（与 gcomp 分工：我只管无障碍面）：用 gcomp 选定的输入元素（`examples/input.rs` 那种 `EntityInputHandler` 自写，或组件库的）→ 外层包一层，照 `input_field.rs` 的结构设角色、名称、冻结值、TextRun、选区、`SetValue` 动作；多行用 `Role::MultilineTextInput`（AXTextArea）。上游未合 PR #57579（macOS 全键盘控制下的文本输入，5 月开）在合并前不依赖它

## 6. 树导出与测试（问题 6）

- `Window::debug_a11y_tree_json()`（`window.rs:7493`）、`Window::is_a11y_active()`（`:7488`）：接口没变，只在辅助技术激活后有内容（`window.rs:1776-1790` 激活回调）
- **测试平台仍然激活不了**：`platform/test/` 无 a11y 代码，`PlatformWindow::a11y_init` 默认空（`platform.rs:1484`）
- **新知道的一点**（读 PR #65740 的 diff）：GPUI **自己 crate 里的测试**可以直接换掉窗口的 a11y 状态——`window.a11y = A11y::new(Arc::new(AtomicBool::new(true)), false, None)`，然后画一帧、读 `last_tree_update()`。但 `A11y`、`window.a11y` 都是 `pub(crate)`（`window/a11y.rs:126`、`window.rs:1322`），**外部 crate（我们的 atg-gui）用不了**。所以给上游加一个 test-support 的公开开关（例如 `VisualTestContext` 上「激活无障碍」），改动小、有先例可抄——见第 8 节 P6
- **在那之前 GUI 测试分两层**（推断，给 gssot 定 `specs/gui` 测试正本用）：
  1. **纯函数层（CI 能跑）**：薄壳把「视图模型 → 每个元素该设的无障碍属性」写成纯函数，输出本项目自己的小结构（角色、名称、标识、选中……），快照测它。GPUI 只是把这个结构抄进 `.role()` / `.aria_*()`。这一层不需要 GPUI 激活
  2. **真窗口层（本机、要前台、≤1 分钟）**：exp003 的做法照旧——进程内问 contentView 激活、导出 GPUI 树 + macOS 层逐项问
- ⚠️ **macOS 层的「焦点元素」要问两处**（新 issue **#66691**，11-13 开，读 issue）：AccessKit 子类化的是 contentView，但 GPUI 把自己的 `GPUIView` 设成第一响应者，所以 `[NSApp accessibilityFocusedUIElement]` 答的是 **AXWindow**，只有直接问 contentView 才得到真焦点。exp003 问的是 contentView，所以没暴露这个问题。**用户感受**：VO 切回本应用时落在窗口上，要自己往里走（issue 作者只做了进程内验证，没听 VO）。树导出要同时记「contentView 答的」和「NSApp 答的」。issue 评论里的权宜做法：启动时用 `class_addMethod` 给 `GPUIView` 类加 `accessibilityFocusedUIElement`，转问父视图（AccessKit 的答案）——这是薄壳可以先做的绕法，同时是一个上游 PR 题目（第 8 节 P10）
  - **实测（exp009）**：输入框获焦时 contentView 答 `AXTextField「输入消息」`，`NSApp` 答 `AXWindow「exp009」`，第一响应者类名 `GPUIView`——复现。装上权宜做法后 `NSApp` 也答 `AXTextField「输入消息」`——7/7 次运行复现，装上后 `NSApp` 的答案与 contentView（AccessKit）一致 7/7，**进程内有效**；VO 切回应用时实际落在哪，没听，要人听
- **最小脚本**：exp009 的 `src/ax.rs`（激活、两处焦点、截获通知）可作树导出器的起点

## 7. 版本锁定（问题 7）

- **crates.io 上的 `gpui` 停在 0.2.2（2025-10）**，在 AccessKit 接入（2026-05）之前，不能用（crates.io API，2026-11-14 查）
- `gpui-pre`（0.3.12 = zed@d2a4fb6，每周一版）是**第三方**发的快照（crates.io 所有者是组件库 gpui-component 的作者，不是 Zed 官方）。好处是走 crates.io；坏处是多一个供应链环节、版本跟着别人走、要打补丁还得再套一层。**不建议**
- **建议：git 依赖 + 锁 rev**（官方仓，可复现，CI 能拉）：

  ```toml
  gpui = { git = "https://github.com/zed-industries/zed", rev = "<40 位>" }
  gpui_platform = { git = "https://github.com/zed-industries/zed", rev = "<同一个>", features = ["font-kit"] }
  ```
  另外**必须**照抄那个 rev 的 Zed 根 `Cargo.toml` 里 `[patch.crates-io]` 中 gpui 依赖树要的几条。⚠️ 这张表会随 rev 变：`79fe6a6` 时 async-task 用 Zed 的分支，`15d136b` 已改成 smol-rs 的 `41fd335`，还多了 async-process（`Z:../Cargo.toml:1044-1048`）——exp001 的 Cargo.toml 已经过期。升级时这是第一个要对的东西（可以写个脚本从 Zed 根 Cargo.toml 抽，交给 gssot 定是否做成正本比对）
- **打补丁时**（common 第 15 条）：fork 上开 `atg/<缺口名>` 分支，本项目用 `[patch."https://github.com/zed-industries/zed"] gpui = { git = "<fork>", rev = "<补丁 rev>" }`；AccessKit 层的补丁用 `[patch.crates-io] accesskit_macos = { git = "<fork>", rev = … }`（必须与 Zed 锁的 0.26.x 同一大版本，否则两份 accesskit 类型对不上）。**不建议 subtree**：tdlib-rs 那样搬进本仓对 Zed 不现实（整个仓 191 MB 只是不含历史 blob 的检出）
- **升级成本**：改 rev → 对 `[patch.crates-io]` 表 → 编 → 重跑播报自测（第 3 节 0.27 的坑）+ 树导出比对（001 R7）。三周 273 个提交、a11y 零改动，说明 a11y 部分最近不活跃，升级主要成本在编译和非 a11y 的接口变动（如 #64864 去手写 `IntoElement`）
- **工具链**：Zed 要求 macOS 12 + SDK 26（#65994）；本机 SDK 27.0、Rust 1.99.1，Zed 锁 1.98.1（`rust-toolchain.toml`）——够用（读命令输出）
- **冷编译与盘占用（exp009，2026-11-14；⚠️ 待重测）**：最小 GPUI 应用（gpui + gpui_platform，path 依赖 `15d136b`）debug 冷编译 **29 秒**（第一次量；`target/` 删过再编，但日志只存了末 6 行，证明不了是真冷编译，已申请名额重测），`target/` **2.1 GB**；改本实验代码后增量 **2.2 秒**。机器 Apple M5 10 核；crate 下载已在本机缓存（只算编译）；没有 sccache。比 `.42cog/real.md` 担心的「几十 GB」小得多——那是编整个 Zed 的量级，只编 gpui 不是。release 与整个 atg-gui 未量
- ⚠️ **本机没有离线编 Metal 着色器的工具**（实测）：`xcode-select` 指向 Command Line Tools，`xcrun metal` 找不到；指到 `/Applications/Xcode.app` 也报「missing Metal Toolchain」，要 `xcodebuild -downloadComponent MetalToolchain`（改系统，没做）。默认特性下 `gpui_apple` 的 build.rs 因此失败（`out/cold-build-attempt1-metal-missing.log`）。**绕法**：`gpui_platform` 开 `runtime_shaders` 特性（`Z:gpui_platform/Cargo.toml:25`），运行时编着色器，exp009 就是这么编过的。代价：启动时多一次着色器编译（未量）；CI 机器同样要定。要人定（现状页）

## 8. 补丁方案（问题 8；按 common 第 15 条从「缺口清单」升级）

**三条先决事实**：

1. **AccessKit 升到最新也补不上 macOS 那批**：accesskit_macos 上游 main（0.27.2）向 NSAccessibility 暴露的方法（`method(...)` 列表）与 0.26.3 **逐个相同**（diff 为空）；`accessibilityAttributeValue:` 里多送了 `AXDOMIdentifier` 等网页用的字符串属性，与本项目缺的那几项无关；`expanded`、`position_in_set`、`size_of_set`、`keyboard_shortcut`、`busy`、`invalid`、`described_by`、`custom_actions` 在 main 的 macOS 适配层仍零引用（读源码）
2. **两个上游的投稿规矩不一样**（读各自 CONTRIBUTING）：
   - **AccessKit 不收外部贡献者的 PR**：先开 issue，写实现方案、考虑各平台对等、**对照 Chromium 的做法**；方案被认可后给仓库成员资格，才能提 PR。提交标题用 Conventional Commits，不手改 CHANGELOG，不 `cargo update`，最低 Rust 1.87，`cargo test -p accesskit_macos` 要过
   - **Zed**：同时开着的 PR ≤ 3 个；**功能**要先有 staff 确认过的 issue 或先开 discussion，修 bug 可直接 PR（最好对着有复现步骤的 issue）；非小改必须带测试；**AI 政策**：可以用 LLM 写代码，但要有真懂的人在环，PR 描述和回复不能全靠模型写（这与本项目「PR 描述由人写」一致）
   - 先例：#61926（加 `accessibility_id`）、#65740 都是外部贡献者的 GPUI a11y PR，前者已合；#65740 描述里写了用 Claude 辅助
3. ⚠️ 本节的改动量都是**读源码估的**，没写补丁、没编译

> **编号**：以补丁正本 `specs/gui/upstream-patches.md` 的 `P<n>` 为准（gssot 2026-11-14 定稿，含本节新增的 P10–P13；P14、P15 是 gssot 另收的）。下表只是各条的技术细节来源。
> **按 B19-10 / B19-11（`docs/plan/013_decisions-b19-gui.md`）**：GPUI 层的补丁和 Zed `ui` 组件同在**一个 Zed fork、同一 rev**：每个缺口一条 `atg/<缺口名>` 分支、一个小提交（便于单独提上游），另有一条集成分支把在用的补丁合在一起，本项目根 `Cargo.toml` 的 `[patch."https://github.com/zed-industries/zed"]` 只锁这条集成分支的 rev。
> ⚠️ **AccessKit 层（P1–P4、P12）放不进 Zed fork**：Cargo 只认**根工作区**的 `[patch]`，依赖自己 `Cargo.toml` 里的 `[patch]` 会被忽略。所以这几条要么等 AccessKit 上游（先开 issue，草稿在 `upstream-drafts/`），要么另开 AccessKit fork、由本项目根 `Cargo.toml` 的 `[patch.crates-io] accesskit_macos = …` 锁——版本必须与 Zed 锁的 0.26.x 同一大版本。

### 8.1 GPUI 层（Zed 仓）

| # | 缺口 | 补在哪 · 大致改动 | 上游风格的测试 | 接受可能性 | 优先级（影响的任务） |
|---|---|---|---|---|---|
| P9 | `div` 缺 `aria_disabled` / `aria_required` / `aria_modal` / `aria_live` / `aria_busy` / `aria_invalid` | `gpui/src/elements/div.rs`：`AriaProperties` 结构加字段（`:2199`）、照 `:1441-1504` 加方法、`write_a11y_info`（`:3635`）加写入；约 60–100 行 | 照 `div.rs:5431` `test_accessibility_id_builder_writes_author_id`：建 Interactivity → 写节点 → 断言字段 | **高**：照抄现有模式，#61926 同类已合。是小功能，最好先开 discussion | 中：disabled / required 影响 T01、T04；live 让播报器不用出口（但出口已能用）；busy / invalid macOS 不送，只为跨平台 |
| P9 的关系类部分（暂不做） | `labelled_by` / `described_by` / `controls` 关系 | 难点是 GPUI 元素不知道别的元素的 NodeId（由整条 GlobalElementId 路径哈希来，`element.rs:230-254`）；要一个「按 ElementId 引用」的 API 和帧末解析。改动大，`window/a11y.rs` + `div.rs`，约 150–300 行 | 两个兄弟 div，一个 `aria_labelled_by(另一个的 id)`，画一帧断言 TreeUpdate 里的关系 | 中低：要设计讨论 | 低：名称直接写进 `aria_label` 就够；macOS 只用得上 controls |
| P10 | **应用级焦点报成 AXWindow（#66691）** | `gpui_macos/src/window.rs` 的 `a11y_init`（HEAD `:2512-2521` 用 `SubclassingAdapter::for_window`）：让 `GPUIView` 回答 `accessibilityFocusedUIElement`（转问 contentView 的 AccessKit 答案），或让 AccessKit 子类化 GPUIView；约 20–60 行 | 照 issue 的复现：进程内问 `[NSApp accessibilityFocusedUIElement]`，断言角色是文本框而不是 AXWindow（macOS 端测试要真窗口，可能只能写进 PR 的手工测试说明） | **高**：bug 有复现、有人确认影响听写软件 | **高**：VO 每次切回应用都先落在窗口上，影响全部任务 |
| P11 | **缓存视图丢节点（#66690）** | `gpui/src/window.rs` 缓存重放路径要连同 a11y 节点 / 焦点 / 动作监听一起重放；改动中等（要碰帧缓存），约 100–200 行 | 照 issue：缓存视图里放带焦点的 TextInput，只 notify 别的视图，断言节点还在（需 P6 的测试开关） | 中：动到渲染缓存，要 Zed 的人看 | 中：我们禁用 `.cached` 就绕开了（第 4 节） |
| P6 | **测试平台激活不了无障碍** | `gpui/src/platform/test/window.rs` 实现 `a11y_init`（存回调、加一个 `simulate_a11y_activation()`），或给 `VisualTestContext` 加 test-support 公开方法，内部就是 #65740 测试里那一行；约 30–60 行 | 外部视角：用公开 API 激活 → 画一帧 → `debug_a11y_tree_json()` 非空 | 中高：纯测试支持，对 Zed 自己也有用（#65740 的测试就是绕着写的）；可能要先问维护者偏好 | **高**：决定 GUI 单测能否直接断言树（`specs/gui` 的测试正本） |
| P5 | #65740 重复节点 | 别人已提，未合。**不重复提**；本项目需要时把它的提交打进补丁分支 | 已有 | 已在评审 | 低（第 4 节的写法能避开） |
| P8 | 没有文本框获焦时，方向键也先交给输入法（exp003b A） | `gpui_macos/src/window.rs` 按键处理（HEAD `:2900-3070` 三处 `inputContext handleEvent:`；exp003b 引的是老行号 `:2633-2770`）：当前没有 InputHandler 时不调 `inputContext handleEvent:`；约 10–30 行 | 难在单测；PR 里给手工复现（双拼输入法 + 方向键） | 中：要讲清楚对 IME 用户的影响，最好先开 issue | 中：本项目已有按键监视器绕法；双拼会不会吞还没测（001 R3） |
| P7 | 窗口被挡住时不出帧，无障碍树滞后（exp004） | `gpui_macos` 出帧条件；被挡住时仍为 a11y 更新跑一次 prepaint。改动面大 | — | 低 | 低：提示改走直发即可 |

### 8.2 AccessKit 层（accesskit_macos；必须先开 issue）

| # | 缺口 | 补在哪 · 大致改动 | 测试 | 接受可能性（要准备的 Chromium 对照） | 优先级 |
|---|---|---|---|---|---|
| P3 | `position_in_set` / `size_of_set` 不送 | `adapters/macos/src/node.rs` 加方法（候选：`accessibilityIndex`，或 Chromium 用的 `AXARIAPosInSet` / `AXARIASetSize` 属性，走 `accessibilityAttributeValue:`，现有盲文标签就是这么加的 `ak:node.rs:1066-1077`）；约 20–40 行 | accesskit_macos 现有测试的写法（建一棵树、问 PlatformNode） | 中：要先实测 VO 读不读（exp003 方案 C 的小实验）；Chromium 有对应映射可引 | VoiceOver 读法：低（人定不读位置，001 拍板 2）；agent：中（用 computer use 的 agent 读的是 macOS 层，读不到 GPUI 树里的位置） |
| P2 | `expanded` 不送 | 同文件加 `isAccessibilityExpanded`（及对应 `AXExpanded`）；约 10–20 行 | 同上 | 中高：NSAccessibility 有标准方法，Chromium 也映射 | 低（T06「按需展开」若做成可展开项才用） |
| P4 | `keyboard_shortcut` 不送 | 候选 Chromium 的 `AXKeyShortcutsValue` 属性；约 10–20 行 | 同上 | 中：VO 是否读要实测 | 中：快捷键可发现性（`.42cog/real.md` 记过） |
| P1 | `custom_actions` 不送 | 加 `accessibilityCustomActions`，返回 `NSAccessibilityCustomAction`，触发时发 `Action::CustomAction`；约 60–100 行 | 同上 + 触发回调 | 中：数据结构 AccessKit 早就有，只是适配层没接；要考虑 Windows / AT-SPI 对等 | 中：消息上的次要操作（第 2.4 节） |
| P12 | 选区变化通知不带 userInfo（光标移动 VO 读字 / 词，第 5 节） | `adapters/macos/src/event.rs:289-296` 带上 `AXTextStateChangeType` 等；要知道移动粒度，改动中等，约 50–150 行 | 难，要真 VO | 中：先要人试确认问题存在；Chromium / WebKit 都发这些 | **高（若人试确认）**：T04 输入框 |

### 8.3 建议顺序

1. **P10**（焦点落窗口，exp009 已复现 7/7，权宜做法进程内有效）和 **P6**（测试开关）先做：一个影响全部任务的体验，一个决定 GUI 测试正本。两者都小、都在 Zed 仓、接受可能性高
2. **P9 的属性部分**（disabled / required / modal / live）其次：先开 discussion
3. **P2、P3、P4**（合一份 issue）、**P1**、**P12**：先做人试 / 小实验确认 VO 行为，再给 AccessKit 开 issue（附 Chromium 对照）；在 issue 被认可前，本项目可在自己的 fork 分支上先用
4. P11、P8 视 GUI 开发里是否真碰到；P9 的关系类部分、P7 暂不做；P5 跟踪别人的 PR
5. 补丁清单（缺口 · 仓 · 文件 · 测试 · 上游 PR 状态 · 撤补丁条件）的正本位置由 gssot 定（common 第 15 条）；本节只是草稿来源

## 9. 要人定的事

（见现状页 `state/handoff/ggpui.md`「要人定的事」，本文件不重复。）
