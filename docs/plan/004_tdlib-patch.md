# 004 · tdlib-rs 补丁：用上 TDLib 1.8.69（issue #3）

> 2026-10-25 · 手下 patch · 依据：拍板 001-2（自维护补丁、做好反馈上游）、S0（补丁做完前第一刀一块都不开）
> 补丁仓说明正本（英文，给上游看）：`src/tdlib-rs/ATG.md`。本文只记本项目怎么用、东西在哪。
> 2026-10-25 起（issue #25）补丁以 `git subtree` 搬进本仓 `src/tdlib-rs`（fork `atg/tdlib-1.8.69` @ `4521d6d`，带完整历史）；第一节记的是搬家前的事实，项目外的旧克隆 `../tdlib-rs` 已删（人定 2026-10-25）。

## 一、补丁是什么

- 仓：人的 fork `AllenChen-Xingan/tdlib-rs`，本机克隆在项目目录**外** `../tdlib-rs`，分支 `atg/tdlib-1.8.69`，基于上游 v1.4.0（`3bfef94`）
- 改五件事（第 4、5 件照 `005_decisions-b3.md` 补丁 ①②）：
  1. TDLib 1.8.61 → **1.8.69**：`tl/api.tl` 换成 `resources/td` 提交 `ea3f69ce…7486f` 的 `td_api.tl`；版本号改在 `build.rs:21`、`src/build.rs:9`、`Cargo.toml:18`、README、CI
  2. **生成器修补**（计划外，不修编不过）：1.8.69 新增 `richTextButton button:inlineButton`。生成器判断递归类型时，只跟进大写开头的类型名，不跟进小写开头的裸类型，结果没给这一环加 `Box`，编译报 `E0072`（类型无限大）。改了 `tdlib-rs-gen/src/metadata.rs`，加 3 个单测。对外接口只多 2 处 `Box`：`RichText::Button`、`InlineButton::InlineButton`
  3. **日志隐私**：更新解析失败时，原来打印整条 JSON 外加 serde 原错误（会引用字段值）→ 现在只记 `@type`（只认标识符字符，否则记 `<invalid @type>` / `<missing @type>`）。顺带照 `001_arch-stack.md:109`：`receive()` 遇到不是 JSON、缺 `@client_id` 时不再崩，记一行（不带内容）后跳过。单测喂一条带假秘密、解析不了的更新：先确认 serde 原错误**确实**含秘密，再确认日志里没有（`tdlib-rs/src/lib.rs` 的 `tests`）。交代里的「错误位置」**做不到**：serde 处理带 `@type` 标签的枚举时先缓存整个对象，行列恒为 0:0；`serde_path_to_error` 给出的路径也只是 `.`（都实测过），所以不记位置
  4. **同步接口 `td_execute`**：新增 `tdlib_rs::execute(Value) -> Option<Value>`。TDLib 默认日志级别会把整条 update（含消息内容）打到 stderr；现在可以在建客户端之前先把级别降到 1。`version_check` 已改这么做，stderr 从 24 行变成 0 行。TDLib 把结果放在线程私有的缓冲区里（`ClientJson.cpp:74`），绑定当场复制，多线程安全。单测：不建客户端就能设置、读回日志级别
  5. **生成函数崩溃文本只留类型名**：生成代码不再 `from_value(..).unwrap()`，改走 `crate::from_response`。返回值对不上时仍然崩（那是 bug），但崩溃信息只有期望的 Rust 类型和返回值的 `@type`。单测同样先确认 serde 原错误带假秘密，再确认崩溃信息里没有；生成器另有单测，保证生成代码里没有 `.unwrap()`
- 改动清单与原因：`src/tdlib-rs/ATG.md` 末节

## 二、本项目怎么用它（issue #25 定）

**工作区：方案 B**——根 `Cargo.toml` 只收 `src/atg-*` 为成员，`exclude = ["src/tdlib-rs"]`；`src/tdlib-rs` 保留自己的 `[workspace]`，原样不动。`atg-core` 以可选 path 依赖引用：

```toml
# src/atg-core/Cargo.toml
tdlib-rs = { path = "../tdlib-rs/tdlib-rs", optional = true }

[features]
tdlib-api = ["dep:tdlib-rs"]                     # 代码开关：只拉类型，不链接
tdlib = ["tdlib-api", "tdlib-rs/local-tdlib"]   # 真接 TDLib：链接 LOCAL_TDLIB_PATH 下的库
```

- **两个特性为什么分开**：`td` 模块里用 tdlib-rs 的代码一律挂 `#[cfg(feature = "tdlib-api")]`。`tdlib-api` 不选链接方式，`cargo check` / `clippy` 不需要 TDLib 库，CI 靠它让禁打印 lint 管到 `td`——那是唯一碰 TDLib 对象的地方。要 build / test / run 就开 `tdlib`（单开 `tdlib-api` 去 build 会在链接时缺符号）。试过用 tdlib-rs 的 `docs` 特性跳过链接，但它和 `local-tdlib` 同开时 tdlib-rs 的 build.rs 编不过（`TDLIB_VERSION` 被 `docs` 条件编译掉了，E0425），要改上游代码，放弃

