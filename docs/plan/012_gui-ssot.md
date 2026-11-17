# 012 · gui-ssot · 图形界面里每样东西的正本在哪

> 2026-11-14 · 手下 gssot 起草 · 状态：**第二段草稿**（三份调研已并进，含 Quill；人定 B19-1～B19-12 已落）。剩下的 ⏳ 多是要实测或要人听的
> 依据：skill `single-source-truth`、`complexity-budget` · `specs/architecture-rules.md`（守则 1–10）· `docs/plan/001_arch-stack.md` 第一、二节 · `002_roadmap.md` 第六、七节 · `specs/content-tree.md` · `specs/contracts/`（`actions.md` `testing.md` `errors.md` `cli.md`）· 实验 exp001 / exp003（含 b、b-v2）/ exp004
> 规约（产出该长什么样）在 `specs/gui/`；本文只回答「谁是正本、其余怎么跟上」

## 〇、一句话

**界面上每样东西只在一处定义：数据和读法在 `atg-core`（已有）；界面自己的八张表——按键、保留键、动作、无障碍树契约、焦点规则、播报路由、文案、视觉令牌——各写成一张 Rust 表，放在一个不依赖 GPUI 的小 crate 里；帮助页、菜单、规约里的表、给 agent 的自描述都从这八张表生成，生成不了的由脚本比对，对不上 CI 就红。**

### 术语

- **正本**：一样东西唯一被定义的地方；别处都是它的派生物
- **派生物**：从正本**生成**出来的（帮助页、JSON、规约里的表），或手写但有**比对**检查盯着的
- **金样文件**：已提交的生成结果；测试重新生成一遍、和它逐字比，不同就红，并提示跑哪条命令更新
- **稳定标识**：无障碍树里一个元素对外的名字（macOS 上叫 AXIdentifier），跨帧、跨运行都不变；agent 靠它找元素
- **两层树**：GPUI 层（`Window::debug_a11y_tree_json()`）和 macOS 层（VoiceOver 实际问到的，`001` 第二节第 3 小节）

---

## 一、五条总规矩

1. **机读正本写成 Rust 表，放在类型检查得到的那一侧。** 理由：skill `single-source-truth` 第 2 步「选有类型检查、离运行最近的一侧」。表的每个字段是枚举，不是字符串；界面代码 `match` 这些枚举时不写 `_ =>`（同 `content-tree.md` 第七节的做法），加一行表、漏一处处理就编不过
2. **表放在不依赖 GPUI 的模块里**，让 CI 不编 GPUI 也能生成、比对（第五节第 1 条：工作区通配会把 `atg-gui` 拉进 CI）。放哪见 **卡 G1**（AI 已定：小 crate `src/atg-gui-spec`，可推翻）
3. **正本移交**：代码还没建之前，`specs/gui/*.md` 里的表就是 v0 正本；**建表的那个 PR 起，Rust 表是正本，md 里的表改成生成物**（加金样比对），md 只留规则、格式、正反例。移交那一刻在 md 文件头写一句「本表由 X 生成，别手改」
4. **对外的机读契约带版本号和内容哈希**：界面自描述 JSON（第二节第 14 行；由 `atg-gui-spec` 自带的生成程序（`cargo run -p atg-gui-spec --bin gui-schema`，不编 GPUI；`atg-gui --schema` 只是把同一份输出原样转给 agent） 生成）带 `v`（代码常量 `GUI_SCHEMA_V`）和 `x-content-hash`，用 `schema-stamp.py check` 进 CI（这个脚本现在在 skill `single-source-truth` 的 `scripts/` 里，本仓还没有；建表的那个 PR 先把它引进 `scripts/`，只用标准库，对金样 JSON 规范化后整份算哈希）：内容变了哈希没跟着改 → 红，逼人有意识地升版本。升版本规则抄 `content-tree.md` 第八节的口径（加字段、加取值不加一；改名、删、改义加一）
5. **界面里任何「记住的位置」都按稳定 id，不按下标**：选中项、焦点锚点、展开过的条目、滚动目标，一律存 `MsgId` / `ChatId` 或第三节第 3 小节的元素键；每帧按 id 重算下标（`001` 第一节第 3 小节「焦点按 `(会话 id, 消息 id)` 锚定」）

---

## 二、正本表（总表）

「生成」= 从正本自动产出；「比对」= 手写但有脚本盯着；「—」= 只有一处，不需要。脚本名带 `（待写）` 的由 GUI 代码手下写。

