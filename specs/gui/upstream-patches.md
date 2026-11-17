# 上游补丁清单（GPUI · accesskit · accesskit_macos）

> 2026-11-14 · AI 起草 · 状态：**格式 v0 + 候选**。本表是补丁的**唯一账**（人定 B19-1，`docs/plan/013_decisions-b19-gui.md`）
> 人定（2026-11-14 原话，B19-1）：「如果gpui 本身的无障碍有问题，我们可以直接自己打补丁，做好之后提交回上游」。做法照 tdlib-rs 补丁先例 `docs/plan/004_tdlib-patch.md`
> `.42cog/intent.md`「不做什么」已随之改写（B19-9）
> 机器比对：`check-gui-patches.py`（待写；012 第二节第 19 行）

## 一、规矩

1. **一个缺口一行、一个小提交**，带上游风格的测试。编号 `P<n>` 只增不复用；撤掉的留在表里，状态写「已撤」
2. **补在缺口所在的那一层**：GPUI 的缺口补 zed 仓；accesskit / accesskit_macos 的补 AccessKit 仓。fork 在人的 GitHub 账号，分支 `atg/<缺口名>`，根 `Cargo.toml` 用 `[patch]` 锁 git rev
3. **fork、推送、开上游 PR 由大总管经人同意后做**；上游 PR 的描述由人写。手下只出补丁内容和说明草稿
4. **规约里用到补丁才有的能力，必须标「依赖 P<n>」**；补丁还没进「在用」之前，规约同时写出没有补丁时的退路（下文「退路」列）
5. **不长期分叉**：定期跟上游 rebase；上游合了（或上游另有等价修法）就撤补丁，规约里的「依赖 P<n>」随之删去
6. 能用公开出口做到的（如 `a11y_synthetic_children` 的 `parent_node()`），本项目先走出口、不等补丁；但照人定 B19-5（`docs/plan/013_decisions-b19-gui.md`：「开源已有的就用……缺的补进上游」），这类缺口照样进表、可提上游，标「不阻塞」

## 二、表的格式（一行一个缺口）

| 字段 | 填什么 |
|---|---|
| 编号 | `P<n>` |
| 缺口 | 一句话：什么做不到、出处（源码路径:行号 / 实验编号）、可信度 |
| 补在哪 | 仓（zed / AccessKit）· crate · 文件 · fork 分支 `atg/<缺口名>` |
| 测试 | 上游风格的测试叫什么、在哪；本项目侧怎么验（树快照 / 实验 / 人听） |
| 上游 PR | 链接与状态：未开 · 草稿 · 已开 · 已合 · 被拒（附理由） |
| 本项目状态 | 候选 · 在做 · 在用（`[patch]` 里锁了 rev）· 已撤 |
| rev | 在用时锁的 commit（与 `Cargo.toml` `[patch]` 一致，脚本比对） |
| 撤补丁条件 | 上游合了哪个 PR / 发了哪个版本之后撤 |
| 退路 | 没有补丁时本项目怎么办（规矩 4 要求必写） |
| 规约里谁依赖它 | `specs/gui/` 的文件与小节 |

**候选阶段只填 5 列**（编号 · 缺口 · 补在哪 · 退路 · 规约里谁依赖）；转「在做」时补齐其余字段（测试、上游 PR、本项目状态、rev、撤补丁条件）。`check-gui-patches.py` 只核状态为「在做 / 在用」的行。

## 三、候选（第一段：从 `docs/research/004_gui/findings-gpui.md` 第 1、2 节整理，全部未开工，等人排）

