# 交接 · srcmove（issue #25：所有代码放 src/）

> 2026-10-25 · 交代 `_tmp/20261025-b6/srcmove.md` · 分支 `chore/25-src-tdlib` · 没做任何 git 写操作 · 名额已还

## 状态：已收尾（做完，两视角自审已修，复验全绿）

## 改了哪些文件

| 文件 | 改了什么 |
|---|---|
| `Cargo.toml` | 方案 B：`members = ["src/atg-*"]`、`exclude = ["src/tdlib-rs"]`，注释写明理由 |
| `src/atg-core/Cargo.toml` | 可选 path 依赖 tdlib-rs；特性 `tdlib-api`（只拉类型，代码开关）/ `tdlib`（+ `local-tdlib` 真链接）；description 改 |
| `Cargo.lock` | 记下 tdlib-rs 的依赖（+约 700 行，特性关着也记） |
| `src/tdlib-build/build.sh`（新） | 按 `ea3f69c` 取源码（先本地 `resources/td`，再 GitHub）、只编 tdjson、布置前缀、`build-info.txt`；`path` 子命令；自动绕路径空格；查 openssl@3 与架构，不自动装 |
| `.github/workflows/ci.yml` | 判断 crate 改看 `src/atg-*`；加 `clippy -p atg-core --features tdlib-api`；加 tdlib-rs gen/parser 测试 |
| `scripts/check-no-debug.sh` | b 条也抓 `package = "tdlib-rs"` 改名依赖 |
| `docs/plan/004_tdlib-patch.md` | 第二节重写（B 的理由、两个特性、CI 表、subtree 推回命令）；第三节改成脚本；第四节加搬家后复验 |
| `src/README.md` | 重写（五个子目录各是什么、常用命令） |
| `.42cog/cog.md` | 实体表加 `src/tdlib-rs`、`src/tdlib-build` 两行 |

`src/tdlib-rs` 一行没改。

## 验证（结果在 004 第四节末表）
TDLib 181 秒编好 · version_check 1.8.69 / waitTdlibParameters / rc 0 / stderr 0 行 · atg-core `--features tdlib` check+test 过 · `tdlib-api` clippy 不用库过 · 根 fmt / no-debug / clippy / test 45 · gen 21 + parser 19

## 自审（两视角：Cargo/CI · 构建脚本/文档）已修
- P1 CI 编不到 `td` 里的 tdlib-rs 代码 → 拆 `tdlib-api` 特性 + CI 新步（试过 `tdlib-rs/docs`，与 `local-tdlib` 同开 build.rs E0425，弃）
- P1 `resources/td` 是浅克隆，`--shared` 被静默忽略，且本地仓换版本后 checkout 会死 → 改按提交号 `fetch --depth 1 file://…`，失败退 GitHub（实测 0.1 秒，6.5 MB）
- P2 Rosetta 架构不一致先拦 · `mkdir` 原子 · 提示旧构建目录 · P3 文档措辞、`path extra` 报错、check-no-debug 改名依赖
- 未修（记着）：嵌套工作区那步 CI 无缓存（gen/parser 依赖少，影响小）

## 人定（2026-10-25，三条都选建议，大总管转达）—— 已落
1. `src/tdlib-rs/ATG.md` 用法三节（Using / Building / Checking）改指 path 依赖 + `src/tdlib-build/build.sh`；上游 PR 事实那节没动
2. `LOCAL_TDLIB_PATH` 每次用 `build.sh path` 导出，不写 `.cargo/config.toml`；004 第二节写明
3. `../tdlib-rs` 与 `_build/20261025-161449/` 大总管已删；004 两处措辞同步
