# exp009-gpui-head · Zed 主干上 GPUI 无障碍的五个小问题

> 2026-11-14 · 手下 ggpui · Zed `15d136b`（gpui + accesskit_macos 0.26.3，MAIN `resources/zed-20261114/`）· Rust 1.99.1 · macOS 27 · Apple M5 10 核
> 一次性实验，假数据。本实验不去开 VoiceOver、不放声音；⚠️ 但 R6、R7 两次实测本机 VoiceOver **开着**（`isVoiceOverEnabled = true`）；R1–R5 没记 VO 状态，推断当时也开着，所以各次运行的窗口内容和播报句很可能被 VO 念出来了。R1–R6 跑时 `run.sh` 还没有「VO 开着就不跑」的门；门是 R6 之后加的（common 第 16 条），R7 经人同意后以 `EXP009_VO_OK=1` 放行。服务 `docs/research/004_gui/findings-gpui.md` 第 3–7 节。

**运行表**（一共 7 个进程；下文一律用 R1–R7，分母都是 7，除非另说）：

| 编号 | 时间 | 模式 | 代码版本 | 结果文件 |
|---|---|---|---|---|
| R1 | 15:03 | reveal | 初版（无 `--list-mode`，步骤记录只有 focus） | `out/result-reveal-run1.json`（手工改名） |
| R2 | ~15:08 | reveal | 加 `--list-mode`；步骤记录加 `gpui_focus` / `ad` / `item_in_tree` | `out/result-reveal-run2.json`（手工改名） |
| R3 | 15:08 | measure | 同 R2 | `out/result-measure.json` |
| R4 | 15:08 | scrollto | 同 R2 | `out/result-scrollto.json` |
| R5 | ~15:11 | step | 同 R2 | `out/result-step-run2.json`（手工改名） |
| R6 | 15:11 | step | 加 `on_a11y_action(Focus)` 记录、VO 状态 | `out/result-step.json` |
| R7 | 15:19 | reveal | 同 R6，另加每步 `t_ms`（仓里现在的代码） | `out/result-reveal.json` |

R1–R5 的代码没有单独留存；与现在的差别就是上表「代码版本」一列写的几处。

## 问题

1. 播报节点只设值 / 只设名称 / 两者都设，在 accesskit_macos 0.26.3 下各发不发公告（findings 第 3 节：0.27 起改按名称触发）
2. 变高 `list` 上「先滚后选 + active descendant」能不能让选中项每次进树、焦点在列表（第 4 节）
3. 应用级焦点元素（上游 issue #66691）：contentView 和 `NSApp` 各答什么；issue 评论的权宜做法有没有用（第 6 节）
4. 输入框（`Role::TextInput` + 一个 `TextRun` + 选区）移光标时发不发 `AXSelectedTextChanged`、带不带 userInfo（第 5 节）
5. 缓存视图（上游 issue #66690）：只重画根视图后，缓存视图里的按钮还在不在树里（第 4 节）
6. 顺带：只编 gpui 的冷编译时间和 `target/` 盘占用（第 7 节）

## 做法

- `src/main.rs`：一个窗口，自上而下：播报节点（`Role::Status`，经 `a11y_synthetic_children` 的 `parent_node()` 设 Live）、300 条变高假消息的 `list`（容器 `Role::List` 持焦点，选中项 `aria_active_descendant`）、输入框（照 Zed 设置页输入框的结构自写，不抄代码）、一个用 `.cached(..)` 画的子视图（里面一个按钮）。启动后自动跑完五项、写结果、退出
- `src/ax.rs`：进程内问 macOS 层（不需要辅助功能权限）；截获本进程发出的无障碍通知（导出与 AppKit 同名的两个函数，记下后经 `dlsym(RTLD_NEXT)` 转给 AppKit，同 exp001 的思路）；#66691 的权宜做法（`class_addMethod` 给第一响应者的类加 `accessibilityFocusedUIElement`，转问父视图）
- `--list-mode`：`reveal`（默认，overdraw 0 + `scroll_to_reveal_item`）· `measure`（`ListState::measure_all()`）· `scrollto`（`scroll_to(ListOffset)`，目标项放到顶端）· `step`（overdraw 200 px，像按 ↓ 那样一次走一项，0→60→0）

## 跑法

```sh
# 编（要先向大总管领重活名额）
export CARGO_TARGET_DIR="<MAIN>/_build/20261114-exp009"   # MAIN = 主工作区「telegram accessibility 」（目录名末尾有一个空格，Cargo.toml 和 run.sh 的相对路径都依赖它）
cargo build            # 在本目录
# 跑（开窗、抢前台每次 5–20 秒，30 秒强杀；先报大总管）
# VoiceOver 开着时 run.sh 直接退出（exit 2）；人同意后才用 EXP009_VO_OK=1 放行
./run.sh                       # reveal
./run.sh --list-mode measure   # 等等
```

输出在 `out/`（不入库）：`result-<模式>.json`、`tree-end-<模式>.json`、`cold-build.log`。

⚠️ 本机没有离线 Metal 编译器（只有 Command Line Tools；Xcode 缺 Metal Toolchain 组件），所以 `gpui_platform` 开了 `runtime_shaders` 特性。默认特性下的失败记录在 `out/cold-build-attempt1-metal-missing.log`。

## 结果

### 编译（实测）

| 项 | 数 |
|---|---|
| debug 冷编译（`target/` 清空后） | 29 秒（⚠️ 日志只留了末 6 行，证明不了是真清空后编的；待重测，见下） |
| `target/` 盘占用 | 2.1 GB |
| 改本实验代码后增量 | 2.2 秒 |

crate 下载已在本机缓存，只算编译；没有 sccache。

