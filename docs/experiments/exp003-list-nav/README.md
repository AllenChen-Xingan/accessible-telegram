# exp003-list-nav · 方向键驱动的虚拟化列表

> 2026-10-23 · 手下 exp003 · GPUI = `resources/zed` commit `79fe6a6` · accesskit_macos 0.26.3

## 假设

1000 条假消息放进 `uniform_list`（只渲染可见区），由**应用自己的方向键**驱动：

1. 容器持焦点 + 选中项 `aria_active_descendant`；按 ↓/↑ 先把下一项滚进可见区再选中
2. VoiceOver 每次读出当前项，并读出「第几项，共几项」（`aria_position_in_set` / `aria_size_of_set`）
3. 跨屏连续按 ↓ 走过几百条，焦点不丢、不跳出列表
4. 另记：Home / End、Page Up / Page Down 能否同样工作

## 做法

- 代码：`src/main.rs`（列表与按键）、`src/ax.rs`（读 macOS 那一层）
- 每条读法：「发送人，第N条：内容，时间」
- 按键：先 `scroll_to_item(i, Nearest)` 再改选中；两件事在同一帧生效（`uniform_list.rs:440-492` 在 prepaint 里处理延迟滚动），所以被选中的项一定已渲染、已进树
- 四种模式（命令行参数）：

  | 参数 | 列表 / 项的角色 | 「第几项共几项」放哪 |
  |---|---|---|
  | （默认） | `List` / `ListItem` | 只放 aria 属性 |
  | `--pos-in-label` | `List` / `ListItem` | 也拼进名称末尾 |
  | `--listbox` | `ListBox` / `ListBoxOption` | 只放 aria 属性 |
  | `--listbox --pos-in-label` | `ListBox` / `ListBoxOption` | 也拼进名称 |

- **自动验证**（`--selftest`）：
  - 读屏没开时 GPUI 不建树（`findings-gpui.md` 第 5 条）。绕法：在本进程里调窗口 contentView 的 `accessibilityFocusedUIElement`，AccessKit 就激活了（`accesskit_macos subclass.rs:183-191`）。进程内调用，**不需要辅助功能权限**
  - 依次模拟按键：顶端 ↑、↓×999 走到底、底端 ↓、↑×30、Home、End、Home、PageDown×95、PageUp×5、Cmd+↑、Cmd+↓
  - 每按一次：等新帧 → `Window::debug_a11y_tree_json()` 导出树 → 检查：键盘焦点在列表容器；active descendant 的名称、`position_in_set`、`size_of_set`、`selected` 都对；全树只有一项 selected
  - 同时在 macOS 那一层（VoiceOver 实际会问的对象）读焦点元素的角色、标题、是否选中，以及有没有任何「位置」类方法
  - 顺带量：按键到新帧的时间
- 跑法：`./run.sh selftest`（四种模式各一遍，结果写 `out/`）；人工试用见 `人工试用.md`

## 结论

> 机器能判的部分已跑完；**VoiceOver 实际说出什么，等人按 `人工试用.md` 走一遍**。

命令（2026-10-23，debug 构建，四种模式各一遍）：

```
./run.sh selftest
```

| 模式 | 步数 | 失败步 | 每帧最多渲染 | 按键→新帧 p50 / p95 / max |
|---|---|---|---|---|
| 默认（List，位置只放属性） | 1137 | **0** | 12 项 | 14.6 / 15.2 / 31.0 ms |
| `--pos-in-label` | 1137 | **0** | 12 项 | 14.6 / 15.5 / 19.1 ms |
| `--listbox` | 1137 | **0** | 12 项 | 14.4 / 14.9 / 19.3 ms |
| `--listbox --pos-in-label` | 1137 | **0** | 12 项 | 14.4 / 14.9 / 19.8 ms |

原始数据：`out/<模式>-summary.json`、逐步记录 `out/<模式>.jsonl`、第 0 / 500 / 1000 步的整棵树 `out/<模式>-tree-step*.json`。

逐条对假设：

1. **容器持焦点 + active descendant、先滚再选：成立。** 每一步键盘焦点都在列表容器上，active descendant 正是目标项，名称、`selected` 都对，全树只有一项选中（每步检查，见 `check_tree`）。滚动和选中落在同一帧，没有出现「选中项不在树里」
2. **「第几项，共几项」：GPUI 树里有，macOS 那一层没有。** GPUI 导出的树里每项都带正确的 `position_in_set` / `size_of_set: 1000`。但 macOS 层（VoiceOver 实际查询的对象）上一个「位置」类方法都没有：`accessibilityIndex`、`accessibilityRowIndexRange`、`accessibilityRowCount` 全不存在（每步记录的 `position_apis_available: []`）。**卡在 accesskit_macos 0.26.3**：`src/node.rs:386-1207` 实现的 NSAccessibility 方法里没有这几个，也没读 `position_in_set`。所以光靠属性，VoiceOver 拿不到「第 501 项共 1000 项」
   - 拼进名称（`--pos-in-label`）后，macOS 层的标题就是「……，第 501 项，共 1000 项」，这条路通
   - ⚠️ 风险（待人验证）：VoiceOver 可能按 AXList 的子元素自己数，说出「第 5 项，共 12 项」（只算可见的 12 项），会误导人。`人工试用.md` 第一轮第 12 步专门听这个
