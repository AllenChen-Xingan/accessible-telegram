# 交接 · 手下 media · #4 听语音 + #59 语音转文字

> 2026-11-13 · 交代 MAIN `_tmp/20261113-b18/media.md` · 大总管 `telegram-accessibility-dd` · arch `arch-4d` · 分支 `feat/4-voice`

## 做到哪

- [x] exp007、exp008、设计 `docs/experiments/exp008-local-stt/design.md`（大总管与 arch 已定）
- [x] codex 017（实验 + 设计）13 轮收敛
- [x] PR #85（下载 + 播放，8ba74c7，大总管选合成一个）：代码、契约、测试全绿；atg-review 自审（P1 1 条已修）；codex 018 三轮收敛（第 2、3 轮无 P0/P1）；超线文件全比 main 短（arch d 方案）。PR 草稿 MAIN `_tmp/20261113-b18/pr-media.md`，大总管提交并开 PR
- [ ] 合 main（#52 发送，PR #90）进 feat/4-voice：10 个冲突已解（两边功能都留；Submit 进 fake/call.rs、td/tests.rs 换成 main 版；超线文件仍全比 main 短）；全量 test、三遍 clippy（含 tdlib-api 与 atg-cli/tdlib）、fmt、check-no-debug 全绿（#79 偶发一次，重跑过），名额已还，等大总管提交合并
- [ ] 测试缺口 issue（标记请求进行中取消且失败）：大总管开
- [~] 下一个 PR：转写（#59）。草稿在 scratch（`…/scratchpad/pr3/`）：`stt-helper/main.swift`、`build.sh`、`stt.rs`（协议、选语种表、看门 + 守卫生命周期）、`fake-helpers/*.sh`（8 个假辅助程序：正常、无版本行、版本不对、卡死、缺行、坏 JSON 带暗号、单边失败、全不可用）。**都未编译**。等本 PR 合并、大总管从新 main 开分支后挪进 `src/atg-audio/`
- [ ] 人：终端里听一条、暂停一次（M2.6 验收）；收藏夹语音验下载与解码；读法实听；断网转写

## PR1 改了哪些文件

`src/atg-core/src/tree/{mod.rs,media.rs}`（`FileRef`；`Expirable` 挪进 media.rs 抵消行数）· `tdjson.rs`（语音带文件 id）· `backend.rs` `td.rs` `fake.rs`（`file_download`）· `tests/reducer.rs`（构造器补字段）· `src/atg-cli/src/{download.rs,voice/mod.rs,voice/tests.rs,args.rs,main.rs,session.rs}`、`history.rs`（三个助手函数改 `pub(crate)` 复用）· `specs/contracts/{cli.md,actions.md}`、`specs/content-tree.md`

## 名额

领过两次（swiftc 探针 + 转写；.app 授权补测），都已归还。PR1 编译测试名额已申请。

## 已定（2026-11-13）

大总管：模块划分 · 语种双跑取高 + `--lang` · 标点只在读法里补 · 转写只在内存 · v0 按需下（已改 roadmap、inventory:163/165、实验索引）
arch：`FileRef` 共用 · 转写表用稳定键、reducer 删迁 · `stt_unavailable` 单开 · 辅助程序放 `src/atg-audio/stt-helper/` 脚本编 · `Doc::Playback` · 组合动作放 `atg-cli/src/voice/` 库代码 · v0 不转发（档案被占报 profile_busy）

## 待人验证 / 待定

- 断网时转写是否仍可用（exp008 未断网实测）
- 读法句子（「没有听出内容」「已转文字」进 detail 等）待人实听
- 文件行数：`td.rs`、`fake.rs`、`backend.rs` 加了 `file_download`（各 +10–20 行），`td.rs`/`fake.rs` 已在 arch 的超标基线里——请 arch 看是否接受