### 各项结果（R1–R7，无 panic，没出现 `Duplicate a11y node id`）

| 项 | 结果 | 次数 | 判定 |
|---|---|---|---|
| 1 播报 | 只设值 → 1 条公告；只设名称 → **0**；两者 → 1 | 7/7 | 实测只覆盖 accesskit 的「新增节点」分支（每次换节点键，`ak:event.rs:236`）；「同一节点值变化」分支（`:299-305`）与 0.27 的变化是读源码 |
| 3 应用级焦点 | contentView 答焦点元素，`NSApp` 答 `AXWindow「exp009」`，第一响应者 `GPUIView` | 7/7 | #66691 复现 |
| 3 权宜做法 | 装上后 `NSApp` 的答案与 contentView（AccessKit）一致 | 7/7 | 进程内有效；VO 切回应用时实际落在哪没听。R2 那次两者都答列表项（焦点那时就在列表上） |
| 4 移光标 | 每次 4 移 → 4 条 `AXSelectedTextChanged`，全不带 userInfo | 7/7（共 28 条） | P12 的前提成立；VO 实际读不读要人试 |
| 5 缓存视图 | 「只重画根视图后按钮不在树里」 | 6/7（R5 例外） | #66690 复现，跟帧时序有关 |
| 5 缓存视图 | 第一次查时按钮在树里 | 1/7（R3） | 这一查在前几项把根视图重画过多次之后，不是窗口刚开的「首次」 |

**2 变高 list「先滚后选」**（`step` 模式的结果文件只记失败的步骤，所以 `fails: 0` 时 `steps` 是空数组；步数 120 来自代码里的序列）（跳跃序列 1, 5, 40, 41, 120, 299, 298, 150, 0；`step` 是 1→60→0 逐项）：

| 运行 | 模式 | 预判（读 `list.rs`） | 错步 | 说明 |
|---|---|---|---|---|
| R1、R2、R7 | reveal（overdraw 0 + `scroll_to_reveal_item`） | 错 8/9 | 8/9、8/9、7/9 | 往下跳到没量过的项**不滚**，目标项不渲染、不进树，active descendant 落空。R7 比前两次少错 1 步：最后一步选回第 1 项时，R1、R2 的焦点正好漂到输入框（见下），R7 没漂 |
| R3 | measure（`measure_all()`） | 9/9 | 0/9 | 有全部高度可算；代价是首帧把 300 项全排一遍版（耗时没量） |
| R4 | scrollto（`scroll_to(ListOffset)`） | 9/9 | 0/9 | 目标项放到顶端，不依赖高度 |
| R5、R6 | step（逐项 + overdraw 200 px） | 0/120 | 4/120、0/120 | R5 错的 4 步（选 39→36）目标项**在树里**，是焦点漂到了输入框 |

**6 焦点漂移**（代码只在动列表时，GPUI 焦点跑到输入框，之后又回来）：

| 运行 | 有没有焦点记录 | 漂移 | 外部「设焦点」 |
|---|---|---|---|
| R1 | 无 | 有（选 120 起 5 步） | 没记 |
| R2 | 无 | 有（选 299 起 4 步） | 没记 |
| R3、R4 | 无 | 无 | 没记 |
| R5 | 无 | 有（选 39→36） | 没记 |
| R6 | 有 | 无 | 1 次：455 ms，目标**列表**（早于本程序进程内激活的 800 ms） |
| R7 | 有 | 无（7 步 active descendant 落空，焦点仍在列表） | 1 次：307 ms，目标**列表** |

R6、R7 给列表和输入框都挂了 `on_a11y_action(Focus)`：记下时间、照常移焦点（与 GPUI 内置处理相同）。发请求的进程在进程内拿不到（AppKit 不暴露 AX 请求的调用方）。

解释（推断）：① VoiceOver「键盘焦点跟随 VO 光标」——VO 光标落到输入框时发 Focus，GPUI 移走键盘焦点；② 应用自己让焦点节点消失、VO 另找落点——R7 里落空 7 步也没漂，**被削弱**；③ 人当时在操作电脑（如点了输入框；本实验没记 Click 动作）。**原因未定，不再追**；建议 `atg-gui` 调试日志记下每个外部 Focus / Click 动作及目标，GUI 开发中真碰到再查。

## 结论

1. **播报**：0.26.3 下只设名称不播；名称和值设同一句，7/7 都播（只测了「新增节点」分支；0.27 半边是读源码）
2. **变高 `list` 的先滚后选**：`scroll_to_reveal_item` 只在目标项已量过高度时有效。逐项 ↓ 用它 + overdraw；**跳跃（PageDown、End、落到第一条未读、从通知跳到某条）用 `scroll_to(ListOffset)`**；不用 `measure_all()`（长会话不行）。active descendant 本身没问题：目标项进了树就是它
3. **#66691**：复现 7/7；权宜做法进程内有效 7/7
4. **选区通知不带 userInfo**：7/7
5. **#66690**：复现 6/7，跟帧时序有关
6. **焦点漂移**：3/7（都在没带记录的 R1、R2、R5）；原因未定（推断），不再追

## 改变了哪一个取舍

- 播报器：名称和值设同一句（findings 第 3 节）——0.26.3 下实测
- 应用级焦点：薄壳先装 #66691 的权宜做法，同时上游提 P10——进程内有效，VO 下待人听
- 缓存视图：`atg-gui` 禁用 `.cached(..)`——已复现
- 消息列表滚动：逐项 ↓ 用 `scroll_to_reveal_item` + overdraw，跳跃用 `scroll_to(ListOffset)`，不用 `measure_all()`——实测
- 焦点漂移：原因未定，不定「外部设焦点」策略（现状页第 5 条搁置）；`atg-gui` 调试日志记外部 Focus / Click
