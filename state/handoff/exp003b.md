# 现状页 · exp003b

- 手上：`_tmp/20261025-batch/exp003b-v2.md` —— 专门消息列表 + Tab / VO+←/→ 到达 + ↑/↓ 浏览
- 做到哪：**可以收尾**。新交互写完，`selftest-tab` 两轮各 45 步 0 失败；README「后续 · exp003b」含做法 → 自测 → 推荐；人工试用.md 新增「第三次试用」
- 下一步：等人按「第三次试用」试听（含输入法对照 A/B/C）；试完跑 `python3 summarize-live.py` 看按键是否到达应用
- 要人拍板：列表名带不带「共 N 条」（默认不带，`--list-count` 对照）；VO 开着时 ↑/↓ 能否到达、双拼是否吞键——只能人试

## 流水（时间 · 做了什么 · 路径或结果）

- 2026-10-23 · 读交代与共同规矩；报到大总管（已发）· `_tmp/20261023-exp/exp003b.md`
- 2026-10-23 · 读 exp003 README、人工试用.md、现状页 · `docs/experiments/exp003-list-nav/`、`state/handoff/exp003.md`
- 2026-10-23 · 读源码 · GPUI `window.rs:7078-7135`（a11y 动作只处理 Click/Focus/Blur）；`div.rs:1550`（`on_a11y_action`，加监听即向 AccessKit 声明该动作）；accesskit_macos `node.rs:638-666`（VO 设焦点 → Focus 动作，仅可聚焦节点）、`node.rs:1180-1205`（唯一的滚动类动作是 AXScrollToVisible → ScrollIntoView）；`window/a11y.rs:331-339`（节点 id = 父 id + 元素 id 的哈希，跨帧稳定，滚动后 VO 光标不丢）；`list.rs:1297`（`list()` 的 overdraw 只量不画，进不了树）；`div.rs:1424`（`a11y_synthetic_children` 可加不画出来的无障碍节点）
- 2026-10-23 · 读源码 · `gpui_macos/src/window.rs:2633-2770`：方向键（无字符、无修饰）**先交给输入法 `inputContext handleEvent:`**，输入法说「我处理了」就直接返回，GPUI 的按键绑定不再跑
- 2026-10-23 · 查环境 · 本机当前输入法 = 简体双拼（`com.apple.inputmethod.SCIM.Shuangpin`）；VoiceOver 正在运行（pid 13298）；本会话**没有发系统级按键的权限**（`CGPreflightPostEventAccess=false`、`AXIsProcessTrusted=false`）
- 2026-10-23 · 假设 A1：双拼输入法吞了方向键 → 用进程内真实 NSEvent（`NSApp postEvent`，走 AppKit → GPUIView keyDown → 输入法 全路径）验证
- 2026-10-23 · 写代码 · `docs/experiments/exp003-list-nav/src/ax.rs` 追加：真实按键（CGEvent→NSEvent→`NSApp postEvent`）、NSEvent 本地按键监视器、模拟 VO+←/→ 的走查（`vo_land_on` 做 AXScrollToVisible / setAccessibilityFocused）、现场记录 `log_event`
- 2026-10-23 · 改 `Cargo.toml` · 加 `block2 = "0.6"`、`async-channel = "2"`（锁文件里已有同版本）
- 2026-10-23 · 重写 `docs/experiments/exp003-list-nav/src/main.rs` · 默认读法去掉「第N条」；新增 `--key-fix on|off`（AppKit 监视器截方向键直交 GPUI，绕输入法）、`--sync none|scroll|focus|both`（接 VO 的 ScrollIntoView / Focus，同步选中并在近边缘时滚到中间）、`--overdraw K`（上下各多渲染 K 项）；自测 `--selftest-real`（真实 AppKit 按键）、`--selftest-vo`（模拟 VO+→/←）；人工试用自动记 `out/live-*.jsonl`
- 2026-10-23 · 改 `run.sh` · 新增 `./run.sh selftest-b`（9 轮）；`zsh -n` 通过
- 2026-10-23 · 申请编译名额（`cargo build` + `./run.sh selftest-b`）· 大总管回：排队中（名额在 exp004）；系统级发键权限不开，用进程内 AppKit 事件验证并在 README 写明局限；输入法疑点由大总管问人
- 2026-10-23 · 改 `docs/experiments/exp003-list-nav/人工试用.md` · 新增「第二次试用（exp003b）」一轮，以 VO+←/→ 为主，14 步，附两个可选对照
- 2026-10-23 · 拿到编译名额（只限 cargo build；弹窗口的自测要等人同意）· 开始编译
- 2026-10-23 · `cargo build` · 一次编过（17 秒）· 等人同意后跑 `./run.sh selftest-b`
- 2026-10-23 · 归还编译名额（人说「等我说再跑」，selftest-b 暂不跑）
- 2026-10-23 · 改 `docs/experiments/exp003-list-nav/人工试用.md` · 第一步先切 ABC；新增必做「输入法对照」A/B/C（ABC+不绕 / 双拼+不绕 / 双拼+默认绕）
- 2026-10-23 · 写 `docs/experiments/exp003-list-nav/README.md` 「后续 · exp003b」草稿 · 做法、源码依据、局限已写；结果与推荐待自测
- 2026-10-25 · 改向：读 `_tmp/20261025-batch/exp003b-v2.md`（专门消息列表 + Tab / VO+←/→ 到达 + ↑/↓ 浏览）；向新大总管 telegram-accessibility-40 报到（已发）
- 2026-10-25 · 改 `src/main.rs` · 新交互：界面加「会话：王小明」「搜索」（按钮）、「输入消息（占位）」（输入框）、「发送」；Tab 顺序 标题→搜索→列表→输入→发送，Tab/Shift+Tab 绑 `focus_next/prev`；列表名默认「消息」，`--list-count` 改为「消息，共 1000 条」；`--sync` 默认改 none（降为次要，代码保留）；key-fix 改为：Tab 任何时候截、↑/↓ 只在列表有焦点时截（`LIST_FOCUSED` 由 on_focus/on_blur 维护）
- 2026-10-25 · 改 `src/ax.rs` · 加 Tab/Shift+Tab 键码；`vo_stops()`（macOS 层 VO+←/→ 会经过的元素）；`vo_focus_list()`（模拟 VO 光标落到列表时设键盘焦点）
- 2026-10-25 · 加自测 `--selftest-tab`（38 步：Tab 环、列表内 ↑/↓ 跨屏、离开/回来选中保持、输入框时 ↓ 不动列表、VO 设焦点到列表后 ↓ 生效）；`run.sh selftest-tab`（绕/不绕输入法各一轮）
- 2026-10-25 · 写 `docs/experiments/exp003-list-nav/summarize-live.py` · 汇总人工试用现场记录（按键是否到达应用 / 被 GPUI 认出 / 列表是否移动）
- 2026-10-25 · 改 `人工试用.md` · 新增「第三次试用（exp003b-v2）」17 步（第一步切 ABC），保留输入法对照 A/B/C，可选 --list-count 对照
- 2026-10-25 · 拿到名额 + 窗口许可（cargo build + selftest-tab）
- 2026-10-25 · `cargo build` 通过 · 第一次跑 selftest-tab 两轮都崩：objc2 类型编码不符（`eventWithCGEvent:` 要 `^{__CGEvent=}`）→ 改 `src/ax.rs` 给 CGEvent 定类型
- 2026-10-25 · 重跑：key-fix off 45 步全过；key-fix on 崩（「MessageList 正在更新时再次更新」）——是我的 bug：绕过输入法那条通道用 `WindowHandle<MessageList>::update` 借出了根视图 → 改 `cx.update_window(handle.into(), …)`
- 2026-10-25 · 重跑 `./run.sh selftest-tab` · **两轮都 45 步 0 失败**；窗口全程是 key；VO 会经过的元素依次为 按钮「会话：王小明」→ 按钮「搜索」→ **AXList「消息」** → 文本框「输入消息（占位）」→ 按钮「发送」；按键→新帧 p50≈15.3 ms · `out/v2-tab-keyfix-on-summary.json`、`out/v2-tab-keyfix-off-summary.json`
- 2026-10-25 · 注意：自测时本机输入法是 U.S.（人已切走双拼），所以本次没测到「双拼吞方向键」；不擅自切输入法（会动人的全局状态），留给人工试用对照 A/B/C
- 2026-10-25 · 写 `README.md`「后续 · exp003b」：补「改向 · exp003b-v2」（做法、自测表、VO 会经过的元素、三条局限）与「推荐」；A/B 两节结果栏注明未单跑的原因
