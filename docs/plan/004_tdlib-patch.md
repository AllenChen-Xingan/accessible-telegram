# 004 · tdlib-rs 补丁：用上 TDLib 1.8.69（issue #3）

> 2026-10-25 · 手下 patch · 依据：拍板 001-2（自维护补丁、做好反馈上游）、S0（补丁做完前第一刀一块都不开）
> 补丁仓说明正本（英文，给上游看）：`../tdlib-rs/ATG.md`。本文只记本项目怎么用、东西在哪。
> 文中 `../tdlib-rs` 都相对**项目根**（即与本项目目录并列）。⚠️ 补丁仓改动**尚未提交**，提交并推到 fork 之前，下面的 git 依赖解析不了。

## 一、补丁是什么

- 仓：人的 fork `AllenChen-Xingan/tdlib-rs`，本机克隆在项目目录**外** `../tdlib-rs`，分支 `atg/tdlib-1.8.69`，基于上游 v1.4.0（`3bfef94`）
- 改五件事（第 4、5 件照 `005_decisions-b3.md` 补丁 ①②）：
  1. TDLib 1.8.61 → **1.8.69**：`tl/api.tl` 换成 `resources/td` 提交 `ea3f69ce…7486f` 的 `td_api.tl`；版本号改在 `build.rs:21`、`src/build.rs:9`、`Cargo.toml:18`、README、CI
  2. **生成器修补**（计划外，不修编不过）：1.8.69 新增 `richTextButton button:inlineButton`。生成器判断递归类型时，只跟进大写开头的类型名，不跟进小写开头的裸类型，结果没给这一环加 `Box`，编译报 `E0072`（类型无限大）。改了 `tdlib-rs-gen/src/metadata.rs`，加 3 个单测。对外接口只多 2 处 `Box`：`RichText::Button`、`InlineButton::InlineButton`
  3. **日志隐私**：更新解析失败时，原来打印整条 JSON 外加 serde 原错误（会引用字段值）→ 现在只记 `@type`（只认标识符字符，否则记 `<invalid @type>` / `<missing @type>`）。顺带照 `001_arch-stack.md:109`：`receive()` 遇到不是 JSON、缺 `@client_id` 时不再崩，记一行（不带内容）后跳过。单测喂一条带假秘密、解析不了的更新：先确认 serde 原错误**确实**含秘密，再确认日志里没有（`tdlib-rs/src/lib.rs` 的 `tests`）。交代里的「错误位置」**做不到**：serde 处理带 `@type` 标签的枚举时先缓存整个对象，行列恒为 0:0；`serde_path_to_error` 给出的路径也只是 `.`（都实测过），所以不记位置
  4. **同步接口 `td_execute`**：新增 `tdlib_rs::execute(Value) -> Option<Value>`。TDLib 默认日志级别会把整条 update（含消息内容）打到 stderr；现在可以在建客户端之前先把级别降到 1。`version_check` 已改这么做，stderr 从 24 行变成 0 行。TDLib 把结果放在线程私有的缓冲区里（`ClientJson.cpp:74`），绑定当场复制，多线程安全。单测：不建客户端就能设置、读回日志级别
  5. **生成函数崩溃文本只留类型名**：生成代码不再 `from_value(..).unwrap()`，改走 `crate::from_response`。返回值对不上时仍然崩（那是 bug），但崩溃信息只有期望的 Rust 类型和返回值的 `@type`。单测同样先确认 serde 原错误带假秘密，再确认崩溃信息里没有；生成器另有单测，保证生成代码里没有 `.unwrap()`
- 改动清单与原因：`../tdlib-rs/ATG.md` 末节

## 二、`atg-core` 将来的依赖写法

```toml
tdlib-rs = { git = "https://github.com/AllenChen-Xingan/tdlib-rs", rev = "4521d6df471979550bf15f759b2c05ca67f21ce9", features = ["local-tdlib"] }
```

- 已锁定提交 `4521d6d`（fork 分支 `atg/tdlib-1.8.69`，2026-10-25 推送）；补丁再改时同步更新这里的 `rev`，免得分支动了 `Cargo.lock` 跟着漂
- 编译时要有环境变量 `LOCAL_TDLIB_PATH=<前缀>`；建议写进仓根 `.cargo/config.toml` 的 `[env]`（相对路径用 `relative = true`），免得每个终端都 export。**这一步等 S0 解除、开第一刀时由那条 issue 做**，本次不动 `src/atg-core`

