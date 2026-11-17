# 004 · gui · 取舍卡（给人拍板）

> 2026-11-14 · AI 综合三份调研（`findings-gpui.md` · `findings-a11y.md` · `findings-components.md`，含第九节 Quill）与正本方案（`docs/plan/012_gui-ssot.md`）· 状态：**草稿**
> 人定的结果正本在 `docs/plan/013_decisions-b19-gui.md`（B19-n）；本文只留题面存档，已定的卡标「**已定 → 013 B19-n**」
> 每张待定卡都有：题面 · 选项 · 代价 · 建议 · 默认（人不选就按默认走）
> 卡号是 G1–G17；别处引用一律写「卡 G5」，和清单条目、补丁号区分开（`specs/gui/README.md` 第二节「编号空间」）

## 一、还要人定的

### G9 · 借鉴 Quill 哪些、不借鉴哪些

- **题面**：Quill = 用 GPUI + gpui-kit 写的 Telegram 桌面客户端（MIT，约 115.7 万行 Rust，功能对标官方客户端；`findings-components.md` 第九节，索引 `quill-index.md`；克隆在 MAIN `resources/quill-20261114/`，只读）。它的无障碍底座扎实（`.aria_label` 555 处、`.role(` 433 处、有逐条读法 `quill:src/message_a11y.rs`），但新消息不播报、没有通用 live 机制、VoiceOver 实际读什么从没核过（`quill:STATUS.md:1172-1173` 未勾）——**当「GPUI 上做到哪一步、踩了哪些坑」的地图，不当无障碍标杆**
- **选项**：A. 照下表；B. 人增删
- **代价**：A 里「借鉴」都是读懂后自己写（铁律 3、B19-11），不搬代码
- **建议**：A
- **默认**：A

| | 什么 | 落到本项目哪 |
|---|---|---|
| **借鉴** | 一个收线程 + 序号 + 单 reducer；关闭先 close 等 Closed；回放夹具测 reducer；数据库密钥放钥匙串（`quill:DECISIONS.md:26-47`） | 与 001 同构，拿它的边界情况对照 |
| **借鉴** | 十个坑（`findings-components.md` 第九节）：动作默认吃键、Cmd / Ctrl 双绑抢文本键、回车不带组字标志 | `keymap.md` 规矩 8、10、11 |
| **借鉴** | 同上：焦点监听帧末 notify 不排帧、按会话恢复滚动把人带离未读 | `focus.md` 规矩 6、7 |
| **借鉴** | 同上：建树不停且废缓存、缓存视图要带树（它为此打了 gpui 补丁） | `upstream-patches.md` P11；`a11y-tree.md` 第一节第 10 条 |
| **借鉴** | 同上：主线程同步等 close 会卡住界面 | 关闭流程（001 第一节第 4 小节）在后台等 Closed、主线程不阻塞 |
| **借鉴** | 同上：不在 `.app` 里通知会中止、ad-hoc 签名每次弹钥匙串 | M4.4 之前先做最小 `.app`（`findings-components.md` 第七节） |
| **借鉴** | 同上：测试被墙钟动画和全局量搞乱、界面测试不进 CI | 测试两层（012 第四节）：纯函数层进 CI，时钟一律注入 |
| **借鉴** | 逐条读法照官方桌面客户端移植（`quill:src/message_a11y.rs`） | 只作 `specs/style/` 读法定标时的对照，读法正本仍是 `atg-core::speech` |
| **借鉴** | 改键四条：只替换自己命名空间的绑定、保存前先解析校验、固定键不许改、两个动作不共用一键（`quill:docs/decisions/parity-platform-custom-keybindings.md`） | 以后做改键时照做；改键表也从按键表正本出 |
| **借鉴** | 每个补丁一份决定记录、升级各成一件活 | 与 `upstream-patches.md` 的账同一取向 |
| **借鉴** | 冷启动优化记录（`quill:docs/decisions/launch-*.md`） | 「速度很快」那把尺子，开 M3.1 时读 |
| **借鉴** | Esc 关闭顺序表（`codex-esc-overlays.md`）、关浮层后焦点回原处（`focus-return-affordances.md`）（`quill-index.md` 标「高」） | `focus.md` 的「回打开之前的元素」与 `UiState.return_to` 栈；多层浮层时 Esc 一次关最上一层 |
| **不借鉴** | 按键表「表格化了但不是唯一正本」：参考页一张表，另拼三组不上参考页的绑定，改键又是第二张表（`quill:src/ui/keybindings/`） | 我们只有一张（`keymap.md`，012 第二节第 7 行） |
| **不借鉴** | 否决 tdlib-rs、直接绑 tdjson | 我们选了自维护补丁（`docs/plan/004_tdlib-patch.md`）；它列的理由（应答与更新分两路、未知类型把原始 JSON 打进日志）正是我们 001 R8、R13 记过的坑，已在补丁里处理 |
| **不借鉴** | gpui-kit + gpui-pre 底座、vendor 12 个 gpui 相关 crate（补丁记录约 2500 行，两个月重搬约 6 次） | B19-10 |
| **不借鉴** | 打开会话落在未读分隔条上（`unread-open-landing.md`） | 与 B19-7 相反：我们落第一条未读、名称加「未读：」 |
| **不借鉴** | 视觉复刻官方客户端 | `.42cog/intent.md`「不做什么」第一条 |
| **不借鉴** | 代码照抄 | 铁律 3（`CLAUDE.md` 第四节）、B19-11 |