| # | 关注点 | 正本（文件 / 将来的模块） | 派生物 | 生成还是比对 | 门槛（脚本 / 测试） | 版本与哈希 |
|---|---|---|---|---|---|---|
| 1 | **数据状态**（会话、消息、已读、撤回占位……） | `atg-core::store::Store`（已有，`src/atg-core/src/store/mod.rs:225`），GUI 里包成一个 GPUI 实体，**全进程一份** | 视图（每帧投影，不存） | — | reducer 单测（已有）；GUI 不许另存一份消息列表：`check-gui-state.py`（待写）扫 `atg-gui` 里有没有 `Vec<Message>` / `HashMap<MsgId, …>` 字段 | 跟 `SCHEMA_V` |
| 2 | **纯界面状态**（焦点在哪一区、各列表的选中锚点、展开过的条目、阅读视图开没开、输入框草稿） | 一个结构体 `UiState`（`atg-gui` 内，见第三节第 2 小节） | 视图 | — | 守则：字段只许是 id 或枚举，不许缓存读法字符串、不许存消息列表的下标（评审项 + `check-gui-state.py`）。**例外白名单**（只此三项）：输入框草稿与搜索查询是人打的字（输入框的值，不是读法）；链接列表、帮助页的锚点用序号——这两个列表打开期间内容不变 | — |
| 3 | **读法 / 无障碍名称** | `atg-core::speech`（`brief` / `detail` / `full` / `chat_brief`，已有） | 列表项名称、提示语、标题里的新消息数、播报的句子 | 生成（直接调函数） | 守则 3（已有）；`check-gui-strings.py`（待写）扫 `atg-gui` 里的中文字面量，只许出现在第 4 行的文案表里 | 跟 `SCHEMA_V` |
| 4 | **固定文案**（按钮「发送」、列表名「消息」、输入框名「输入消息」、确认框按钮……） | 文案表 `strings`（见第三节第 4 小节；放哪同 卡 G1） | 界面、帮助页、`--schema` | 生成 | 同上一行的扫描；文案表里每条都被至少一处引用（未引用即红） | 进 `GUI_SCHEMA_V` 的哈希 |
| 5 | **无障碍树契约**（每类元素的角色、名称来源、状态、稳定标识、动作） | 契约表 `a11y`（规约 `specs/gui/a11y-tree.md`） | `--schema` 的 `elements` 段；`a11y-tree.md` 里的表（移交后） | 生成 | `check-a11y-tree.py`（`testing.md` 第一节第 6 小节已点名，待写）：读树快照 + 契约 JSON，逐节点核角色、名称非空、标识合法 | `GUI_SCHEMA_V` + 哈希 |
| 6 | **稳定标识的写法** | 契约表里的标识规则（`specs/gui/a11y-tree.md` 第二节） | 每个元素的 `.accessibility_id(..)` | 生成（界面代码只调一个函数 `aid(…)`，不手拼） | 同上；另扫 `atg-gui` 源码里 `.accessibility_id(` 的参数只许是 `aid(…)` 的返回值 | 同上 |
| 7 | **按键表** | 按键表 `keymap`（规约 `specs/gui/keymap.md`） | GPUI 按键绑定、帮助页、菜单、`--schema` 的 `keys` 段、读法里的按键提示（`SpeechCtx.listen_key` / `expand_key`，`speech.rs:37-40`） | 生成 | 单测：同一视图里没有两条绑同一个键；每条都不撞第 8 行的保留键（除非写明理由）；`SpeechCtx` 里的键从表里取（扫源码不许出现 `Some('m')` 这类字面量） | 同上 |
| 8 | **保留键**（VoiceOver、系统、输入法占着的键） | 保留键表 `reserved`（同 `keymap` 一处；每条带出处） | 按键表的冲突检查；`keymap.md` 的冲突一节 | 生成 | 同上 | 同上 |
| 9 | **动作**（界面上能做的每件事） | 动作枚举 `GuiAction`（规约 `specs/gui/keymap.md` 第三节）；每个能力类动作写明对应的 `actions.md` 动作名、`Store` 查询名、core 的公开函数（`speech::*`、`notify` 的方法、`atg-audio` 的播放接口）或 `atg-cli` 子命令名 | 按键表引用它；菜单；`--schema` 的 `actions` 段 | 比对 | `check-gui-sources.py`（待写，或并进 `check-sources.py`，见 卡 G4）：每个能力类动作点名的 `Backend` 动作必须在 `actions.md` 动作表里、点名的 core 公开函数必须在 `src/` 里搜得到（`pub fn` 名），点名的 CLI 子命令必须在 `atg-cli/src/args.rs` 的 `enum Command` 里——**界面不许有 CLI 没有的能力**（守则 5） | 同上 |
| 10 | **焦点规则**（切会话、发送、关对话框、出错、新消息……之后焦点落哪） | 焦点规则表 `focus`（规约 `specs/gui/focus.md`）；`FocusManager` 照表查，不另写判断 | `focus.md` 的表（移交后）、`--schema` 的 `focus` 段 | 生成 | 单测：每个触发事件恰好一条规则；本机自测（`selftest-tab` 式）逐条走一遍 | 同上 |
| 11 | **播报路由**（一句话走 Live 节点、直发公告、提示音、写进名称还是系统通知） | 路由表 `announce`（规约 `specs/gui/announce.md`） | `announce.md` 的表（移交后）、`--schema` 的 `announce` 段 | 生成 | 单测：每种来源 × 每种应用状态（前台 / 被挡 / 后台）恰好一条路由 | 同上 |
| 12 | **播报策略**（提不提示、何时说、去重、节流、防抖） | `atg-core::notify`（已有）与 `errors.md` 第一节第 4 小节的状态播报规则 | 界面播报器只执行 | — | 已有假时钟单测；界面里不许出现节流、防抖的常数（`check-gui-state.py` 顺带扫 `Duration::from_` 字面量），白名单只有路由表里的两个出声时机常数（焦点错开、搜索结果数防抖，`specs/gui/announce.md` 第四节）和渲染相关的 | 跟 `SCHEMA_V` |
| 13 | **组件清单** | `specs/gui/README.md` 第三节（底座按人定 B19-10：Zed `ui` + 自写缺口；B19-11：别人的代码先推上游，否则照思路重写） | — | — | 守则 4「一个能力只做一次」；评审项：新增组件先查清单；用了现成组件的，在清单「来源」一列写明 crate 与版本 | — |
| 14 | **给 agent 的自描述** | 由第 4、5、7、9、10、11 行生成：`atg-gui-spec` 自带的生成程序（`cargo run -p atg-gui-spec --bin gui-schema`，不编 GPUI；`atg-gui --schema` 只是把同一份输出原样转给 agent）（形状对齐 `cli.md` 第一节第 6 小节的 `atg-cli schema`） | 金样文件 `…/tests/golden/gui-schema.json` | 生成 + 金样比对 | 金样测试；`schema-stamp.py check` | **`GUI_SCHEMA_V`** + `x-content-hash` |
| 15 | **视觉令牌**（对比度、字号缩放、减少动态效果、焦点框） | 令牌表 `tokens`（同 卡 G1 一处，几个常量） | 视图样式 | 生成 | 单测：前景 / 背景配对的对比度——正文 ≥ 4.5:1（大字 ≥ 3:1）、图标与焦点指示 ≥ 3:1，浅深两套都过（WCAG 1.4.3、1.4.11；`specs/gui/checklist.md` 清单 V1、V2）；视图代码里不许出现颜色字面量（`check-gui-strings.py` 顺带扫 `rgb(` / `hsla(`） | 进哈希 |
| 16 | **测试数据（夹具）** | `src/atg-core/fixtures/`（`testing.md` 第一节第 3 小节，已定「只有一处」） | `atg-gui --fixture <场景>` 读同一份 | — | `check-fixtures.py`（`testing.md` 已点名，待写） | — |
| 17 | **树快照** | 导出命令 `atg-gui --fixture <场景> --dump-tree <目录>` 的规范化输出 | `src/atg-gui/tests/a11y/<场景>.gpui.txt` / `.macos.txt`（`testing.md` 第一节第 6 小节） | 生成（本机真窗口）+ 规则检查（CI） | `check-a11y-tree.py`；PR 贴 `git diff` | 快照文件头一行写导出时的 `GUI_SCHEMA_V`、GPUI commit |
| 18 | **帮助页 / 菜单** | 第 4、7、9 行 | 帮助视图（本身也是一个 `NavList`）、菜单栏 | 生成 | 帮助视图的树快照 | — |
| 19 | **上游补丁清单**（GPUI / accesskit / accesskit_macos 的无障碍缺口，人定 2026-11-14「直接自己打补丁，做好之后提交回上游」） | `specs/gui/upstream-patches.md` 的补丁表（一个缺口一行，编号 `P<n>` 永不复用） | 根 `Cargo.toml` 的 `[patch]` 段（锁 git rev）；规约里标「依赖 P<n>」的条目 | 比对 | `check-gui-patches.py`（待写）：`[patch]` 里每个 git 依赖都有一行状态为「在用」的补丁、rev 一致；状态「在用」的每行都在 `[patch]` 里；`specs/gui/` 里引用的每个 `P<n>` 都在表里；状态「已撤」的不许再被引用为可用 | — |

