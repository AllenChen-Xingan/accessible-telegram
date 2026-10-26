# 交接 · 手下 tdlib · issue #16（第一刀第 3 块：接 TDLib）

> 2026-10-25 · 交代 `_tmp/20261025-b9/tdlib.md` · **终版**
> 状态：**代码完成，多视角自审 P0/P1/P2 清零，本机检查全绿**。codex 审查按人定临时例外记进补审 issue #32（11-01 恢复后补）。**本机冒烟已过**（2026-10-25）：测试服务器 → `wait_phone`，正常关闭。

## 一句话

`atg-core` 能起停 TDLib 了：收线程带序号进事件通道、消费方单线程跑 reducer；收线程故障时在途请求失败、新请求被拒；真实后端与假后端共用同一个收线程运行器和同一条映射路径，所以 CI 里用假后端就能测到真逻辑。

## 设计

- **映射只有一条路**：tdlib-rs 的 `Update` → `serde_json::to_value` → `tdjson::classify`（授权 / 连接两条线先截走，其余走已测过的 `tdjson::event`）。往返测试（`td.rs` 末尾，14 条完整样本，`tdlib` 特性下跑）证明序列化回去映射结果不变
- **收线程是通用运行器** `link::spawn_receiver(shared, 来源闭包)`：真实后端的来源是 `tdlib_rs::receive()`，假后端是测试喂的队列。序号、三线合成、故障广播、监督都在一处
- 序号与发送在同一把锁里：通道里的序号严格递增，不管事件来自收线程、起停流程还是故障
- 请求一律 `link::guard`：等应答、故障、已关，谁先到算谁；故障后的新请求**不被轮询**，根本不交给 TDLib
- 不引异步运行时：`link::block_on` / `block_on_timeout` 极小执行器；事件流用 futures-channel 无界通道（`next_batch` 异步 / `blocking_batch` 阻塞 / `try_batch`）
- 消费方 `pump::Pump`：成批喂 reducer，每批后查 `needs_refetch()`，为真就发 `Refetch` 并交回新凭据；`resent_inputs` 把 `resendMessages` 应答变成 `Input::Resent`（null 跳过）
- 三条线（`CoreState` / `AuthStep` / `NetState`）分开存，`user_status` 纯函数合成；测试穷举 5×13×6 种组合
- 起步顺序：校验路径 → 档案锁 → `.env.local` → 钥匙串 → `execute` 降 TDLib 日志到 0 → 运行中标记 → 建客户端 + 收线程 → `setTdlibParameters` → `online=false`。客户端建好后再失败，调用方仍要调 `shutdown`
- 关闭：`close` → 等 `authorizationStateClosed`（限时 10 秒）；TDLib 已自己关了就直接收尾；正常关闭拿掉运行中标记、放档案锁

## 文件（全在 `src/atg-core/`，另有授权改的 `src/README.md`）

| 文件 | 内容 |
|---|---|
| `src/error.rs` | `CoreError`（20 个机读码）、`map_td_error`（规约 0–10 条顺序）、`message_zh`（占位措辞）、`retryable`；只有登记过的码保留 TDLib 原文 |
| `src/status.rs` | `CoreState` / `AuthStep` / `NetState`、`user_status` 合成、TDLib 类型名 → 线 |
| `src/config.rs` | `.env.local` 读 `ATG_API_ID/HASH`（整份文件按秘密清零）、档案（`server-kind` 标记、名字校验、`0700`）、`atg.lock`（`profile_busy` 带占用方）、`running` 标记 |
| `src/keystore.rs` | `KeyStore` 特征 + `MemoryKeyStore` + `Keychain`（服务名 `atg.tdlib-db-key`，账户 = 档案名，32 字节随机；长度不对不覆盖，报 `db_key_invalid`） |
| `src/secret.rs` | `Secret`（drop 时 volatile 清零，Debug `<secret>`）、`base64` |
| `src/logline.rs` | 白名单日志：字段只能是数字 / `&'static str` / 清洗过的类型名 |
| `src/link.rs` | `Shared`、`guard`、`spawn_receiver`、`EventStream`、`block_on(_timeout)` |
| `src/pump.rs` | `Pump`、`BatchOutcome`、`resent_inputs` |
| `src/backend.rs` | `Backend` 特征（本块只有生命周期：`start` `subscribe` `status` `auth_state` `shutdown`）、`StatusReport` |
| `src/fake.rs` | `FakeBackend`：编排 start（成功 / 错误 / 挂起）、关闭挂起、崩溃、来源断；记账。起停前置与真实后端一致 |
| `src/td.rs` | `TdBackend`（`tdlib-api` 特性）：一进程一个；`setTdlibParameters`（加密、`use_test_dc` 只从档案标记）；关闭途中的 500 归 `disconnected`；TDLib 自己的日志写 `~/Library/Logs/atg/tdlib-<档案>.log`（上限 10 MB） |
| `src/model.rs` | `SendFailed.td_error_code` → `error: CoreError`；`CoreEvent` 加 `AuthStepChanged` `NetStateChanged` `CoreStateChanged` |
| `src/tdjson.rs` | 加 `classify`；`SendFailed` 过 `map_td_error`；夹具 `World` 走 `classify` 并按三线合成状态 |
| `src/store/mod.rs` | 三个新事件不改状态 |
| `src/lib.rs` | 新模块；「注释里的代号」小词表 |
| `examples/live_wait_phone.rs` | 本机冒烟（`ATG_LIVE=1`，只跑 `test` 档案，只打印状态名、计时、事件数） |
| `tests/errors.rs` `tests/lifecycle.rs` `tests/config.rs` | 10 + 16 + 10 个测试 |
| `tests/reducer.rs` | 跟着 `SendFailed` 改字段 |

