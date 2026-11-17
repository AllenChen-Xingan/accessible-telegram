# 无障碍树契约 v0（agent 的接口）

> 2026-11-14 · AI 起草 · 状态：**草稿 v0**（三份调研已并进，2026-11-14）。⏳ = 要实测、要人听，或在等组件重评
> 管：界面上每类元素进树时的**角色、名称、提示语、状态、稳定标识、动作**；树导出的规范化格式
> 读者有两类，读的是同一棵树（`.42cog/cog.md`「一棵无障碍树，两类读者」）：VoiceOver 用户，和只读树、不看像素就能操作界面的 agent（`.42cog/intent.md`「AI 友好」第 ② 条）
> 机读正本（移交后）：契约表 `a11y`，由 `atg-gui --schema` 的 `elements` 段导出（`docs/plan/012_gui-ssot.md` 第二节第 5、14 行）

## 一、十二条总规矩

1. **能操作的元素都进树，且有角色、名称、稳定标识三样。** 缺一样 `check-a11y-tree.py` 就红。GPUI 里元素要同时有 `.id(..)` 和 `.role(..)` 才进树（`gpui/src/element.rs:427-442`，`docs/research/001_stack/findings-gpui.md` 第 1 条）
2. **名称只从两处来**：带数据的取 `atg-core::speech`；不带数据的控件名取文案表。界面代码不拼名称（`specs/architecture-rules.md` 第 3 条）
3. **树的顺序就是读的顺序。** 子节点顺序 = VoiceOver 走过去的顺序 = Tab 顺序（Tab 能停的那些）。快照保留顺序（`testing.md` 第一节第 6 小节）
4. **名称里不放位置。** 不读「第几条共几条」（人试听 exp003；001 拍板 2）。位置照设 `aria_position_in_set` / `aria_size_of_set`：VoiceOver 拿不到（exp003 结论 2），但 GPUI 层的树里有，agent 用得上
5. **只在画面上有的信息不算有。** 颜色、图标、粗细表达的状态（未读、静音、发送失败），一律也进名称、提示语或状态字段
6. **一个属性只有一个主人。** 提示语（`aria_description`，macOS 上是 AXHelp，VoiceOver 读完名称停一下才念，exp004 模式 7）在消息项上归读法的「详情提示」（001 拍板 3 B）；新消息数写进窗口标题和列表名，不进提示语
7. **同一字段只从一处设。** `div` 没有禁用、必填、模态、Live 等方法，要经 `a11y_synthetic_children` 的 `parent_node()` 直接改 AccessKit 节点；那里再设同名字段会覆盖 `aria_*` 设的值，GPUI 不防（`findings-gpui.md` 第 2.2 节，读源码）。所以每个字段在契约表里写明从哪个出口设
8. **名称以画面上看得见的文字开头、不含角色词**（写「发送」不写「发送按钮」）：语音控制用户说可见文字才点得到（WCAG 2.5.3；`findings-a11y.md` 第一节第 2 小节）；盲文显示器只显示前几十个字，区分信息放最前
9. **获焦不能有副作用。** 「键盘焦点跟随 VO 光标」开着时，VO 路过就把焦点拉过去（`findings-a11y.md` 第三节第 6 小节）：选中或获焦时不标已读、不自动播放、不开会话；这些只在人按键时做
10. **不用 `.cached(..)` 包任何视图**：缓存画出的视图不推无障碍节点，焦点退回整窗（上游 issue #66690；`findings-gpui.md` 第 4 节）。上游修好（`upstream-patches.md` P11）之前一律禁用，`check-gui-state.py` 扫
11. **消息项里只放普通 div 和文字**，不嵌编辑器、markdown 这类会在 prepaint 里请求自动滚动的元素；滚动：逐项 ↑↓ 用 `ListState::scroll_to_reveal_item` + overdraw；跳跃（PageUp / PageDown、Home / End、落到第一条未读、从通知或搜索跳到某条）用 `scroll_to(ListOffset)`；两者都不经 `request_autoscroll`。只用前者的话跳到没量过的项根本不滚：exp009 跳跃序列错 8/9、8/9、7/9，换 `scroll_to(ListOffset)` 0/9（`findings-gpui.md` 第 4 节）。否则碰上 `list` 同帧重做 prepaint、无障碍节点推两遍（调试构建 panic、发布构建丢节点；上游 PR #65740，`upstream-patches.md` P5）
12. **输入框、阅读视图要有 TextRun 子节点和选区**，VoiceOver 才读得出光标位置、能逐字读（Zed 设置页 `settings_ui/src/components/input_field.rs` 的做法；`findings-gpui.md` 第 5 节）；获焦期间值冻结在获焦那一刻，失焦再同步（读屏在获焦控件的值每变一次就整段重读）