---

## 三、几处展开

### 1. 状态：一份数据，一份界面状态，别的都现算

- **数据**：GUI 进程里只有一个 `atg-core::store::Store`，由 `001` 第一节第 3 小节的前台任务喂。视图每帧从它投影（`tree` 的投影函数 `message` / `window` / `chat_list` / `chat_item`，拍板 006 乙9），**不缓存投影结果、不缓存读法字符串**
- **界面状态**只放 Store 里没有、且只跟「这个人正在看哪」有关的东西（第 2 小节）
- **派生的东西不存**：下标、可见区间、「这一条是不是选中的」、列表名里的新消息数——都由「数据 + 界面状态」每帧算出
- 实体：一个 `Entity<Store>`，各视图 `cx.observe` 它（`findings-components.md` 第一节；Zed、gpui-kit 同法）

### 2. `UiState` 的字段（草案）

| 字段 | 类型 | 说明 |
|---|---|---|
| `region` | 枚举，取值照 `specs/gui/keymap.md` 第二节 `view` 一行（去掉 `global`）：`chat_list` · `message_list` · `search_results` · `link_list` · `composer` · `search` · `reading` · `dialog` · `login` · `help` | 焦点在哪一区；具体落在哪个元素由焦点规则表决定 |
| `chats_anchor` | `Option<ChatId>` | 会话列表选中哪个 |
| `msgs_anchor` | 每个会话一个 `Option<MsgId>` | 消息列表选中哪条；离开再回来保持（exp003b-v2） |
| `revealed` | `HashSet<MsgId>` | 人按 D 展开过隐藏内容的（003 R1、B19-6：再按一次 D 移出）。投影每一条时按 `revealed.contains(id)` 算出这一条的 `SpeechCtx.reveal`（它是 bool，`src/atg-core/src/speech.rs:43`） |
| `return_to` | 元素标识的栈 | 打开对话框、帮助页、链接列表、搜索、阅读视图时压入当前焦点所在元素，关掉时弹出（`focus.md` 的「回打开之前的那个元素」都查它）；切回应用不用它（用 `last_focus`） |
| `search` | `Option<{mode: 消息 / 会话, query: String, anchor: Option<命中>}>` | 搜索开着没有；命中 = 消息 `MsgId` 或会话 `ChatId`。查询文字只在内存 |
| `links` | `Option<{msg: MsgId, anchor: 序号}>` | 链接列表开着哪条消息的链接、选中第几个 |
| `help_anchor` | `序号` | 帮助页选中项 |
| `last_focus` | 元素标识 | 切走应用那一刻的焦点元素（`window_reactivated` 用它，不用 `return_to`） |
| `last_chat`（**跨运行**） | `Option<ChatId>` | 上次退出时选中的会话（`app_started_signed_in` 用）。唯一要落盘的界面状态：放档案目录的 `gui-state.json`，只存 id、不存内容（`specs/contracts/config-privacy.md` 第一节第 1 小节） |
| `reading` | `Option<MsgId>` | 阅读视图开着哪条（M3.5） |
| `composer` | 每个会话一份草稿文字 + 回复目标 `Option<MsgId>` | 草稿只在本机内存，不同步（`draft_set` 是第二批，L2 同级把关） |
| `in_flight` | 每个会话 × 每种 L2 动作（发送 / 删除 / 重发）一个 bool | 按下到交出段有结局之间为真（`specs/gui/focus.md` 第六节第 3、4 条）：同一动作的键被吃掉；发送在途时另把输入框设只读（删除、重发在途不锁输入框） |
| `unknown_pending` | 临时 id 的集合 | 收尾段已按「未知」播过的待发消息；同一临时 id 晚到成败时据此改念晚到句，念完移出（`specs/gui/focus.md` 第六节第 5 条） |
| `send_again_armed` | 每个会话一个 bool | 上次发送返回 `duplicate_recent` 且 `last_outcome` 是 `succeeded`、之后草稿与回复目标未改、键盘焦点未离开输入框时为真；任一条件破了置假（`keymap.md` `send_again`） |
| `dialog` | `Option<对话框种类>` | 确认框（只有打开链接 / 附件；发送、删除、重发不弹框，#113）、错误框；关掉后按焦点规则表回去 |

