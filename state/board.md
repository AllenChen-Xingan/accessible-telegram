# 无障碍 Telegram（macOS · VoiceOver 优先） · 状态板（给 AI · 跨会话唯一接续点）

> 开工先读 `CLAUDE.md` + **`.42cog/` 四份**（`intent` · `real` · `cog` · `meta`）+ 本文件 + `state/memory/MEMORY.md`。
> **非轮规则：每轮有效工作必更新本文件**（倒序追加，新的在上，带日期与 commit hash）。
> 每完成一个可命名的逻辑单元存一次；破坏性操作之前也存一次。这是给你自己留的后路，不是给别人看的历史。

> ## 🔑 #17 atg-cli login 完成（2026-10-25）
> - login / logout / auth 分步；输错验证码读出原因（T01 判据）；新错误码 `auth_phone_not_for_profile`；契约补齐
> - **正式账号已登录**（档案 main，人给码、大总管代输）；测试服务器正确码被拒 → #35；登录计数 → #36
> - **下一步**：#18 `atg-cli chats`（列会话）

> ## 🔌 #16 接 TDLib 完成（2026-10-25）
> - 收线程 + 序号 → 单线程 reducer；关闭（TDLib 先关不卡死）；监督与故障广播；Backend 真 / 假两份；`.env.local`；本地库加密 + 钥匙串；开发证书 `atg-dev`（`scripts/setup-dev-codesign.sh`）
> - 测试服务器冒烟：wait_phone、正常关闭、凭据出现 0 次；运行需 `DYLD_LIBRARY_PATH="$LOCAL_TDLIB_PATH/lib"`
> - codex 额度用完（11-01 恢复）→ 人定「自审通过就合并，事后补审」，#32 记账
> - **下一步**：#17 `atg-cli login`

> ## 🧠 #15 model + reducer 完成（2026-10-25）
> - `atg-core`：model · store（`store/table.rs` 唯一写入口，受限原文不留）· 投影 · 类型级脱敏 · 缓存上限（非当前会话 50 条）· 撤回占位跨重拉保留；115 测试含暗号穷举
> - codex 十三轮收敛；#28 一并修掉；扫描脚本剩余 P2 → #30；人定 → `docs/plan/006_decisions-b7.md`
> - 新规则：每次审查都过 codex，停止线 = P0/P1 清零 + 连续两轮无 P0/P1（`CLAUDE.md`）；skill `codex-review` 已装
> - **下一步**：#16 接 TDLib（交代已备 `_tmp/20261025-b9/tdlib.md`）

> ## 📁 #25 所有代码放 src/（2026-10-25）
> - `src/tdlib-rs`（subtree，未改一行库代码）· `src/tdlib-build/build.sh`（按 `ea3f69c` 拉取编译 TDLib 1.8.69，181 s）· 根工作区 `src/atg-*` + exclude tdlib-rs · `atg-core` 特性 `tdlib-api` / `tdlib`
> - **下一步**：#15 第 2 块 model + reducer

> ## 🦴 #14 骨架完成（2026-10-25）
> - `src/atg-cli` 骨架（子命令占位，输出契约格式 `not_implemented`）；`atg-core` 模块占位；禁打印两层（crate 根 deny + `scripts/check-no-debug.sh`，CI 已接）；45 测试
> - 人定：所有代码放 `src/`（`CLAUDE.md`）→ issue #25（tdlib-rs 搬进 `src/tdlib-rs`，TDLib 用脚本拉取）
> - **下一步**：#25 → #15 第 2 块 model + reducer

> ## 🔧 issue #3 补丁完成（2026-10-25）
> - fork `AllenChen-Xingan/tdlib-rs` 分支 `atg/tdlib-1.8.69` @ `4521d6d`：TDLib 1.8.69、生成器递归修补、日志与崩溃不带内容、`execute()`；45 单测；测试服务器连到 waitPhoneNumber，日志里凭据 0 次
> - `.env.local` 两值齐（apikey 手下 + 人）；issue #4–#22 已开；拍板 N1–N8 与补丁 ①–③ → `docs/plan/005_decisions-b3.md`
> - **下一步**：#14 建骨架（第一刀第 1 块）设为 `status:active`，派手下

> ## ✅ PR #2 合并（2026-10-25，人明确同意，squash → `c9db792`）
> - 本地改在 `chore/3-tdlib-patch`（从新 `main` 拉）；旧分支本地与远程都已删
> - sync 的文档改动将来单独开 PR，与 issue #3 补丁分开提交