## 二、稳定标识（`.accessibility_id(..)` → macOS AXIdentifier）

**出口**（读源码）：GPUI `Z:gpui/src/elements/div.rs:1373` 的 `accessibility_id` 写入 AccessKit 的 `author_id`，注释说明「进程外可见」；`ak:node.rs:502-508` 把它作为 `accessibilityIdentifier` 交给 macOS。上游 `15d136b` 没改名（`findings-gpui.md` 第 1 节）。

**写法**：`<种类>[:<数字>]*`。种类是小写字母和连字符；数字是十进制整数，可带负号（超级群 id 以 `-100` 开头）。整个标识在一棵树里唯一。

| 种类 | 写法 | 例 | 说明 |
|---|---|---|---|
| 窗口 | `window` | `window` | 一个进程一个主窗口 |
| 区块 | `region-chats` · `region-messages` | | 地标：会话区、消息区 |
| 会话列表 | `chats` | `chats` | |
| 会话项 | `chat:<chat_id>` | `chat:101` | |
| 消息列表 | `messages:<chat_id>` | `messages:101` | 带会话 id，切会话时整列表换身份（`001` 第二节第 1 小节） |
| 消息项、系统消息项、撤回占位 | `msg:<chat_id>:<message_id>` | `msg:-100123:4567` | 三种共用一个种类：撤回后原位留占位（001 拍板 7），**标识不变**，变的是名称 |
| 未读分隔 | `divider-unread:<chat_id>` | `divider-unread:101` | 不进 ↑↓ 顺序、不可停（B19-7）；只是画面上的一条线，进树时是不可聚焦的装饰 |
| 输入框 | `composer:<chat_id>` | `composer:101` | |
| 发送按钮 | `send:<chat_id>` | `send:101` | |
| 搜索框 / 结果列表 / 结果项 | `search` · `search-results` · 消息命中 `hit:<chat_id>:<message_id>`、会话命中 `chat-hit:<chat_id>` | `hit:101:9` · `chat-hit:101` | 结果列表是 `NavList`（卡 G17 人选 A：↓ 进去、Esc 回搜索框） |
| 状态栏 | `status` | `status` | 常驻用户状态（`errors.md` 第一节第 4 小节「常驻」） |
| 播报节点 | `live:<序号>` | `live:42` | 同一句重播要换节点（exp001 结论 3），序号单调增 |
| 阅读视图 | `reading:<chat_id>:<message_id>` | `reading:101:1001` | |
| 对话框及其按钮 | `dialog-<种类>`，种类：`confirm-open-link` · `confirm-open-file` · `error`（发送、删除、重发不弹框，#113）；按钮 `confirm-ok` · `confirm-cancel` | `dialog-confirm-open-link` | 同一时刻只开一个对话框，所以按钮不必带对话框名（冒号后只许数字） |
| 登录页各框 | `login-phone` · `login-code` · `login-password` · `login-email` · `login-email-code` | | 对应 `actions.md` 的 `auth_submit_*` |
| 帮助页 | `help` · `help-key:<序号>` | `help-key:3` | 由按键表生成 |
| 链接列表 / 链接项 | `links:<chat_id>:<message_id>` · `link:<chat_id>:<message_id>:<序号>` | `link:101:1001:2` | `l` 打开（`keymap.md` `list_links`） |

