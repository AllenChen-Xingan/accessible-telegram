# 现状页 · gui（GUI 写代码线）

## b21 接手（2026-11-24）

- **手上**：M3.1 GUI 骨架 + 树导出（交代 MAIN `_tmp/20261124-b21/gui.md`）
- **做到哪**：计划已批（第五节 6 条全按建议定，2026-11-24）。PR-1 代码已写完并编过：`src/atg-gui-spec/`（aid · a11y · text · project · schema + `gui-schema` 程序 · 金样测试 · 投影快照测试）、`scripts/schema-stamp.py` + `scripts/tests/test_schema_stamp.py`、`.42cog/cog.md` 与 `src/README.md` 各加一行。PR-2 的 core 文件清单已交大总管转 arch（MAIN `_tmp/20261124-b21/plan-m31-pr2-core.md`）
- **名额**：PR-1 用完已归还（2026-11-24）
- **PR-1 状态**：提交 `be6524a` `4b48088` `9638c25`（#169）。atg-review 第 1 轮（P1×2：一条成立已修，一条降 P2）、第 2 轮（P1×1：schema-stamp.py 不在 GATE_FILES，已修 + 新自测 `test_gate_list.py`，改坏会红）。第 2 轮的修复**还没编译**，等名额（msg2 之后）→ 重出金样（名称来源写法改成 `text:<文案>+alert`）→ 请提交 → 第 3 轮 atg-review → codex 064
- **已开 issue**：#171（第二节标识表移交）· #172（speech 拼「文案 + 新消息提醒」的接口）· #174（ALL 数组编译期保证）
- **PR-1 待办 / 留意**：① 合并要 `baseline:loosen`（动了门槛目录 `scripts/tests`，大总管已记）②③ 已定并做完（放进 PR-1）；PR 说明要逐条列出改了 a11y-tree.md 的文件头、第三节、第四节

## 已定（大总管 2026-11-24 批计划时）

计划第五节 6 条全按建议：PR-2 归我（core 清单先过 arch）· 执行方选 A（`atg-cli` 加库目标，PR-4 前先交边界方案过 arch）· `atg-gui-spec` 含纯函数投影 · M3.1 只做夹具 + 测试服务器档案 · CI 先开 GUI 任务、PR-3 量时长 · 三处别人的文件准在对应 PR 里改、PR 说明逐条列。GPUI 钉 `15d136b`、不引 Zed `ui`。自审在编译绿、提交检查点之后再起。

## 要人定 / 要大总管定的事

- PR-2：arch 三条条件 + 「不碰钥匙串」三层证明已认可，见 MAIN `_tmp/20261124-b21/plan-m31-pr2-core.md` 第五、六节