- **为什么不选 A（三包并进根工作区）**：cargo 给每个包往上找最近的工作区根，三包会先找到 `src/tdlib-rs/Cargo.toml`，和根工作区同时认领就报错。A 就得删掉 `src/tdlib-rs/Cargo.toml` 的 `[workspace]`，并把它的 `version` / `authors` 等继承字段搬进根——这些改动一推回 fork 就把上游的工作区弄坏，对上游毫无意义。B 下 `src/tdlib-rs` 一行不改
- `exclude` 不能省：位于工作区目录下的 path 依赖默认会被当成成员。成员写成 `src/atg-*` 而不是 `src/*`：`src/tdlib-build` 是脚本、没有 `Cargo.toml`
- 嵌套工作区能编：tdlib-rs 的 `version.workspace = true` 等由 cargo 按它自己的工作区根（`src/tdlib-rs/Cargo.toml`）解析。从根工作区编时，依赖版本锁在根 `Cargo.lock`（特性关着也会写进去，约 700 行）；单独在 `src/tdlib-rs` 里编时用它自己的锁文件（上游忽略、不入库）
- **默认不编 TDLib**：两个特性默认都关，`cargo build` / `cargo test --workspace` 都不碰 tdlib-rs。要接 TDLib：先编库（第三节），再 `LOCAL_TDLIB_PATH=… cargo build -p atg-core --features tdlib`
- `cargo fmt --all` 会连 path 依赖一起检查（`-v` 实测列出了 tdlib-rs 三包的文件），所以根上的 fmt 已经覆盖它们；clippy / test `--workspace` 只管成员
- 只有 `atg-core` 的 `td` 模块许用 tdlib-rs（`scripts/check-no-debug.sh` 第 b 条）。该脚本只扫 `src/atg-*`；`src/tdlib-rs` 是上游代码、`src/tdlib-build` 是 shell 脚本，都不归它管
- `LOCAL_TDLIB_PATH` 不写进 `.cargo/config.toml`（人定 2026-10-25）：构建前缀带时间戳、每次不同，写死会过期。每个终端用前先 `export LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"`
- 建客户端前先 `tdlib_rs::execute(setLogVerbosityLevel 1)`（见 `src/tdlib-rs/tdlib-rs/examples/version_check.rs`）

### CI 怎么管 tdlib-rs

| 步骤 | 范围 | 理由 |
|---|---|---|
| fmt | 根 `--all`，含 tdlib-rs 三包 | 上游本来就 fmt 干净，不费事 |
| clippy `-D warnings` | `atg-*`；另加一步 `-p atg-core --features tdlib-api`（编到 `td` 里用 tdlib-rs 的代码，不用库）；不对 subtree 跑 | subtree 是上游代码；新版 clippy 出新 lint 时，不该让本项目 CI 为不归我们管的代码变红。补丁改动在本地跑过 clippy（第四节） |
| test | `atg-*` + tdlib-rs 的 `tdlib-rs-gen`、`tdlib-rs-parser` 两包（`cargo test --manifest-path src/tdlib-rs/Cargo.toml -p tdlib-rs-gen -p tdlib-rs-parser`） | 生成器与解析器不要 TDLib 库，几秒跑完，守着第一节第 2 件（递归修补）与第 5 件的生成器测试。`tdlib-rs` 包要 TDLib 库，跳过（上游 `make test` 也跳过它） |

### 把 `src/tdlib-rs` 的改动推回 fork

改动先在本仓正常提交（一次提交别同时改 `src/tdlib-rs` 和别处，推回去的历史才干净）。然后（只记命令，由人或大总管执行）：

```sh
# 推到 fork 的补丁分支（会把只涉及 src/tdlib-rs 的提交拆出来）
git subtree push --prefix=src/tdlib-rs https://github.com/AllenChen-Xingan/tdlib-rs atg/tdlib-1.8.69
# 反过来，fork 上有新提交时拉回来
git subtree pull --prefix=src/tdlib-rs https://github.com/AllenChen-Xingan/tdlib-rs atg/tdlib-1.8.69
```

给上游提 PR：从 fork 的 `atg/tdlib-1.8.69` 开，PR 描述由人写，事实材料见 `src/tdlib-rs/ATG.md` 末节。

## 三、TDLib 库放哪、怎么重编

**一条命令**（在仓库任意位置）：

```sh
bash src/tdlib-build/build.sh                              # 约 3–4 分钟；最后一行就是 export LOCAL_TDLIB_PATH=…
export LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"   # 以后取最近一次编好的前缀
```

