# 评审 004 · 基础契约 v0（codex · GPT 系 · read-only · 一级评审）

> 2026-10-25。对象：`specs/contracts/`（`actions.md` · `cli.md` · `errors.md` · `config-privacy.md` · `testing.md`）。原文：`_tmp/20261025-b2/codex-infra-out.md`（不入库）。总评「修改」。
> 处理：P0×1、P1×14、P2×2 由 infra 逐条回源码核实，**全部属实，全部采纳**，驳回 0。其中 2 条的改法里留了「未核」项（第 4、10 条）。
> 同一轮还落实了人追加的拍板第 11 条「直发模式」（`docs/plan/001_arch-stack.md` 第七节），不属于评审发现，见末段。

## 发现与处理

行号指评审时的契约版本。

- **P0｜actions.md:130 正式服务器收藏夹允许终端确认**：契约自己承认 tmux、伪终端能代按，AI 能无人在场完成真实发送；拍板 10 只给「测试服务器 + 收藏夹」免确认。
  - 核实：属实（逻辑；拍板 10 原文）
  - 处理：默认模式下终端确认不再开凭据；正式服务器一切目标（含收藏夹）要 Touch ID / 系统密码。`testing.md` 矩阵同步
- **P1｜actions.md:24 L1 把「GUI 按键触发」当成人确认**：computer use 一样能按键、打字。
  - 核实：属实（逻辑）
  - 处理：L1 副作用（已读、已听、在线、正在输入、草稿）在规则定下前一律默认关闭（`l1_disabled`），把关落在 `atg-core` 入口，不看界面来源；GUI 草稿默认只存本机
- **P1｜actions.md:91 打开链接 / 附件归为无闸 L0**：会向外站发请求，任意协议和文件处理器可能触发别的应用。
  - 核实：属实（逻辑）
  - 处理：新等级「L1 外部」；只许 `http(s)`；文件只许常见只读类型，可执行类改为「在访达中显示」；用系统接口打开、不经 shell；打开前展示并确认
- **P1｜actions.md:103 TDLib 持久排队与「不排队」冲突**：60 秒凭据管不住已交给 TDLib 的请求。
  - 核实：属实（TDLib 把未发出消息存库、恢复后续发）
  - 处理：写明「授权包含断网期间排队、恢复后发出」，并写进确认说明；本项目自己在连接未就绪时拒绝提交；MVP 补「待发查询 + 撤销 + 启动时列出」。⚠️ 未核：`deleteMessages` 能否取消未发出消息（`td_api.tl:13222-13226` 注释没写），能否抢在重启自动补发前撤销——列为 M2.1 实测项
- **P1｜errors.md:46 `retryable` 按种类生成，没有「结果未知」**：AI 按 true 重发会重复。
  - 核实：属实
  - 处理：新增 `send_outcome_unknown`（带临时 id，CLI 退出码 10）；L2 错误的 `retryable` 一律 false
- **P1｜actions.md:99 重发条件不全**：`need_drop_reply` 自动去掉回复、`retry_after`、逐条 null。
  - 核实：属实（`td_api.tl:3458-3465`、`:13188-13195`）
  - 处理：新增 `resend_prepare`，逐条检查并写明是否去掉回复；凭据绑重发后的实际内容；逐条返回；付费重发、需换发送者 / 换引用一律拒
- **P1｜actions.md:105 没有话题规则**：`topic_id` 是独立参数。
  - 核实：属实（`decision.md:25`、`002_roadmap.md` M2.2）
  - 处理：准备阶段解析最终话题（回复沿用被回复那条的；话题群里不回复的新消息拒发），进计划、进摘要、发送时校验；补用例
- **P1｜actions.md:89 `viewMessages`「按条标」低估范围**：TDLib 取最大 id 推进已读位置。
  - 核实：属实（`MessagesManager.cpp:16961` `read_dialog_inbox`）
  - 处理：`mark_read(chat, up_to)`，写明连带标记此前消息、说明里讲清范围。⚠️ `decision.md` F1 的「焦点经过哪条标哪条」依据不成立，F1 归 `api`，已报大总管
