# specs/gui · 图形界面规约 v0（索引 + 命名）

> 2026-11-14 起草 · 状态：**草稿 v0**（三份调研已并进）。⏳ = 要实测、要人听，或在等组件重评
> 管：`atg-gui` 的产出该长什么样——无障碍树、按键、焦点、播报
> 不管：读法句子（`specs/content-tree.md` 与 `specs/style/`）；core 的动作和错误（`specs/contracts/`）；正本在哪、怎么比对（`docs/plan/012_gui-ssot.md`）

## 一、文件

| 文件 | 管什么 | 机读正本（移交后） |
|---|---|---|
| [`a11y-tree.md`](a11y-tree.md) | 每类元素的角色、名称、状态、稳定标识、动作；树导出格式。**agent 的接口** | 契约表 `a11y` |
| [`keymap.md`](keymap.md) | 按键表的格式、初版条目、保留键与冲突；界面动作 `GuiAction` 与 core 动作的对应 | 按键表 `keymap` · 保留键表 `reserved` · 动作枚举 `GuiAction` |
| [`focus.md`](focus.md) | 每种事件之后焦点落在哪 | 焦点规则表 `focus` |
| [`announce.md`](announce.md) | 每句播报走哪条出路、什么优先级；去重与节流归谁 | 路由表 `announce` |
| [`upstream-patches.md`](upstream-patches.md) | GPUI / accesskit / accesskit_macos 无障碍补丁的唯一账：缺口、补在哪、测试、上游 PR、撤补丁条件 | 本文件即正本（与 `Cargo.toml` `[patch]` 机器比对） |
| [`checklist.md`](checklist.md) | 无障碍验收清单（从 `docs/research/004_gui/checklist-candidate.md` 收进来，本文为准） | — |

**正本移交**（012 第一节第 3 条）：代码建表之前，本目录各文件里的表就是 v0 正本；建表的那个 PR 起，Rust 表是正本，这里的表改由 `atg-gui-spec` 自带的生成程序（`cargo run -p atg-gui-spec --bin gui-schema`，不编 GPUI；`atg-gui --schema` 只是把同一份输出原样转给 agent） 生成并有金样比对，文件头加一句「本表由 X 生成，别手改」。规则、格式、正反例一直留在这里手写。

## 二、命名

| 东西 | 写法 | 例 |
|---|---|---|
| 稳定标识（AXIdentifier） | 小写种类 + 冒号 + 数字段，见 `a11y-tree.md` 第二节 | `msg:-100123:4567` · `chat:101` · `messages:101` |
| 界面动作 `GuiAction` | Rust 变体用大驼峰；JSON、文档里用 snake_case | `OpenDetail` ↔ `open_detail` |
| 视图（按键作用范围、焦点所在的区）| snake_case；**取值正本是 `keymap.md` 第二节 `view` 那一行**，`UiState.region`（012 第三节第 2 小节）和焦点规则表都用这一套 | `chat_list` · `message_list` · `search_results` · `link_list` · `composer` · `search` · `reading` · `dialog` · `login` · `help`；按键表另有 `global` |
| 焦点触发事件 | snake_case，「名词 + 过去分词」（发生了什么） | `chat_opened` · `message_sent` · `dialog_closed` |
| 播报来源 | snake_case，与 `notify` 判定或树节点种类同名 | `new_message` · `status_bad` · `send_ok` |
| 文案 `TextId` | 大驼峰；JSON 里 snake_case | `SendButton` ↔ `send_button` |
| 树快照 | `<场景名>.gpui.txt` / `<场景名>.macos.txt`，场景名同夹具（`specs/contracts/testing.md` 第一节第 3、6 小节） | `basic-4-chats.macos.txt` |

本目录的文件是规约，**不编号、不带日期**（`specs/README.md`「命名」）。

### 编号空间（同一个字母在不同文件里是不同的东西）

本目录引用别处的编号时，**一律带上出处前缀**，不写裸号。