3. **跨屏连续走几百条、焦点不丢：成立（树层和 macOS 层）。** ↓×999 从第 1 条走到第 1000 条，再上下来回，每帧只渲染 12 项（虚拟化正常），焦点没离开过列表。macOS 层每一步的焦点元素标题都等于期望的那条（窗口在前台时）
4. **Home / End / Page Up / Page Down / Cmd+↑↓：都工作。** 两端夹紧也对：顶端按 ↑ 停在第 1 条，底端按 ↓ 停在第 1000 条，连按 95 次 Page Down 停在第 1000 条

角色映射（macOS 层实测）：

| 模式 | 列表 | 每一项 | 列表的「行 / 已选子项」 |
|---|---|---|---|
| List / ListItem | AXList | **AXGroup**（「组」） | 无 / 无 |
| ListBox / ListBoxOption | AXList | AXStaticText | 12 / 1 |

ListBox 模式下列表多报了「行」和「已选子项」，但「行」只有可见的 12 项。哪种听着顺，由人试用来判断。

其他实测到的事：

- **读屏没开时不建树，可以绕过且不需要权限**：自测开始时 `is_a11y_active=false`、没有树；在本进程里调一次 `contentView.accessibilityFocusedUIElement`，GPUI 打出 `Accessibility activated`，下一帧就有完整的树。以后做「无障碍树导出对比」可以直接用这条路（`src/ax.rs` 的 `activate`）
- **窗口不在前台时，macOS 层不报焦点**：有一轮从第 211 步起拿不到焦点元素。加了窗口状态记录后又重跑两次，拿不到焦点的步数和窗口失去前台的步数完全一样（987/987、682/682），GPUI 树层同时全部正确。原因是测试中途别的窗口抢了前台；accesskit_macos 只在窗口是 key window 时报焦点（`adapter.rs:248-256` 经 `state.focus()`）。这是测试环境的问题，不是列表的问题；以后跑自测要保证窗口一直在前台
- 速度：按键到新帧约 14.5 ms，基本就是一帧（60 Hz 下 16.7 ms），列表这一段没有可感知的延迟

**没验证到的（留给人）**：VoiceOver 是不是每次 active descendant 变化都开口；按住 ↓ 时会不会排队或吞字；「组」「文本」这些叫法听着怎么样。accesskit_macos 每次焦点变化都会发 `FocusedUIElementChanged`（`event.rs:314-319`），但本实验没法在进程内听到 VoiceOver 实际说了什么。

## 改变了哪一个取舍

- **拍板 3（方向键驱动）：维持 A，不用改 B。** 跨屏连续浏览在树层和 macOS 层都走通了；能不能接受，最终以人试用为准
- **新增一处要定的**：「第几项共几项」不能靠 `aria_position_in_set` / `aria_size_of_set` 交给 VoiceOver。可选：
  - A. 拼进每条的名称（已验证可行，代价是每条多读一句）
  - B. 不放进名称，另设一个查询键，按了才播报位置
  - C. 给 accesskit 上游提 PR，在 macOS 适配层实现 `accessibilityIndex` 一类方法（符合「上游缺口就提 PR」；但 VoiceOver 会不会对 AXList 的子项读这个值，还要另外实测）

  建议：先用 B，再做 C 的小实验。读法是人定的事，等人试用第二轮后拍板
- 对 `findings-gpui.md` 第 1 条的修正：「第几项共几项」在 GPUI 层是原生的，**在 macOS 层是缺的**

---

## 后续 · exp003b（2026-10-23）

> 起因：人第一次试听发现两件事——① 单独按 ↑/↓ 没反应（关了快速浏览也不行）；② 使用者习惯 VO+←/→ 自由浏览，但走到可见区最后一项就走不动了。另：人不要听「第几条，共几条」。
> 上面 exp003 的结论不改；本节只追加。exp003 的「自测全过」是在 GPUI 内部模拟按键，绕过了 AppKit 和输入法，所以没暴露 ①。

### A. 单独按 ↑/↓ 没反应

