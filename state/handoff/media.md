# 交接 · 手下 media · #4 听语音 + #59 语音转文字


## b20 接手（2026-11-16）

> 交代 MAIN `_tmp/20261116-b20/media.md` · 大总管 `allenchen-37` · 分支 `feat/59-transcribe`（e77a200 WIP + 9 个文件未提交）

- 手上：3c `transcribe` 命令（codex 039）。旧进度：全部写完、**从未编译**；下一步本是领名额编译 → atg-review → codex 039
- 未提交 9 个文件（+44/−28）：`stt_unavailable` 按 `what` 分四种修法提示、`FakeBackend::settle` 改 pub 做测试屏障、build.sh、transcribe 外壳收窄、mask/media/transcription 小改。看着是收尾小修，完整性要编译才知道
- lease.rs 对 `origin/feat/59-lease`（PR #106）：本分支多出 `Guard`（转写用）+ 注释两处，其余同一份；即 #106 + 3c 增量
- 合 main 预计冲突（`git merge-tree` 试算）：`docs/reviews/README.md`、`scripts/file-size-baseline.json`、`specs/contracts/{actions,cli,errors}.md`、`src/atg-cli/src/watch.rs`。自动合并：args/main/send/serve/session/watch/tests、error.rs、config-privacy.md
- 未提交的 `error.rs`、`fake.rs` 与 main 也改了同文件（不同段）：合前需先把 WIP 提交，否则 git merge 拒绝
- Touch ID 删除对我的影响：我新写的 transcribe 代码不用被删的东西；本分支改过的 send.rs（`send::remember` 抽出）、session.rs、serve.rs、args.rs 与 main 删确认的段落相邻，合后要编译核对
- 大总管已提交 WIP 0b038d9 并 merge origin/main；我解了 6 处冲突（未 add、未 commit）：README/actions/cli/errors 两边条目都留、按日期排；watch.rs 留 main 的 `mod end`，`close_chat` 不要（已由 `lease::close` 取代）；基线 --tighten（watch 837、speech 825，notify.rs 已不超线）；watch/tests.rs 合后 601 行，删掉第 2 行空行回到 600。check-file-size 0 报错
- 确认逻辑残留：我改过的 send/session/serve/args 对 main 的差异里没有任何确认、td_backend_with（transcribe 用 `td_backend()`）。仅剩 main 自己的 `src/atg-cli/README.md:21` 还写着 Touch ID（不是我的线，报大总管）
- 合并已由大总管提交 829d0eb；名额排第二（fix 之后）。README.md:21 不归我
- 静态通读 voice/transcribe.rs、transcribe.rs：引用的 API（stt::locate、Hub::leases、Feed::drain/pump_mut、store 凭据与裁决、Transcription::hidden）都在；SttError 是 Copy
- 名额到手后的测试计划（一次领、连着跑）：① `cargo build --workspace` + `-p atg-cli --features tdlib` ② `cargo test --workspace`（重点 atg-audio stt/locate 5 条、voice/tests/transcribe 7 条、transcribe/tests 5 条、watch/tests/hub、serve/tests/lifecycle、atg-core privacy/coverage）③ clippy 三遍（默认、tdlib-api、atg-cli/tdlib）④ fmt --check、check-no-debug、check-file-size ⑤ 真辅助程序 ignored 测试一遍（`stt/tests/real.rs`，不出声、不弹窗）
- 名额跑完（2026-11-16）：build（全工作区 + atg-cli tdlib）过；test 第一次编译错 1 处（transcribe/tests.rs 少引 `Outcome`，已补）→ 517 过 0 败 5 ignored；clippy 三遍过；fmt 改了 error.rs 一处排版；check-no-debug、check-file-size 过；stt-helper build.sh 编出 `_build/20261116-161819/atg-stt`，atg-audio ignored 5 条过（含真辅助程序 4 条，静音、未弹窗）。名额已还
- 大总管已提交 b061316（tests 补引、fmt、watch/tests 空行、现状页）
- atg-review（wf_2a8176f3-c56）：无 P0/P1；P2×5 全属实已改（ATG_STT_HELPER 只认绝对路径 + 测试；helper_unsafe / unavailable 提示；what 码正本 `atg_audio::stt::what::ALL` + out.rs 三处比对测试；transcript_stale 注明可重试例外）；P3 改 6 条（cli.md 行序与 redact 句、模块头、lease 文档、content-tree 类型名、引用分组）。未编译，已申请名额复验
- 留着的 P3（建议攒一个 issue）：`send::remember` 挪共用模块；`Outcome` 改名；`Guard` Drop 写死 Stdio；L0/F4/R1/场景目录等代号；build.sh 注释术语
- 已定（大总管 2026-11-16）：`transcript_stale` 不挪、按注明处理；P3 五条随 PR 开一个 issue；名额排第四（arch → msg2 → fix → 我）
- codex 039 干跑过：`_tmp/reviews-039/`，类型 diff、范围 origin/main（main 已在 HEAD 里）、隐私扫描无拦截（13 条测试假数据提示）
- codex 039：第 1 轮 P2×2（测试靠时序、租约测试不全）属实已改：闸门式假辅助程序、18+ 途中变化测试、守卫共存 / panic 测试，删 ok-zh-slow.sh；第 2 轮 P2×2（下载闸门、假后端没开不推，都要改超标的 fake.rs）→ 满足停止线，开 issue。记录 docs/reviews/039_transcribe-codex.md
- 复验（名额，已还）：test 520 过 0 败；clippy 三遍、fmt、门槛过。守卫测试并行时偶发 auth_required（没等授权就绪）已修。已有偶发：serve lifecycle `two_watches_on_same_chat_share_one_open` 13 次挂 1 次（#106 那条，非 3c）
- 大总管已提交 a7a1a6e、开 PR #125（先合 #106）；issue 甲 #122、乙 #123、丙 #124
- #121（serve `two_watches_on_same_chat_share_one_open` 偶发）读代码分析（未编译）：
  - 现象：同步环里每个探针接入时报「现有 3 个」直到 10 秒期限
  - 原因（推断）：`serve` 主循环只在 `accept(POLL)` 返回、`handle` 做完**之后**才 `hub.reap()`；`watch 接入（现有 N 个）` 数的是 `hub.len()`，含已走但还没摘的。上一个探针停掉后，若下一个探针在服务端这一轮 `accept(200ms)` 超时前就连上，它先被接入、数里带着上一个死探针 → 3；只有两次接入间隔超过 `serve::POLL`（200ms）、中间转过一圈空轮才会摘掉。测试只睡 50ms，一轮约 100–200ms，卡在边上：慢机器 / 并行时可能连续 10 秒都不超 200ms
  - 修法（建议甲）：产品代码在接入新连接前先摘死订阅者（Conn 分支里 `handle` 之前 `lease::close(b, feed.hub.reap(), …)`），「现有 N 个」与租约放回都不再等空轮；测试去掉 50ms 等待环，探针一次即判。乙：只改测试，睡过 POLL（仍靠时序，不推荐）
  - 复现计划（要名额）：把测试里的 50ms 改 0 应几乎必挂、改 300ms 应必过，确认推断；再按甲修、连跑 20 次
  - 待大总管定：修在 #106 分支（4-media-lease，serve.rs 迁移与这条测试都来自它）还是本分支
  - **更正（2026-11-16）**：#106 分支早已由 06803cb 修过（执行方记「watch 离开（现有 N 个）」、测试按事件等、去掉探针与 sleep）；本分支叠在旧 #106（a5b4786）上，没有 06803cb 等 4 个提交。建议不改代码，由大总管把 origin/feat/59-lease 合进本分支（预计 serve.rs、lifecycle.rs 冲突，取 #106 版），合后领名额复验并连跑 20 次。已报大总管，等回复
  - 大总管合 origin/feat/59-lease 进本分支（9ce7840，serve.rs 冲突我解）。复验（名额，已还）：test 520 过 0 败、clippy 三遍、fmt、门槛过；two_watches 单跑 20/20、整套 20 次未再挂 → #121 已修
  - 新见偶发（非 3c，main 原有）：`serve_replaces_stale_socket_and_refuses_busy_profile`「关掉后锁放了」整套 20 次挂 1 次；建议另开 issue，已报大总管
