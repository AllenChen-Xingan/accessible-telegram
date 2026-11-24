# 评审 043 · GPUI 应用架构与组件先例调研（findings-components、quill-index、exp010）（codex · gpt-6-astra · read-only）

> 对象：`docs/research/004_gui/findings-components.md`、`docs/research/004_gui/quill-index.md`、`docs/experiments/exp010-ui-closure/`。依据 `docs/plan/001_arch-stack.md`、`docs/plan/013_decisions-b19-gui.md`，以及从 MAIN `resources/`（仓外，codex 读不到）截出的源码摘录（`_tmp/gcomp-review/`，不入库）。**最新一轮：第 6 轮（2026-11-14）· 总评「修改」（仅 P2）· 满足停止线（第 5、6 轮连续无 P0/P1），不再审；剩余 P2 已直接改掉，无 issue**。
> 之前：atg-review 自审一轮（三视角），P1×6 全修（含两条实质更正：Tab/回车的输入法走向、Zed `settings_ui` 有输入框无障碍样本），P2/P3 大部分顺手改。
> 已定事项：组件路线甲（B19-10）、别人代码先推上游否则重写（B19-11）。
> 累计处理：P1×8、P2×9 逐条回源码核实，属实 15、部分属实 1、驳回 1。

## 第 6 轮（2026-11-14）· 总评「修改」（仅 P2）· 停

原文：`_tmp/gcomp-review/out/findings-components-codex-6.md`（不入库）。effort medium。
本轮 P0/P1：0 条 · 停止线：**满足**（第 5、6 轮连续无 P0/P1）

- **P2｜第四节第 3 小节写「表里不要用 Control+Option 组合」，与人定 B19-4「允许但非首选」冲突**（第 6 轮 #1）
  - 核实：属实
  - 处理：改为「允许但非首选，每用一个都要实测能否到达应用」。改动一行，不另开 issue

## 第 5 轮（2026-11-14）· 总评「修改」（仅 P2）

原文：`_tmp/gcomp-review/out/findings-components-codex-5.md`（不入库）。effort medium。上一轮三条处理 codex 均核实已落实。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮无 P0/P1

- **P2｜README「做法」与 `closure.py` 说明仍有「最多多出什么」的上界口吻**（第 5 轮 #1）
  - 核实：属实
  - 处理：两处统一改称「静态差集估算、不保证严格上界」；重型 crate 排除只引不减基线的上界清单

### 引用抽查

- 「不符」：无；「核实属实」9 条；无法核实 3 条（完整闭包要 Zed 全部 Cargo.toml、Cargo 具体诊断、Quill 原件——都在仓外或需实跑）

### 是否再审

无 P0/P1 但只连续 1 轮 → 再审一轮确认

## 第 4 轮（2026-11-14）· 总评「修改」

原文：`_tmp/gcomp-review/out/findings-components-codex-4.md`（不入库）。effort medium。
本轮 P0/P1：1 条 · 停止线：未满足

- **P1｜建议照 Quill 打印每次按键解析结果，没限制文本输入——会把手机号、密码、正文记进日志**（第 4 轮 #1）
  - 核实：属实（`.42cog/real.md`「隐私」；001 第四节第 1 小节）
  - 处理：第六节改为只记动作名、路由分支、修饰键，不记字符；配一条「开追踪打假号码、日志里没有它」的测试
- **P2｜基线默认模式按子串匹配 target，可能把 `not(macos)` 算进基线，差集不保证是上界**（第 4 轮 #2）
  - 核实：属实
  - 处理：默认模式排除含 `not(` 的条件，重算（数字不变）；README 与 findings 改称「静态差集」，写明 cfg 是粗判、不是严格上界；重型包排除仍靠不减基线的完整上界清单
- **P2｜「不组字时 Cmd + 键不经过输入法」超出源码：未处理且不是菜单快捷键时后段仍交 `inputContext`**（第 4 轮 #3）
  - 核实：属实（`zed-new:gpui_macos/src/window.rs` 第 2920 行之后的 key-equivalent 段）
  - 处理：第〇节第 9 条与表格改为「先走按键匹配，匹配不上可能再交输入法」

