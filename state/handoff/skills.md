# 现状页 · skills（批次 b5）

- **手上**：读全部对话与工具记录 → 找反复出现的问题 → Opus 子代理用 skill-creator 起草跨项目 skills（只出草稿）
- **交代**：`_tmp/20261025-b5/skills-miner.md`
- **状态**：**完成，可以收尾**（2026-10-25）
- **做到哪**：
  1. ✅ 抽取脚本 `_tmp/20261025-b5/scripts/extract.py` + 脱敏复扫 `privscan.py`（20 项目 / 118 会话 / ~1,370 条人话）
  2. ✅ `_tmp/20261025-b5/patterns.md`：7 新建 · 2 扩展 · 8 不做
  3. ✅ 7 个 Opus 子代理起草 → `_tmp/20261025-b5/skills/<name>/`（141 文件，732 KB，均未安装）
  4. ✅ 审稿 `_tmp/20261025-b5/review-notes.md`；规则检查 `scripts/check-skill.py` 全过；隐私扫描全为占位
  5. ✅ `_tmp/20261025-b5/report.md`（装哪、待人拍板 9 条、未做）
- **待人拍板**：见 report 第四节（装哪些 · 登录接力严格度 · 改稿阈值 · 待定格式 · PDF 覆盖策略 · author 自动填 · 进度 skill 是否单独 · anren 扩展 · 密钥轮换）
- **转交大总管**：① 旧草稿 `2-aartifacts/skills/agent-coordinator/` 建议退役（未动）；② 装前 `coordinator-dispatch/scripts/spawn-worker.sh` 需真窗口验一次；③ 装前统一跑 skill-creator 的 quick_validate（需 PyYAML）
- **红线核对**：未改 `~/.claude/projects/`；未安装 skill；未碰 git；未跑 cargo；输出无凭据 / 手机号 / 邮箱 / 人名 / 聊天原文