`scripts/setup-dev-codesign.sh` 不是我写的（codesign 手下的）。

## 验证（2026-10-25，第二次名额）

- `cargo fmt --check`、`scripts/check-no-debug.sh`：过
- `cargo clippy -D warnings`：workspace 默认 · `atg-core --features tdlib-api` · `--features tdlib`，全过
- `cargo test --workspace`：全过（lifecycle 16 · config 10 · errors 10 · reducer 77 · 快照 9 未变 · lib 27）；`--features tdlib` 下往返测试过
- 测试里的假凭据是占位形状（`ATG_API_ID=1234`、`FAKE-HASH-PLACEHOLDER`），号码样例用 `+1 555 0100` / `99966 1`
- **冒烟过**（人给私钥加 codesign 分区名单后）：签名立即返回、不弹框（`Identifier=atg.dev` · `Authority=atg-dev`）；钥匙串建钥匙也没弹框。输出：`started use_test_dc=true previous_unclean=false` → `auth="wait_phone" after_ms=54 status="signed_out"` → `events=133 last_seq=133` → `shutdown=ok core="closed"`，退出码 0。程序输出与 `~/Library/Logs/atg/tdlib-test.log`（`600`、空）里 api_id / api_hash 出现次数都是 0
- 冒烟第一次跑 dyld 找不到 `libtdjson`（程序里没写库搜索路径）→ 运行命令加 `DYLD_LIBRARY_PATH="$LOCAL_TDLIB_PATH/lib"`，README、示例头部、本文件已同步。正式分发怎么带库（rpath / 打进 app 包）留给打包那块

## 冒烟怎么跑（拿到名额后）

```sh
export LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"
cargo build -p atg-core --features tdlib --example live_wait_phone
codesign -s atg-dev -i atg.dev --force target/debug/examples/live_wait_phone
DYLD_LIBRARY_PATH="$LOCAL_TDLIB_PATH/lib" ATG_LIVE=1 target/debug/examples/live_wait_phone   # 输出另存；数凭据出现次数，应为 0（不打印值）
```

通过标准：授权状态到 `wait_phone`（或已登录的 `ready`），`shutdown` 返回 Ok，退出码 0。任何弹框都停下报大总管。

## 开发版签名

钥匙串条目的访问名单记建它的程序的签名要求。所有开发版都用**同一个固定标识**签：
`codesign -s atg-dev -i atg.dev --force <程序>`。标识不固定（默认取文件名，测试程序名还带哈希）就每次都变，读钥匙会弹框。
前提：codesign 用 atg-dev 私钥不弹框（人点「始终允许」或给私钥设分区名单）。

## 规约冲突 / 缺口（报大总管，未改规约）

1. `errors.md` 没有「配置错」这一码：`.env.local` 读不到、档案标记缺失、档案种类不符，现在都报 `internal` 带 `what`
2. `testing.md` live 前置要求号码 `99966` 开头，但 `.env.local` 里没有 `ATG_TEST_PHONE`；冒烟不登录、不发号码，所以只检查档案与标记
3. `config-privacy.md` 说 TDLib 日志级别 0，004 说 1：按前者（0）
4. 钥匙串访问控制「传统访问列表还是访问组」：用了 security-framework 的默认（传统钥匙串、访问名单 = 建条目的程序）

## 没做 / 留给后续块

- 登录（#17）、列会话（#18）、发送闸与确认凭据
- `flood.json`、本机接口套接字、直发留痕、panic 钩子（进程入口的事，CLI 接线时做）
- `sync_done`（初始同步结束信号，R11 未定）
- codex 补审（#32）

## 自审（/atg-review，3 视角）

3 P1 + 9 P2 + 7 P3，P1/P2 全修并有测试：

| 级 | 发现 | 处理 |
|---|---|---|
| P1 | TDLib 自己先关（远端退出登录、会话被终止）后再 `shutdown`：close 的应答没人收，永久卡死 | `guard` 认「已关」；`shutdown` 先查已关就直接收尾；close 请求限时。测试 `shutdown_after_core_closed_itself_returns_promptly`、`request_pending_when_core_closes_returns_disconnected` |
| P1 | 夹具 `World` 没走 `classify`，连接状态两份映射 | `World::apply` 走 `classify`；删 `net_status` |
| P1 | 冒烟示例缺前置 | 示例头部补齐，`src/README.md` 补运行前提（大总管授权） |
| P2 | `start` 先占资源后校验路径 | 路径校验挪到最前 |
| P2 | 假后端起停前置与真实不一致 | 补齐。测试 `second_backend_on_same_profile_is_busy`、`start_twice_and_shutdown_before_start`、`unclean_exit_is_reported_on_next_start` |
| P2 | 代号、发送闸原因、DC/99966、406/420、`model.rs` 旧说法 | 加代号表、补注释 |
| P3 | 若干注释与分组 | 已改；`tdjson.rs` 里 `World` 排序未挪，头部已说明 |

验证中另修：正常 `shutdown` 后档案锁到对象丢掉才放 → 两份后端正常关闭即放锁。
