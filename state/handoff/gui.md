# 现状页 · gui（GUI 写代码线）

## b21 接手（2026-11-24）

- **手上**：M3.1 GUI 骨架 + 树导出（交代 MAIN `_tmp/20261124-b21/gui.md`）
- **做到哪**：计划已批（第五节 6 条全按建议定，2026-11-24）。PR-1 代码写完、**还没编译**：`src/atg-gui-spec/`（aid · a11y · text · project · schema + `gui-schema` 程序 · 金样测试 · 投影快照测试）、`scripts/schema-stamp.py` + `scripts/tests/test_schema_stamp.py`、`.42cog/cog.md` 与 `src/README.md` 各加一行。PR-2 的 core 文件清单已交大总管转 arch（MAIN `_tmp/20261124-b21/plan-m31-pr2-core.md`）
- **名额**：PR-1 用完已归还（2026-11-24）
- **PR-1 状态**：编译、clippy、test（19 过 0 败）、自测 90 过、四道门槛脚本全过；金样已盖章（6a3834991b1e76a7）。已请大总管提交检查点
- **下一步**：检查点提交后 → atg-review（范围按那个提交）→ codex 064 → PR 说明
- **PR-1 待办 / 留意**：① 合并要 `baseline:loosen`（动了门槛目录 `scripts/tests`，大总管已记）② `specs/gui/a11y-tree.md` 元素表那 8 行的正本已在 `atg_gui_spec::a11y::TABLE`，但 md 表还没改成生成物（012 第一节第 3 条）——PR-1 里做还是另排，回报时问 ③ GPUI 层导出格式规约里没有 `roledesc`（只 macOS 层有），投影里写了 `roledesc="消息"`，要不要进 GPUI 层格式，回报时问

## 已定（大总管 2026-11-24 批计划时）

计划第五节 6 条全按建议：PR-2 归我（core 清单先过 arch）· 执行方选 A（`atg-cli` 加库目标，PR-4 前先交边界方案过 arch）· `atg-gui-spec` 含纯函数投影 · M3.1 只做夹具 + 测试服务器档案 · CI 先开 GUI 任务、PR-3 量时长 · 三处别人的文件准在对应 PR 里改、PR 说明逐条列。GPUI 钉 `15d136b`、不引 Zed `ui`。自审在编译绿、提交检查点之后再起。

## 要人定 / 要大总管定的事

- （暂无新的；PR-1 待办 ②③ 回报时一起问）
- PR-2：arch 三条条件 + 「不碰钥匙串」三层证明已认可，见 MAIN `_tmp/20261124-b21/plan-m31-pr2-core.md` 第五、六节
