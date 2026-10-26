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
cargo test --workspace                       # 不编 TDLib。经 runner 先用开发身份签名（见下）；
                                             # 本机还没有开发身份时：ATG_DEV_SIGN=0 cargo test --workspace（单测不碰钥匙串）
cargo test --manifest-path src/tdlib-rs/Cargo.toml -p tdlib-rs-gen -p tdlib-rs-parser

# 要接 TDLib 时
bash src/tdlib-build/build.sh                # 约 3–4 分钟，只需一次
export LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"
cargo build -p atg-core --features tdlib
cargo clippy -p atg-core --features tdlib-api --all-targets -- -D warnings   # 不用库

# 真连 Telegram 还要（都不入库）：仓库根 .env.local 写 ATG_API_ID / ATG_API_HASH（自己在 my.telegram.org 申请）；
# 开发签名身份，只需一次：在 Xcode 领免费的 Apple Development 证书；bash scripts/setup-dev-codesign.sh 检查并给出步骤。
# 所有开发版都用这个身份、固定标识 atg.dev 签名，本地库钥匙在钥匙串里才不弹框（issue #38）。
# 不签名的程序读钥匙会弹框。runner = cargo 运行编出来的程序之前先调的包装脚本：.cargo/config.toml 设成
#   scripts/dev-sign-run.sh，所以 cargo run / cargo test 直接运行的程序都自动签名，并把 TDLib 库目录带给程序
#   （只配了 Apple 芯片本机目标；没有身份时 runner 报错不运行，只跑单测可设 ATG_DEV_SIGN=0）
# 只有跑过旧版（atg-dev 自签名证书时期，2026-10-26 以前）建过 test / main 条目的机器才要迁移：换身份后第一次读
#   各弹一次框，点「始终允许」（可能先要输登录密码），之后不再弹。新机器没有旧条目，忽略这条。
#   test 由下面的冒烟（live_wait_phone）读到；main 由 src/atg-cli/live/login-main.sh 或带 TDLib 的 atg-cli 读到
# 本机冒烟：测试服务器 → 授权状态到 wait_phone，然后正常关闭（不登录、不发消息）
ATG_LIVE=1 cargo run -p atg-core --features tdlib --example live_wait_phone
# 不经 cargo 直接跑时（程序里没写库搜索路径，要指给它）：
#   bash scripts/dev-sign.sh target/debug/examples/live_wait_phone
#   DYLD_LIBRARY_PATH="$LOCAL_TDLIB_PATH/lib" ATG_LIVE=1 target/debug/examples/live_wait_phone
```

依赖怎么接、CI 管到哪、怎么把 `tdlib-rs/` 的改动推回 fork：`docs/plan/004_tdlib-patch.md` 第二、三节。
