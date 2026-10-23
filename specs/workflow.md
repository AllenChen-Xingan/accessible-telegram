# 开发流程规约：issue · 分支 · PR · CI · 合并

> 依据：开智「AI 自主系统训练营」ch04/ch05（本地笔记 `resources/aias-course-notes.md`，不入库）。这里只写**规矩本身**，理由去那份文档看。
> 能用机器拦的都已经交给机器：分支保护（GitHub ruleset）、CI（`.github/workflows/ci.yml`）、本地 hook（`.claude/hooks/guard-git.sh`）。

## 1. 一条主干

- 唯一长期分支：`main`。`main` 上的每一个提交都能编译、测试全过
- **禁止直接推送 `main`，禁止强推，禁止删除 `main`**
- 合并方式只用 **squash**，保持线性历史；一个 PR 在 `main` 上就是一个提交

## 2. issue：状态持久化的唯一地方

- **一件事一个 issue。** 想到的、发现的、要修的，先进 issue，再开工
- **意图类 issue 由人写**（功能、产品方向、读法标准）。AI 不代写、不扩写、不润色人写的 issue
- AI 运行中发现的问题，可以由 AI 开 issue，**必须打 `by:ai` 标签**，且只写事实（现象、复现步骤、位置），不写「应该怎么做」
- **同一时间只推进一个活跃 issue**（标签 `status:active`）。并行用 worktree 时，每个 worktree 对应一个活跃 issue，总数不超过当前磁盘承受得了的数量
- 估计超过 3 天的 issue：先拆子 issue，不无人值守
- 同一类问题出现第三次：停下来，把它沉淀成规则（`CLAUDE.md` 第五节或 `state/memory/`），再继续

### 标签（清单以 `.github/labels.yml` 为准）

| 组 | 标签 | 用途 |
|---|---|---|
| 类型 | `type:bug` · `type:feature` · `type:a11y` · `type:perf` · `type:agent` · `type:chore` · `type:docs` | 对应收敛方向三把尺子：`a11y` 无障碍、`perf` 速度、`agent` AI 友好 |
| 优先级 | `P0` · `P1` · `P2` · `P3` | P0：无法使用 / 安全问题；P3：可有可无 |
| 状态 | `status:active` · `status:blocked` · `status:needs-decision` | `needs-decision` = 等人拍板（见下方「确定性分流」） |
| 来源 | `by:ai` | AI 提的 issue |
| 任务 | `task:T01` … `task:T10` | 对应 `specs/tasks.md` 的核心任务 |

## 3. 分支

- 从最新的 `main` 拉：`git switch main && git pull && git switch -c <type>/<issue号>-<短名>`
- 命名：`feat/12-chat-list`、`fix/31-focus-jump`、`a11y/40-announce-new-msg`、`chore/…`、`docs/…`
- 一个分支只对应一个 issue；合并后**立刻删除**本地与远程分支

## 4. 开工前：确定性分流

| 情形 | AI 怎么做 |
|---|---|
| 根因明确、范围清楚 | 在 issue 里留一句计划，直接开做 |
| 多根因 / 需要设计 / 碰到产品方向或读法标准 | 列出关键决策，打 `status:needs-decision`，**停下等人** |
| 可能已部分实现 | 先核对现状，只补差量 |
| 值不值得做存疑 | 在 issue 里给「做 / 不做 / 关闭」结论和理由，等人拍板 |
| 估计超过 3 天 | 建议拆分，不开工 |

## 5. PR

- **一个 PR 只做一件事**，标题写成提交信息格式（`feat: …` / `fix: …`），正文必须有 `Closes #<issue号>`
- 按 `.github/pull_request_template.md` 填：改了什么 · 怎么验证的（贴命令和输出）· 三把尺子的影响 · 无障碍树有没有变
- **能用证据就不用断言**：贴测试输出、无障碍树导出的差异、耗时数字，而不是写「已测试」
- 改到界面（`atg-gui`）的 PR：必须附无障碍树导出对比；涉及读法的：必须附读法快照的差异
- 同时打开的 PR 不超过 3 个

## 6. 评审

两条独立的线，都过才合：

1. **CI**（机器，下限）：`cargo fmt --check` · `cargo clippy -- -D warnings` · `cargo test`
2. **评审**（上限），按风险分级：

| 级别 | 什么改动 | 评审要求 |
|---|---|---|
| 一级（从严） | 登录 / 会话文件 / 密钥处理、发消息等对外动作、依赖升级（尤其 GPUI、grammers）、CI 与分支保护配置 | 双谱系评审（另一家 CLI + 另一家模型，如 `codex exec`）+ 人看过 |
| 二级 | 一般功能、读法规则、焦点与键盘逻辑 | 双谱系评审 |
| 三级（从简） | 文案、样式、文档、注释 | 本地多视角自检（`/atg-review`）即可 |

- 评审意见只追**影响正确性或违反需求**的；风格偏好不追，防止无限循环
- 先用便宜模型修完能修的，终审再交给贵模型

## 7. 合并：人点

- **合并由人来点。** AI 可以建 PR、推分支、等 CI，但不执行 `gh pr merge`——本地 hook 会拦
- 人确认后，AI 收尾一句话流程：切回 `main` → `git pull` → 删本地与远程分支 → 关 issue（`Closes` 已自动关的就核对一下）→ 更新 `state/board.md`

## 8. 分支保护（GitHub ruleset）

远程仓库建好后执行（需要仓库管理员权限）：

```bash
gh api repos/<owner>/<repo>/rulesets --method POST --input scripts/github-ruleset.json
```

ruleset 内容（`scripts/github-ruleset.json`）：

| 规则 | 设置 | 为什么 |
|---|---|---|
| 作用分支 | 默认分支（`main`） | |
| 必须通过 PR | 开；**需要的批准数 = 0** | 个人仓库，作者不能批准自己的 PR；批准数设 1 会把自己锁死。审查靠双谱系评审 + 人点合并 |
| 必须通过状态检查 | `check`（CI 里的任务名）；严格模式（分支须与 `main` 同步） | 防止「各自通过、合在一起坏掉」 |
| 必须解决所有评论 | 开 | 评审意见不被跳过 |
| 只允许 squash 合并 | 开 | 线性历史 |
| 禁止强推 / 禁止删除 | 开 | |
| 绕过名单 | 空 | 包括自己在内都走 PR；真的紧急时临时在网页上关掉，事后记进 `docs/` 的 fix 文档 |

## 9. 令牌与密钥

- 给 AI 的 GitHub 令牌用**细粒度令牌**，只授权本仓库；权限：Contents、Pull requests、Issues 读写，Metadata 只读；**有效期 90 天**，到期轮换
- Telegram `api_id` / `api_hash`、会话文件永远不进仓库、不进 issue、不进 PR 描述、不进 CI 日志
- CI 不需要也不允许连真实 Telegram 账号；需要联网的测试用 Telegram 测试服务器，且默认不在 CI 里跑

## 10. 决策文档

放 `docs/plan/`，按流水编号，文件名带类型前缀：

| 类型 | 文件名 | 写什么 |
|---|---|---|
| arch | `NNN_arch-<短名>.md` | 选型与权衡：为什么选 X 不选 Y、数据模型；结论精确到参考仓库的文件和行号 |
| ops | `NNN_ops-<短名>.md` | 构建、打包、签名、配置、踩过的坑 |
| fix | `NNN_fix-<短名>.md` | 值得复盘的故障：时间线、根因、修复、预防 |
