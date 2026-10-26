# src — 作品区：所有代码都在这里

> 规则（人定 2026-10-25，`CLAUDE.md` 第五节）：本项目写的、Telegram 要用的、要编译的代码一律放 `src/`，每个部件占一个根子目录。
> 每个子目录在 `.42cog/cog.md` 实体表里各占一行（是什么 · 谁产生 · 谁是谁的输入）；Rust 工作区成员以根 `Cargo.toml` 为准。

| 目录 | 是什么 | 归谁 |
|---|---|---|
| `atg-core/` | 能力层 crate：内容树、读法；`td` 模块接 TDLib（`tdlib-api` 只拉类型 / `tdlib` 真链接，默认都关） | 本项目 |
| `atg-cli/` | 命令行入口 crate（JSON 行输出，AI 的首选接口），见 `atg-cli/README.md` | 本项目 |
| `atg-gui/` | 界面入口 crate（GPUI），还没建 | 本项目 |
| `tdlib-rs/` | 打过补丁的 tdlib-rs（Rust 绑定 + 代码生成器 + TL 解析器三包），fork `AllenChen-Xingan/tdlib-rs` 分支 `atg/tdlib-1.8.69` 的 `git subtree`。**自成工作区，不是根工作区成员**；改动要对上游有意义，好推回 fork。说明见 `tdlib-rs/ATG.md` | fork（上游代码 + 我们的补丁） |
| `tdlib-build/` | 拉取并编译 TDLib C++ 库的脚本。TDLib 源码**不进库**，按锁定提交拉到 `_build/<时间戳>/` | 本项目 |

## 常用命令

```sh
cargo fmt --all --check && bash scripts/check-no-debug.sh
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                       # 不编 TDLib
cargo test --manifest-path src/tdlib-rs/Cargo.toml -p tdlib-rs-gen -p tdlib-rs-parser

# 要接 TDLib 时
bash src/tdlib-build/build.sh                # 约 3–4 分钟，只需一次
export LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"
cargo build -p atg-core --features tdlib
cargo clippy -p atg-core --features tdlib-api --all-targets -- -D warnings   # 不用库
```

依赖怎么接、CI 管到哪、怎么把 `tdlib-rs/` 的改动推回 fork：`docs/plan/004_tdlib-patch.md` 第二、三节。
