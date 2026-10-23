# 核查结果 · GPUI 无障碍接口（命门 1、2）

> 2026-10-23 · 手下 gpui · 材料：`resources/zed`（浅克隆，HEAD `79fe6a6` 2026-10-23；gpui 为 Apache-2.0）+ accesskit_macos 0.26.3 源码（Zed 锁定版本）。只读源码，未编译。
> 下文路径省略前缀 `resources/zed/crates/`；`ak:` = accesskit `platforms/macos/src/`。

## 逐条核查

**1. 列表项进树（角色/名称/选中/第几项共几项）—— 原生 ✅**
`gpui/src/elements/div.rs:1357` `.role()`、`:1379` `aria_label`、`:1441` `aria_selected`、`:1510/1516` `aria_position_in_set/size_of_set`，统一写入 `:3589-3640`。前提：元素必须同时有 `.id()` 和 `.role()`，否则不进树（`gpui/src/element.rs:427-442`）。官方示例 `gpui/examples/a11y.rs:201-216` 就是 List/ListItem。

**2. 代码移焦点、VoiceOver 跟随 —— 原生 ✅（有前提）**
`window.focus(&handle)` 后下一帧 `div.rs:2378-2386` 把带焦点的节点报给树 → `gpui/src/window/a11y.rs:564-583` 写入 `TreeUpdate.focus` → `ak:event.rs:314-319` 发 `FocusedUIElementChanged`。另有「容器持焦点、选中项冒充焦点」的 `aria_active_descendant`（`div.rs:1418`，`a11y.rs:258-272`），正适合聊天列表。前提：目标项必须正在被渲染；没节点时退回整窗（`a11y.rs:193-201`）。

**3. 虚拟化列表屏幕外的项 —— 没有 ❌（预判成立）**
`uniform_list` 只渲染可见区间（`gpui/src/elements/uniform_list.rs:475-492`），`list` 只多渲染 overdraw 一段（`list.rs:1078-1082`）；节点只在 prepaint 时推入（`element.rs:427`），所以屏幕外的项根本不在树里。读屏自己发的滚动类动作（ScrollIntoView/ScrollDown 等）GPUI 不处理，只记日志（`gpui/src/window.rs:7130-7135`）。可补：`size_of_set` 照样报总数；用 active_descendant 由本项目响应方向键先滚再选；必要时 `on_a11y_action`（`div.rs:1550`）自己接滚动动作。

**4. 主动播报 —— 部分（预判被部分推翻）**
GPUI 没有 `aria_live` 一类专用接口（`div.rs:3589-3640` 不写 live）。但公开出口能直接拿到 AccessKit 节点：`a11y_synthetic_children` 的 `parent_node()`（`a11y.rs:364`）或自定义元素的 `write_a11y_info`（`element.rs:121`），且 `gpui.rs:99` 整个导出 accesskit。设 `Live::Polite/Assertive` + `value` 后，accesskit_macos 在节点新增或值变化时发 `NSAccessibilityAnnouncementRequestedNotification`（`ak:event.rs:35-44, 65-99, 236, 299-305`），Polite→中优先级、Assertive→高优先级。不需要另调 macOS。兜底：用 objc2 拿 NSWindow 直接发同一个通知。注意：同一句话重复不会再播（值没变）；是否打断当前朗读未实测。

**5. 导出无障碍树 —— 部分**
公开 `Window::debug_a11y_tree_json()`（`window.rs:7055`，实现 `a11y/debug.rs`），Zed 的 `dev: dump/copy accessibility tree` 就是调它（`zed/src/zed.rs:162-176`，PR #60397）。但只有读屏等辅助技术激活后才建树（`window.rs:1679, 1703`）；测试平台不实现 `a11y_init`（`platform.rs:1418` 默认空实现，`platform/test/` 无 a11y 代码），**所以在 GPUI 单元测试里拿不到树 ❌**。可行路径：真窗口 + 用 macOS AX 接口查询本进程（会触发激活）后导出；或给上游提 PR 让测试平台可激活。

## 判断

**首选：改造**——直接用 GPUI 的 AccessKit 接口，在本项目里补一层薄壳（不 fork）：播报元素（Live 节点）、聊天列表包装（active_descendant + 总数 + 先滚后选）、测试用的树导出器。理由：命门 1 原生满足，命门 2 有公开出口且 macOS 适配层已实现公告。
**备选：直接用**——若只靠 active_descendant + 键盘导航，不让 VoiceOver 自由游走，薄壳可更薄。

**三个风险**
1. 接口很新、变得快：AccessKit 接入 2026-05 才合并（#56065），之后 #60397、#61926、#64143 连续改动，跟上游要付升级成本。
2. 播报未实测：中优先级是否被 VoiceOver 丢弃或排队、重复内容不播，需要第一个原型就验证。
3. 虚拟化缺口 + 测试缺口：VoiceOver 自由浏览（VO+方向键）走到可见区边缘会断；单元测试拿不到树，验收得靠真窗口 + AX 接口。