**查到的（读源码）**：`gpui_macos/src/window.rs:2633-2770`——不带字符、不带修饰键的键（方向键、Home/End、PageUp/Down 都是），GPUI **先交给输入法**（`inputContext handleEvent:`）；输入法回「我处理了」，GPUI 就直接返回，不再跑按键绑定。本机当前输入法是简体双拼（`com.apple.inputmethod.SCIM.Shuangpin`）。假设：双拼在没有输入框时也认领了方向键。

**做法**：`--key-fix on`（默认）——在 AppKit 层装一个 NSEvent 本地监视器（`src/ax.rs` `install_key_monitor`），窗口是 key window 时把方向键类按键截下，直接交给 GPUI 的按键分发，不经过输入法。带 Ctrl / Option / Shift 的键（包括 VO 键）一律不碰。`--key-fix off` 保留原路径作对照。同一个监视器把收到的每个按键记进 `out/live-*.jsonl`，人试听后能看出按键到底有没有进应用、有没有被 GPUI 认出。

**自测**：`--selftest-real`——用 CGEvent 生成真实键盘事件，转成 NSEvent 投进本进程事件队列，走 AppKit → GPUIView → 输入法 → GPUI 的真实路径。

⚠️ **局限**：本会话没有系统级发键权限（`CGPreflightPostEventAccess=false`、`AXIsProcessTrusted=false`；属系统安全设置，不开）。所以自测的按键**只投给本进程，不经过 VoiceOver**。「VoiceOver 开着时按键到不到得了应用」只能靠人试听 + `out/live-*.jsonl` 判断。

**结果**：A 节的单独自测（`selftest-b` 前三轮）未跑——拍板 3 改向后只跑新交互那一版，真实按键的结果见下面「改向 · exp003b-v2」。

### B. VO+←/→ 走出可见区

**查到的（读源码）**：

- accesskit_macos 0.26.3 唯一的滚动类动作是 `AXScrollToVisible` → `ScrollIntoView`（`node.rs:1180-1205`）；VO 设键盘焦点走 `setAccessibilityFocused:` → `Focus`，只对可聚焦的节点（`node.rs:638-666`）。GPUI 只处理 Click / Focus / Blur，其余只记日志（`window.rs:7098-7135`）；但元素上挂了 `on_a11y_action`，就会先调这个监听，并且自动向 AccessKit 声明支持该动作（`div.rs:1550`、`div.rs:3659`）
- 节点 id = 从根到该元素的整条元素 id 路径的哈希（`gpui/src/element.rs:230-254`，在 `:431` 取用；2026-10-25 改正：原引 `window/a11y.rs:331-339` 是合成子节点的算法），只要各层元素 id 不变就跨帧稳定：列表滚动后，VO 光标所在那一项还是同一个节点，不会丢
- 无障碍节点在 prepaint 时进树，不看裁剪（`element.rs:427-442`）：渲染了但被裁掉的项，VO 照样能走到

| 方向 | 做法（参数） |
|---|---|
| 1. 接住滚动动作 | `--sync scroll`：每项挂 `on_a11y_action(ScrollIntoView)`；VO 落到离可见区边缘不到 2 项的地方，就把它滚到中间，下一屏进树 |
| 2. VO 光标同步为选中项 | `--sync focus`：每项挂 `on_a11y_action(Focus)`（于是每项在 macOS 层可聚焦）；VO 落到哪项就选中哪项，同样近边缘时滚到中间。之后单独按 ↑/↓ 从 VO 光标处接着走 |
| 3. 多渲染屏幕外的项 | `--overdraw K`：uniform_list 上下各多 K 行，外层裁掉；首尾加 K 行垫行，让第 1 条和最后一条也能落在可见区 |

改向后默认 `--sync none --overdraw 0`（见下）。

**自测**：`--selftest-vo`——在 macOS 层模拟 VO+→：拿列表（AXList）的子元素，落到下一项时照 VO 的做法调 `AXScrollToVisible`（如果该项支持）和 `setAccessibilityFocused:`（如果允许），等两帧，再找下一个兄弟；从第 1 项走到走不动，再 VO+← 走回去。
⚠️ 局限：模拟的是「VO 会调这两个接口」这一假设；**VO 实际调不调**，看人试听时 `out/live-*.jsonl` 里有没有 `a11y_action` 记录。

**结果**：未跑。拍板 3 改向后这条降为次要（见下），代码保留，`./run.sh selftest-b` 随时可跑。

### C. 默认不读位置

默认读法改为「发送人：内容，时间」，去掉「第N条」；列表名改为「消息列表」（去掉「共 1000 条」）。`--pos-in-label` 仍可把「第 N 项，共 M 项」加回来作对照。

### 改向 · exp003b-v2（2026-10-25）

人改了拍板 3：消息列表是**一个专门的控件**，用 Tab 切到它，或用 VO+←/→ 找到它；找到后 ↑/↓ 逐条浏览。于是上面 B（让 VO+←/→ 逐条走遍全部历史）降为次要，代码保留，默认关。