**消息 id 用当前的**：发送成功 TDLib 换 id（`actions.md` 事件 `send_succeeded`），对外标识跟着换成新 id，与 `atg-cli` 输出的 id 对得上；GPUI 元素 id 则沿用本次运行里最初的那个键，节点身份不断（`docs/plan/012_gui-ssot.md` 第三节第 3 小节）。⏳ 换标识那一帧 VoiceOver 会不会重读，待实测。

**界面代码只调一个函数** `aid(种类, 数字…)` 拿标识，不手拼字符串（`check-gui-strings.py` 扫）。

## 三、元素表 v0

角色用 AccessKit 的名字（`accesskit::Role`）；macOS 那一列出自 `findings-gpui.md` 第 2.3 节（读源码）和 exp003（实测 List / ListItem）。「状态」列里 VoiceOver 拿得到的照写；macOS 层没有的（位置 / 总数）注明「只进 GPUI 层」，只给 agent 和快照用；`expanded`、快捷键这些 macOS 层没有（同上第 2.1 节），要用得等 `upstream-patches.md` 的补丁。

| 元素 | 角色（macOS） | 名称来自 | 提示语 | 状态 | 动作（agent 怎么按） | 出处 |
|---|---|---|---|---|---|---|
| 窗口 | 根节点 `Window` → macOS 上是**不带标题的 AXGroup**（`ak:node.rs:306-313`：根带标题 VO 会坏）；VoiceOver 读的窗口名是 **NSWindow 的标题**，经 GPUI 设窗口标题 | 窗口标题 = 文案「无障碍 Telegram」+ `speech` 的新消息提醒（`Alert`）+ 用户状态（`Status`） | — | — | — | 001 拍板 3 ⑥；`errors.md` 第一节第 4 小节；findings-gpui 第 2.3 节 |
| 区块（会话区、消息区） | `Navigation` / `Main` → AXGroup + 地标（VO 转子可跳） | 文案「会话」「消息」 | — | — | — | findings-gpui 第 2.3 节（Zed #60397 同法）；⏳ 人听地标是否多余 |
| 会话列表 | `List`（AXList） | 文案「会话」+ 新消息提醒 | — | 焦点；active descendant = 选中项 | 设焦点（AX Focus）后 ↑↓ 等键（`keymap.md`） | 001 拍板 1；exp003 |
| 会话项 | `ListItem`（AXGroup，exp003 实测） | `speech::chat_brief` | ⏳ | 选中；位置 / 总数（只进 GPUI 层） | 回车 = 打开（`keymap.md`） | T02；003 R2 |
| 消息列表 | `List`（AXList） | 文案「消息」+ 新消息提醒 | — | 同会话列表 | 同会话列表 | 001 拍板 2；exp003b-v2 |
| 消息项 | `ListItem`（AXGroup），`role_description` 设「消息」（macOS 上是 AXRoleDescription，VoiceOver 读「消息」不读「组」；卡 G15 人选 B，2026-11-16；文案表取） | `speech::brief` | 详情提示「按 D 听详情」（键从按键表取）。有隐藏内容时正文里另有「隐藏内容，按 D 展开」（读法的 `expand_key`）——两句说的是同一个键，各在各的位置（名称 / 提示语），不重复 | 选中；位置 / 总数 | D = 读详情，有隐藏内容同时展开，再按收起（B19-6）；回车 = 阅读视图 | 001 拍板 3 B；003 R1；M3.5 |
| 系统消息项 | 同消息项 | `speech::brief`（`Doc::Service`） | — | 同上 | — | `content-tree.md` 第三节第 4 小节 |
| 撤回占位 | 同消息项 | `speech::brief`（`Doc::Tombstone`） | — | 同上 | — | 001 拍板 7；003 R7 |
| 日期分隔 | `Heading`（VO+Cmd+H 能按日期跳；清单 S14） | `speech` 的日期读法（树节点待加，`content-tree.md` 第三节第 1 小节「以后加」已列） | — | — | — | 清单 S14 |
| 第一条未读的消息项 | 同消息项 | `speech::brief`，名称前加「未读：」（树节点或投影字段待加，012 第八节第 1 条） | 同消息项 | 同消息项 | 同消息项 | 003 F2；B19-7 |
| 未读分隔线 | 不可聚焦的装饰（不进 ↑↓ 顺序、不是 Tab 站） | — | — | — | — | B19-7 |
| 输入框 | `MultilineTextInput`（AXTextArea） | 文案「输入消息」；回复态加 `speech` 的「回复 X」 | — | 焦点；值 = 草稿 | 直接打字；回车 = 发送 | 001 第二节第 5 小节；T04 |
| 发送按钮 | `Button`（AXButton） | 文案「发送」 | — | 禁用（输入为空时；经 `parent_node().set_disabled()`） | AX Press | exp003b-v2 |
| 搜索框 | `SearchInput`（AXTextField + AXSearchField） | 文案「搜索消息」/「搜索会话」（不写「搜索框」） | — | 焦点；值 | 打字；↓ 进结果列表；Esc 关 | T08；清单 K14 |
| 搜索结果 | `List` + `ListItem`（同 `NavList`，卡 G17 人选 A） | 列表名「搜索结果」+ 结果数（`speech`）；消息项 = `speech::brief`，会话项 = `speech::chat_brief` | — | 同 `NavList` | 回车打开；Esc 回搜索框 | M4.5 |
| 链接列表 | `List` + `ListItem`（同 `NavList`） | 列表名文案「链接」；项 = 链接文字 + 地址（`message_links`；隐藏的链接读「隐藏内容」，`content-tree.md` 第八节） | — | 同 `NavList` | 回车 = 打开（过 `open_link` 的确认） | M3.5 |
| 帮助页 | `List` + `ListItem` | 列表名文案「快捷键」；第一项是使用前提（关快速导航，B19-3），其余每项「<键的读法>：<说明>」 | — | 同 `NavList` | Esc 回去 | `keymap.md` 第六节 |
| 状态栏 | `Status`（AXGroup / AXApplicationStatus；VO 读法未实测） | `speech` 读 `Doc::Status` | — | — | — | T10 |
| 播报节点 | `Status`，不可见，经 `parent_node().set_live()` 设 Live | 要播的那句（来自 `speech`）；**名称和值设同一句**（`announce.md` 第四节：AccessKit 0.27 改了触发字段） | — | Live：Polite / Assertive | — | exp001；`announce.md` |
| 阅读视图 | ⏳ `Document` 或只读 `MultilineTextInput`。⚠️ 若选只读输入框，`keymap.md` 的 `list_focused` 谓词里的 `!TextInput` 要改成「可编辑的输入框」，否则阅读视图里的 `l`、Esc 失效 | `speech::full` | — | 焦点 | Esc 回原处 | M3.5 |
| 确认框 | `AlertDialog`（AXWindow / AXDialog；业界附在窗口上的确认用 AXSheet，⏳ 选哪个） | 只有两种：打开链接（完整网址，域名单独读出）/ 附件（文件名、类型）（`actions.md` `open_link` / `open_file`）；打开即读正文（APG alertdialog）。发送、删除、重发不弹框，计划改由播报读（`announce.md` `send_plan`，#113） | — | 模态（经 `parent_node().set_modal()`） | 确认 / 取消两个 `Button` | M3.5、M3.7；`actions.md` 第一节第 2 小节「L1 外部」 |
| 登录页各框 | `TextInput`；密码框 `PasswordInput`（AXSecureTextField） | 文案（「手机号」「验证码」……）；出错时提示语 = `CoreError::message_zh` | 出错原因 | 焦点；必填（经 `parent_node().set_required()`）。`invalid` macOS 层不送，出错只靠提示语和播报 | 打字；回车 | T01 |

