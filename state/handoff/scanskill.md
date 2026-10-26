# handoff · scanskill（批次 b10 · 2026-10-25）——已完成，待大总管覆盖安装

交代：`_tmp/20261025-b10/scanskill.md`。依据：`docs/reviews/006_codex-review-skill-codex.md` 末尾「人拍板」表。
**回报正文：`_tmp/20261025-b10/scanskill-report.md`**（改动清单、放松/收紧、评审轮次、剩余 P2、要人定的事）。

## 状态
- [x] 工作副本 `_tmp/20261025-b10/codex-review/`（安装位置未动）：拆出 `scripts/privacy-scan.sh`、放松误报、`--force-secrets`、issue #29（since=N）、SKILL.md 与 references 同步
- [x] 自测 `_tmp/20261025-b10/tests/run-tests.sh`：195/195（/bin/bash 3.2，codex 用桩）
- [x] 真仓库 specs/docs/state/scripts/.42cog/src（`--exclude .github`）扫描通过
- [x] 多视角自审 ×2 → codex 11 轮（全属实全修）→ codex 额度用完，按 CLAUDE.md 临时例外改为最终多视角自审 ×2 + 收尾核查，P0/P1 清零
- [ ] 待 codex 补审（11-01 恢复，issue #32）
- [ ] 剩余 P2 开 issue（清单在回报里）

## 人的授权（经大总管转达）
- `tests/run-tests.sh` 假值可 `--force-secrets`；`--previous` 里 codex 自造示例逐轮列明即可放行（每轮记在 `codexrounds/response-N.md`）
- 边界：扫描器只防常见写法，为绕过构造的排版最高 P2（已写进 SKILL.md 威胁模型）

## 记录位置
`_tmp/20261025-b10/selfreview/`（leak / regress / final-leak / final-regress / final-verify）· `_tmp/20261025-b10/codexrounds/`（scanskill-codex-1..11、response-1..11、focus.md）