**换 id 迁移**（正本；`specs/gui/focus.md` 第六节第 5 条引这里）：一条消息换身份（发送成功换正式 id、失败换新 id、重发交出换新临时 id，`specs/contracts/actions.md` 事件 `send_succeeded` / `send_failed`）时，本表里所有持有该消息 `MsgId` 的字段（含写在元素标识里的，如 `msg:<chat>:<id>`）一起原地改——实现上统一遍历，不靠逐字段手写；现有字段：`msgs_anchor`、`revealed`、`return_to` 与 `last_focus` 里的消息元素标识、`search.anchor`（消息命中）、`links.msg`、`reading`、`composer` 的回复目标、`unknown_pending`、`send_again_armed` 记的那条；新增持有 `MsgId` 的字段必须加进这一句（单测：对每个字段造一次换 id，断言改到）。

滚动位置**不存**：由选中锚点决定（「先滚再选」，`001` 第二节第 1 小节）；GPUI 列表自己的滚动句柄不算界面状态，是渲染细节。

### 3. 稳定标识：两样东西，各管一件

GPUI 里有两个「id」，别混：

| | 是什么 | 谁看得到 | 本项目怎么定 |
|---|---|---|---|
| **元素 id**（`.id(..)`） | GPUI 内部用；无障碍节点 id 由「从根到它的整条元素 id 路径」哈希而来（`gpui/src/element.rs:230-254`） | 只在进程内 | 决定 VoiceOver 认不认得「还是那个节点」。列表项用**本次运行内不变的键**（发送成功换 id 时沿用最初那个，`content-tree.md` 第六节的 GUI 待办）；上层容器的 id 也要稳，切会话时列表 id 带上会话 id（`001` 第二节第 1 小节） |
| **对外标识**（`.accessibility_id(..)` → `author_id`） | 读源码：GPUI `resources/zed/crates/gpui/src/elements/div.rs:1365-1375`（注释写明「在进程外可见」、macOS 上映射为 AXIdentifier）；accesskit_macos 0.26.3 `node.rs:501-504` 把 `author_id` 交给 `accessibilityIdentifier` | VoiceOver 不读；**agent、自动化脚本、树快照读** | 按 `specs/gui/a11y-tree.md` 第二节的写法，如 `msg:-100123:4567`；用**当前**的消息 id，和 CLI 输出的 id 对得上。换 id 时对外标识跟着变、节点身份不变（⏳ VoiceOver 对此有没有反应待实测） |

