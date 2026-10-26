# 交接 · skeleton（issue #14 第一刀第 1 块：建骨架）

> 2026-10-25 · 交代 `_tmp/20261025-b4/skeleton.md` · 分支 `chore/14-skeleton`（大总管切的）· 未提交，没做任何 git 写操作

## 状态：已收尾（做完，自审已修，复验全绿，三条拍板已落进契约）

- `cargo fmt --all --check` 过 · `cargo clippy --workspace --all-targets -D warnings` 过 · `cargo test --workspace` 全过（atg-core 原 35 个不变、快照没动；atg-cli 新增 4 单测 + 6 集成测试）· `bash scripts/check-no-debug.sh` 过 · `cargo tree` 里没有 gpui
- 自审：`/atg-review` 三视角（工作流 `wf_557d6b60-ade`），0 个 P0；范围内的 P1/P2/P3 已修，范围外的列在下面「转交」

## 改了哪些文件

| 文件 | 改了什么 |
|---|---|
| `src/atg-cli/`（新） | `Cargo.toml`（依赖 atg-core、clap 4、serde_json）· `src/main.rs` · `src/args.rs`（子命令名与全局选项，照 `specs/contracts/cli.md` 第一节第 2 小节）· `src/out.rs`（外壳、`at` 毫秒时间、错误行、退出码）· `tests/cli.rs`（跑真二进制）· `README.md` |
| `src/atg-core/src/{td,model,store,notify,backend}.rs`（新） | 空模块，各一段文档注释，名字对齐 001 第一节第 2 小节 |
| `src/atg-core/src/lib.rs` | 加上述模块 + 补丁 ③ 的 lint 行；内容树已有代码与快照没动 |
| `scripts/check-no-debug.sh`（新） | 补丁 ③ 的脚本层 |
| `.github/workflows/ci.yml` | fmt 后、clippy 前加一步 `bash scripts/check-no-debug.sh` |
| `.42cog/cog.md` | 实体表那句：`atg-cli` 标已建 |
| `Cargo.lock` | 加 clap 一族 |

## 行为

- `atg-cli version` / `--version`：一行 `kind:"version"`（`atg_cli`、`v`、`tdlib:null`）+ 一行 `result`，退出码 0
- 其余 23 个子命令：一行 `kind:"error"`，`code:"not_implemented"`，带 `command`，退出码 1
- 参数不对 / 缺子命令：一行 `code:"usage"`，退出码 2；clap 原文只进标准错误（可能带用户输入，测试守着）
- `--help`：给人看，退出码 0；`--format text`：一句中文

## 补丁 ③ 怎么落的（都实测过正反例）

1. **clippy**：每个 crate 根 `#![cfg_attr(not(test), deny(clippy::use_debug, clippy::dbg_macro, clippy::unwrap_used, clippy::expect_used))]`。实测 clippy 1.99 的 `use_debug` **只管 print 系和 write 系，不管 `format!` / `panic!` / `assert_eq!` / 日志宏**
2. **脚本** `scripts/check-no-debug.sh`（CI 跑，只扫 `src/atg-*`）补 clippy 管不到的：a 每个 crate 根有那行 · b `tdlib_rs` 只在 `atg-core/src/td*`，只有 atg-core 许依赖它 · c 日志宏 `?字段` · d 非测试代码里的 `{…?}` 占位符、`assert_eq!/assert_ne!`、`fn main() -> Result`
3. 已知拦不到（写在脚本头）：手写「Display = Debug」的类型；宏生成的代码

## 已知缺口

- 占位子命令把后面的参数整个收下，放在其后的 `--format text` 靠原始参数补认（`main.rs`），各块定好真参数后去掉

## 「src/ 只放 crate」的假设（大总管问的）

- **我的脚本**：已改成只扫 `src/atg-*`，`src/tdlib-rs` 搬进来不受影响
- **CI**：`ls src/*/Cargo.toml` 只判断「有没有 crate」，不受影响；但 fmt / clippy / test 都是 `--workspace`，范围由根 `Cargo.toml` 定
- ⚠️ **根 `Cargo.toml` 的 `members = ["src/*"]`**（不在我能改的文件里）：`src/tdlib-rs` 一搬进来就会被当成成员。它自带 `[workspace]` 的话 cargo 直接报「嵌套 workspace」；不带的话 CI 会对它跑 fmt --check 和 clippy -D warnings，大概率红。建议改成 `members = ["src/atg-*"]`（或加 `exclude = ["src/tdlib-rs"]`），依赖走 `path = "../tdlib-rs/tdlib-rs"`

## 要人定的事 —— ✅ 已定（2026-10-25，人三条都选建议，大总管转达）

> 落地：`specs/contracts/errors.md`「CLI 自有码」加 `not_implemented`；`specs/contracts/cli.md` 自有码、退出码表、`version` 行（`--version` 别名）各改一处，两份各加一行修订记录；`src/atg-cli/src/out.rs` 注释同步。`Cargo.toml` members 归 issue #25。下面题面保留作依据。


1. **`not_implemented` 怎么登记**（契约里 CLI 自有码只有 `usage`、`interrupted`）
   - A. 保留 `not_implemented`、退出码 1，登记进 `errors.md` 作 CLI 自有码 ← **建议**（AI 能按码分支，和「真出 bug」的 `internal` 分开）
   - B. 改用 `internal`（不加码；但 AI 分不清「没做」和「坏了」）
   - C. 新开退出码（如 11）
2. **补丁 ③ 的范围**
   - A. 非测试代码一律不许 Debug 格式化、unwrap、expect（现状）← **建议**（隐私红线：model 类型同样带消息和名字）
   - B. 只管 `td` 模块（松，但 TDLib 内容映射进 model 后就没人管了）
3. **`--version` 选项**（交代要求有，契约里只有 `version` 子命令）
   - A. 在 `cli.md` 登记为 `version` 的别名 ← **建议**
   - B. 删掉，只留子命令

## 转交：自审发现、不在我范围里的

| 等级 | 哪里 | 什么 |
|---|---|---|
| P1 | `src/atg-core/src/json.rs:20-33` 对 `src/atg-cli/src/out.rs` | 外壳两份：core 拼 `{v,kind,data,speech}` 没 `at`，cli 拼 `{v,kind,at,data}`。我在 out.rs 注明「内容行以后从 json::value 取再补 at」；core 那份要不要改（含快照）归内容树 / 第 6 块 |
| P1 | `src/README.md` | 还是初始化模板「这个目录现在是空的」，和现状矛盾 |
| P2 | `json.rs:12-18` | Alert / Status / Unknown 都映射成 `kind:"event"`，和 `cli.md` 的事件名表、独立的 `status` kind 对不上 |
| P2 | `src/atg-core/tests/fixtures/` | `testing.md:32-35` 定的位置是 `src/atg-core/fixtures/`（`--fake` 也读那里） |
| P2 | `tree/mod.rs:527-536` | `UserStatus` 缺规约里已有的 `Stopping`（content-tree.md:275 记了 S0 暂停，代码里没指针） |
| P2/P3 | tree / speech / tdjson 注释 | 「拍板 7」「T02」「投影」「infra」「td_api.tl:行号」等没出处或没解释 |
