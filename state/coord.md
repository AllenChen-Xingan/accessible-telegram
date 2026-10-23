# 大总管看板（coord）

> 只有大总管跟人说话。手下做完、卡住、要名额，只报给大总管。
> 大总管会话名：`telegram-accessibility-c7`（换班后在这里改）。tmux 工作区：`aiw`。
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
| `tg`（会话 `telegram-accessibility-b3`） | 001 · 命门 3：grammers vs TDLib | `_tmp/20261023-dispatch/tg.md` | `state/handoff/tg.md` | 完成，已收尾（窗口已关） |

## 规矩

- **交接写成文件**：每个手下维护 `state/handoff/<窗口名>.md`（手上是什么 · 做到哪 · 下一步）
- **只有大总管提交、推送、开 PR**；手下只写自己负责的文件，不碰 git 的写操作
- **重活名额：每台机器同一时刻只许一处重编译 / 全量测试**，名额由大总管发
- **会话勤轮换**：做完一个自然段落就写交接、收尾，由大总管起新的接班
- 评审分级、先便宜后贵，见 `specs/workflow.md` 第 6 节

## 名额

| 名额 | 当前持有 |
|---|---|
| 重编译 / 全量测试（1 个） | 空 |

## 待定清单（攒着等人一次看完）

- [ ] `001_stack/must-know.md` 三条命门对不对
- [ ] `001_stack/decision.md` 拍板：Telegram 层 · 列表浏览方式 · 实验顺序（tdlib-rs 已定 B：自维护补丁，做好反馈上游）
- [ ] 主要用哪种辅助功能：VoiceOver / 键盘 / 缩放（线索：Claude Code 设置里开着读屏模式）
- [ ] 放权边界：哪些动作不用逐条确认
