# 现状页 · gcomp（b19 · GPUI 应用架构与组件先例）

大总管：`telegram-accessibility-98`

- **已收尾**（2026-11-14 大总管 -98 验收通过；`docs/reviews/README.md` 那行保留）
- 写了（都没 git add，交大总管提交）：
  - `docs/research/004_gui/findings-components.md`（285 行）
  - `docs/research/004_gui/quill-index.md`（150 行，大总管批准加入白名单）
  - `docs/research/004_gui/sources.md` 的 `## gcomp` 一节（C1–C12）
  - `docs/experiments/exp010-ui-closure/`（README、closure.py、5 份输出）
  - `docs/reviews/043_gui-components-codex.md` + `docs/reviews/README.md` 索引一行
- 仓外：MAIN `resources/gpui-component-20261114/`、`resources/quill-20261114/`（大总管批准，只读、已忽略）；工作区 `_tmp/gcomp-review/`（codex 提示词、摘录、原始输出，不入库）
- 评审：atg-review 一轮（P1×6 全修）；codex 043 六轮，第 5、6 轮连续无 P0/P1，满足停止线；剩余 P2 已直接改，无 issue
- 要人定的事：H1（B19-10）、H2（B19-11）已定；无新的
- 没做：任何编译（exp010 只静态算闭包；乙′ 实验人定不做）

## 对后续代码手下的转交

详见 `docs/research/004_gui/findings-components.md`，下面几条写代码时最容易漏：

1. **按键追踪日志只记动作名、路由分支（组字 / 可打印 / 无字符 / 按键匹配）和修饰键，绝不记字符**：输入框里是手机号、验证码、二步密码和正文（`.42cog/real.md`「隐私」）。配一条测试：开追踪往输入框打假号码，断言日志里没有它（findings 第六节）
2. **`scripts/dev-sign.sh` 要扩展成能签 `.app` 目录**：现在 `:20` 用 `-f`，传目录直接退出。签包沿用同一身份和标识 `atg.dev`，不公证，否则钥匙串会弹框（findings 第七节）。系统通知只能在 `.app` 里用
3. **M3.4 要自己量切会话耗时**（切过去到第一条消息可读）：Quill 和上游都没有这个数字（findings 第八节）
4. 消息列表跨屏：逐项 `scroll_to_reveal_item` + overdraw，跳跃用 `scroll_to(ListOffset)`，目标进树再设 active descendant；不用 `.cached(..)`（findings 第五节，与 ggpui exp009 对齐）
5. 需要 KeyMonitor 绕输入法的只有无字符键（方向、Esc、PageUp/Down、Home/End、Shift+Tab，不带 Ctrl/Fn/Cmd）；Tab、回车、Cmd+键不组字时先走按键表（findings 第四节第 4 小节）