可信度：读源码，没跑过。ggpui 核过上游 `15d136b`（2026-11-14）：接口没变，`div.rs:1373`，macOS 上 `ak:node.rs:502-508`（`findings-gpui.md` 第 1 节）。

### 4. 文案：只有一份中文，不上本地化框架

- 第一阶段只有中文、只有一个使用者（`.42cog/real.md`）。上 Fluent、gettext 这类框架是「加一层抽象」，删不掉任何重复（守则 7），**不上**
- 但文案收成一张表：枚举 `TextId` → 中文。好处有三：帮助页、菜单、`--schema` 能从同一处取；`check-gui-strings.py` 能把「界面代码里出现中文字面量」判为红；以后真要多语言，只换这一张表
- **边界**：带数据的句子（「3 条新消息」「已发送」「没有更多未读」）不是文案，是读法，走树 + `speech`（守则 3）。文案表只放不带任何数据的控件名和固定提示

### 5. 按键、动作、能力：三层，一条链

`按键 → GuiAction → (Backend 动作 | Store 查询 | 纯界面动作)`

- 按键表只引用 `GuiAction`，不直接写「发消息」这种逻辑
- `GuiAction` 分两类：**能力类**（要 core 做事：发送、标已读、拉历史……）必须点名对应的 core 动作和 CLI 子命令；**界面类**（移焦点、展开、开阅读视图）只动 `UiState`
- 例外一类，写死在表里并说明出处：**入口策略不同**——进会话就标已读、界面开着就在线、打字发正在输入（003 F1、003 A2，`actions.md` 第一节第 2 小节），能力 core 有、CLI 刻意不调。这不算「界面有 CLI 没有的能力」，表里标 `entry_policy` 并引 `actions.md` 第一节第 4 小节第 9 条

---

## 四、门槛清单：谁比谁，在哪跑