### G11 · 具体按键（取向已定：B19-3、B19-4）

- **题面**：`specs/gui/keymap.md` 第五节标「草案」的那些键
- **选项**：A. 照草案；B. 人逐条改
- **代价**：A 有几处是 AI 挑的，实用后可能要改；B 要人花时间过一遍
- **要人看一眼的几处**：
  - 下一个未读会话：列表里 `n`；任何焦点下 `cmd-shift-u`。原草案 Slack 的 `alt-shift-down` 撤掉了：Opt+Shift+↓ 在 macOS 是按段扩选（清单 K9）
  - 本会话第一条未读：`cmd-j`（Slack 同键同义）。Cocoa 应用「跳到所选内容」的 Cmd-J 是菜单项，本应用菜单栏由动作表生成、不带它，所以不撞
  - 进输入框：列表里 `i`；任何焦点下 `cmd-l`
  - 搜索：`cmd-f` 搜当前会话的消息、`cmd-k` 搜会话（Slack 同键）；焦点留在搜索框，↑↓ 移高亮
  - 静音 `shift-m`（`m` 已给收听）；删除 `cmd-backspace`；回复 `r`；重发验证码 `cmd-r`
- **建议**：A
- **默认**：A

### G15 · 消息项的角色名「组」要不要换成「消息」

- **题面**：`ListItem` 在 macOS 上是 AXGroup，VoiceOver 读「组」（exp003 实测）。AccessKit 能设 `role_description`（macOS 上是 AXRoleDescription，`findings-gpui.md` 第 2.2 节）把「组」换成「消息」
- **选项**：A. 不换；B. 换成「消息」；C. 人先听两种再定
- **代价**：B 每条多一个字段；习惯「组」的人可能反而觉得怪；C 要一次试听
- **建议**：C（第一个原型里做对照）
- **默认**：A，直到人听过

### G16 · 外部 Focus 要不要拦（真碰到再问）

- **题面**：VoiceOver「键盘焦点跟随 VO 光标」开着时、或语音控制 / agent 发 Focus 时，GPUI 默认照常移键盘焦点。曾见焦点漂移，原因未定（GPUI 调研不再追）
- **选项**：A. 照 GPUI 默认，照常移；B. 外部 Focus 只移 VO 光标、不移选中项；C. 只在列表内部拦（列表外照常）
- **代价**：A 简单，但若漂移确是外部 Focus 引起，人会觉得「焦点乱跳」；B、C 要自己接 `on_a11y_action(Focus)`，和「键盘焦点跟随 VO 光标」的系统设置对着干，语音控制可能点不动
- **建议**：A，先靠日志（`specs/gui/focus.md` 第三节）攒证据
- **默认**：A；真碰到漂移再拿日志问人

### G17 · 搜索结果的焦点模型（实现不了的那一种先排除）

- **题面**：调研建议的 combobox（焦点一直留在搜索框，用 active descendant 指高亮的结果）在 GPUI 上做不出来：GPUI 只在获焦节点是目标项的**祖先**时才认 active descendant，认下以后还把报给读屏的焦点直接换成那一项（`Z:gpui/src/window/a11y.rs:258-271, 502, 564`；单测 `:720` active_descendant_ignored_when_focus_in_other_subtree；`findings-gpui.md` 第 4 节「active descendant 只在…焦点是它的祖先时才生效」）。结果列表若是搜索框的兄弟，高亮项永远读不出；塞进搜索框子树，读屏的焦点又落到结果项上
- **选项**
  - A. **↓ 把焦点移进结果列表**（结果列表就是一个 `NavList`，和会话列表、消息列表同一个组件）；在结果列表里打字字母回到搜索框接着输入，Esc 回搜索框、再 Esc 关搜索
  - B. 焦点留在搜索框，↑↓ 只改高亮，**由播报器读出高亮项**（来源 `search_hit`，Live 低档）；agent 靠结果列表的 `selected` 状态读
  - C. 给 GPUI 打补丁，让 active descendant 能指向非子孙节点（照 ARIA 的 `aria-activedescendant` 语义），并提上游