- 下一步：等大总管推送 #125、定要不要我查新偶发

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
- [ ] PR3 拆三段（大总管定）：3a 底座（codex 024）→ 3b 进树（025）→ 3c 命令（026，租约模块先给 arch 过目）。3a 草稿在 scratch `pr3/`：stt.rs（加了 Drop 守卫、坏协议立即收尾、可注入期限）、stt-tests.rs、fake-helpers（hang/no-hello 改 exec，加 stderr-secret）、build.sh（path 只认含 atg-stt 的目录）。已搬进 feat/59-stt-base 并领名额编过：atg-audio 21 过、真模型 0.84 秒判 zh-CN、clippy/fmt/门槛全绿。补了「父进程死后辅助程序 1 秒内退出」ignored 测试（未跑，下次名额）。atg-review 自审无 P0/P1、P2×9 全改并复验（含两条真辅助程序测试：中文判 zh-CN、父进程死后 1 秒内退出）；codex 024 第 1 轮 P1×3 P2×1 全属实已改（Phase 单一赢家、Swift 错误统一分类、--parent-pid + hello 在兜底线程之后、deaf 夹具），领名额复验全绿（24 过、三条真辅助程序 ignored 过、clippy/fmt/门槛过），记录 docs/reviews/024_stt-base-codex.md；第 2 轮 P1×1 P2×2 全属实已改（计时起点挪到起子进程前、父进程测试改 exec 3<&0 并断言无结果行、设计改成各语种各自一个分析器），复验全绿；第 3 轮 P1×1 P2×2 全属实已改（看门/主线程共用判据 overdue + Phase::settle、总期限测试用 SLOW_HELLO、新增真辅助程序单边失败测试），复验全绿；第 4 轮 P1×1 P2×2 全属实已改（退出码分支也走 settle(due())、单边失败测试断言原始协议、Swift 兜底改单调时钟、真辅助程序测试搬到 stt/tests/real.rs），复验全绿；第 5 轮 P1×1（握手期限竞态，又是终态判定 / 计时这一处），停下交人裁决，人定丙：已做甲（握手时刻在 phase 锁里记、overdue 按时刻判、看门线程判与定同锁），规则乙记进 024，复验全绿（28 过）；第 6 轮通过、第 7 轮仅 P2×1 → 满足停止线；P2（stderr-marker 夹具验证不到退出码检查）请大总管开 issue。**3a 已由大总管提交、开 PR #93**；测试缺口开 #94
- [ ] 3b 进树（codex 025）：在 feat/59-stt-base 上接着写（#93 合并后大总管从新 main 开 3b 分支或快进）。PR 草稿以后放主仓 `_tmp/20261113-b18/`。store/mod.rs、tree/mod.rs、speech.rs 已超 600 行（棘轮），新代码进新文件。方案 `主仓 _tmp/20261113-b18/media-3b-plan.md`；人定凭据 = 消息 id + 语音文件 id（#59 设计变更）、删和迁收成 apply 末尾整理；大总管同意转写挂消息 `transcript`（JSON `data.transcript`），不挂 voice 附件。草稿已写：tree/media.rs（Transcript）、tree/status.rs（搬出）、store/ticket.rs + transcripts.rs、speech/media.rs（dur 搬入 + 读法）、mask.rs 遮 18+、redact、tests/transcripts.rs、privacy.rs、design.md、content-tree.md、基线 --tighten。已编译全绿；atg-review 自审 P2×8 全改（表键改按现在的 id、整理时挪键；18+ 详情也不说；模块名对齐 010：speech/content.rs、tree/top.rs）；大总管提交到 `feat/59-transcript`（b2c1b2b、d205102）；codex 025 第 1 轮仅 P2×3 全改、第 2 轮通过 → 满足停止线。PR 草稿在主仓 `_tmp/20261113-b18/media-pr-3b.md`
- [x] 租约小 PR（codex 026，PR #106）：`atg-cli/src/lease.rs` + Hub / serve / 直跑 watch 迁移；自审 P2×4、codex 两轮各 P2×2 全改，满足停止线。CI 慢机器上 serve 同会话测试挂过：探针同步点靠时序，改成执行方记「watch 离开（现有 N 个）」按事件等（临时 worktree `atg-wt/4-media-lease`，待名额连跑 10 次）
- [~] 3c `transcribe` 命令（codex **039**，分支 `feat/59-transcribe` 叠在 #106 上）：计划 `主仓 _tmp/20261113-b18/media-3c-plan.md`。已定：退出码 11「本机缺能力」（arch）、18+ 不加 `--reveal` 照转但输出 `hidden`（大总管）、辅助程序查找加固（arch：环境变量只认它、仓库根要认人标记、权限校验）。已写未编译：`Doc::Transcription`（tree/json/speech/mask、content-tree.md）、`CoreError::SttUnavailable`（errors.md、cli.md 退出码表、out.rs）、`stt/locate.rs`（+5 测试）、`lease::Guard`、`voice/transcribe.rs`、`transcribe.rs` 外壳、args/main/session、`send::remember` 抽出共用、测试（`voice/tests/transcribe.rs` 7 条、`transcribe/tests.rs` 5 条、cli.rs、privacy.rs）、actions.md、design.md。下一步：领名额编译测试 → atg-review → codex 039