| 门槛 | 读什么 | 在哪跑 | 红的条件 |
|---|---|---|---|
| 表的单测（重键、每个事件恰好一条规则、每种来源 × 状态恰好一条路由、对比度） | Rust 表 | CI（不编 GPUI：表放在不依赖 GPUI 的小 crate，卡 G1） | 任一条不满足 |
| `--schema` 金样 | 生成的 JSON vs 已提交的金样 | CI | 不一致；提示「跑 `… --schema > …` 更新」 |
| `schema-stamp.py check`（先引进本仓 `scripts/`，见第一节第 4 条） | 金样 JSON 的内容哈希 | CI | 内容变了哈希没改 |
| `check-gui-sources.py`（或并进 `check-sources.py`） | `GuiAction` 点名的动作 / 子命令 vs `actions.md`、`args.rs` | CI（只读文本） | 点名的不存在 |
| `check-gui-strings.py` | `atg-gui` 源码 | CI | 视图代码里有中文字面量、颜色字面量、`.accessibility_id(` 不经 `aid(…)` |
| `check-gui-state.py` | `atg-gui` 源码 | CI | 另存消息集合、存下标、出现节流常数、**出现 `.cached(`**（上游 #66690：缓存视图丢无障碍节点，`upstream-patches.md` P11） |
| `check-a11y-tree.py` | 树快照 + 契约 JSON | CI（只读快照） | 可操作元素缺角色或名称；标识不合规；列表项标识不带 `(chat_id, message_id)`；快照里出现 `names.txt` 以外的名字 |
| 树导出 | 真窗口 | 本机，每次 ≤ 1 分钟（001 拍板 6） | 由 PR 的 `git diff` 给人和评审看 |
| **纯函数层的无障碍属性快照** | 「视图模型 → 每个元素该设的角色、名称、标识、状态」写成纯函数，输出本项目自己的小结构，快照测它；GPUI 只把这个结构抄进 `.role()` / `.aria_*()`（`findings-gpui.md` 第 6 节建议） | CI（不需要激活 GPUI 的无障碍；P6 补上后可和下一行合成一层） | 快照变了 |
| 无头观察器（照 gpui-kit 思路自写，`findings-components.md` 第二节第 3 小节） | prepaint 时自己建 AccessKit 节点、调 `write_a11y_info` 读回属性，在 `TestAppContext` 里断言 | CI（要编 GPUI，看 卡 G2） | 可操作元素缺角色或名称 |

**比对要能红**（skill 底线）：每个脚本落地时，在一份故意改坏的副本上跑一次，确认会红（`state/memory/20261025-silent-check-failure.md`）。

---

## 五、现在已经散落或会出问题的地方

1. **CI 会被 GPUI 拖垮**（对策见卡 G2：AI 已定，人可推翻）。 `Cargo.toml` 工作区成员是通配 `src/atg-*`，`ci.yml` 跑 `cargo clippy --workspace` / `cargo test --workspace`（macos-latest）。`src/atg-gui` 一建，CI 每次都要编 GPUI（`.42cog/real.md`：`target/` 几十 GB）。`001` 第四节第 2 小节写的「只编 `atg-core` 和 `atg-cli` 时不用拉 GPUI」到那时就不成立了
2. **读法里的按键提示已经有一处要对齐。** `SpeechCtx.listen_key`（「按 M 收听」）、`expand_key`（「按 D 展开」）由调用方传字符（`src/atg-core/src/speech.rs:37-40`）。GUI 若在调用处手写 `Some('m')`，和按键表就是两份 → 第二节第 7 行的扫描
3. **D 键一键两用。** 003 R1 定「隐藏内容，按 D 展开」（`003_decisions-b2.md` R1）；001 拍板 3 B 的例子是「按 D 听详情」，M3.5 又要「一个键读 `detail`」。→ **已定：一个键（B19-6）**，`keymap.md` `read_detail`
4. **「已发送」「没有更多未读」「下载完成」「已听完」这些句子还没有树节点。** 守则 3 要求先进树再由 `speech` 出；`content-tree.md` 第三节第 1 小节「以后加」只列了未读分隔、日期分隔、登录步骤、发送结果、服务通知。M3.6–M4.6 会用到的反馈句缺节点 → 列进第八节，报大总管
5. **列表名「消息」、按钮名「发送」现在只在实验代码里**（exp003b-v2 `src/main.rs`）。正式代码不许照抄字面量，进文案表
6. **快捷键的冲突检查没有数据。** `001` 第二节只写了「VO 组合键到不了应用」（exp004 结论 4）；哪些键被 VoiceOver 单键导航、系统、输入法占了，没有一张表 → 已按 `findings-a11y.md` 第三节收进 `specs/gui/keymap.md` 第四节
7. **焦点规则只有三句**（`001` 第二节第 4 小节：只在按键时移动；新消息、更新、加载历史不移动），「切会话后、发送后、关对话框后、出错后」的表从没列过 → `specs/gui/focus.md` 初版
8. **播报出路的选择分散在三处**：`001` 第二节第 2 小节（四条出路）、拍板 3 / 4（提示音 + 写名称 + 估算后说汇总；后台走系统通知）、`errors.md` 第一节第 4 小节（状态播报）。没有一张「来源 × 应用状态 → 出路」的表 → `specs/gui/announce.md` 初版
9. **树导出的规范化格式只有原则**（`testing.md` 第一节第 6 小节），没有字段清单和例子 → `specs/gui/a11y-tree.md` 第四节
10. **提示语（`aria_description`）一格多人要。** 拍板 3 B 给了读法的「详情」提示；001 R12 记过冲突。按键表生成「按 D 听详情」这类提示时，要写明这一格的唯一主人 → `a11y-tree.md` 第三节写死