- **代价**：A 是已实测过的路（exp003 的 `NavList`，1137 步 0 失败），但比 combobox 多一次「回搜索框」的动作，打字与浏览要切换；B 每移一次高亮就多一句公告，和「播报越少越好」冲突，VoiceOver 的「组」「列表」上下文也听不到；C 改 GPUI 的焦点语义、上游可能不收，周期长
- **建议**：A（先能用，再看 C 能不能回上游）
- **默认**：A。规约已按 A 写（`specs/gui/keymap.md` 搜索几行、`focus.md` `search_results_entered`、`a11y-tree.md` 搜索框与搜索结果两行、清单 K14、S28）

## 二、已定（题面存档，结果见 013）

| 卡 | 题面 | 结果 |
|---|---|---|
| G5 | D 键：展开隐藏内容与读详情，一个键还是两个 | **已定 → 013 B19-6**：一个键 D，读详情，有隐藏内容同时展开，再按收起 |
| G6 | 未读分隔进不进 ↑↓ 顺序；落点说不说 | **已定 → 013 B19-7**：不进顺序；落第一条未读，名称前加「未读：」 |
| G10 | GPUI 底座跟哪条线（Zed 主干 git + `ui` / gpui-kit + gpui-pre） | **已定 → 013 B19-10**：Zed `ui` + 自写缺口，同一个 fork、同一 rev；不用 gpui-kit / gpui-pre |
| G12 | `intent.md`「不做什么」那句怎么改 | **已定 → 013 B19-9**：照草拟改，大总管已改 |
| G13 | 新消息播报的详略设置 | **已定 → 013 B19-8**：MVP 就做（汇总 / 只计数 / 关） |
| G14 | 加载更早消息后说不说「已加载 N 条」 | **已定 → 013 B19-12**：不说，选中项原地不动 |
| （findings-components 的 H2） | 别人的开源代码怎么用 | **已定 → 013 B19-11**：先推上游，否则照思路重写 |

## 三、AI 已定的技术取舍（人可推翻，不必逐条看）

| 卡 | 定了什么 | 理由 / 出处 |
|---|---|---|
| G1 | 八张界面表（按键、保留键、动作、树契约、焦点、播报路由、文案、视觉令牌）写成 Rust 表，放在**不依赖 GPUI 的小 crate**（暂名 `src/atg-gui-spec`），`atg-gui` 依赖它 | CI 不编 GPUI 也能生成、比对（012 第一节第 2 条）。新建 crate 守 `specs/architecture-rules.md` 第 10 条，新共享模块按第 4 条先报大总管、过 arch 线 |
| G2 | CI：工作区通配改显式清单，`atg-gui` 单独一个 CI 任务 | 通配 `src/atg-*` 会把 GPUI 拉进 `cargo test --workspace`（012 第五节第 1 条）；编译成本还没实测，数字到了再核 |
| G3 | 稳定标识写法 `msg:<chat>:<msg>`（冒号式），走 `.accessibility_id()` → AXIdentifier | `specs/gui/a11y-tree.md` 第二节；两份调研都建议走这个出口 |
| G4 | 动作比对并进 `scripts/check-sources.py`（一个比对入口） | 它是门槛文件（`specs/architecture-rules.md` 第 1 条的 `GATE_FILES`），合并要人或大总管加 `baseline:loosen` 标签 |
| G7 | 文案不上本地化框架，收成一张 `TextId` 表 | 012 第三节第 4 小节 |
| G8 | 正本移交：建表的 PR 起，md 里的表改由 `atg-gui-spec` 自带的生成程序（`cargo run -p atg-gui-spec --bin gui-schema`，不编 GPUI；`atg-gui --schema` 只是把同一份输出原样转给 agent） 生成 | `specs/gui/README.md` 第一节 |
| — | 补丁编号只有一套（`specs/gui/upstream-patches.md` 的 P 号） | 大总管 2026-11-14 |
| — | 测试分两层：纯函数「视图模型 → 无障碍属性」快照进 CI；真窗口导树在本机。P6 补上后合成一层 | `findings-gpui.md` 第 6 节 |
| — | `atg-gui` 禁用 `.cached(..)`；消息项只放普通 div + 文字 | `findings-gpui.md` 第 4 节；`a11y-tree.md` 第一节第 10、11 条 |
| — | 「上一个未读」GUI 先不绑，记进 012 第九节等 CLI 做 | 守则 5；大总管同意 |
| — | D 键的详情走直发、高档；焦点不动；想反复看按回车开阅读视图 | `announce.md` 来源 `detail`；`focus.md` `detail_read` |