表注：

- **未读分隔不进 ↑↓ 顺序**（B19-7）：分界靠第一条未读名称前的「未读：」，这个前缀不随时间消失（清单 S15）
- **「位置 / 总数」只进 GPUI 层**：macOS 层没有这类方法（exp003 结论 2，accesskit_macos `node.rs:386-1207`）
- **快捷键属性**：`aria_keyshortcuts`（`div.rs:1398`）照按键表设上，但还没传到 VoiceOver（`.42cog/real.md`「技术栈现实」），agent 要从 `atg-gui --schema` 的 `keys` 段或帮助页拿按键
- **agent 能用的 AX 动作**：accesskit_macos 0.26.3 不送自定义动作（AccessKit 节点存得下，适配层不读；`findings-gpui.md` 第 2.4 节）；GPUI 只内置 Click / Focus / Blur（`Z:gpui/src/window.rs:7537-7573`）。所以 agent 的操作 = **AX Press 按钮 + AX Focus 设焦点 + 发按键**。每个能力类动作必须至少有一个按键（`keymap.md` 第一节第 5 条）。自定义动作补上（`upstream-patches.md` P1）以后是**多出的一条路，不是唯一入口**
- **macOS 层的焦点要问两处**：contentView 和 `NSApp`。GPUI 把 `GPUIView` 设为第一响应者，`[NSApp accessibilityFocusedUIElement]` 答的是 AXWindow（上游 issue #66691，`upstream-patches.md` P10）。树导出两处都记；P10 补上之前薄壳里先做权宜绕法
- **AX Press 要求元素有位置**：GPUI 的内置 Click 在节点中心模拟一次点击（`window.rs:7538-7557`），没有 bounds 的节点按不动
- **消息项要能被语音控制点到**：active descendant 模式下，每条可见消息仍是有名称、可按下的元素（`findings-a11y.md` 第一节第 5 小节）。⏳ 待实测

