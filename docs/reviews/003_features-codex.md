# 评审 003 · 功能清单与路线图（codex · GPT 系 · read-only）

> 2026-10-25。对象：`docs/research/002_features/decision.md`、`docs/plan/002_roadmap.md`（连带 `inventory.md`、`content-types.md`）。原文：`_tmp/20261025-b2/codex-api-out.md`（不入库）。总评「修改」。
> 处理：P1×5、P2×1 由 api 逐条回 `td_api.tl`（TDLib 1.8.69）与 `.42cog/intent.md` 核实，**全部属实，全部采纳**，驳回 0；另按「行号抽查」改准一处措辞。新出一件要人定的事（声音放哪个模块）列为 `decision.md` F8。

## 发现与处理

行号指评审时的文件版本；`tl:` 指 `resources/td/td/generate/scheme/td_api.tl`。

- **P1｜content-types.md:31 把 `ephemeral_content` 当可忽略字段**：字段说明要求存在时用它替代普通内容。
  - 核实：属实（`tl:3542` 类型说明、`tl:3594` 字段说明 "must be shown instead of the regular content"；类型 `tl:3547`，更新 `updateMessageEphemeralContent` `tl:11223`）
  - 处理：content-types 第一节单列一行，批次改 M——建树前先选有效内容；更新列表加 `tl:11223`；roadmap M0.2 写进 reducer。已告 tree
- **P1｜content-types.md:31 忽略 `restriction_info`**：字段说明要求必须应用于内容。
  - 核实：属实（`tl:3591` "must be applied to the message content"；`restrictionInfo` `tl:2738` 带 `restriction_reason`、`has_sensitive_content`）
  - 处理：单列一行，批次改 M——有限制时内容换成「受限」节点并读出原因；decision「读消息」与 roadmap M0.2 同步。已告 tree
- **P1｜decision.md:82 自建通知，却把全局通知设置整体留到第二批**：`use_default_mute_for` 为真时要继承范围或论坛的设置，只看会话自身会误报静音。
  - 核实：属实（`tl:3795`；`chatNotificationSettings` `tl:3811`、`scopeNotificationSettings` `tl:3823`、`getScopeNotificationSettings` `tl:14672`、`updateScopeNotificationSettings` `tl:11489`）
  - 处理：「有效静音」进 MVP（读取与计算；编辑界面仍在第二批）：decision「提示」行、inventory 全局通知设置行（批次改「MVP 读 / 二 编辑」）、content-types 会话项 `notification_settings` 行、roadmap M1.7 与 M2.3 的 `notify` 策略。已告 tree
- **P1｜roadmap.md:71「CLI 不必能播」，GUI 却有播放控制**：违反 `intent.md:16`「每项功能先在 atg-cli 里做通」。
  - 核实：属实
  - 处理：与 infra 商定，不破例。roadmap M2.6 改为 `atg-cli play`：stdin 收控制命令，stdout 逐行出播放状态，默认不标已听；外壳写进 `specs/contracts/cli.md`（infra）。infra 指出「音频出口放 `atg-core`」与 `docs/plan/001_arch-stack.md:51`（`atg-core` 不管任何发声动作）冲突——改架构边界要人定，列为 `decision.md` **F8**（A 新建 `atg-audio` / B 放 `atg-core` 并改 001 / C 各写一份），建议 A
- **P1｜decision.md:32 草稿「CLI 不开放」**：命令行优先没有排除 L1 或第二批的动作。
  - 核实：属实
  - 处理：与 infra 商定，不破例。CLI 提供 `draft set / clear`，因为草稿会同步到人手机的输入框、等于借人手发，按发送同级把关（同一张确认凭据；清草稿也过闸）；正本在 `specs/contracts/actions.md`、`cli.md`。decision 第二批「草稿」一项改成指针
- **P2｜inventory.md:18 登录时密码找回引用了 `requestPasswordRecovery`**：等密码这一步应走认证专用的找回链路。
  - 核实：属实（`requestPasswordRecovery` `tl:12361` 是登录后用的；等密码阶段用 `requestAuthenticationPasswordRecovery` `tl:12264` → `checkAuthenticationPasswordRecoveryCode` `tl:12268` → `recoverAuthenticationPassword` `tl:12274`，三者说明都写着 "Works only when the current authorization state is authorizationStateWaitPassword"）
  - 处理：inventory 第 ① 组改成这三步

## 行号抽查

- **属实**：codex 核了 `tl:110–122`、`3598`（含 `3551`）、`11208`、`11214`、`12502`、`12507`、`13143`、`14199`、`14205`、`15001`、`11921`，全部吻合
- **措辞不准（已改）**：inventory「不调 openChat，已读不生效」过于绝对。`tl:14198`：`viewMessages` 带 `force_read=true` 时会话关着也能标已读；`openChat` 的作用是让 TDLib 知道会话在看（`tl:14186`：超级群和频道只有打开的会话才收全更新）。inventory 第 ② 组、第 ⑥ 组与「注意 A-3」三处改准；infra 契约里本就没写「必须 openChat 才能标已读」，无需改

## 待人拍板的变化

- 新增 `decision.md` **F8**（声音放哪个模块）
- 其余 F1–F7、roadmap S0–S3 不变

结论「修改」→ 已修。