| 写法 | 指什么 | 在哪 |
|---|---|---|
| `清单 S1` · `清单 F6` · `清单 K9` · `清单 A3` · `清单 V1` · `清单 X3` · `清单 AG1` | 无障碍验收清单条目 | `checklist.md` |
| `卡 G5` | 取舍卡（题面 · 选项 · 代价 · 建议 · 默认） | `docs/research/004_gui/decision.md` |
| `P10` | 上游补丁 | `upstream-patches.md`（只有这一套；findings-gpui 里的旧号 G1–G8 / A1–A5 已作废） |
| `B19-6` | 人定（批次 b19） | `docs/plan/013_decisions-b19-gui.md` |
| `003 F1` · `003 F2` · `003 F6` · `003 A3` · `003 R1` · `003 E1` · `003 S1` | 人定（批次 b2：功能 F、契约 A/D/C/E/P/T、读法 R、排期 S） | `docs/plan/003_decisions-b2.md` |
| `005 N5` | 人定（批次 b3） | `docs/plan/005_decisions-b3.md` |
| `006 甲4` · `006 乙9` | 人定（#15 model） | `docs/plan/006_decisions-b7.md` |
| `008 H1` | 人定（#21 history 等） | `docs/plan/008_decisions-b16-b17.md` |
| `findings-components H2` | 组件调研自己的取舍编号（H1 底座、H2 别人的代码） | `docs/research/004_gui/findings-components.md` 第十一节 |
| `调研 003 发5` | 调研条目（发 = 发消息那组） | `docs/research/003_send-receive-prior-art/README.md` 第一节 |
| `007 V12` | 人定（读法 V1–V23） | `docs/plan/007_decisions-b15.md` |
| `009 第 2 条` | 人定（notify v0） | `docs/plan/009_decisions-notify-v0.md` |
| `001 拍板 3` · `001 R3` | 架构方案第七节的拍板 / 第五节的风险 | `docs/plan/001_arch-stack.md` |
| `M3.5` | 路线图的块 | `docs/plan/002_roadmap.md` 第六、七节 |
| `T01`–`T10`（两位数） | 核心任务 | `specs/tasks.md` |
| `exp003` | 实验 | `docs/experiments/exp003-*/README.md` |
| `守则 5` | 架构守则 | `specs/architecture-rules.md` |
| `L0` · `L1` · `L1 外部` · `L2` | 对外等级（L2 = 要过发送闸；闸开 `direct_mode` 或 `test_exempt` 凭据、不弹框，#113。要人当场确认的只剩 L1 外部的打开链接 / 附件） | `specs/contracts/actions.md` 第一节第 2 小节 |

### 源码路径的写法

| 前缀 | 指 | 版本 |
|---|---|---|
| `Z:` | Zed 主干 `crates/`（MAIN 工作区 `resources/zed-20261114/`） | `15d136b`（2026-11-14） |
| `zed:` 或裸写 `gpui/src/…` | Zed 旧克隆 `crates/`（MAIN 工作区 `resources/zed/`） | `79fe6a6`（2026-10-23）；无障碍部分两版逐字相同，只是行号可能不同（`findings-gpui.md` 第 1 节） |
| `ak:` | accesskit_macos 0.26.3 `src/`（本机 cargo 缓存） | 0.26.3 |
| `akc:` | accesskit_consumer 0.38.0 `src/` | 0.38.0 |
| `gc:` | gpui-kit（原 gpui-component）`crates/`（MAIN `resources/gpui-component-20261114/`） | `4379eeb` |
| `src/…` | 本仓 | 当前分支 |

`resources/` 只在大总管那份工作区（MAIN）里有，不入库；本工作副本里打不开。

### 术语

