# 无障碍 Telegram（macOS · VoiceOver 优先） — 现实约束

<meta>
  <document-id>atg-real</document-id>
  <version>1.0.0</version>
  <project>无障碍 Telegram（macOS · VoiceOver 优先）</project>
  <type>Reality Constraints</type>
  <created>2026-10-23</created>
</meta>

## Document Purpose

这个系统**改不了的那些条件**。

决策前置化的载体：**开工前把需要你拍板的现实一次性定死，干活中不再反复问你。**
这里写的是**现实**（就是这样），不是**规矩**（应该这样）——规矩在 `CLAUDE.md`。

---

<real>

## 日程

- 截止日期 / 交付节奏：无外部截止；个人学习 + 自用项目（待确认节奏）
- 一次交付的形态是什么：一个可运行的 macOS App（开发期 `cargo run`，之后打成 `.app`）
- 谁在等这个东西：我自己

## 底线红线

- **账号安全**：`api_id` / `api_hash`、登录验证码、二步验证密码、会话文件（session）等同于账号本身，泄露就是被盗号。一律只放 `.env.local` 和仓外目录，永不入库、永不发给任何模型以外的服务。
- **不替我对外发消息**：开发和测试时，向真实联系人发送、转发、删除、标记已读都是不可逆动作。自动化测试只能发到「收藏夹（Saved Messages）」或 Telegram 测试服务器（test DC）。
- **账号封禁风险**：Telegram 对第三方客户端的异常行为（高频请求、批量操作）会限流甚至封号；必须遵守 FLOOD_WAIT，必须用自己申请的 `api_id`。
- **隐私**：聊天内容、联系人、手机号属于个人隐私，不进仓库、不进日志、不进截图（调试截图要先打码）。
  - **人定例外（2026-10-25）**：图片可发给外部视觉模型生成画面描述（F5，见 `docs/plan/003_decisions-b2.md`）；细则开该 issue 时定。
- 必须保住的最低标准（＝验证红线）：见 `intent.md` 的「作品区」小节

## 资源现实

- 模型 / 账号 / 额度：Claude Code（Opus 5.5）为主；本机另有 `codex`，可做第二谱系评审
- 硬件 / 网络：一台 Mac（macOS 27）。Rust 1.99.1 与 Metal 工具链已装（2026-10-23）。Rust + GPUI 编译很吃磁盘和 CPU，一个 `target/` 目录几十 GB 很常见，开多个 worktree 前先看剩余空间
- 人手：我一个人 + AI（投入时间待确认）

## 使用者现实

- 使用者：我自己。在 Mac 上用 Telegram 时依赖 macOS 无障碍功能（具体是 VoiceOver / 键盘操作 / 缩放 / 其他，**待我确认**；线索：Claude Code 设置里开着读屏模式 Screen Reader Mode）
- 现有工具：现有的 Telegram macOS 客户端都不太好用，主要卡在无障碍
- 一次能记住几件事：快捷键要少而一致；同一个操作在所有界面用同一个键
- 使用环境：macOS，日常联网；需要考虑断网 / 重连时读屏能告诉我状态
- 技术栈现实：GPUI 的 AccessKit 支持 2026-05 才合并进主干（zed PR #56065），还在快速演进，大概率得直接从 git 引用 GPUI，接口可能变；`aria_keyshortcuts` 目前还没传给读屏

</real>

---

**任何一条变了，回来改。** 现实变了而这份没变，后面所有判断都是按旧世界做的。
