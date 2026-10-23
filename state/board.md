# 无障碍 Telegram（macOS · VoiceOver 优先） · 状态板（给 AI · 跨会话唯一接续点）

> 开工先读 `CLAUDE.md` + **`.42cog/` 四份**（`intent` · `real` · `cog` · `meta`）+ 本文件 + `state/memory/MEMORY.md`。
> **非轮规则：每轮有效工作必更新本文件**（倒序追加，新的在上，带日期与 commit hash）。
> 每完成一个可命名的逻辑单元存一次；破坏性操作之前也存一次。这是给你自己留的后路，不是给别人看的历史。

> ## 🛡️ 开发流程落地（2026-10-23）
> - 依据：课程 ch04/ch05（本地笔记 `resources/aias-course-notes.md`，付费课程内容，**不入库**）→ 规约 `specs/workflow.md`
> - 已落：PR / issue 模板与标签（`.github/`）· 分支保护 ruleset（`scripts/github-ruleset.json`，**待远程仓库建好后应用**）· 本地守门 hook（`.claude/hooks/guard-git.sh`，15 条用例全过）
> - 模块划分改为 `atg-core` / `atg-cli` / `atg-gui`（见 `.42cog/cog.md`）
> - 远程仓库：https://github.com/AllenChen-Xingan/accessible-telegram（公开；免费账号私有仓库不支持分支保护）

> ## 🧭 第三步配置落地（2026-10-23）
> - 已填：`.42cog/` 四份草稿 · `specs/tasks.md`（核心任务 T01–T10）· `scripts/check-tools.sh` Rust 段 · `rust-toolchain.toml` · 根 `Cargo.toml` workspace · `.github/workflows/ci.yml`
> - **待人确认**：① ~~收敛方向~~（已定：速度很快、AI 友好、无障碍友好；三把尺子待人过目） ② 有没有可模仿的已跑通项目 ③ 远程仓库地址（**待配**）④ 放权边界 ⑤ 使用者现实（VoiceOver / 键盘 / 缩放）
> - **待装**：rustup / cargo、Metal 工具链（`check-tools.sh` 报缺）
> - **下一步**：确认上面几条 → `atg-research`：实测 GPUI 无障碍接口在 VoiceOver 下的表现、选定 grammers 还是 TDLib

> ## 🌱 系统开仓（2026-10-23 第一轮）
> `aias-meta-init` 生成六组骨架：① `README.md` `CLAUDE.md` `.42cog/` `specs/` ② `vault/` `notes/` `resources/` ③ `skills/` `scripts/` `plugin.json` ④ `src/` ⑤ `state/` `docs/` ⑥ `_build/` `_tmp/` `_archive/`（忽略）。
> **收敛方向**（草稿，待人确认）：见 `.42cog/intent.md`——**那句话只有一份，别抄到这里来**。
> **下一步**：确认收敛方向 → `atg-research` 找依据、排真相源权重 → 回来改这一句。方向第一版粗是正常的，四步是循环。