### 引用抽查

- 「不符」：无；「核实属实」8 条；无法核实同前两条

### 是否再审

有 P1 → 改完开第 5 轮

## 第 3 轮（2026-11-14）· 总评「修改」

原文：`_tmp/gcomp-review/out/findings-components-codex-3.md`（不入库）。effort medium。
本轮 P0/P1：2 条 · 停止线：未满足

- **P1｜两个上界相减不是「新增」的上界，会扣掉 ui 才打开的依赖；重型 crate 检查在减法之后做**（第 3 轮 #1）
  - 核实：属实（逻辑问题）
  - 处理：`closure.py` 的基线改按默认模式算（`--minus-upper` 才按上界）；另存不减基线的上界闭包 `out-ui-upper.txt`（21 个 crate，重型 crate 一个不中）；`ui` 上界 − 基线默认 = 9 个（7 个 GPL + gpui 可选特性带的 `bench_metrics`、`zlog`），约 4.3 万行。README 与 findings 第二节 ① 同步，并写明本项目实际特性与平台接进工程时用 `cargo tree` 再确认
- **P1｜消息列表建议仍把 `scroll_to_reveal_item` 当通用跨屏方案，与 ggpui 实测反例（exp009：往下跳到没量过的项时根本不滚，9 步错 8 步）相矛盾**（第 3 轮 #2）
  - 核实：属实（`docs/research/004_gui/findings-gpui.md` 第 4 节）
  - 处理：第五节与 M3.4 改为「逐项用 `scroll_to_reveal_item` + overdraw，跳跃用 `scroll_to(ListOffset)`，目标进树再设 active descendant」；删去「同一帧生效」对 `list` 的照搬。顺带对齐 ggpui 同节的另一条实测：第一节原写 Quill 用 `Entity::cached` 拆切片，改为「我们不用 `.cached(..)`」（#66690），M3.1 同步
- **P2｜最小打包路径说用 `dev-sign.sh` 签整个包，但脚本只收普通文件**（第 3 轮 #3）
  - 核实：属实（`scripts/dev-sign.sh:20` 用 `-f`）
  - 处理：第七节与 M4.4 写明要先扩展脚本或另写一步同身份、同标识签包

### 引用抽查

- 「不符」：无
- 「核实属实」7 条
- 无法核实：「Cargo 会报 patch 未被使用」（已标推断）；Quill 原件（仓外）

### 是否再审

有 P1 → 改完开第 4 轮

## 第 2 轮（2026-11-14）· 总评「修改」

原文：`_tmp/gcomp-review/out/findings-components-codex-2.md`（不入库）。effort medium。上一轮驳回的 #2（settings_ui 挂接）codex 看扩大后的摘录后接受。
本轮 P0/P1：2 条 · 停止线：未满足

- **P1｜exp010 的排除结论仍未证成：只在已发现的 crate 里搜终点名，排除不了经漏掉的中间包抵达；没有基线差集**（第 2 轮 #1）
  - 核实：属实（第 1 轮的复核只看了 7 个 GPL crate 的可选项与 19 个 crate 的终点名）
  - 处理：`closure.py` 加上界模式（`--upper`：可选、构建、全 target 依赖全算）与基线差集（`--minus gpui,gpui_platform`）；两种模式下比基线多出的都是同样 7 个 GPL crate、39110 行，重型 crate 一个不中。README「做法 / 结果 / 复核记录 / 局限」重写；findings 第二节 ① 同步。外部依赖不展开的理由写明（外部包不可能依赖 Zed 未发布的内部 crate）
- **P1｜按键走向摘要与表格仍矛盾：空格是可打印键；无字符键漏了「不带 Ctrl/Fn/Cmd」；Ctrl 行漏了组字例外**（第 2 轮 #2）
  - 核实：属实（`zed-new:gpui_macos/src/events.rs:367-369` 空格的字符是普通空格；`window.rs` 无字符键分支要求不带 control / function / platform）
  - 处理：第〇节第 9 条改为「空格与字母同规则、Tab 回车是控制字符」；表中无字符键行加修饰键条件，Ctrl 行加组字例外