**做法**（`src/main.rs`）：

- 界面加 4 个可聚焦控件：按钮「会话：王小明」「搜索」、文本框「输入消息（占位）」、按钮「发送」。Tab 顺序：标题 → 搜索 → **消息列表** → 输入 → 发送 → 绕回标题；Tab / Shift+Tab 绑 GPUI 的 `focus_next / focus_prev`（`tab_index` + `tab_stop`）
- 列表进入后仍是 exp003 的做法：容器持焦点 + 选中项 `aria_active_descendant`，↑/↓/Home/End/PageUp/PageDown/Cmd+↑↓ 由应用驱动；离开再回来，选中项保持
- 列表名默认「消息」；`--list-count` 改成「消息，共 1000 条」。**默认不带数字**，理由：人明确说过不要「共几条」，这是最强的信号；进入时读一次总数到底要不要，留给人在第三次试用里听了再定
- VO+←/→ 落到列表：VO 的「键盘焦点跟随 VO 光标」会对列表调 `setAccessibilityFocused:` → AccessKit `Focus` → GPUI 内置处理，把键盘焦点给列表（`window.rs:7120-7126`）。所以落到列表后不用再按 Tab，↑/↓ 就能用（前提是 VO 那个设置开着，人工试用第 16 步验证）
- `--key-fix`（绕过输入法）收窄：Tab / Shift+Tab 任何时候都截；↑/↓ 等**只在列表有焦点时**截（以后输入框是真的，方向键要留给输入法和光标移动）

**自测**（`./run.sh selftest-tab`，45 步，按键全是 AppKit 真实事件）：

| 轮 | 失败步 | 窗口非 key 步 | 按键→新帧 p50 / p95 / max |
|---|---|---|---|
| `--key-fix on` | **0** / 45 | 0 | 15.3 / 19 / 21 ms |
| `--key-fix off` | **0** / 45 | 0 | 15.5 / 19.1 / 20.8 ms |

逐项（每步都查 GPUI 焦点在谁身上、列表选中第几项）：Tab 环一圈 + Shift+Tab 反向都对；进列表后 ↓×20 跨屏、↑×3、PageDown、End、↓（夹在 1000）、Home、↑（夹在 1）、PageDown 都对；Tab 离开到输入框，此时按 ↓ 列表不动；Shift+Tab 回来选中项保持，↓ 接着走；焦点在输入框时模拟「VO 落到列表」（对 AXList 调 `setAccessibilityFocused:`），焦点立刻到列表，↓ 生效。

macOS 层 VO+←/→ 会经过的元素（`ax::vo_stops`，从 contentView 往下取）：按钮「会话：王小明」→ 按钮「搜索」→ **列表「消息」** → 文本框「输入消息（占位）」→ 按钮「发送」。列表是独立的一站，可以被 VO+←/→ 找到。

原始数据：`out/v2-tab-keyfix-{on,off}-summary.json`、逐步记录 `out/v2-tab-keyfix-{on,off}.jsonl`。

⚠️ **局限**：

1. 按键只投给本进程，**不经过 VoiceOver**（没有系统级发键权限）。VO 开着、快速浏览关着时 ↑/↓ 到不到应用，要靠人工试用；试用时 `out/live-*.jsonl` 逐条记下 AppKit 收到的键、GPUI 认出的键、列表动没动，`python3 summarize-live.py` 汇总
2. 自测时本机输入法是 U.S.，**双拼吞方向键的假设没测到**；没有擅自切输入法（会改人的全局状态）。key-fix on / off 在 U.S. 下都过，说明绕过本身不坏事；有没有必要，看人工试用的输入法对照 A/B/C
3. 「VO 落到列表就把键盘焦点给列表」依赖 VO 的「键盘焦点跟随 VO 光标」设置；如果人关了它，落到列表后还得按一次 Tab 或 VO+Shift+↓——第 16 步专门听这个

### 推荐

- **采用 v2 交互**：Tab / VO+←/→ 找到列表 → ↑/↓ 浏览。自测全过，而且比「VO 逐条走遍历史」简单得多：不用接 VO 的滚动动作、不用多渲染
- **`--key-fix` 默认开、只管列表**：U.S. 下无害；若人工对照证实双拼会吞方向键，这就是必须的修法。长远应给 GPUI 提 PR：没有输入处理器（没有文本框获焦）时，不把方向键交给输入法（`gpui_macos/src/window.rs:2633-2770`）
- **列表名默认不带「共 N 条」**，等人听过 `--list-count` 再定
- `--sync` / `--overdraw` 保留不删，默认关；VO+←/→ 逐条走历史目前不追求