## 四、树导出格式（规范化后、存快照前）

两层各一份文本，**一行一个节点**，缩进两格表示层级，顺序即子节点顺序。

**GPUI 层**（`<场景>.gpui.txt`），每行：

```
<角色> <标识> "<名称>" [desc="<提示语>"] [<状态>…]
```

状态只写有的：`focused` · `selected` · `active=<标识>`（active descendant 指向谁）· `pos=<第几>/<共几>` · `expanded` · `busy` · `disabled` · `required` · `modal` · `invalid` · `live=polite|assertive` · `value="<值>"`。

**macOS 层**（`<场景>.macos.txt`），每行：

```
<AXRole>[/<AXSubrole>] [roledesc="<AXRoleDescription>"] <AXIdentifier> "<AXTitle 或 AXDescription>" [value="<AXValue>"] [help="<AXHelp>"] [placeholder="…"] [AXSelected] [AXFocused] [focusable] [disabled] [required] [modal] [actions=Press,ScrollToVisible…] [linked=<标识>…]
```

名称一格：有 AXTitle 取 AXTitle，否则 AXDescription，后面标来源 `(t)` / `(d)`。文本类元素另加 `sel=<起>-<止>` · `caret=<行>`（X3）。

文件开头两行记**两处焦点**（P10 修好之前两处答案不同）：`# focus contentView=<标识>` · `# focus NSApp=<AXRole 或标识>`。macOS 层没有 active descendant 字段——它在这一层就体现为「焦点元素是那一项」，所以 F 组、S28 查这两行。

另出一份**通知记录** `<场景>.notify.txt`：一行一条 `<序号> <通知名> <目标标识>`（Created / UIElementDestroyed / TitleChanged / ValueChanged / FocusedUIElementChanged / AnnouncementRequested〔带优先级，不带文字〕/ TextInputMarkingSession*），给 S21、A20、A21、A16 判。转子（S17）不在任何一份导出里，等 P15 实验定格式。

