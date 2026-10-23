# 无障碍 Telegram（macOS · VoiceOver 优先） — 认知模型

<meta>
  <document-id>atg-cog</document-id>
  <version>1.0.0</version>
  <project>无障碍 Telegram（macOS · VoiceOver 优先）</project>
  <type>Cognitive Model</type>
  <created>2026-10-23</created>
  <depends>intent.md, real.md</depends>
</meta>

## Document Purpose

这个系统里**有哪些东西**，谁产生它们，它们之间什么关系。

---

<cog>

## 核心实体

| 实体 | 是什么 | 存在哪 | 谁产生 |
|---|---|---|---|
| **工作单元：一项核心任务** | 一件 VoiceOver + 键盘下要能顺畅完成的日常操作（如「打开未读最多的会话并读最新 5 条」） | `specs/tasks.md` 一行 | 人提出，AI 补全步骤与判据 |
| **交付物：App** | 能双击打开、登录后能聊天的 macOS 客户端 | `src/` 编译产出（`_build/`） | AI 写代码，人验收 |
| **读法规约** | 每类消息 / 每个界面元素该怎么读（顺序、详略、正反例） | `specs/` | 人定标，AI 起草 |
| **真相源** | VoiceOver 实测记录、AccessKit / GPUI 源码、Telegram API 文档 | `vault/truth/` | `atg-research` 整理 |

### 作品区 `src/`：按 crate 分（清单以根 `Cargo.toml` workspace 为准）

> 子目录建出来那一刻在这里补一行。以下是计划中的划分，**还没建**。

| crate | 是什么 | 谁产生 | 谁是谁的输入 |
|---|---|---|---|
| `src/atg-core` | 能力层：Telegram 连接（封装 TDLib + tdlib-rs：登录、会话、消息、更新推送、本地缓存；见 `docs/research/001_stack/decision.md`）+ 读法（`speech` 模块：把一条消息 / 一个会话变成读屏要读的那句话，纯函数、可快照测试） | AI 写，读法按人定的规约定标 | 输出数据与读法字符串 → `atg-cli`、`atg-gui` |
| `src/atg-cli` | 入口一（先做）：命令行列会话、读消息（JSON 输出）、发消息（须人确认）。AI 的首选接口 | AI | 输入 `atg-core`；输出 → AI 代理 / 脚本 / 人 |
| `src/atg-gui` | 入口二（后做）：GPUI 界面，负责角色标注、焦点管理、键盘导航、播报；无障碍树同时服务 VoiceOver 与 computer use | AI | 输入 `atg-core`；输出 → 人 / computer use 代理 |

**为什么是 core + cli + gui**：沿用 `-cli` / `-core` 的 Rust 强约定（依据课程 ch04/ch05，本地笔记 `resources/aias-course-notes.md`，不入库），界面只是第二种作品形态。读法放进 `atg-core::speech`，命令行和界面读出来的是同一句话。

## 参与者

| | 负责 | 不负责 |
|---|---|---|
| **人（我）** | 定方向 · 提核心任务 · 开 VoiceOver 实测 · 定读法标准 · 最终拍板 | 不做能被规则判对错的检查 |
| **AI** | 调研 · 写代码 · 写测试 · 导出无障碍树自查 · 对抗性评审 | 不定方向、不定读法标准、不替我对外发消息 |

## 关键关系

- **命令行优先**：一项功能先在 `atg-cli` 做通，再接到 `atg-gui`；界面不许有命令行做不到的功能逻辑
- **一棵无障碍树，两类读者**：VoiceOver 用户和用 computer use 的 AI 读的是同一棵树。树做好了，两边同时受益；为一边改树，要验另一边
- **核心任务 → 一切**：`specs/tasks.md` 里的一项任务，决定这一轮动哪个 crate
- **读法规约定 `atg-core::speech` 的标准**：人改过的那一版读法，就是快照测试的期望值
- **`atg-core` 的数据模型一变，`atg-cli` 和 `atg-gui` 就得跟着变**
- **GPUI 上游一变，`atg-gui` 就得跟着验**：升级 GPUI 版本后必须重新导出无障碍树对照

</cog>