- **P1｜config-privacy.md:73 TDLib 日志级别 1 仍漏正文**。
  - 核实：属实（`MessageContent.cpp:7696` `LOG(ERROR)` 打印改动前后内容）
  - 处理：默认级别 0；只有测试档案可临时调高；「暗号」测试覆盖原生日志，并专门跑一次编辑消息
- **P1｜config-privacy.md:18 下载附件明文落盘**。
  - 核实：属实（`FileDownloader.cpp:274-281`）
  - 处理：写明库加密不覆盖附件；目录 `0700`、不自动下载、7 天清理、外部打开用 `0700` 临时副本且退出即删；剩余风险列待人拍板 P5
- **P1｜config-privacy.md:40 钥匙串没规定谁能读**。
  - 核实：属实
  - 处理：正式档案的钥匙访问控制只认正式发布签名；覆盖一切正式档案而不只是名为 `main` 的；加拒绝测试。⚠️ 用传统访问列表还是访问组，开工实测再定
- **P1｜config-privacy.md:122 建议测试档案不加密**，与拍板 8、roadmap M1.1 冲突。
  - 核实：属实
  - 处理：撤回该建议；测试档案照样加密，开发证书只许用于测试档案的钥匙
- **P1｜errors.md:50 「全大写」当作安全判断**。
  - 核实：属实（格式检查不是脱敏）
  - 处理：只放行登记过的码；限流只带解析出的秒数；406 在建附件前截断
- **P1｜actions.md:169 删除事件丢了 `is_permanent` / `from_cache`**。
  - 核实：属实（`td_api.tl:11523`）
  - 处理：事件带上两个字段；只有永久且非缓存移除才算撤回；补用例
- **P1｜actions.md:164 事件表不足以维护内容树**。
  - 核实：属实（`td_api.tl:11233,11239,11244,11257,11334,11367,11387,11489`）
  - 处理：补 `chat_read_outbox`、`message_interaction_changed`、`message_unread_reactions`、`message_content_opened`、`message_pinned_changed`、`chat_notification_changed`、`scope_notification_changed`；写明内容变化整体替换、有效静音由 store 合成
- **P2｜cli.md:24 子命令没覆盖 MVP**。
  - 核实：属实（对照 `002_roadmap.md`）
  - 处理：补 `auth email` / `auth email-code`、`resend`、`mute`、`download`、`links`、`open`、`pending`
- **P2｜errors.md:91 状态合成不完整**：`core=closing` + `net=ready` 会落成 online。
  - 核实：属实
  - 处理：改成先中先出的完整表，`online` 只在三条线都就绪时；新增 `stopping`；单测穷举全部组合。⚠️ `UserStatus` 定义在 `atg-core::tree`，`stopping` 要 `tree` 跟进（tree 窗口已关，已报大总管）

## 源码引用抽查

- codex 抽查 6 处，全部属实；它指出的三处「不符」（日志级别 1、重发只看 `can_retry`、已读只影响指定条目）就是上面三条 P1，已改

## 同轮：拍板第 11 条「直发模式」

写进 `actions.md` 第一节第 5 小节（正本）、`cli.md`（全局选项 `--unsafe-direct-send`）、`config-privacy.md`（`direct-send.log`）、`testing.md`（矩阵）。要点：CLI 与 GUI 同一个启动参数、只认命令行参数；所有会话的「发出」类 L2 免确认，到进程退出为止；测试档案仍只许发收藏夹；启动提示 + 每条只记 id 与类型的本地留痕。按人的要求没有加 Touch ID、白名单之类的门槛。原 A4（要不要放宽 Touch ID）被吸收、关闭；新增 D1（直发管不管改 / 删 / 回应）。

结论「修改」→ 已修。
