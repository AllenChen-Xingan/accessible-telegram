# 大总管看板（coord）

> 只有大总管跟人说话。手下做完、卡住、要名额，只报给大总管。
> 大总管会话名：`telegram-accessibility-ca`（换班后在这里改）。tmux 工作区：`aiw`。
> 做法依据：阳志平《AI 自主系统的高级玩法：建一个大总管》（2026-10-15，安仁书院）。

## Top 4（眼下最重要）

1. 系统分析 001：**取材、分析、双谱系评审已完成**，等人拍板 `docs/research/001_stack/decision.md` 的 4 条
2. 人确认 `001_stack/must-know.md` 的三条命门（命门 1 可能因拍板 3 放宽）
3. 安装 Rust（小实验要编译）
4. 初始化放行检查（冒烟：只喂 intent.md 问三句）

## Top 3（次要）

1. 开第一个 issue（人写）
   - tdlib-rs 补丁：fork 到本人账号，`local-tdlib` 编 TDLib 1.8.69 + 同步 `tl/api.tl` 与库文件名（`build.rs:93,310`）；要编译名额；做好后向上游提 PR（PR 描述人来写）
2. 三个实验 `exp001-announce` → `exp003-list-nav` → `exp002-tdjson-latency`（要 Rust + 编译名额）
3. 第一个原型：`atg-cli` 登录 + 列会话

## 手下

| 窗口 | 任务 | 交代文件 | 现状页 | 状态 |
|---|---|---|---|---|
| `gpui`（会话 `telegram-accessibility-c5`） | 001 · 命门 1、2：GPUI 无障碍接口 | `_tmp/20261023-dispatch/gpui.md` | `state/handoff/gpui.md` | 完成，已收尾（窗口已关） |
| `exp001`（会话 `telegram-accessibility-23`） | 实验：播报会不会打断朗读 | `_tmp/20261023-exp/exp001.md` | `state/handoff/exp001.md` | **完成，已收尾**；剩人工试听 |
| `exp002`（会话 `telegram-accessibility-fc`） | 实验：TDLib 往返、重绘、冷启动分段计时 | `_tmp/20261023-exp/exp002.md` | `state/handoff/exp002.md` | **完成，已收尾**（拍板 1 坐实） |
| `exp003`（会话 `telegram-accessibility-be`） | 实验：方向键跨屏读 1000 条 | `_tmp/20261023-exp/exp003.md` | `state/handoff/exp003.md` | **完成，已收尾**；剩人工试听 |
| `exp003b`（会话 `telegram-accessibility-15`） | 列表：修真实按键 + VO+←/→ 走出可见区 | `_tmp/20261023-exp/exp003b.md` | `state/handoff/exp003b.md` | 已派 |
| `exp004`（会话 `telegram-accessibility-e7`） | 新消息提示不打断朗读（5 种候选） | `_tmp/20261023-exp/exp004.md` | `state/handoff/exp004.md` | 已派 |
| `tg`（会话 `telegram-accessibility-b3`） | 001 · 命门 3：grammers vs TDLib | `_tmp/20261023-dispatch/tg.md` | `state/handoff/tg.md` | 完成，已收尾（窗口已关） |

## 规矩

- **交接写成文件**：每个手下维护 `state/handoff/<窗口名>.md`（手上是什么 · 做到哪 · 下一步）
- **写任何东西、做任何工作都要回报**（人定 2026-10-23）：当场记进现状页，重要节点再发消息
- **只有大总管提交、推送、开 PR**；手下只写自己负责的文件，不碰 git 的写操作
- **重活名额：每台机器同一时刻只许一处重编译 / 全量测试**，名额由大总管发
- **手下可以自己派子代理**（人定 2026-10-23），子代理同样受名额与 git 规矩约束
- **会话勤轮换**：做完一个自然段落就写交接、收尾，由大总管起新的接班
- 评审分级、先便宜后贵，见 `specs/workflow.md` 第 6 节

## 名额

| 名额 | 当前持有 |
|---|---|
| 重编译 / 全量测试（1 个） | **exp004**（exp002 已归还） |

排队（先来先得；GPUI 首次编译最重，之后共享 `_build/20261023-154000`）：

- （空）

## 发现（实验中途）

- exp003：accesskit_macos 0.26.3 不把 `position_in_set` / `size_of_set` 交给 macOS，「第几项共几项」大概率听不到；对照组试把它直接拼进 `aria_label`

- exp002：进程内调内容视图的 `accessibilityChildren` 即可激活 GPUI 无障碍树，**不需要辅助功能权限**（accesskit_macos `subclass.rs:92-99`）；用 Zed 的 `Cargo.lock` 作种子可共享编译产物。已转给 exp001、exp003

- exp001（初步）：GPUI 原生 `Live` 出口能发公告（Polite 优先级 50 / Assertive 90，改值到发通知 5–21 ms），无焦点变化；同一句重复不播，「换节点」「零宽字符」两种绕法都能补；兜底直发也通。`docs/experiments/exp001-announce/out/result.md`。**是否打断朗读仍要人用 VoiceOver 听**

- **人工试听推翻 exp001 假设 3**：礼貌式公告也会插进正在进行的朗读。「新消息不打断朗读」要另想办法

- exp003（初步）：四种模式各 1137 步在无障碍树层面全部通过，`docs/experiments/exp003-list-nav/out/`；VoiceOver 实听待人

- **人试听 exp003**：只能用 VO+←/→ 切换，单独方向键没反应（待确认快速浏览）。使用者习惯 VO 自由浏览 → 拍板 3（应用方向键驱动）可能要改，候选：接住 VoiceOver 的滚动请求（`on_a11y_action`），让 VO+→ 能走出可见区

- exp002（debug 上界）：TDLib 往返中位 0.02–0.1 ms、约 2.2 万条/秒，不是瓶颈；新消息→无障碍树更新 50 项中位 21 ms / p95 31 ms，500 项 48 / 56 ms（大头在 GPUI 渲染）；冷启动→第一帧 160 ms，→树可查 284 ms（p95 742 ms）。release 暂不跑

## 待定清单（攒着等人一次看完）

- [x] 人工试听 exp001：礼貌式**插进来播**、打断式掐断立即播 → 「不打断朗读」靠 Live 做不到，需后续实验（exp004）
- [ ] **人工试听 exp003**：`docs/experiments/exp003-list-nav/人工试用.md`（第一轮 13 步 + 第二轮）
- [x] 位置怎么读：**不读**（人：听着奇怪）；A 否决
- [ ] `001_stack/must-know.md` 三条命门对不对
- [ ] `001_stack/decision.md` 拍板：实验顺序（已定：列表浏览 = 应用方向键驱动；Telegram 层 = TDLib + tdlib-rs；tdlib-rs = B 自维护补丁，做好反馈上游）
- [ ] 主要用哪种辅助功能：VoiceOver / 键盘 / 缩放（线索：Claude Code 设置里开着读屏模式）
- [ ] 放权边界：哪些动作不用逐条确认
