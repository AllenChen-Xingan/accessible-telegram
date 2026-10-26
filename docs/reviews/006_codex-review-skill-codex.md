> 2026-10-25 · 对象：skill `codex-review`（装在 `~/xingan-chen/B-MyCreate/2-aartifacts/skills/codex-review/`）· 手下 codexskill 回报原文 · 剩余 P2 见 issue（上面那条）

# codexskill 回报 · codex-review skill（批次 b8 · 2026-10-25）

## 结论
skill 草稿完成，codex 自审 11 轮后满足人定停止线（第 10、11 轮均无 P0/P1）。待大总管：安装 + 落评审记录 + 开 1 个 P2 issue。

## skill 结构 `_tmp/20261025-b8/codex-review/`（共 1092 行）
- `SKILL.md`（94 行）：触发/不触发（用 codex 时优先于 cross-model-review；审查收尾、开 PR 前主动提议，项目规定必过时直接用）· 8 步流程 · 五类对象与 effort 表 · 停止线（P0/P1 清零，连续两轮无 P0/P1 即停，P2 开 issue；项目可更严）· 隐私红线 · 威胁模型（防误发与误判收敛，不防本机恶意构造）
- `references/templates/`：doc / research / contract(high, 先列攻击面) / diff(只追正确性与需求) / outbound + `_output.md`（`P1 #3｜文件:行号：问题 —— 为什么 —— 怎么改`、抽查引用、总评）+ `_round2.md`
- `references/record-format.md`（docs/reviews/NNN_<对象>-codex.md + 索引）· `references/codex-cli.md`（v0.163.0 参数核对）
- `scripts/codex-review.sh`（744 行，bash 3.2）：填模板 → 隐私扫描（密钥/api_id/手机号/聊天 JSON；只发 UTF-8/ASCII 纯文本；禁区路径、非文本、超大、读失败硬拦，--force 只放行内容误报）→ `codex exec -s read-only … - < prompt` → 解析计数 + 格式闸（退出码 6）+ stop-line 行。提示词只走 stdin。

## 真跑
- 004_tdlib-patch.md（doc，medium）：31s，P2×3，总评：修改；被审文件未动。输出 `_tmp/20261025-b8/trial/`
- 自审：两个对抗视角 28 条全改；codex 11 轮（high）：P1 7→2→2→3→2→0→1→2→1→0→0，每轮 66–135s，全部逐条核实（`_tmp/20261025-b8/selfreview/response-1..10.md`）
- 第 2 轮隐私扫描命中 codex 自编测试串，人经大总管确认后 --force（已记入 response-1.md）

## 待大总管做
1. 安装：拷到 `~/xingan-chen/B-MyCreate/2-aartifacts/skills/codex-review/` + `~/.claude/skills/` 软链接（拷贝时排除 dryrun/selfreview，它们不在 skill 目录内，无需处理）
2. 评审记录：按 record-format 落 `docs/reviews/005_codex-review-skill-codex.md`（材料：selfreview/ 各轮原文 + response-*.md），README 加一行
3. 开 issue（by:ai），正文草稿：
   > codex-review.sh：已有历史输出但缺 identity 文件时补建身份，会让旧轮次在之后的运行中被信任进停止线（第 11 轮 codex P2 #1，scripts/codex-review.sh:459）。建议记录身份验证起始轮次，此前历史不参与停止线。
4. `cross-model-review/SKILL.md` 已加一句指针（「## 流程（五步）」上方）；该目录不是 git 仓库，无版本

## 要人定（选项 + 建议）
1. **只读沙箱不防读**：codex 仍能读仓库内 `.env.local`。选项：a) 维持提示词禁区 + 扫描（现状）；b) 密钥文件移到仓库外（建议 b）
2. **隐私扫描松紧**：国内手机号与 + 号码全拦，测试快照/上游代码常误报需 --force。建议维持严格
3. **模型**：现用 codex 配置默认（本次为 gpt-6-astra）。是否在项目钉死？建议不钉，记录里写明模型即可
4. **脚本体量**：744 行，主要是隐私与停止线防护。若嫌重，可拆出 `privacy-scan.sh` 单独复用（建议后续块再说）

## 人拍板（2026-10-25）

| # | 事项 | 人选 |
|---|---|---|
| 1 | `.env.local` 是否移出仓库 | **留在仓库里**（与建议不同）：靠提示词禁区 + 发送前扫描；`.env.local` 等禁区路径仍硬拦，不可 `--force` |
| 2 | 隐私扫描松紧 | **松一点**（与建议不同）：减少误报，但密钥、`api_hash` 形状、禁区路径照拦 |
| 3 | 是否固定模型 | 不固定，记录里写明用的模型 |
| 4 | 脚本是否拆分 | **现在就拆**（与建议不同）：拆出可复用的 `privacy-scan.sh` |

### 放行授权（人定 2026-10-25）

- 测试脚本 `tests/run-tests.sh`（故意构造的假值）可 `--force-secrets` 附给 codex——人「同意」
- `--previous` 传入的 **codex 自己上一轮输出**里、经手下逐条确认是 codex 自造示例的命中：以后每轮都可放行，记录里列明——人选「以后每轮都可放行，列明即可」
- 其他文件的命中照旧要人确认；`src/tdlib-rs/.github` 里上游的公开测试凭据不放行，审 `src/` 时排除