---

## 六、待人定

题面、选项、代价、建议、默认的正本是 `docs/research/004_gui/decision.md`（卡 G1–卡 G18；本文不留副本）。本文其余各节提到的 G 号都指那里。

---

## 七、等调研回答的空位

| 空位 | 在哪 | 等谁 |
|---|---|---|
| ~~GPUI 最新版无障碍接口有没有改名、新增 live 接口没有~~ **已答**（findings-gpui 第 1、2 节：`79fe6a6..15d136b` 无障碍接口一行没变；没有 live 接口；`div` 缺禁用 / 必填 / 模态 / Live，走 `parent_node()`；macOS 层不送 expanded、位置、快捷键）→ 已并进 `a11y-tree.md`、`upstream-patches.md` | — | — |
| 编译成本（时间、盘占用） | 卡 G2 | `findings-gpui.md` 第 7 节：**未实测**，等名额 |
| ~~保留键~~ **已答**（findings-a11y 第三节）→ 已并进 `keymap.md` 第四节、第一节第 7、8 条 | — | — |
| ~~焦点规则的先例~~ **已答**（findings-a11y 第二节：APG dialog / alertdialog）→ 已并进 `focus.md` | — | — |
| 对比度阈值、字号缩放、减少动态效果的做法 | 第二节第 15 行 | `findings-a11y.md` |
| ~~GPUI 应用怎么分实体、按键绑定怎么注册~~ **已答**（findings-components 第一、四节：一个 `Entity<Store>` 各视图 `observe`；动作按名字构造 `build_action`、绑定从字符串 `KeyBinding::load`、反查 `bindings_for_action`）→ 已并进 `keymap.md` 第六节 | — | — |
| ~~组件选哪家~~ **已定**（B19-10：Zed `ui` + 自写缺口）→ 已并进 `specs/gui/README.md` 第三节 | — | — |
| ~~无障碍清单候选~~ **已收**进 `specs/gui/checklist.md`（以它为正本） | — | — |

---

## 八、要改别人的文件（只列，不改；已报大总管）