| 编号 | 缺口 | 补在哪 | 退路（没补丁时） | 规约里谁依赖 |
|---|---|---|---|---|
| P1 | `custom_actions` 存得进 AccessKit 节点，但 accesskit_macos 不送 `NSAccessibilityCustomAction`，VO-Cmd-Space 的动作菜单是空的（findings-gpui 第 2.4 节，读源码） | AccessKit · accesskit_macos · `node.rs` | 应用快捷键 + 菜单栏项（`keymap.md` 第一节第 5、7 条） | `keymap.md`、`a11y-tree.md` 第三节表注 |
| P2 | `expanded` 不送 macOS（`ak:` 零引用；findings-gpui 第 2.1 节） | AccessKit · accesskit_macos | 不靠「展开 / 收起」状态；状态写进名称或不用 | `a11y-tree.md`（目前没有依赖） |
| P3 | `position_in_set` / `size_of_set` 不送 macOS（exp003 结论 2；findings-gpui 第 2.1 节） | AccessKit · accesskit_macos（`accessibilityIndex` 一类） | 人定不读位置（001 拍板 2），只给 agent 读 GPUI 层 | `a11y-tree.md` 第一节第 4 条 |
| P4 | `keyboard_shortcut` 不送 macOS（findings-gpui 第 2.1 节；`.42cog/real.md`「技术栈现实」） | AccessKit · accesskit_macos | 帮助页 + `atg-gui --schema` 的 `keys` 段 | `keymap.md` 第一节第 6 条 |
| P5 | `list` 子项请求自动滚动时同一帧重做 prepaint，无障碍节点推两遍：调试构建 panic、发布构建丢节点（上游 PR #65740 未合；findings-gpui 结论 3） | 不自己提：跟踪 #65740，需要时把它的提交打进补丁分支 | 只在列表项里调 `window.request_autoscroll` 时触发（Zed 的编辑器 / markdown）；消息项只放普通 div + 文字、滚动只用 `ListState` 自己的方法（逐项 `scroll_to_reveal_item` + overdraw，跳跃 `scroll_to(ListOffset)`，都不经 `request_autoscroll`）就碰不到（findings-gpui 第 4 节）——**变高 `list` 不必等它** | `focus.md` 规矩 4「先滚再选」 |
| P6 | 测试平台不实现 a11y，外部 crate 的单测拿不到树：`A11y`、`window.a11y` 是 `pub(crate)`（findings-gpui 第 1、6 节） | zed · gpui · `platform/test/window.rs` 实现 `a11y_init`，或给 `VisualTestContext` 加公开的「激活无障碍」开关（约 30–60 行） | 测试分两层：纯函数「视图模型 → 无障碍属性」快照（CI）+ 真窗口导树（本机，`testing.md` 第一节第 6 小节） | `a11y-tree.md` 第四节；`docs/plan/012_gui-ssot.md` 第四节 |
| P7 | 窗口被挡或后台时 GPUI 不出帧，Live 节点与树都滞后（exp004 结论 5） | zed · gpui（出帧条件） | 被挡 / 后台走直发、提示音、系统通知（`announce.md` 第三节） | `announce.md` |
| P8 | 没有文本框获焦时方向键仍先交给输入法（`gpui_macos/src/window.rs:2633-2770`；`001` R3） | zed · gpui_macos | `KeyMonitor` 截键（exp003b-v2） | `keymap.md` 第四节 |
| P9 | `div` 没有 `aria_disabled` / `aria_required` / `aria_modal` / `aria_live` / `labelled_by` 等方法（findings-gpui 第 2.2 节） | zed · gpui · `elements/div.rs` | 走 `parent_node()` 出口；**同一字段只从一处设**（`a11y-tree.md` 第一节第 7 条）。有公开出口，**可提上游、不阻塞**（B19-5「缺的补进上游」；改动小、照 #61926 的模式，`findings-gpui.md` 第 8.1 节第一行） | `a11y-tree.md` 第三节 |

**第二段补充**（`findings-gpui.md` 第 3–8 节落了之后加的；编号只增不复用，所以接着排）：