> ## ✅ PR #1 合并（2026-10-25，人在对话里明确同意，`ATG_CONFIRMED=1` 放行，squash → `5c9031a`）
> - PR #2 改 base 为 `main`；squash 造成的假冲突用 ours 合并记录消除（`cc79d59`，文件内容不变）
> - 分支 `docs/research-001-stack` 本地与远程都已删除（远程已被 GitHub 合并后自动删除）

> ## 📦 批次 b2 完成（2026-10-25 · 分支 `docs/research-002-features`，未提交）
> - 交付：系统分析 002 功能清单 + 路线图（api）· 内容树规约 + `src/atg-core` 原型 35 测试（tree）· 基础契约 v0 五份（infra）· codex 评审 003 / 004 全修
> - 人拍板：001 第 11 条直发模式；b2 共 30 余条 → `docs/plan/003_decisions-b2.md`
> - **S0 = 全部等补丁**：`atg-core` 原型暂停；下一件事 = tdlib-rs 补丁（编 TDLib 1.8.69，要 issue + 重编译名额）
> - **待办**：把 003 的拍板同步进契约 / 内容树 / 快照 / 路线图（纯文档，不算第一刀）；F7 各条与第一刀各块逐个开 issue（意图由人写）

> ## 🗺️ 路线（2026-10-25 · 已推 PR #1：6939dd0 fc35dc3 6013bd8）
> - **人**：① 合并 PR #1 ② 写 issue（意图一句话）：tdlib-rs 补丁 1.8.69；第一刀第 1、2 块 ③ 有空时试听 exp003b「第三次试用」、exp004 11 步
> - **可并行（不依赖 TDLib 版本）**：第 1 块 crate 骨架 → 第 2 块 model + reducer（假事件单测）；第 6 块 speech v0 + 快照；第 7 块读法规约（AI 起草，人改）
> - **串行（拍板 5：等 1.8.69）**：tdlib-rs 补丁（重编译名额）→ 第 3 块接 TDLib → 第 4 块 login → 第 5 块 chats → 第 8 块 history
> - 拆法见 `docs/plan/001_arch-stack.md` 第六节

> ## 🏗️ 拍板 3 改定 + 架构方案初稿（2026-10-25）
> - 人改拍板 3：专门消息列表，Tab 或 VO+←/→ 找到，进入后 ↑/↓ 浏览（`decision.md`）
> - exp003b-v2 自测两轮各 45 步 0 失败；exp004 补跑 7/7——两者都剩人试听（`state/coord.md` 待定清单）
> - 架构方案 `docs/plan/001_arch-stack.md`：自审 + codex 评审（`docs/reviews/002_arch-codex.md`）全修；第七节 10 条待人拍板
> - **下一步**：人试听 → 定 10 条 → 大总管提交进 PR #1 → 人合并 → 第三步原型（issue 由人写）

> ## 📌 规则入库（2026-10-25）
> - 「大总管派活 · tmux 起手下」写进 `CLAUDE.md` 第〇节，作头号规则（原只在 AI 私有记忆里）
> - 现状未变：exp003b / exp004 仍等人说「跑吧」；`aiw` 里两个窗口还开着

> ## 🔬 系统分析 001 · 实验阶段（2026-10-23 下午）
> - 阶段：四步走第二步「系统分析」收尾——命门 / 取材 / 双谱系评审 / 放行检查已过；拍板 4 条已定；实验 5 个（001–004 + 003b）
> - **结论**：Telegram 层 = TDLib + tdlib-rs（exp002 坐实，TDLib 段 <1%）；GPUI 播报出口可用（exp001），但**礼貌式也会插进朗读**（人试听）→ exp004 推荐「提示音 + 写进名称 + 估算后汇总 + 按 m 收听」，后台提醒走直发；列表虚拟化可跨屏（exp003 树层），但**真实方向键无反应、使用者习惯 VO+←/→、不读位置**（人试听）→ exp003b 在修
> - 速度（debug 上界）：新消息→可读 21–48 ms；冷启动→可读 284 ms
> - **卡点**：两个弹窗测试（exp003b ~5 分钟、exp004 ~80 秒）等人说「跑吧」；输入法是否吞方向键待对照
> - **下一步**：跑弹窗测试 → 人试听 exp003b / exp004 → 改拍板 3（应用方向键 → VO+←/→ 为主）→ 写 `docs/plan/001_arch-stack.md` → 合并 PR #1 → 第三步「系统原型」（`atg-cli` 登录 + 列会话 + 读法定标）
> - 详细：`state/coord.md`

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
