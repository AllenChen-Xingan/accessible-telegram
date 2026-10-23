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
