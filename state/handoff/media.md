# 交接 · 手下 media · #4 听语音 + #59 语音转文字

> 2026-11-13 · 交代 MAIN `_tmp/20261113-b18/media.md` · 大总管 `telegram-accessibility-dd` · arch `arch-4d` · 分支 `feat/4-voice`

## 做到哪

- [x] exp007、exp008、设计 `docs/experiments/exp008-local-stt/design.md`（大总管与 arch 已定）
- [x] codex 017（实验 + 设计）13 轮收敛
- [x] PR #85（下载 + 播放，8ba74c7，大总管选合成一个）：代码、契约、测试全绿；atg-review 自审（P1 1 条已修）；codex 018 三轮收敛（第 2、3 轮无 P0/P1）；超线文件全比 main 短（arch d 方案）。PR 草稿 MAIN `_tmp/20261113-b18/pr-media.md`，大总管提交并开 PR
- [ ] 合 main（#52 发送，PR #90）进 feat/4-voice：10 个冲突已解（两边功能都留；Submit 进 fake/call.rs、td/tests.rs 换成 main 版；超线文件仍全比 main 短）；全量 test、三遍 clippy（含 tdlib-api 与 atg-cli/tdlib）、fmt、check-no-debug 全绿（#79 偶发一次，重跑过），名额已还；大总管已提交推送，PR #85 等人点合并
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
- [ ] PR3 拆三段（大总管定）：3a 底座（codex 024）→ 3b 进树（025）→ 3c 命令（026，租约模块先给 arch 过目）。3a 草稿在 scratch `pr3/`：stt.rs（加了 Drop 守卫、坏协议立即收尾、可注入期限）、stt-tests.rs、fake-helpers（hang/no-hello 改 exec，加 stderr-secret）、build.sh（path 只认含 atg-stt 的目录）。已搬进 feat/59-stt-base 并领名额编过：atg-audio 21 过、真模型 0.84 秒判 zh-CN、clippy/fmt/门槛全绿。补了「父进程死后辅助程序 1 秒内退出」ignored 测试（未跑，下次名额）。atg-review 自审无 P0/P1、P2×9 全改并复验（含两条真辅助程序测试：中文判 zh-CN、父进程死后 1 秒内退出）；codex 024 第 1 轮 P1×3 P2×1 全属实已改（Phase 单一赢家、Swift 错误统一分类、--parent-pid + hello 在兜底线程之后、deaf 夹具），领名额复验全绿（24 过、三条真辅助程序 ignored 过、clippy/fmt/门槛过），记录 docs/reviews/024_stt-base-codex.md；第 2 轮 P1×1 P2×2 全属实已改（计时起点挪到起子进程前、父进程测试改 exec 3<&0 并断言无结果行、设计改成各语种各自一个分析器），复验全绿；第 3 轮 P1×1 P2×2 全属实已改（看门/主线程共用判据 overdue + Phase::settle、总期限测试用 SLOW_HELLO、新增真辅助程序单边失败测试），复验全绿；第 4 轮 P1×1 P2×2 全属实已改（退出码分支也走 settle(due())、单边失败测试断言原始协议、Swift 兜底改单调时钟、真辅助程序测试搬到 stt/tests/real.rs），复验全绿；第 5 轮 P1×1（握手期限竞态，又是终态判定 / 计时这一处），停下交人裁决，人定丙：已做甲（握手时刻在 phase 锁里记、overdue 按时刻判、看门线程判与定同锁），规则乙记进 024，复验全绿（28 过）；第 6 轮通过、第 7 轮仅 P2×1 → 满足停止线；P2（stderr-marker 夹具验证不到退出码检查）请大总管开 issue；PR 正文草稿 _tmp/20261113-media/pr-3a-body.md，等大总管开 PR
