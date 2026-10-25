# 现状页 · infra（批次 b2）

> 交代：`_tmp/20261025-b2/infra.md` · 共同规矩：`_tmp/20261025-b2/common.md`

## 手上
树以外的基础契约 v0：`specs/contracts/` 下 actions · cli · errors · config-privacy · testing 五份。**已完成，自审已修**。

## 做到哪
- 五份已写完，三视角对抗自审（核对员 6 条 · 一致性 + AI 使用者 34 条 · 安全攻击者 23 条）已逐条处理
- 已与 `tree` 对齐：外壳（单一版本号 `v`、`type` 判别、消息身份 `{chat_id,message_id}` 一个对象、int53 用数字 / 更大的写字符串、RFC3339 UTC、读法在外壳 `speech`）；正式类型名 `ChatItem` / `Message` / `UserRef` / `MsgId`；`Status{UserStatus}` 节点，合成函数归 infra
- 已与 `api` 对齐：动作名「名词在前」；L0/L1/L2 = 否/轻/闸；追加动作全部入表；已读把关只在 `decision.md` F1 问；代理 A5 正本在 `actions.md`；音频放哪在 `decision.md` F8；`play` 批次随 F3
- 核实：TDLib 钥匙不对回 401（`TdDb.cpp:184`）→ 单列 `db_key_invalid`；锁文件冲突 → `profile_busy`；测试号规则（core.telegram.org/api/auth）

## 自审后的大改动（给接手的人）
1. **发送确认默认最严**：`/dev/tty` 挡不住 `tmux send-keys` 和 `script` 伪终端，所以终端确认只用于「正式服务器 + 收藏夹」；发给别人 CLI 和 GUI 一律 Touch ID（`actions.md` 第一节第 4 小节，A4 问要不要放宽）
2. **写明闸的威胁边界**：防不了会写代码的 AI → 建议开发版只开 `test` 档案（`config-privacy.md` P4）
3. 测试服务器上只许发收藏夹（照架构方案，矩阵已改）；`test_exempt` 三重判定且每次发送前重查
4. L2 不自动重试、不排队；`flood_until` 写盘跨进程生效；新增 `account_limited`（`PEER_FLOOD` 等）
5. CLI：`--text` 撞名 → 全局改 `--format`；加 `me`、`saved` 别名、分步 `auth`、`next_cursor`、`send` 等成败才结尾；Ctrl+C / `usage` / `interrupted` 理顺
6. 草稿 `draft_set/clear` 按 L2 同级把关、CLI 开放（与 api 商定）；`play` 外壳已写

## 待人拍板（汇总；选项与代价在原文）

> ✅ **已拍板（2026-10-25）**：结果见 `docs/plan/003_decisions-b2.md`。下面的题面保留作依据。
| # | 事项 | 建议 | 位置 |
|---|---|---|---|
| A1 | 标记已读 / 已听 | 见 `decision.md` F1（api 正本） | — |
| A2 | 在线状态与「正在输入」 | CLI 永不设在线；GUI 仅前台；正在输入第一阶段不做 | `actions.md` |
| A3 | 「下一个有未读」到底 | 停住并说没有了 | `actions.md` |
| ~~A4~~ | ~~放宽 Touch ID~~ —— 被拍板 11（直发模式）吸收，关闭 | — | `actions.md` |
| D1 | 直发模式管不管改 / 删 / 回应 | 只管「发出」 | `actions.md` |
| A5 | 代理进不进 MVP（问：开 Telegram 要不要先开梯子？全局还是系统代理？） | 视回答 | `actions.md` |
| C1 | CLI 默认输出 JSON 还是文字 | JSON | `cli.md` |
| C2 | 命令名 `atg-cli` / `atg` | 先 `atg-cli` | `cli.md` |
| E1 | 状态播报防抖时长 | 2 秒 | `errors.md` |
| E2 | 「已连接」要不要说 | 只在播过「断开」后说 | `errors.md` |
| P1 | GUI 开着时 AI 用 CLI | 第一阶段互斥，GUI 阶段做本机接口 | `config-privacy.md` |
| P2 | 开发期钥匙串反复弹框 | 开发证书只用于测试档案的钥匙（测试档案照样加密） | `config-privacy.md` |
| P3 | 日志留不留 id | 留，分享前打码 | `config-privacy.md` |
| P4 | 正式账号谁能打开 | 只有正式签名的安装版；开发版只开 `test` | `config-privacy.md` |
| P5 | 下载的附件明文落盘，接受吗 | 接受（0700 + 不自动下载 + 7 天清理） | `config-privacy.md` |
| T1 | 测试服务器账号 | 固定一个号，被清空自动重注册 | `testing.md` |
| T2 | 树导出谁来跑 | 每个改界面的 PR 自己跑 | `testing.md` |

## 待写（契约里提到、还不存在的脚本）
`scripts/check-fixtures.py` · `scripts/check-a11y-tree.py` · `scripts/check-td-boundary` · `scripts/redact-log`

## 下一步
- 大总管：`specs/README.md` 子目录表加一行 `contracts/`（我没改）；`actions.md` 第一节第 4 小节与 `config-privacy.md` 属一级评审（另一家模型 + 人本人看）
- codex 一级评审 17 条已全部核实采纳，记录 `docs/reviews/004_contracts-codex.md`；拍板 11 直发模式已落实（`actions.md` 第一节第 5 小节）
- 要 tree 跟进：`UserStatus` 加 `stopping`；撤回占位只认永久且非缓存删除（tree 窗口已关，交大总管）
- 要 api 跟进：`decision.md` F1「焦点经过哪条标哪条」依据不成立（`viewMessages` 推进已读位置）
- 本窗口可收尾

## 流水
- 2026-10-25 报到；读完背景
- 2026-10-25 问 tree 外壳 → 同意并补三处；api 发 MVP 动作 → 对齐；写完五份初稿
- 2026-10-25 三视角自审（报告在本会话 scratchpad，未入库）；按报告修完；与 tree 定正式类型名；与 api 定草稿、播放、F3/F8
- 2026-10-25 大总管：README 已加 contracts/，提交由大总管做；等 codex 一级评审结果
- 2026-10-25 codex 一级评审 17 条全采纳并修；落实拍板 11 直发模式（`--unsafe-direct-send`）；写 docs/reviews/004
