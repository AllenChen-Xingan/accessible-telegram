# 现状页 · exp003b

- 手上：`_tmp/20261023-exp/exp003b.md` —— 修真实按键；让 VO+←/→ 走出可见区；默认去掉「第几项共几项」
- 做到哪：读材料、读源码中
- 下一步：定位真实按键为何到不了应用

## 流水（时间 · 做了什么 · 路径或结果）

- 2026-10-23 · 读交代与共同规矩；报到大总管（已发）· `_tmp/20261023-exp/exp003b.md`
- 2026-10-23 · 读 exp003 README、人工试用.md、现状页 · `docs/experiments/exp003-list-nav/`、`state/handoff/exp003.md`
- 2026-10-23 · 读源码 · GPUI `window.rs:7078-7135`（a11y 动作只处理 Click/Focus/Blur）；`div.rs:1550`（`on_a11y_action`，加监听即向 AccessKit 声明该动作）；accesskit_macos `node.rs:638-666`（VO 设焦点 → Focus 动作，仅可聚焦节点）、`node.rs:1180-1205`（唯一的滚动类动作是 AXScrollToVisible → ScrollIntoView）；`window/a11y.rs:331-339`（节点 id = 父 id + 元素 id 的哈希，跨帧稳定，滚动后 VO 光标不丢）；`list.rs:1297`（`list()` 的 overdraw 只量不画，进不了树）；`div.rs:1424`（`a11y_synthetic_children` 可加不画出来的无障碍节点）
- 2026-10-23 · 读源码 · `gpui_macos/src/window.rs:2633-2770`：方向键（无字符、无修饰）**先交给输入法 `inputContext handleEvent:`**，输入法说「我处理了」就直接返回，GPUI 的按键绑定不再跑
- 2026-10-23 · 查环境 · 本机当前输入法 = 简体双拼（`com.apple.inputmethod.SCIM.Shuangpin`）；VoiceOver 正在运行（pid 13298）；本会话**没有发系统级按键的权限**（`CGPreflightPostEventAccess=false`、`AXIsProcessTrusted=false`）
- 2026-10-23 · 假设 A1：双拼输入法吞了方向键 → 用进程内真实 NSEvent（`NSApp postEvent`，走 AppKit → GPUIView keyDown → 输入法 全路径）验证