脚本锁定 TDLib 1.8.69 = `ea3f69ce11aef458bfbb09869b809ede4447486f`：按提交号先从本地 `resources/td` 取（复制约 6.5 MB 对象，不走网络；它是浅克隆，`--shared` 会被 git 静默忽略，所以干脆不用），取不到再从 GitHub 只拉这一个提交；先核对 openssl@3 与终端架构一致（防 Rosetta 终端编 3 分钟后才在链接时失败）；cmake Release 只编 `tdjson`；手工布置前缀；写 `build-info.txt`（版本、提交、用时、dylib 的 sha256）。缺 git / cmake / gperf / Homebrew `openssl@3` 就报错退出，不自动安装。改版本时三处一起改：脚本头、`src/tdlib-rs` 的 `TDLIB_VERSION`、本文。`path` 只认带 `build-info.txt` 的目录（脚本之前手工编的 `_build/20261025-161449/` 已删，人定 2026-10-25）。每次构建约 300 MB，失败的半截目录不自动删（`path` 会跳过它）；脚本结束时提示目录个数，删由人定。

| 东西 | 位置 | 入库？ |
|---|---|---|
| 源码 | 脚本取到 `_build/<时间戳>/td-src/`（`resources/td` 只读，作本地来源） | 否 |
| 构建 | `_build/<时间戳>/build/`（约 260 MB）· 日志 `cmake-*.log` | 否（`_build/` 已忽略） |
| 库（`LOCAL_TDLIB_PATH` 指这里） | `_build/<时间戳>/tdlib/`：`lib/libtdjson.1.8.69.dylib` + `libtdjson.dylib` 软链接 + `include/` | 否 |

脚本照下面三个坑处理好了（留作原因）：

1. **项目路径末尾有空格**（`telegram accessibility `）：TDLib 的 CMake 把 `-Wl,-exported_symbols_list,<源码目录>/tdclientjson_export_list` 原样拼进链接命令、没加引号，路径在空格处被拆成两段，第一次编到最后链接一步失败（`no such file or directory: '/_build/…/tdclientjson_export_list'`）。办法：在**不带空格**的路径建一个软链接指向 `_build/<时间戳>/`，cmake 的源码、构建、前缀全走软链接路径。脚本把软链接建在 `$TMPDIR`，用完即删；`build/CMakeCache.txt` 记的是软链接路径，所以**要重编就再跑一次脚本**（新时间戳目录），别在旧目录里原地重编
2. `--target tdjson` 只编一个目标，`cmake --install` 会报缺文件；手工把 dylib（连同 `libtdjson.dylib` 软链接，链接时用 `-ltdjson`）与三个头文件放进前缀
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

### 搬进 src/ 后复验（2026-10-25，issue #25，macOS arm64）

| 项 | 结果 |
|---|---|
| `bash src/tdlib-build/build.sh` | 181 秒（`-j10`）→ `_build/20261025-171733/tdlib/`，`libtdjson.1.8.69.dylib` sha256 `dbff3f5f6176e65ee92a2135c472bf22561e9334b7e89c3c852f9ff3e78d06e8`（与第一次手工编的不同；两次构建路径不同，没逐字节查原因） |
| `version_check`（`--manifest-path src/tdlib-rs/Cargo.toml -p tdlib-rs --features local-tdlib`，没设 API_ID，不连服务器） | `tdlib version: 1.8.69` · `waitTdlibParameters` · `closed`，退出码 0，stderr 除 cargo 自身输出外 0 行。项目路径带空格，`OUT_DIR` / rpath 都正常 |
| `cargo check` / `cargo test -p atg-core --features tdlib` | 过（真链接） |
| `cargo clippy -p atg-core --features tdlib-api --all-targets -D warnings`（不设 `LOCAL_TDLIB_PATH`） | 过 |
| 根 fmt · check-no-debug · clippy `--workspace` · test `--workspace`（45）· gen 21 + parser 19 | 全过 |

## 五、待定 / 待人

- 复跑测试服务器：在 `src/tdlib-rs` 里（先 `export LOCAL_TDLIB_PATH="$(bash ../tdlib-build/build.sh path)"`）`set -a; . <项目>/.env.local; set +a; API_ID=$ATG_API_ID API_HASH=$ATG_API_HASH ./target/debug/examples/version_check`（例子沿用上游的变量名 `API_ID` / `API_HASH`）
- **归 `atg-core`（拍板 005 补丁 ③）**：生成的类型都派生 `Debug`，对 TDLib 对象用 `{:?}` 会打出全部内容（消息、姓名、手机号）。禁止这样整个打印，用检查规则（clippy 配置或脚本）拦，`atg-core` 开工时落实
- 上游 PR 描述由人写；事实材料在 `src/tdlib-rs/ATG.md` 末节
