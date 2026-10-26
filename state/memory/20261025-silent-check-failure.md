---
name: silent-check-failure
description: 检查脚本里的 `|| true`、吞掉 stderr、对空输入照打「✓」会把检查自身的失败变成「没问题」；只管检查 / 扫描 / 守门脚本，不管普通构建脚本
type: feedback
---

**为什么**：2026-10-25 `scripts/check-no-debug.sh` 改写时，`xargs -0 LC_ALL=C awk` 实际静默失败，原有 `|| true` 把错误吞掉，脚本照样打 ✓——等于扫描被关掉（`docs/reviews/005_model-reducer-codex.md` 第十二轮）。

**怎么用**：
- 守门脚本里不写 `|| true`；工具本身出错（awk / grep 退出码 ≥2、找不到文件）一律让脚本失败
- 每个检查脚本运行时先跑自测：至少一条「必须报」、一条「必须不报」；自测不过脚本就失败
- 改完检查脚本，在注入一处真违规的副本上实扫一次，确认它会报、会以非零退出