1. `specs/content-tree.md` 第三节第 1 小节：界面反馈句要的顶层节点——发送结果（「已发送」、失败原因）、「没有更多未读」（003 A3）、下载进度 / 完成（M3.7）、播放结束（M3.8）、会话已不在列表里；以及「第一条未读」的标记（B19-7：名称前加「未读：」，分隔不进 ↑↓ 顺序——原定的 `divider{kind: unread}` 节点要不要还留着画线用，由改 content-tree 的人定）
2. `specs/contracts/testing.md` 第一节第 6 小节：快照文件头加一行版本（`GUI_SCHEMA_V` + GPUI commit）；`check-a11y-tree.py` 的输入加上契约 JSON
3. `specs/architecture-rules.md` 第 5 条：补「入口策略不同」这一类例外的说法（第三节第 5 小节）
4. `.github/workflows/ci.yml`、根 `Cargo.toml`：照卡 G2 改（`atg-gui` 留在工作区；普通任务 `--workspace --exclude atg-gui`，GUI 任务 `-p atg-gui`；建好后用 `cargo metadata` 核两组命令实际选到的包）；建 `atg-gui` 的那个 PR 一起做
5. `docs/plan/001_arch-stack.md` 第四节第 2 小节「只编 `atg-core` 和 `atg-cli` 时不用拉 GPUI」：照卡 G2 改说法
6. `atg-core::notify`：「停手后再说」用的朗读时长估算（选中项字数 × 180 ms + 1 秒，exp004 模式 5、001 拍板 3 ⑤）现在没有正本，`notify` 只收 `set_user_idle(bool)`（`src/atg-core/src/notify.rs:329`）。建议放进 `notify`（纯函数、时钟可注入），界面只报按键和选中项（`specs/gui/announce.md` 第一节）
7. `atg-core::notify::Config`：加「新消息播报详略」字段（汇总 / 只计数 / 关，B19-8；`specs/gui/announce.md` 第五节）
8. `specs/contracts/actions.md` 第一节第 5 小节：闸对同一会话的「查重复指纹」与「写已提交」要串行、一步做完，否则 CLI 并发、GUI 与 CLI 同时发可能两条都过（即 issue #77「直发防重复查重与登记分两步、可竞态」，fix 线已排；GUI 自己的连按已由在途态挡住）
9. `specs/contracts/actions.md` 第一节第 2 小节「确认时写明『若网络中断，恢复后仍会发出』」：没有确认步骤了，GUI 改成不逐句念、写进帮助页（`specs/gui/announce.md` `send_plan`）；契约这句要随 #115 改成「发送时 / 帮助里说明」（已报大总管，2026-11-16）
10. `specs/contracts/actions.md` 第一节第 4 小节第 4 条「确认时给人看什么」含会话 id：GUI 读计划按卡 G18 人选不念编号（B20-6，`specs/gui/announce.md` `send_plan`）；契约要写明「界面朗读可省 id，树节点 / CLI 照给」（issue #118）
11. `atg-core::send`：给界面分两段的入口——**交出段**（准备 → 闸开凭据 → `send_text`，返回临时 id / 错误 / 临时 id 为 0 的结果未知）立即返回；**收尾段**（`wait` + `finish` 记闸结果、写 `direct-send.log` 结果行）在后台跑，**两段共用一个从开始交出起算的时限**（契约 `send_text` 行，数由 core 定；收尾段只用剩下的时间，同 `send_and_wait`），结局（成功 / 失败 / 结果未知）以回调或事件报给界面；早到的成败（比交出应答先到）由 core 暂存、等交出应答之后按「先交出、再成败」的顺序报给界面，界面不自己排序；报过「未知」之后同一条的**晚到结果**也要报，并让闸改记录（否则晚到失败后同一句被按「未知」拦 24 小时）。另：重发成功后要不要、怎么复核回复关系（`check_kept` 只收 `TextPlan`，重发计划可含「会去掉回复关系」）待定；core 给界面一个准备（只读查询）的期限常数（现在只在 CLI）；删除的交出（等 `Ack`）也要有时限。界面怎么用见 `specs/gui/focus.md` 第六节。现在只有阻塞的 `send_and_wait` / `resend_and_wait` 把两段连着（`src/atg-core/src/send/text.rs` `send_and_wait`），交出限时 `submit_within` 不对外。接口 M4.1 定（issue #119）
12. `atg-core::speech`：计划读法加「长正文截断 + 共 N 字」（卡 G18 人选 ②，B20-6；规则见 `specs/gui/announce.md` `send_plan`「长正文」），CLI `plan` 行不截（issue #120）
13. `atg-core::logline`：加 debug 级入口（同样只收写死的事件名与数字字段，开发构建默认开、发布构建默认关），给 GUI 记外部无障碍动作（`specs/gui/focus.md` 第三节）

---

## 九、将来要先在 core / CLI 补的能力（界面先不做）

守则 5：界面不许有 CLI 没有的能力。下面这些界面想要、CLI 还没有，先记账，等 CLI 做了再绑键。

| 能力 | 界面想怎么用 | 现在 core / CLI 有什么 | 出处 |
|---|---|---|---|
| 上一个有未读的会话 | 草案键待定（Slack 的 `alt-shift-up` 不能用：Opt+Shift+↑ 是按段扩选，`specs/gui/keymap.md` 规矩 9） | 只有 `Store::next_unread_chat(after)`（`src/atg-core/src/store/mod.rs:431`）、`atg-cli next-unread --after` | `specs/gui/keymap.md` 第五节；大总管同意先不绑（2026-11-14） |
