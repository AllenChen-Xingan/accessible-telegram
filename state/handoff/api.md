# 交接 · 手下 api（批次 b2 · 系统分析 002 功能清单 + 路线图）

## 手上
系统分析 002：功能盘点 · 对照 T01–T10 · MVP 切分 · 路线图 · 给 tree 的内容类型清单 · 给 infra 的意图级动作清单

## 做到哪
- [x] 读交代 / common / CLAUDE.md / .42cog / 001_arch-stack / tasks / workflow；已向大总管报到
- [x] content-types.md 初版 → 已发 tree（tree 回：五条提醒全采纳）
- [x] 意图级动作初版（inventory.md 附）→ 已发 infra
- [x] inventory.md（A/B/C 三组合并，抽查 15 处行号全对）
- [x] decision.md / README.md（待人拍板 7 条）
- [x] docs/plan/002_roadmap.md（M0–M6，到 MVP 29 块；待人拍板 3 条）+ plan/README 一行
- [x] 对抗自审 3 视角（核对员 12 条 · 盲人用户+老手 13 条 · 规矩+外行 12 条；P0×3 全修，P1 全修，P2 大部分修）

## 下一步
交付完成，等大总管收；人拍板 F1–F7（decision.md）、S0–S3（roadmap）后回填。未做：未接 codex 跨谱系评审（留给大总管决定）

## 流水
- 2026-10-25 报到；建 docs/research/002_features/
- content-types.md v0 写完发 tree（会话 -2e）；意图级动作写进 inventory.md 末尾发 infra（会话 -8f）
- 发现：real.md 把「标记已读」列为不可逆 → 待人拍板候选
- 派 3 个子代理盘点功能组，输出到 scratchpad/inv/{A,B,C}.md
- infra 回：动作名对齐，L0/L1/L2 = 否/轻/闸；send_text 拆 prepare+token；已读把关与其 A1 合并一条问人。已回告 5 个新增 MVP 动作
- 写完 decision.md、roadmap、plan/README 一行；派 3 视角评审
- 收 3 份评审，主要修改：MVP 补「阅读视图 / 复制全文 / 打开链接」「进会话落点 + 未读分隔（F2）」「GUI 附件打开」「话题群发送不发错」；已读把关并入契约 A1 并补 computer use 漏洞；A2/A5 改为指针不代填；编号改 F1–F7 / S0–S3；roadmap 附件读法挪进 M1、加日常环与 S0（第一刀从哪块算起）；删 roadmap 与 inventory 里的重复清单（唯一编码）；content-types 计数改实数 21/26、泛化改 grep 为准
- 已告 infra（A1 补充、mark_listened、message_links、A5 问法）与 tree（v0.1 变化）
- infra 确认四条已接；draft_set 被 infra 改为 L1、CLI 不开放 → decision.md 第二批「草稿」加注
- 大总管：先不收尾，codex 在审 decision.md + roadmap，结果回来交 api 修；等待中
- tree 确认 v0.1 已跟上：消息头 topic、text.rs links()、未读分隔规约预定 divider{unread|date}、按钮第二批；无需我改动
- codex 评审 003：P1×5 P2×1 全属实全采纳；有效内容/受限内容/有效静音进 MVP（已告 tree）；CLI 草稿与播放按命令行优先不破例（与 infra 商定）；新增 F8 声音放哪个模块；写 docs/reviews/003_features-codex.md + README 一行