## 三、TDLib 库放哪、怎么重编

| 东西 | 位置 | 入库？ |
|---|---|---|
| 源码 | `resources/td`（只读，1.8.69） | 否（资料区） |
| 本次构建 | `_build/20261025-161449/`：`td-src/`（`git clone --shared`）· `build/`（260 MB）· `tdlib/`（安装前缀，24 MB） | 否（`_build/` 已忽略） |
| 库 | `_build/20261025-161449/tdlib/lib/libtdjson.1.8.69.dylib`，sha256 `348a8e832dfcedd4f43a7a5711e7878eb06d9ec3c99d052eadae10d0100f7fb9` | 否 |

重编步骤见 `../tdlib-rs/ATG.md`「Building TDLib」。三个坑：

1. **项目路径末尾有空格**（`telegram accessibility `）：TDLib 的 CMake 把 `-Wl,-exported_symbols_list,<源码目录>/tdclientjson_export_list` 原样拼进链接命令、没加引号，路径在空格处被拆成两段，第一次编到最后链接一步失败（`no such file or directory: '/_build/…/tdclientjson_export_list'`）。办法：在**不带空格**的路径建一个软链接指向 `_build/<时间戳>/`，cmake 的源码、构建、前缀全走软链接路径。本次软链接在会话草稿区，会话结束就没了；`build/CMakeCache.txt` 记的是软链接路径，所以**要重编就新建构建目录**（或重建同名软链接）
2. `--target tdjson` 只编一个目标，`cmake --install` 会报缺文件；照 ATG.md 手工把 dylib（连同 `libtdjson.dylib` 软链接，链接时用 `-ltdjson`）与三个头文件放进前缀
3. 库按绝对路径依赖 Homebrew 的 `openssl@3`；卸了它库就打不开。做成正式安装包时要另想（打包进 .app 或静态链接）

## 四、验证结果（2026-10-25，macOS arm64）

| 项 | 结果 |
|---|---|
| TDLib 编译 | `--target tdjson -j8`，203 秒 |
| `cargo fmt --check` · `clippy --all-targets -D warnings` | 通过，无告警 |
| `cargo test --workspace --lib --bins --tests` | 45 个全过（tdlib-rs 5 · gen 21 · parser 19；含 ④⑤） |
| `version_check` 例子 | `tdlib version: 1.8.69` · 第一条授权状态 `waitTdlibParameters` · 连跑 3 次正常退出。（另：首次运行时 TDLib 自己的日志打出 `updateOption commit_hash = ea3f69ce…`，与 `resources/td` 一致） |
| 连测试服务器 | ✅ `use_test_dc=true`：`waitTdlibParameters` → `waitPhoneNumber`，连接 `Ready`，随后关闭，退出码 0。没登录、没发消息。api_id / api_hash 取自 `.env.local` 的 `ATG_API_ID` / `ATG_API_HASH`，在 shell 里映射给例子；stdout 与 stderr 里两者出现次数都是 0（只数次数、没显示内容，跑完文件已删），临时库目录已删 |

- 2 个 doctest（`tdlib-rs/src/build.rs`）不算在内：它们是给 build.rs 用的示例，要读 `CARGO_CFG_TARGET_OS`，在 `cargo test` 下必然失败，补丁前就这样；上游 `make test` 本来就跳过 `tdlib-rs` 包
- 上游的 `get_me` 例子在编译时用 `env!` 读 `API_ID` / `API_HASH`，编译要给占位值（`API_ID=0 API_HASH=compile-only`），只是为了编得过，不运行它

## 五、待定 / 待人

- 复跑测试服务器：在补丁仓里 `set -a; . <项目>/.env.local; set +a; API_ID=$ATG_API_ID API_HASH=$ATG_API_HASH ./target/debug/examples/version_check`（例子沿用上游的变量名 `API_ID` / `API_HASH`）
- **归 `atg-core`（拍板 005 补丁 ③）**：生成的类型都派生 `Debug`，对 TDLib 对象用 `{:?}` 会打出全部内容（消息、姓名、手机号）。禁止这样整个打印，用检查规则（clippy 配置或脚本）拦，`atg-core` 开工时落实
- `atg-core` 开工时：建客户端前先 `tdlib_rs::execute(setLogVerbosityLevel 1)`（见 `version_check.rs`）
- 上游 PR 描述由人写；事实材料在 `../tdlib-rs/ATG.md` 末节
