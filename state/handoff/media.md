# 交接 · 手下 media · #4 听语音 + #59 语音转文字

> 2026-11-13 · 交代 MAIN `_tmp/20261113-b18/media.md` · 大总管 `telegram-accessibility-dd` · arch `arch-4d` · 分支 `feat/4-voice`

## 做到哪

- [x] exp007 解码、exp008 端侧转写（含 `.app` 方式授权补测）、设计 `docs/experiments/exp008-local-stt/design.md`（大总管与 arch 已定）
- [x] atg-review 自审（第一次误审了 src/，结果已转大总管开 #65；第二次审本人文档，P1×3 已改）
- [x] codex 017：13 轮收敛（第 12、13 轮无 P0/P1），P1×17 / P2×22 全部属实并改，无剩余 P2。记录 `docs/reviews/017_exp007-008-codex.md`
- [x] PR1 下载（实体：`FileRef` · `file_download` · `download` 子命令 · voice 组合模块）：编译测试全绿（2026-11-13）
- [~] PR2 播放：`atg-audio`（decode / player / queue）· `Doc::Playback` · `play` · `mark_listened`（+ `Message.self_destruct`）：编译测试全绿（2026-11-13）；PR 描述草稿 MAIN `_tmp/20261113-b18/pr-media.md`
- [ ] PR3 转写：`Transcript` 与表 · stt 与辅助程序 · `transcribe` · `stt_unavailable`
- [ ] codex 018（代码）

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