只写有的。清单里按 macOS 层判的条目各查哪一项：子角色 S9 · `roledesc` S30 · 值 S7 · `focusable` F10 · `actions` X1、S16、AG7 · `disabled` AG4 · 焦点两行 F4–F6、F17、S28 · `sel` / `caret` X3、K19 · 通知记录 S21、A16、A20、A21。**frame 不进快照**（每次都变，规范化要去掉）：X2 在导出那一刻对着布局现场比，比完只在快照末尾记一行 `# frame-check ok` 或列出超差的标识

**两层共同**：

- 文件第一行：`# gui-schema v<GUI_SCHEMA_V> · gpui <commit> · fixture <场景>`
- 去掉几何位置、时间戳、GPUI 内部节点号（每次都变）；节点用标识定位
- **Live 节点另记一行**：GPUI 的树导出看不到 `live` 字段（exp001 末段），在它那行后加 `live=polite` / `live=assertive`，由导出器从播报器自己的记录补上
- 没有标识的节点（纯装饰容器）只写角色，`check-a11y-tree.py` 只对能操作的角色查标识

**例**（GPUI 层，场景 `basic-3-chats`，进了「林小雨」的会话，名称是读法草稿）：

```
# gui-schema v1 · gpui 79fe6a6 · fixture basic-3-chats
Window window
  Navigation region-chats "会话"
    List chats "会话" active=chat:101
      ListItem chat:101 "林小雨，2 条未读，林小雨：明天见，7:00" selected pos=1/3
  Main region-messages "消息"
    List messages:101 "消息" focused active=msg:101:1001
      ListItem msg:101:1001 "未读：林小雨：在吗，6:58" desc="按 D 听详情" selected pos=1/2
      ListItem msg:101:1002 "林小雨：明天见，7:00" desc="按 D 听详情" pos=2/2
    MultilineTextInput composer:101 "输入消息"
    Button send:101 "发送"
  Status status "已连接"
```

（窗口名「无障碍 Telegram」在 macOS 层是 NSWindow 标题，不在 GPUI 根节点上，所以 GPUI 层这一行没有名称；macOS 层快照里记 `AXWindow "无障碍 Telegram"`。虚拟化：屏幕外的项不在树里，`001` 第二节第 1 小节；所以快照只反映可见区，场景要设计得让要测的项在可见区内。）

## 五、正例与反例

| | 例 | 为什么 |
|---|---|---|
| ✅ | 消息项 `ListItem msg:101:1002 "林小雨：明天见，7:00"` | 名称来自 `speech::brief`，标识带会话 id 和消息 id |
| ❌ | `ListItem item-3 "林小雨：明天见，7:00"` | 标识用了下标：往上加载历史后下标变，agent 存下的标识就指错了 |
| ❌ | `ListItem msg:1002 "…"` | 只有消息 id：TDLib 的消息 id 只在会话内唯一（`001` 第一节第 3 小节「身份」），两个会话会撞 |
| ❌ | `ListItem msg:101:1002 "林小雨：明天见，7:00，第 2 条，共 2 条"` | 名称里放了位置（人定不读） |
| ❌ | `ListItem msg:101:1002 ""` + 画面上一个红色感叹号 | 发送失败只在画面上；要进名称（`speech` 的发送状态读法）|
| ❌ | `Button send:101 "send"`，或 `Button send:101 ""` 只有图标 | 名称缺失或不是中文文案表里的那条 |
| ❌ | 消息项 `desc="3 条新消息，按 M 收听"` | 提示语一格的主人是读法的详情提示（第一节第 6 条）；新消息数写进标题和列表名 |
| ✅ | 撤回后 `ListItem msg:101:1002 "林小雨撤回了一条消息，7:00"` 原位、仍 `selected` | 标识不变、焦点不动（001 拍板 7） |
| ❌ | 撤回后这一项消失，选中跳到下一条 | 焦点乱跳（真难题 1） |
| ✅ | 界面代码 `.accessibility_id(aid(Kind::Msg, &[chat, msg]))` | 只经一个函数 |
| ❌ | 界面代码 `.accessibility_id(format!("msg:{}:{}", c, m))` | 手拼，第二份写法 |