- **P2｜第二节 ③ 仍称「不能作依赖混用」**（第 2 轮 #3）
  - 核实：属实
  - 处理：改为「不能直接组成同一棵组件树；Cargo 层面能否共存未查，不作理由」

### 引用抽查

- 「不符」1 条（空格）：已改（#2）
- 「核实属实」7 条
- 无法核实：「Cargo 会报 patch 未被使用」的具体诊断（findings 已标推断）；Quill 原件（仓外）

### 是否再审

有 P1 → 改完开第 3 轮

## 第 1 轮（2026-11-14）· 总评「修改」

原文：`_tmp/gcomp-review/out/findings-components-codex-1.md`（不入库）。effort medium，耗时 41 秒。行号指评审时的版本。
本轮 P0/P1：3 条 · 停止线：未满足

- **P1｜第四节第 4 小节表：「Cmd + 键从不先给输入法」与源码不符**（第 1 轮 #1）
  - 核实：属实（`zed-new:gpui_macos/src/window.rs:2890` 的 `is_composing` 是独立「或」分支，不看修饰键；可打印键还要处理器的 `prefers_ime_for_printable_keys` 为真，`ElementInputHandler` 取 `accepts_text_input`，`zed-new:gpui/src/input.rs:297-300`，平台默认假 `platform.rs:2555-2562`）
  - 处理：表里加「正在组字时的任何键」一行；Cmd 行改为「不组字时不给」；可打印键行写明三个条件；第〇节第 9 条同步
- **P1｜把未挂接的 TextRun 闭包当作已生效实现**（第 1 轮 #2）
  - 核实：不属实（codex 拿到的摘录截在第 240 行；`zed-new:settings_ui/src/components/input_field.rs:241` 正是 `.a11y_synthetic_children(a11y_text_runs)`，`:226` 取闭包；设置页搜索框同样挂接 `settings_ui.rs:3027-3035`）
  - 驳回理由：挂接点存在。顺手把这几行号补进 findings 的引用，第 2 轮摘录放宽到 220–245 行
- **P1｜exp010 的静态闭包不足以证明「只新增 7 个 GPL crate、不拖 settings / editor」**（第 1 轮 #3）
  - 核实：部分属实（算法确实不解析特性、跳过可选与构建依赖；离线 `cargo tree --offline --locked -p ui` 因 git 依赖未缓存跑不了）
  - 处理：改为直接核排除结论——7 个 GPL crate 的内部依赖无可选项、特性只开 test-support、无构建依赖；19 个 crate 全文检索 settings / editor / workspace 等只在 `ui` 的 dev-dependencies 出现。写进 exp010 README「复核」一节；findings 标「行数是静态下界，排除结论已复核」
- **P2｜「不能装进同一个程序」超出证据**（第 1 轮 #4）
  - 核实：属实
  - 处理：改为「不能组成同一棵组件树；Cargo 不禁止共存，链接冲突未查，是工程取舍」
- **P2｜建议从 Zed assets「用到几个拷几个」违反铁律 3 / B19-11**（第 1 轮 #5）
  - 核实：属实
  - 处理：改为依赖同 rev 的 Zed `assets` crate、自备开源图标或不用图标按钮；明写不从 `resources/` 拷
- **P2｜建议表漏了 B19-8 详略开关**（第 1 轮 #6）
  - 核实：属实
  - 处理：M4.3 一行加「汇总 / 只计数 / 关」开关、三档策略分支与假时钟单测

### 引用抽查

- codex 列的「不符」2 条：window.rs 一条已改（#1）；input_field 一条驳回（#2）
- 「核实属实」5 条
- 无法核实：`zed-new:gpui/Cargo.toml` 的 0.2.2 与 `[patch]` 报错（主会话读过 `version = "0.2.2"`；「未被使用」的具体诊断属 Cargo 规则推断，findings 已标推断）；Quill 原件行号（仓外）

### 是否再审

有 P1 → 改完开第 2 轮，附本记录