- **active descendant**：列表整体拿着键盘焦点，再用一个属性告诉读屏「当前是哪一项」（`docs/plan/001_arch-stack.md` 术语表）
- **VO 光标**：VoiceOver 自己的浏览位置，用 VO+方向键移动；「键盘焦点跟随 VO 光标」开着时，VO 光标走到哪键盘焦点就跟到哪
- **快速导航**：VoiceOver 开着时直接吃方向键或单字母的模式（VO+Shift+Q / VO+Q 切换）；人定用本应用时自己关（B19-3）
- **转子**：VoiceOver 的「按类别跳」菜单（VO+U）
- **AX Press**：辅助技术对元素做「按下」；**AXIdentifier**：给自动化看的元素标识，VoiceOver 不读；**AXGroup**：macOS 的「组」角色
- **Live / Polite / Assertive**：一个节点的值一变就让读屏念出来；礼貌式（优先级 50）/ 打断式（90）
- **直发公告**：应用直接请 macOS 念一句，可带优先级 10 / 50 / 90
- **TextRun**：输入框里的一段文字子节点，带每个字的长度和词边界，读屏靠它逐字读、知道光标在哪
- **组字 / 标记文本**：中文等输入法正在拼、还没上屏的那段字
- **prepaint**：GPUI 每帧画之前的一步，无障碍节点在这一步进树
- **key 窗口**：当前接收键盘输入的窗口
- **地标**：VoiceOver 能直接跳过去的区块（主要、导航……）
- **直接发送（`direct_mode`）**：发送、删除（给所有人）、重发等一切 L2 动作不弹确认框、直接交出——这是唯一方式（人定 2026-11-16 删除 Touch ID，#113；`actions.md` 第一节第 5 小节）。`--unsafe-direct-send` 已无作用、GUI 不认它。契约 `actions.md` 第一节第 5 小节「启动时提示」（GUI 标题加「直发模式」、启动播报）与 002 M4.6「过确认」随直发成唯一方式已不适用，待 #115 改正本；GUI 规约先不做，以 #115 改后的契约为准。**规约里单说「直发」只指「直发公告」那条播报路线**，不指这个
- **发送闸 · 凭据 · 交出**：一切 L2 动作只能经 `atg-core` 的发送闸；闸给每次动作开一张一次性凭据（绑「发给谁 + 发什么」的摘要，防「读的是 A、发出去的是 B」），正式服务器开 `direct_mode`，测试服务器只对收藏夹（及 005 N7 的第二测试号私聊）开 `test_exempt`、别的会话拒；「交出」= `send_text` 这一步：验凭据、复核、把请求交给 TDLib（定义见 `actions.md` 第一节第 4 小节与 `send_text` 行）。凭据不是确认，人不用按任何东西。界面怎么按交出段与收尾段的结局走，见 `focus.md` 第六节
- **发送留痕**：契约要求的 `direct-send.log`（每条直接发送一行，不记内容）与 `send-guard.json`（重复提交记录）；写不进去就拒发。和 GUI 调试日志、「直发公告」都无关
- **临时 id · 待发消息 · 应答**：TDLib 收下一条发送时先给它编一个临时 id（这时它是「待发消息」，契约 `PendingMessage`），服务器收下后换成正式 id；「应答」= 这次交出请求的直接回复，带着临时 id。应答也可能是一个错误（这时没有待发消息）。**临时 id 为 0** = 交出段报结果未知时没拿到临时 id：到时限没拿到应答，或交出途中断线（`specs/contracts/errors.md` `send_outcome_unknown`）
- **交出段 · 收尾段 · 在途态**：界面一次发送分两段——交出段（准备 → 闸 → 交给 TDLib）与后台的收尾段（等成败 → 闸记结果）；从按下到交出段有结局叫在途态。定义、时限与各种结局见 `focus.md` 第六节
- **`Ack`**：调用成功、没有返回值（`actions.md` 第一节）；删除返回 `Ack` 只说明请求被接受，真正删掉以事件 `messages_deleted` 为准
- **撤回占位**：消息被删（不论谁删）后原位留下的一条「已撤回」项，选中不丢（001 拍板 7）
- **gpui-kit**（原名 gpui-component）：第三方的 GPUI 组件库；**gpui-pre**：它依赖的、crates.io 上的 GPUI 每周快照。B19-10 定了不用
- **Zed `ui` crate**：Zed 自己的组件库（GPL-3.0），本项目的组件底座
- **fork / rev**：在自己 GitHub 账号下复制一份上游仓（fork）；rev = 锁定的那个 commit
- **`uniform_list` / `list` + `ListState`**：GPUI 的两种虚拟化列表（等高 / 变高）；`scroll_to_reveal_item` = 把某项滚进可见区
- **`Entity<Store>` / `observe`**：GPUI 里一份共享状态和对它的订阅；**reducer**：「旧状态 + 一件事 → 新状态」的纯函数（001 术语表）
- **上下文谓词**：GPUI 按焦点路径匹配按键时用的条件，如 `NavList && !TextInput`
- **combobox**：焦点留在输入框、用 active descendant 指出下拉结果里高亮项的控件（APG）。本项目的搜索因 GPUI 限制没用它（卡 G17）
- **List / ListItem 与 ListBox / ListBoxOption**：两种列表角色；前者每项在 macOS 上是「组」，后者是「文本」。本项目用前者（001 拍板 1）
- **controls**：「这个元素控制那个元素」的关系，macOS 上是 AXLinkedUIElements
- **APG**：W3C 的 ARIA 编写实践指南，给出列表、对话框等控件的标准键盘约定

