# 评审 011 · #38 开发版改用 Apple Development 签名补审（codex · gpt-6-astra · read-only）

> 对象：PR #51 合并范围 `6a7f1ef..df6882b`（`scripts/dev-codesign-id.sh` · `dev-sign.sh` · `dev-sign-run.sh` · `setup-dev-codesign.sh`、`.cargo/config.toml`、`keystore.rs`、exp006 实验）。#32 补审第 5 块。**最新一轮：第 2 轮（2026-11-13）· 总评「修改」· 已达停止线（两轮无 P0/P1，P2 → #75）**。
> 累计处理：P2×2 逐条回当前源码核实，均属实，驳回 0。

**放行授权**：隐私扫描在 `docs/experiments/exp006-keychain-migrate/migread/src/main.rs:22` 报「密钥赋值」，实为钥匙串服务名字符串 `"atg.tdlib-db-key"`（条目名，非密钥值）。人定放行 `--force-secrets`（2026-11-13，经大总管转达）。本手下会话带该参数的运行被自动权限检查拦下（转达的授权不作本会话授权），第 1 轮由大总管经人授权代跑；第 2 轮同样由大总管代跑（`run-codesign-38-r2.sh`）。

## 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-b18/codex-work/codesign-38-codex-1.md`（不入库）。diff 类型，effort high。行号指当前文件。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：#1、#2 → #75

- **P2｜dev-codesign-id.sh:49 `.env.local` 里带引号的值后面有空白时被误拒**（第 1 轮 #1）
  - 核实：属实。`sed` 先去首尾引号、最后才删空白：`ATG_CODESIGN_ID="<40 位>"␠␠` 的右引号不在行尾而留下，删空白后剩 `<40 位>"`，报「不是 40 位十六进制」，签名与默认的 `cargo run` / `test` runner 一起失败。CRLF 行尾同理。
  - 处理：转 issue。建议先裁两端空白（含 `\r`）再去成对引号，补用例。
- **P2｜dev-codesign-id.sh:52 `security find-identity` 失败时无说明地退出**（第 1 轮 #2）
  - 核实：属实。`set -euo pipefail` 下 `ids="$(apple_dev_ids)"` 管道失败即按 `security` 的退出码直接退出，`2>/dev/null` 又吞了它的错误，不出脚本头部承诺的说明与退出码 2。
  - 处理：转 issue。建议显式捕获，区分「查询失败」与「查询成功但没有有效身份」，固定说明（不含身份名）退出 2。

### 引用抽查

- codex 列的「核实属实」：4 条（零 / 一 / 多候选规则、`CI` / `ATG_DEV_SIGN=0` 跳过签名且签名失败不运行、runner 只配 aarch64、exp006 读取器只读不写）；「无法核实」1 条（exp006 README:38 实测结果，需访问钥匙串）。无「不符」。

### 是否再审

本轮无 P0/P1，连续第 1 轮 → 再跑一轮确认（脚本交大总管跑）。

## 第 2 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-b18/codex-work/codesign-38-codex-2.md`（不入库）。带第 1 轮原文与处理记录（`resp-codesign-38-1.md`），大总管代跑（`run-codesign-38-r2.sh`，`--force-secrets` 同上授权）。effort high。
本轮 P0/P1：0 · 停止线：**满足**（第 1、2 轮均无 P0/P1）· 留作 issue 的 P2：#75

- 本轮两条 P2 即第 1 轮 #1、#2 重述（codex 注明「上一轮 #1 / #2，当前未修」），与处理记录一致，无新增；已在 #75。
- 引用抽查：核实属实 5（另核 `007_decisions-b15.md:41/43` 接受同用户进程静默取钥匙、缓做强化运行时），无法核实 1（exp006 README:38 实测，需访问钥匙串，留实验记录为准）。

是否再审：**停**。剩余 P2 在 #75。
