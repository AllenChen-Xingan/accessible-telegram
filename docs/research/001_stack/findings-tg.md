# 命门 3 核查 · grammers vs TDLib

> 2026-10-23 · tg。材料在 `resources/`（只读）：`grammers-codeberg`（正本）· `td`（1.8.69）· `tdlib-rs`（v1.4.0）。
> ⚠️ 同名不等物：GitHub 上的 `Lonami/grammers` 从 2026-02-10 起停更，README 写明已迁到 codeberg.org/Lonami/grammers。以下 grammers 结论全部以 Codeberg 版为准。

| # | 核查项 | grammers 0.11 | TDLib（+ tdlib-rs） |
|---|---|---|---|
| 1 | 用户登录 + 二步验证 | ✅ `grammers-client/src/client/auth.rs:242` 请求验证码、`:339` 登录、`:416` 二步验证密码。**缺口**：`PaymentRequired` 直接 `unimplemented!()` 会崩（`:271`）；没处理「要求先绑邮箱」的新流程；没有扫码登录 | ✅ 有完整的状态机 `td_api.tl:206-250`（手机号 / 验证码 / 邮箱 / 密码 / 扫码 / 注册），`checkAuthenticationPassword` 在 `:12261` |
| 2 | 实时更新 + 断线补齐 | **部分**：新消息 / 编辑 / 删除有封装（`update/update.rs:25-29`）；已读和「正在输入」只能拿 `Raw`，文档说明 Raw 会在小版本间变动（`:41-48`）。补齐 ✅：`message_box/mod.rs:600` 走 getDifference，需开 `catch_up`（`grammers-mtsender/src/configuration.rs:74`），而且状态要**手动** `sync_update_state`，退出时不会自动保存（`client/updates.rs:51-54`） | ✅ 都是一等更新：`updateNewMessage:11196` · `updateMessageEdited:11230` · `updateDeleteMessages:11523` · `updateChatReadInbox/Outbox:11331` · `updateChatAction:11530`（正在输入）。补齐 ✅ 在 `UpdatesManager.cpp:480` / `:2055` 里自动完成 |
| 3 | 本地缓存与冷启动 | ❌ **预判成立**：SQLite 里只有 dc / peer / update_state / channel_state 几张表（`storages/sqlite.rs:112-151`），没有消息和会话列表，缓存要自己建。peer 缓存 2026-10-11 刚修过「只存了第一个」的 bug | ✅ 库自带：会话库加消息库（`TdDb.cpp:362-371`、`MessageDb.cpp`、`DialogDb.cpp`）；`getChatHistory only_local:true` 可以不联网先出缓存（`td_api.tl:12758`） |
| 4 | 构建 | 基本是纯 Rust，约 3.3 万行；默认带上 `libsql`（会编一段 C 写的 SQLite，`grammers-session/Cargo.toml:18,24`）。一次 cargo 构建，几分钟 | 源码约 44 万行 C++，要 CMake，每个文件编译吃 0.5–1 GB 内存（`README.md:82`），通常要几十分钟。**有预编译库**：tdlib-rs 打开 `download-tdlib` 特性后从 GitHub Release 下载（`build.rs:196-215`），macOS arm64 压缩包 39.7 MB |
| 5 | 测试服务器（test DC） | **部分**：测试服务器的 RSA 密钥已经内置（`grammers-mtproto/src/authentication.rs:697-698`），但 DC 地址写死成正式服务器（`dc_options.rs:25`），要通过 `Session::set_dc_option` / `set_home_dc_id` 自己改（`session.rs:36,53`） | ✅ 一个参数搞定：`setTdlibParameters use_test_dc`（`td_api.tl:12171`） |
| 6 | 维护状态 | 活跃，单人维护：最新提交 2026-10-23；2026-10-21 升到 layer 232（`tl/api.tl:1`）；版本还没到 1.0 | TDLib 很活跃：2026-10-21，layer 232（`Version.h:13`）。**tdlib-rs 落后**：绑定的是 TDLib 1.8.61，最近推送 2026-04-12，108 星，单人维护 |

## 预判复核（must-know 第四节后两行）

- grammers「能登录、有二步验证、没有消息库」：**成立**。补一条预判没想到的：登录流程有会直接崩的分支，「已读」「正在输入」也没有封装。
- TDLib「自带 SQLite、更新最全、C++ 编译重、有预编译」：**成立**。补一条：预编译靠的是 tdlib-rs 作者个人发布的包，比上游落后 8 个小版本。

## 判断

- **首选：TDLib + tdlib-rs（`download-tdlib`），直接用。** 理由：冷启动先出缓存、补齐更新、正在输入和已读，库都现成，直接对上「速度很快」和真难题 1（流动界面要的事件全是一等更新）；测试服务器一个开关就能用，`specs` 要求的自动化测试最省事。
- **备选：grammers，在它基础上改造。** 纯 Rust、构建快、贴合 Rust 技术栈；代价是消息缓存、已读和正在输入的封装、更新状态持久化都要自己写，登录还得补崩溃分支。

## 三个风险

1. **tdlib-rs 跟不上上游**：现在落后 8 个版本、单人维护。缓解办法：用 `local-tdlib` 自己编一次固定版本的 TDLib（要单独申请编译名额），或者自己维护一层薄绑定。
2. **TDLib 是黑盒，还有体积**：dylib 约 40 MB 压缩包，经 tdjson 走 JSON 来回序列化，并自带线程。它跟 GPUI 异步模型怎么衔接、按键到播报的延迟多少，都要用一次小实验实测。
3. **两边 API 都会变**：grammers 还没到 1.0，Raw 更新会在小版本间挪动；TDLib 的 `td_api` 也常有不兼容的改名。读法层要和库类型隔开一层，别让库的类型一路渗到界面。