## 三、组件清单（一个组件只做一次，多处复用）

**底座（人定 B19-10、B19-11，`docs/plan/013_decisions-b19-gui.md`）：Zed `ui` crate（GPL-3.0）+ 自写缺口；补丁和组件在同一个 Zed fork、同一 rev；不用 gpui-kit / gpui-pre。** 别人的代码先推上游、我们用上游的；不收或来不及，就照思路自己重写（铁律 3 不动）。项目许可证改 GPL-3.0 另开 issue，动手前再经人确认（B19-5）。组件调研已按 B19-10 写完（`findings-components.md` 第二节第 4 小节、第十节），本表照它。

| 组件 | 是什么 | 用在 | 来源 | 出处 |
|---|---|---|---|---|
| `NavList` | 虚拟化列表：容器持焦点 + active descendant、先滚再选、↑↓ Home End PageUp PageDown Cmd+↑↓ | 会话列表、消息列表、搜索结果、链接列表、帮助页 | **自写**（Zed `ui` 没有消息列表；`findings-components.md` 第二节）。等高用 `uniform_list`，变高用 `list` + `ListState`；逐项移动用 `scroll_to_reveal_item` + overdraw，跳跃用 `scroll_to(ListOffset)`（exp009，`findings-gpui.md` 第 4 节） | `001` 第二节第 1 小节；exp003、exp003b-v2 |
| `Announcer` | 播报器：一个入口，按 `announce.md` 的路由表执行 | 全局一个 | **自写**（Zed `ui` 和 gpui-kit 都没有播报）；做成可回上游的组件 ⏳ | `001` 第二节第 2 小节；exp001、exp004 |
| `TreeDump` | 真窗口树导出：激活、两层、两处焦点、规范化 | `--dump-tree` | **自写**（exp001 / exp003 的做法） | `001` 第二节第 3 小节 |
| 无头观察器 | 测试期在 `TestAppContext` 里读回每个元素的角色、名称（不开窗口、不靠 AccessKit 激活） | 单测 | 先推上游（B19-11），不收就照 gpui-kit `observe.rs` 的思路重写。它和 P6（让测试平台能激活无障碍）是两条可选的路：P6 合了，观察器就不必要 | `findings-components.md` 第二节第 3 小节 |
| `FocusManager` | Tab 顺序 + 按 `focus.md` 的规则表落焦点；对话框焦点陷阱与关闭后还原 | 全局一个 | **自写**（照 gpui-kit 焦点陷阱的思路重写，B19-11） | `001` 第二节第 4 小节 |
| `Composer` | 多行输入框：TextRun + 选区、组字时回车不发、回车走发送闸（读一遍计划、直接发，不弹框） | 每个会话 | **自写，照 Zed 主干 `Z:settings_ui/src/components/input_field.rs` 写**（`Role::TextInput` + TextRun + 选区 + 获焦期间冻结 `aria_value`；它绑 Editor，不能直接依赖）；写好尽量提交上游 | `001` 第二节第 5 小节 |
| 登录表单各框 | 单行输入（手机号、验证码）、密码 | M3.2 | 同 `Composer` 的做法，角色 `TextInput` / `PasswordInput` | T01 |
| `KeyMonitor` | AppKit 层按键监视器，先判有没有标记文本（组字中一律放行，`keymap.md` 规矩 8）。Shift+Tab **任何时候都截**（全局键，输入框里也要能退出去）；方向、Esc、Home / End、PageUp / PageDown 在不收文字的视图获焦时截；**收文字的视图里只截按键表在该视图明文绑了的那几个**（搜索框的 ↓、Esc，输入框的 Esc），其余留给输入法和光标。Tab 带字符、不先给输入法，可能不用截（⏳ 实测复核，`keymap.md` 第二节 `bypass_ime`） | 全局一个 | **自写**；上游修好（P8）就撤 | `001` 第二节第 1 小节「绕过输入法」 |
| `ConfirmDialog` | 确认框：**只用于打开链接 / 附件**（L1 外部，契约要求当场确认）。AlertDialog，打开即读正文，焦点落在取消。发送、删除、重发**不弹框**（人定 2026-11-16 删除 Touch ID，#113；`actions.md` 第一节第 5 小节），改由 `Announcer` 在调用返回成功时读一遍计划（`announce.md` 来源 `send_plan`） | M3.5、M3.7 | **自写**：对话框角色、模态、焦点陷阱（Zed `ui` 缺；B19-10） | `actions.md` `open_link` / `open_file` |
| `ReadingView` | 只读多行文本，按字 / 词 / 行读，Esc 回原处 | M3.5 | 同 `Composer` 的做法（只读） | `002` M3.5 |
| 按钮、开关、菜单、右键菜单、标签页 | — | 各处 | **Zed `ui`**，rev 与 GPUI、补丁同一个（B19-10），记在根 `Cargo.toml` 的 git 依赖里（正本），`upstream-patches.md` 的 rev 列与它比对。按钮基类默认 `Role::Button`；右键菜单用 active descendant、`aria_keyshortcuts` 从按键表反查（`findings-components.md` 第二节第 4 小节） | — |

实体：一个 `Entity<Store>`，各视图 `observe` 它（`findings-components.md` 第一节）；收通道的前台任务以时间预算为主：第一条立刻处理，之后每批**约 8 ms** 起步（Quill 先例；Zed 终端是 4 ms / 100 条，`zed:terminal/src/terminal.rs:1425-1480`），处理完 `entity.update` 一次、`yield_now()` 让出（`findings-components.md` 第一、十节，M3.1 的建议）；具体数字原型里量。

**用 Zed `ui` 时的底线**：进了本项目的组件同样守本目录的规约（角色、名称、标识、焦点、播报），不达标就补，补的那部分回上游，账记在 `upstream-patches.md`。

## 四、和别处的关系

- **名称从哪来**：一律 `atg-core::speech`（`brief` / `detail` / `chat_brief`）；不带数据的控件名从文案表。界面代码里不写中文字面量（012 第二节第 3、4 行）
- **能力从哪来**：一律 `atg-core` / `atg-cli` 已有的（`specs/architecture-rules.md` 第 5 条）；`keymap.md` 第三节每个能力类动作都点名它对应的 core 动作
- **测试数据**：只用 `src/atg-core/fixtures/`（`testing.md` 第一节第 3 小节）
