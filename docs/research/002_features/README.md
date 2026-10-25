# 002 · 我们具体实现哪些 Telegram 功能、按什么顺序

> 2026-10-25 起 · 手下 api 起草 · 四步走第二步：系统分析（第二轮）。**分析即取舍。**
> 上一轮 `../001_stack/` 定了技术栈（GPUI + TDLib）；架构见 `docs/plan/001_arch-stack.md`。这一轮回答「做什么、先做什么」。

## 这次要决定的

从 TDLib 能做的几百件事里，挑出 **MVP（能日常替代官方客户端读和回消息的最小集）**，再排出第二批、以后、不做；并把它们排成里程碑。

## 四份文件，按这个顺序读

| # | 文件 | 干什么 |
|---|---|---|
| 1 | [inventory.md](inventory.md) | **功能盘点**：按组列 Telegram 用户能做的事，每项带 TDLib 函数（`td_api.tl` 行号）· 读屏价值 · 难度 · 依赖；意图级动作已移交 infra（正本 `specs/contracts/actions.md`） |
| 2 | [content-types.md](content-types.md) | **内容类型清单**：消息内容 108 个子类、文本实体 24 种、系统消息、会话项字段——内容树要覆盖到哪（给 tree） |
| 3 | [decision.md](decision.md) | **取舍卡**：MVP / 第二批 / 以后 / 不做，对照 T01–T10 的缺口与 T11+ 提议，**待人拍板** |
| 4 | [`docs/plan/002_roadmap.md`](../../plan/002_roadmap.md) | **路线图**：里程碑 M0…Mn，每个含功能、先 CLI 后 GUI、验收、依赖、规模 |

## 依据

- TDLib 1.8.69 接口全集 `resources/td/td/generate/scheme/td_api.tl`（只 grep，不整读；提交 `ea3f69c`）
- 官方文档：core.telegram.org/tdlib/getting-started（会话列表、文件、更新的推荐用法）
- `.42cog/intent.md`「不做什么」· `specs/tasks.md` T01–T10 · `docs/plan/001_arch-stack.md` 第六、七节

## 什么时候算够

能列出「MVP 是哪些、谁排第一、哪里要人拍板」，就够了。函数没有一个个试，**行号都 grep 确认过，行为没跑过**——跑是第三步（原型）的事。