| 编号 | 缺口 | 补在哪 | 退路（没补丁时） | 规约里谁依赖 |
|---|---|---|---|---|
| P10 | **应用级焦点报成 AXWindow**（`findings-gpui.md` 第 8.3 节建议优先级最高）：`[NSApp accessibilityFocusedUIElement]` 答窗口，只有问 contentView 才得到真焦点；VO 切回应用先落在窗口上（上游 issue #66691；findings-gpui 第 6 节，读 issue） | zed · gpui_macos · `window.rs` 的 `a11y_init` | 启动时给 `GPUIView` 类加 `accessibilityFocusedUIElement` 转问父视图（issue 评论里的权宜做法，薄壳里做）；树导出两处都问 | `a11y-tree.md` 第四节 |
| P11 | **缓存视图丢无障碍节点**：`.cached(..)` 画出的视图不推节点，焦点退回整窗（上游 issue #66690） | zed · gpui · `window.rs` 缓存重放路径（约 100–200 行） | **`atg-gui` 禁用 `.cached(..)`**（`check-gui-state.py` 扫） | `a11y-tree.md` 第一节第 10 条 |
| P12 | **选区变化通知不带 userInfo**：光标移动时 VO 可能不读字 / 词（`ak:event.rs:289-296`；findings-gpui 第 5 节，推断，**要人试确认**） | AccessKit · accesskit_macos · `event.rs`（先开 issue；约 50–150 行） | 无好退路；先人试 | `a11y-tree.md` 第一节第 12 条、清单 X3 |
| P13 | **普通滚动容器里被裁掉的元素仍报可见**（上游 issue #66730，附修法：完全在裁剪区外就 `set_hidden`） | zed · gpui · `element.rs:435-449`（约 5–10 行 + 测试） | 非虚拟化的滚动区保持短 | — |
| P14 | **窗口命中测试停在窗口层**（gpui-kit 给 NSWindow 类加 `accessibilityHitTest:` 转给 contentView，`findings-components.md` 第二节第 3 小节；原因源码没写，推断） | zed · gpui_macos | 薄壳里照思路自写；exp010 实测装与不装的差别 | 清单 X5 |
| P15 | **自定义转子**：AccessKit 没有这个概念（findings-gpui 第 2.4 节） | 不是补丁：NSWindow 上挂 `NSAccessibilityCustomRotor`（AppKit 自己的对象）——实验候选 | 应用快捷键 | 清单 S17 |

**编号只有这一套**（大总管 2026-11-14 定；findings-gpui 第 8 节改引 P 号）。旧草稿编号对照：G1 / G2 → P9（G2 关系类暂不做）· G3 → P10 · G4 → P11 · G5 → P6 · G6（#65740）→ P5 · G7 → P8 · G8 → P7 · A1 → P3 · A2 → P2 · A3 → P4 · A4 → P1 · A5 → P12。

**建议顺序**（照 findings-gpui 第 8.3 节）：P10、P6 先做（一个影响全部任务、一个决定测试正本，都小、都在 Zed 仓）；P9 其次（先开 discussion）；P12、P1–P4 先做人试 / 小实验确认 VO 行为，再给 AccessKit 开 issue（AccessKit 不收外部贡献者直接提的 PR，要先过 issue 方案）；P11、P8、P13 碰到再做；P5 跟别人的 PR；P7 暂不做。

**投稿规矩**（findings-gpui 第 8 节，读 CONTRIBUTING）：AccessKit 先开 issue、附 Chromium 对照、方案认可后才能提 PR；Zed 同时开着的 PR ≤ 3 个、功能要先有 issue 或 discussion、PR 描述不能全靠模型写（本项目本来就是人写）。

**锁版本的写法**（findings-gpui 第 7 节）：`gpui = { git = "https://github.com/zed-industries/zed", rev = "<40 位>" }`；补丁用 `[patch."https://github.com/zed-industries/zed"]` 指向 fork；AccessKit 层用 `[patch.crates-io]`，大版本必须和 Zed 锁的 0.26.x 一致。还**必须照抄**那个 rev 下 Zed 根 `Cargo.toml` 的 `[patch.crates-io]` 里 gpui 依赖树要的几条（随 rev 变）——这一份做成比对：`check-gui-patches.py` 顺带比「我们抄的」和「那个 rev 的 Zed 根 `Cargo.toml`」（要能读到那份文件，CI 里拉不到就只在本机升级时跑）。
