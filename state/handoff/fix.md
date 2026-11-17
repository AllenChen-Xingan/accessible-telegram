# b20 接手（2026-11-16）
- 手上：先收第四批 #97 #98 #100 #40（c281455 + 自审修复未提交未编译）；再做发送闸 #77 #80 #83 #109（直发是唯一方式，改 gate/** send/** 前先报大总管，msg1 在同片做 #57 #88）
- 做到哪：已向大总管 allenchen-37 报到。合 main 冲突预测：docs/reviews/README.md、scripts/file-size-baseline.json、specs/contracts/cli.md、errors.md、本页
- 2026-11-16 大总管提交 WIP 8ecdea6、merge origin/main；我解了 5 处冲突（README/errors/baseline 取 main；cli.md 正文取本分支含 #98 的一句、变更记录两边都留；本页取本分支），未 commit。codex：第四批 045，发送闸组 048
- 2026-11-16 复跑全过（468 过）；codex 045 第 1 轮 P1×3 属实已改（secret 容量区按 MaybeUninit 清、signals 加 TTY_LOCK + 线程屏蔽、config on_missing 再读标记），P2 #4 → issue #116；记录 docs/reviews/045_*（第 1 轮）。未编译
- 大总管定：#83 等 msg2 #9 合后加 old_id；#80 交出前到点报 network 属技术细节；#109 交 msg1 在 #57 修；发送闸组等 #9 #57 合 main 后另开分支，现在只写测试计划与契约改法
- 2026-11-16 名额复跑 470 过，大总管提交 1ac69f4。codex 045 第 2 轮（改名 045-batch4-main，范围 origin/main...HEAD）P1×2 属实已改（sigaction 两信号都挡；锁里 TCSANOW + tcflush），P2 = #116 契约写例外。未编译
- 2026-11-16 复跑 470 过，大总管提交 72042e2。codex 045 第 3、4 轮通过，达停止线。PR 草稿 MAIN/_tmp/20261116-b20/pr-fix-97-98-100.md。第四批交大总管开 PR
- 2026-11-16 大总管合 origin/main（#117 #110）、开 PR #126；我解 config-privacy.md 变更记录冲突（两行都留）；CI clippy 挂在 tests/config.rs 旧助手 temp_root → 改用 testkit::temp_dir（config.rs 单测同改），复跑 473 过，大总管已提交推送，等 CI
- **第四批已收尾**（PR #126 待人合并）
- 现在：待命。下一步：等 #9 #57 合 main → 大总管从新 main 开分支 → 发送闸组 #77 #80 #83（codex 048；`token: ConfirmToken` 隐私扫描误报按 common.md 第 15 条报大总管核准再 --force-secrets）
- 2026-11-17 换工作副本 atg-wt/fix-gate（分支 fix/77-80-83，从 main 266f5f3）。#109 已由 msg1 在 #134 修掉。按新 main 复核清单已报大总管：#77 不改 verify/before_submit 签名（again 按 digest 记在闸里、每次 verify 覆盖）；#80 照 react_within 口径给 submit_within 加 handed；#83 Gated 加默认 log_items、DirectLogLine 加 old_id（放 reaction 之后，与 msg2 的 from_chat_id 相邻）、加 after_item。等大总管放行
- 2026-11-17 放行开工。#77 #80 #83 代码 + 测试 + 契约写完（16 文件），未编译；已申请名额。注意：测试与修法同时写，没在修前实测红
- 2026-11-17 复跑 574 过；会红验证：5 红 2 绿（2 绿是交出后到点的回归测试），git diff 与存档补丁 cmp 一致。大总管提交检查点 8dcf473
- atg-review（3 视角，范围 origin/main...HEAD）：P0/P1 无；P2×7（多为重复）+ P3×9 全改：again 表只记「进防重复 + 带 --again」、不带的删条目；hit/claim 共用 latest_hit；留痕标记改 `not_resent`（与 CLI 同名，注明不是错误码）；handed 注释写清 block_on_timeout 只在 await 看期限；契约 send_text/resend 两行与 cli resend 行补 #80、errors network 行补、30 秒从验凭据算、--again 规则挪到放行条、testing.md:82 留痕行数、变更记录来源统一。未编译
- 下一步：名额复跑 → 提交 → codex 048 → PR
- 要人定的事：无

# fix 现状页（批次 b18）

## 第一批（已合 PR #76）
第一批：#68（置顶 / 投票隐藏内容补「按 D 展开」）+ #70（链接部分剧透，读法整条遮）。分支 fix/68-70-speech。

## 做到哪
- 共用判定 `tree::text::link_hidden`（mask / links_marked / speech 三处都调它，规则只写一处）
- speech：`service()` 回传隐藏标记，置顶带内容的给按键提示；投票题目隐藏也算进提示；`span` / `full_span` 先判 link_hidden 整条遮；「隐藏内容，？」→「隐藏内容？」（V22，规约 message.md 第三节）
- 行数守则：speech.rs、tree/text.rs 行内测试挪到 speech/tests.rs、tree/text/tests.rs（speech.rs 900→~851，text.rs 1058→~671）；arch 门槛先合的话要 --tighten
- 夹具 hidden.json 加 3006（url 末段剧透）、3007（text_url 部分剧透）、3008（置顶 3007）；新测试 partly_hidden_links_agree_across_outputs
- 规约 specs/style/message.md 加例 M17b/M17c/M17d

- 名额跑过一次（已归还）：fmt ✓ · clippy ✓ · test -p atg-core 215 过 · check-no-debug ✗（见下）
- atg-review 一轮：P1 快照未更新（已修）；P2/P3 已修 message.md 正例与旧说明、hide 改名、投票隐藏改走 spans；content-tree.md 未同步（不在可写清单，报大总管）
- 自审修改后的小改（poll 判定走 spans、改名）还没编译过，下次领名额一并跑
- codex 020 第 1 轮在跑（_tmp/reviews/）

## 手上（2026-11-13）
第二批 PR B：#63 #73 #78 #79。分支 fix/63-bounded-waits（从 main 9b192a5，不含 #61——#61 是 PR #91），codex 编号 022。

## 做到哪（PR B 第一段：link + login + watch，没编译过）
- 重盘：比原清单多 send.rs 4 处、voice 3 处（大总管定并入第二段）；core 3 处有意无期限（td.rs 交出线程、link blocking_batch、example）标 allow
- arch 定：文件 `link/block.rs`（block_on / block_on_until / block_on_timeout 放一处），结果类型叫 `Until`（core 已有 send::Waited）
- `block_on_until`：每次轮询前先看 stop（叫停后不发新请求），每片 ≤100 ms
- `atg-cli/src/deadlines.rs`：AUTH/SYNC/LOAD/QUERY/START/STEP/AUTH_POLL/TOPIC/ME/FRAME + `within`（看 stop）/`bounded`（不看 stop，收尾用）/`stalled`
- 新取值：START_TIMEOUT 30 秒（人可推翻）、QUERY 10 秒（大总管定按 LOAD）、DOWNLOAD 120 秒（第二段，大总管定）
- login：每步提交与等授权线共用截止时刻；wait_line 每片 ≤200 ms 看 stop；drive 带 stop；main run_job 装 signals
- watch：start / chat_get / chat_open / chat_close 用 bounded（不看 stop，Ctrl+C 仍走结尾 result）；fetch_topics 带 stop
- 测试：login/tests.rs + tests/{bounded,interactive,stepwise}.rs；watch/tests.rs；#79 暗号「丙丁」只查 data.message 与 speech；新测 link/block/tests.rs 5 条、login bounded 6 条、watch 话题 2 条
- 假后端：`FakeBackend::HANG` 注入一直不应答；auth_call / ready_or_401 合一个 `injected`（fake.rs 1155 → 1153）
- 测试放行：lib.rs / main.rs `#![cfg_attr(test, allow(clippy::disallowed_methods))]`；core 两份集成测试文件头 allow
- 其余 14 个函数暂标 expect（serve 3、chats 1、history 4、send 3、voice 3）
- 名额已申请

## 下一步
编译 → 第一段交大总管提交 → 第二段（chats/history/serve/send/voice 收口、expect 清零）→ atg-review → codex 022 → 期限对照表进 PR 描述

## 大总管定（PR B，2026-11-13）
- C 三处 allow；A（send）B（voice）并入第二段，撞车由大总管排；查询类 10 秒；DOWNLOAD_TIMEOUT 120 秒，超时报 network，Ctrl+C 照旧能停

## 要人定 / 等大总管
- docs/reviews/README.md 的 020 索引行（不在我的可写范围）
- 「隐藏内容」后问号不加逗号，等实听确认

---

# 第二批修法设计（#61 + #63 + #73）· 2026-11-13 · 只设计，未动代码

依据：issue #61 #63 #73；MAIN `docs/reviews/007_tdlib-16-codex.md:13`、`008_login-chats-17-18-codex.md:11-23`、`012_watch-notify-53-codex.md:11-13`；契约 `config-privacy.md:33`、`errors.md:28`、`cli.md:128`。行号按 main 9d2e459，#52 合并后开工前重核。

## #61 档案缺标记被补写
- **`config.rs:173` `Profile::create`**：`MarkerError::Missing` 分支只在「新档案」时写标记。新档案 = `db/` 不存在或为空，且 `files/` 不存在或为空（档案目录本身刚被 `ensure_private_dir` 建出来也算）。否则报 `internal{what:"profile_marker_missing"}`，不写任何东西。判空用 `read_dir().next().is_none()`，读目录出错按「不是新的」处理，宁可拒绝
- **`session.rs:51` `open_profile`**：`Err(_) if main` 收窄成只接 `profile_marker_missing`。`unreadable` 等别的错误原样报，不再转 `create`。配合上一条，缺标记但已有库时报 missing，不补写
- **测试**（`atg-core/tests/config.rs`）：
  - 已有非空 `db/`、缺标记 → `create(Test)` 和 `create(Main)` 都报 missing，标记文件仍不存在
  - 空目录或全新目录 → 写标记
  - 标记读不懂 → unreadable
  - 再加 session 层：正式档案缺标记，跑 `--test-dc` 不会被改标成测试档案
- `profile_marker_missing` 已在 `errors.md:35` 的 `what` 列表里（作为 `internal`），不用改契约

## #63 login / auth 请求无超时、Ctrl-C 不收尾
**共用能力（新增，要先报 arch，守则第 4 条）**：在 `atg-core/src/link.rs` 加
`block_on_until(fut, deadline: Instant, stop: &AtomicBool) -> Waited<T>`，`Waited` 取 `Done(T)` / `TimedOut` / `Stopped` 三种。
- 实现：`park_timeout` 每片最多 100 ms，醒来先看 `stop`。信号不会唤醒 park，所以只能切片
- 现有 `block_on_timeout` 改成调它（`stop` 传一个永远是 false 的标志），`chats.rs:176`、`td.rs:610` 行为不变
- 超时或停下时直接丢掉 future，有先例 `chats.rs:176`（迟到的应答在 link 那边被丢弃）。开工时再核一遍 link 对已丢弃请求的处理，不能 panic，也不能留下挂着的 waker

**login.rs**：
1. 每一步提交和随后的等状态共用一个截止时间：`deadline = now + STEP_TIMEOUT`（30 秒）。涉及 `submit`、`register`、`resend`（:244 / :336）、`logout`（:385），全改走 `block_on_until`。超时报 `network`（沿用 `stalled()`，内核故障时报 `disconnected`）
2. `advance` / `settle` 的 `wait_auth*`：改成按剩余时间切片调用，每片 ≤ 200 ms，片间看 `stop`。`Backend` 接口不改
3. Ctrl-C：
   - `main.rs` 在 login、`auth *`、logout 这几条路上也调 `signals::install()`，现在只有 watch / serve 调
   - `drive` 把 `&signals::STOP` 传给 login 函数；单测传自己的 `AtomicBool`，不碰全局
   - 停下 → `Fail::Interrupted`（已有，退出码 130、`interrupted`），`drive` 照常 `shutdown`，出结尾行，清运行标记
   - 第二次 Ctrl-C 沿用 signals 的 `_exit(130)`
   - 读秘密时 `tty.rs` 关掉了 ISIG，Ctrl-C 走 `ReadEnd::Interrupted`，已有，不变
4. `drive` 里的 `block_on(b.start(profile))` 不在本 issue 范围；同类问题记一条 P2 给大总管
5. **测试**：
   - 假后端加「请求永不应答」的开关，验证 submit / resend / logout 在截止时间后报 `network`，并且 `shutdown` 被调用（测试里用短时限，不真等 30 秒）
   - 在另一线程把 stop 置真，验证结果是 `Interrupted`，并且已收尾
   - 伪终端端到端（cli.md:128）可选，看名额再定

## #73 watch 取论坛话题无期限
- `watch.rs:841` `fetch_topics(b, feed)` 加一个参数 `stop`；`watch::run` 和 `serve.rs:156` 都已经持有 STOP，直接传进来
- 每个话题用 `block_on_until(.., now + TOPIC_TIMEOUT, stop)`：
  - 超时：走现有的「取不到」分支，标准错误记 `network`，本次订阅不再取这个话题
  - 停下：剩下的话题留着不取，直接返回，让主循环去收尾
- **测试**：假后端话题请求挂住 → `fetch_topics` 在时限内返回、记了错误码；stop 置真后立即返回

## 行数守则（login.rs 1077 · watch.rs 1343 都在基线里，只许变小）
- **login.rs**：行内测试 :393–1077 约 685 行，挪出后超过 600 行，要按主题拆成 `login/tests.rs`（公共夹具 + `mod interactive; mod stepwise;`）、`login/tests/interactive.rs`、`login/tests/stepwise.rs`。check-no-debug 这种布局已在 #76 支持。正文剩约 390 行，加完新代码仍会远小于 1077
- **watch.rs**：测试 :874 起约 470 行，挪到 `watch/tests.rs`
- 合并后对这两个文件跑 `check-file-size.py --tighten`

## 要大总管 / 人定
1. **新共用函数 `link::block_on_until` 及 `Waited` 类型**（守则第 4 条，需 arch 看过）。建议：放 `atg-core/link.rs`，与 `block_on_timeout` 同处，后者改成它的薄包装
2. **`TOPIC_TIMEOUT` 取多少**：A 5 秒（serve 最多被拖 5 秒/话题，建议）· B 10 秒 · C 改成异步待办（不阻塞主循环，改动大，留作后续）。代价：时限太短，弱网时话题设置取不到，那条消息按 `topic_unknown` 不提示（宁可不吵，现有取舍）
3. **分支怎么开**：#61、#63、#73 一个分支一个 PR（同属「请求期限与收尾」，login.rs 和 link.rs 是一个人改），还是拆两个（#61 单独，改动小、风险低）。建议拆：#61 单独先出，#63 + #73 一起
4. `drive` 里 `start` 无期限、`chat_open`（watch.rs:782）无期限：同类，是否另开 issue

## 大总管定（2026-11-13）
1. `block_on_until` 放 `atg-core/link.rs`，`block_on_timeout` 改成薄包装，「有期限的等待」只此一处正本——**等 arch 回复再动**
2. `TOPIC_TIMEOUT` = 5 秒
3. #61 单独一个 PR；#63 + #73 + #78 一个 PR（Closes #63 #73 #78）
4. #78：全仓的无期限 `block_on` 一次清干净，加门槛或测试防止再出现（放哪和 arch 商量）
开工条件：#52 合并通知

## #78 盘点（main 9d2e459，非测试代码里的 `block_on(` 共 24 处，开工前按新 main 重盘）
chats.rs:133 · history.rs:171 215 365 638 · login.rs:103 196–200 244 336 385 · serve.rs:113 207 372 · session.rs:297（start）· watch.rs:673 709 782 843 868 · link.rs:473（`block_on` 本身的定义 / 内部用）
门槛设想：check 脚本或 clippy `disallowed-methods` 禁止非测试代码调 `link::block_on`，只许 `block_on_until` / `block_on_timeout`。clippy.toml 的 `disallowed-methods` 编译期就能拦、不用自己写词法，倾向这个；待和 arch 商量

## arch 意见（arch-4d，2026-11-13）
1. `block_on_until` 放 link.rs 可以；加完 link.rs ≤ 600 行（现 519），超了挪 `link/wait.rs` 再重新导出，不改名成 `link/mod.rs`（基线认路径）。`Waited` → `Fail` 映射只写一处，login、watch 共用
2. 门槛这批不加：check-sources.py、scripts/tests/ 已是门槛文件（改要 baseline:loosen）；裸 block_on 做成按文件计数的棘轮，由 arch 在本批合并后加。本批只要新代码不加裸 block_on
3. check-no-debug 认 `<模块>/tests/<主题>.rs`：#76 已做
**与大总管 ④ 有出入**（大总管：#78 全仓清干净 + 加门槛；arch：门槛归 arch，棘轮意味着旧的可以分批清）→ 已报大总管定

## 大总管定 A（2026-11-13）
- #63 + #73 + #78 这批把 24 处裸 block_on 全换成有期限的等待，#78 一次关掉；门槛归 arch 后加，棘轮起点 0
- 拆两个提交（提交由大总管做，我按两段交付）：第一段 login / watch（+ link 的 block_on_until）；第二段 chats / history / serve / session
- 每处期限值与超时说法收成一张表进 PR 描述
- **期限常量集中一处定义**（不散在各文件）：设想放 `atg-core/link.rs`（或 `link/wait.rs`）旁的一个 `deadlines` 小节，开工时先报 arch 定落点

## arch 第二条意见（2026-11-13，可能早于大总管转告定 A）
- 门槛改用 clippy `disallowed-methods`（根目录 clippy.toml 禁 `atg_core::link::block_on`）；`block_on_until` 内部 `#[allow]` 写明理由；测试模块整体 allow
- 现有未修处标 `#[expect(clippy::disallowed_methods, reason = "待收口：#issue")]`（修好后 expect 自报警告 → CI 红 → 逼删）
- arch 说 history / serve / session / chats 由 arch 第二段收——**与大总管定 A（本批全清）冲突**，已报大总管
- clippy.toml 是 arch-guard 管的门槛文件：arch PR 先合的话，我新建它的 PR 要大总管加 baseline:loosen
按定 A 合起来的做法：本批全清 24 处 → 不需要任何 expect 标注；第一段提交就加 clippy.toml（未修的四个文件那几处先标 expect，第二段提交修完删掉），两段合完清单为 0

## 最终口径（大总管 2026-11-13，已与 arch 对齐）
- 一个 PR（#63 #73 #78）改全部 24 处；根目录加 clippy.toml（disallowed-methods 禁 `atg_core::link::block_on`，reason 指向 `block_on_until`）；`block_on_until` 实现处 `#[allow]` 写理由；测试模块整体 allow
- 第一段：clippy.toml + link / login / watch，其余四个文件暂标 `#[expect(clippy::disallowed_methods, reason = "待收口：#78")]`；第二段：修完 chats / history / serve / session、删光 expect，结尾清单 0
- clippy -D warnings 过 = 门槛生效；baseline:loosen 由大总管加；arch 第二段不再管 block_on
- 期限常量落点：问 arch

## arch 定：期限常量落点（2026-11-13）
- 产品取舍类 → `atg-cli/src/deadlines.rs` 一个文件：登录每步 30 秒（STEP_TIMEOUT）、话题 5 秒（TOPIC_TIMEOUT）、chats 的 AUTH/SYNC/LOAD、history 与 watch 两份 ME_TIMEOUT 合成一个（收掉 010 S12）、forward 的 FRAME_TIMEOUT、第二段新加的
- 内核机制类留 atg-core 原处：backend::SHUTDOWN_TIMEOUT、ipc 的 REQUEST/WRITE_TIMEOUT
- `block_on_until` 只收截止时间，link.rs 不放期限常量
- deadlines.rs 每个常量一行注释（等什么 · 为什么这个数 · 出处契约行或 issue）；别处不再写数字字面量
- 波及文件比原清单多 forward.rs（FRAME_TIMEOUT 搬家）

## 加项 #79（大总管 2026-11-13，并入 #63 #73 #78 那个 PR，Closes #79）
- 根因：`watch.rs:1058` `spoiler_hidden_unless_reveal` 断言整行 JSON `to_string()` 不含 "42"；行里有真实时间戳（at 等），时间里出现 42 就挂
- 修法：只查 `data.message`（序列化后）与 `speech`；暗号「42」换成时间里不可能出现的字（如汉字「丙丁」，与 hidden.json 夹具同风格）
- 位置：watch.rs 测试挪到 watch/tests.rs 后在那边改

**2026-11-13 第一段编译全绿、已归还名额、报大总管待提交。** 编译时改：expect 挪到 main.rs 的 5 个 mod 声明；td.rs 改用 link::block_on_guarded（pub(crate)，tdlib-api）；serve 测试挪到 serve/tests.rs，快照随之挪到 serve/snapshots/；login 测试夹具 started() 先等授权线离开「等参数」（并行偶发失败）。基线已 --tighten。下一步：等第一段提交后开第二段（删 main.rs 那 5 行 expect）。

**2026-11-13 第二段写完，等名额编译。** chats/history/serve/send 用 bounded（START / QUERY）；voice 的 find/download/fetch/run_download 加 stop 参数，下载 within DOWNLOAD_TIMEOUT 120 秒；main.rs 的 5 行 expect 删了；send 测试挪到 send/tests.rs；serve/tests.rs 拆出 tests/lifecycle.rs（第一段漏的：未跟踪文件 check-file-size 不数）。教训：第二段改完没领名额跑过一次 clippy，已向大总管认错。

**2026-11-13 PR B 收尾。** codex 022 两轮收敛（第 1 轮 P2×5、第 2 轮 P2×4；第 2 轮 #1 #3 #4 停止线后修、有测试，#2 由大总管开 issue）。全量编译测试全绿（atg-cli 138+21）。PR 草稿：MAIN `_tmp/20261113-b18/pr-fix-63.md`。待大总管提交最后一批（login 片内再看停止、ensure_ready 到点报 network、退格连组合符号删、022 记录第 2 轮、README 索引、基线）并开 PR。

# 第三批修法设计（#62 + #64 + #95）· 2026-11-13 · 只设计，未动代码

大总管派：等 PR #96 合并后从新 main 开分支，一个 PR，codex 027。下面行号按 fix/63-bounded-waits 当前版本，开工前按新 main 重核。

## #62 td.rs 收尾三处（`src/atg-core/src/td.rs`，在基线里 874 行，只许变小）

1. **`start` 先查后写的竞态**（:313 查 `run.is_some()`，:346 才写）。中间全是同步代码（拿档案锁、读凭据、钥匙串、`mark_running`、建客户端、起收线程），没有 await。
   - 修法：`run` 从 `Option<Running>` 改成三态 `enum Run { Idle, Starting, Running(Running) }`。进来时在锁里 `Idle → Starting`，不是 `Idle` 就报 `already_started`；放锁再做慢活（钥匙串可能弹窗，不能一直攥着锁、挡住 `client_id()` / `status`）；成功写 `Running`。失败时用守卫退回 `Idle`（`Drop` 里判一个 `committed` 标记）。
   - 测：真后端起不来测不了并发；把「占位 → 提交 / 退回」做成小类型，单测「占着时第二个 start 报 already_started、失败后能再起」。
2. **重复 `shutdown` 再次 `mark_clean`**（:855）。第二次 `shutdown`：`client_id()` 仍是 Some、`is_closed` 为真、跳过 close、`wait_closed` 立刻真 → 又 `mark_clean`。第一次已经放了档案锁，别的进程可能已经起来、写了自己的运行标记，被我们抹掉。
   - 修法：只在还攥着档案锁时动标记：`if r.lock.is_some() { r.profile.mark_clean(); r.lock = None; }`。用「锁还在」当作「标记还是我的」，不另加字段。
   - 测：td 测不了真关；`config` 那边已有标记读写测试。这条改成可读的不变式（注释 + 小函数 `release(&mut Running)`），在 td/tests.rs 用假 `Running` 测「放两次只抹一次」，能构造就测，构造不了就在评审记录写明。
3. **`spawn_receiver` 失败时已建的客户端没人关**（:345 `?` 直接返回）。档案锁随局部变量放掉，但运行标记已写（`mark_running`），下次启动会误报上次没收尾；TDLib 客户端留着。
   - 修法：`spawn_receiver` 出错时 ① 交一次 `close`：tdlib-rs 1.4.0 的 `send_request` 是 async fn，第一次 poll 就同步 `tdjson::send`（`lib.rs:61-68`），所以 `link::block_on_timeout(functions::close(client_id), Duration::ZERO)` 正好交出去、不等应答（没有收线程，也等不到）；② `profile.mark_clean()`；③ 再返回错误。依赖 tdlib-rs 的实现细节，注释里写明版本与行号，升级时要核。
   - 测：线程起不来没法造；只能评审核。
4. **行数**：三处净增约 25 行，td.rs 已在基线（874）。先把 `start` 里组 `set_tdlib_parameters` 的那段或 update 重序列化（`source` / `Step`）挪到 `td/` 下的子文件，腾出空间。挪什么开工时看 arch 守则再定。

## #64 login 三处

1. **用法错误时 clap 原文在标准错误回显秘密参数**（`main.rs` `parse_failed`，`eprint!("{}", e.render())`）。人误把验证码写成参数（`atg-cli auth code 40419`），clap 报「unexpected argument '40419'」，原样进标准错误（终端、AI 抓的日志）。
   - 修法：登录这组（argv 里第一个子命令是 `login` / `auth` / `logout`）不回显 clap 原文，只写「参数不对」加上 `e.kind()` 的类别和 `atg-cli <子命令> --help`；别的命令照旧（原文里有 `--foo` 这类提示，有用）。
   - 抽一个 `usage_text(e, argv) -> String`，单测：`auth code 40419` 和 `login --phone +99966…` 的 stderr 文本里不含那串值；`chats --bogus` 照旧含 `--bogus`。
   - 要定（建议）：所有命令都不回显，还是只登录组不回显？建议只登录组，秘密只会出现在这组。
2. **`set_auth` 分两次加锁**（`link.rs:182`）。先在锁里换提示、代次加一，放锁，再 `set_lines` 改授权线。窄窗口里 `wait_auth_after` 看到代次变了，拿到的却是旧步骤。
   - 修法：把 `set_lines` 的锁内部分抽成 `fn apply_lines(st: &mut State, f)`（不加锁）；`set_lines` = 加锁 + apply + 唤醒；`set_auth` 在同一把锁里换提示、代次加一、apply 改授权线，放锁再唤醒。事件（`AuthStepChanged`、`UserStatusChanged`）照旧在 apply 里发，顺序不变。
   - 测：一千次循环：记下代次 → 另一线程 `set_auth(步骤 i)` → `wait_auth_after` 返回的必须是步骤 i（修前偶发错，修后不会误报）。
3. **分步 `auth` 落到正式档案的 `wait_registration` 仍报成功**（`login.rs` `auth_step` 结尾，:405）。`errors.md:84` 要求 `login` / `auth *` 遇到 `unsupported`、`wait_other_device`、正式档案的 `wait_registration` 都以 `auth_wrong_state` 结尾、退出码 3。`login` 已经这样做，`auth_step` 只处理了测试档案的注册。
   - 修法：`advance` 之后：`WaitRegistration` 且测试档案 → `register`；`WaitRegistration`（正式）/ `WaitOtherDevice` / `Unsupported` → `Err(Fail::Unsupported(next))`（`out.rs` 已映射成 `auth_wrong_state`，退出码 3）。`login` 和 `auth_step` 共用一个 `fn unsupported(step, kind) -> Option<Fail>`。
   - 要定：`auth state` 本身是查询，现状落在这几步时要不要也报错？契约写的是「`auth *`」，字面包括 `state`。建议 `state` 也按契约报（退出码 3、`expected` 写现在那一步），让脚本一眼看出卡住了；不同意的话契约要改成「除 `auth state` 外」。
   - 测：`stepwise.rs` 加三条：正式档案提交号码后走到 `wait_registration` → 退出码 3；`wait_other_device`、`unsupported` 同。假后端要能推这几个状态（先看 `FakeAuth` 支不支持，不支持就加一个字段）。

## #95 watch 订阅前 Ctrl+C 的结尾行

- 现状：`watch_loop` 开头 `chats::ensure_ready` 失败走 `fail_line`，这时还没挂订阅者，`fail_unless_stopped` 用不上。
- 修法（最小）：`ensure_ready` 失败时先看停止标志：立起了就用临时的 `Watcher::new(cx, format, None, clock).since(started)` 写一行 `result_line()`、`out.finish(0)`、返回 0；没立起照旧报错。不挪订阅顺序（挪了会把 `status` 行和挂订阅者的次序一起变，影响转发那边的「中途接入」逻辑）。
- 测：`watch/tests/ctrl_c.rs` 加一条：停止标志一开始就立起、假后端没登录（`ensure_ready` 立刻报 `auth_required`）→ 退出码 0、最后一行 `result`；对照：不立 → `error`。
- `serve` 的 `join_watch` 不受影响（执行方早已就绪）。

## 顺序与名额
#64.2（core link）→ #62（core td，要腾行数）→ #64.1 / #64.3（cli）→ #95；编译一次领一次名额。自审 atg-review → codex 027 → 记录 `docs/reviews/027_*`。

## 要大总管 / 人定
1. #64.1 只登录组不回显 clap 原文（建议），还是所有命令都不回显。
2. #64.3 `auth state` 落在不支持的步骤时也报 `auth_wrong_state`（建议，照契约字面），还是契约改成除外。
3. #62.4 td.rs 腾行数挪哪一段，开工时问 arch。

## 大总管定（第三批，2026-11-13）
1. #64.1：**所有命令**都不回显用户输入的原文值，只报哪个参数 / 哪类错（`send --text` 的正文也是隐私，`real.md`）。一处正本 `usage_text` 统一处理，单测覆盖 login 与 send 两类。
2. #64.3：照契约字面，`auth state` 落在不支持的步骤也报 `auth_wrong_state`，与 `login` 共用判断函数。
3. #62.4：td.rs 腾行数开工时问 arch。
状态：等 #96 合并通知再开分支。

## 第三批进度（2026-11-14，分支 fix/62-64-95）
- 已写（未编译）：#64.2 `link.rs` `apply_lines` + `set_auth` 一把锁，`link/tests.rs` 千次测；#62 按 arch（td/boot.rs 挂 tdlib-api、td/lifecycle.rs 纯类型可单测）拆，td.rs 874→832；#64.1 `src/atg-cli/src/usage.rs`（所有命令不回显原文），`cli.md` 退出码 2 一行写明；#64.3 `login.rs` `dead_end` 共用（login / auth * / auth state），stepwise 两条新测；#95 `watch/end.rs` `before_status`，watch.rs 864→849，`ctrl_c.rs` 新测。
- 下一步：领名额 → fmt / clippy 两套 / test → `--tighten` → atg-review → codex 027。
- 2026-11-14 大总管已提交 3663e8a。atg-review：P1×1（dead_end 误拦正式档案等注册时的 `auth phone` 换号）已修 + 测；P2×4、P3×8 已改（cli.md #95 三处 + 变更记录、errors.md:84 补句）。待名额复跑（排在 arch、msg1 之后）→ 请大总管提交 → codex 027（重点文件 scratchpad codex027-focus.md）。
- 2026-11-14 codex 027：第 1 轮 P2×2 属实已修（d59ee6f），第 2 轮无发现，达停止线。记录 `docs/reviews/027_batch3-62-64-95-codex.md` + README 一行（待提交）。PR 草稿 `_tmp/20261113-b18/pr-fix-62-64-95.md`。状态：交大总管开 PR。

# 第四批修法设计（#97 + #98 + #100 + #40）· 2026-11-14 · 只设计，未动代码

大总管派：分支 `fix/97-98-100`（从 origin/fix/62-64-95 顶开，#107 合并后由大总管合 main），一个 PR，codex 045。serve 的 #99 等 #106 合并后另做。

## #97 两个进程同时首次建同一档案，误报标记缺失 / 读不懂（`src/atg-core/src/config.rs`）
- 两个窗口：① `write_marker` 用 `create_new` 建好空文件再写内容，对方这时 `read_to_string` 读到空 / 半截 → `unreadable`；② 对方 `read_marker` 得 `Missing` 之后、`is_fresh` 之前我方发布了标记，`is_fresh` 看到标记文件 → 不算新 → `missing`。
- 修法：
  1. **原子发布**：内容先写进 `profiles/` 下的临时文件（`.<档案名>.server.<pid>.<纳秒>`，`create_new` + 0600 + `sync_all`），再 `fs::hard_link(临时, 档案/标记)` 发布——`link` 撞上已有文件报 `AlreadyExists` 不覆盖（`rename` 会覆盖，不用）；不论成败删临时文件。临时文件放在档案目录**外**（同一文件系统），免得对方的 `is_fresh` 看见它算「不新」。
  2. `is_fresh` 把标记文件本身也算在「新」里（只有在 `read_marker` 刚说 `Missing` 之后才会走到这里，这时出现标记 = 别的进程刚建好）；随后 `write_marker` 撞 `AlreadyExists`，`open` 按落盘的标记核种类，原有逻辑不变。
- 测：`config` 测试里 8 个线程同时 `create` 同一个新档案（同种类）重复 50 轮：全部成功、种类一致；混种类：只许 `Ok` 或 `profile_kind_mismatch`，绝不出 `missing` / `unreadable`。修前能稳定复现 ① 的话附上（先写会红的测试）。

## #98 读输入期间不看停止标志 / 外部 SIGTERM 不打断读 / 第二次信号后终端不回显（`src/atg-cli/src/tty.rs`、`signals.rs`）
- 现状：`signals::install` 用 `libc::signal`，macOS 上是 BSD 语义（`SA_RESTART`），SIGTERM 来了 `read` 被内核重启，人不敲键就一直等；`read_raw` 不看 `STOP`；第二次信号 `_exit(130)` 跳过 `Restore`，终端停在不回显。
- 修法：
  1. **读之前与读的途中看停止标志**：`Tty` 读字节改成 `poll(fd, POLLIN, 100ms)` 一片片等，每片前后看停止标志（`Tty` 持有 `&'static AtomicBool`，正式用 `signals::STOP`）；立起了就 `Interrupted`。`poll` 被信号打断（`EINTR`）当作一片结束、再看标志。不改信号的 `SA_RESTART` 语义（改了会波及 `watch` / `serve` 别处的阻塞调用）。
  2. `read_raw_line` 收一个「读一个字节」的抽象：`Ok(Some(b))` / `Ok(None)`（终端没了）/ `Err(Interrupted)`；单测用内存里的序列 + 一个「读到第 N 个字节时报停止」的假源。
  3. **第二次信号前把终端放回**：`signals.rs` 加 `arm_tty(fd, &termios)` / `disarm_tty()`——原子标记 + 一份 `termios` 副本（只在没武装时写，处理函数只读，`SAFETY` 写明）；`on_signal` 第二次时先 `tcsetattr(fd, TCSANOW, 副本)`（POSIX 列为异步信号安全）再 `_exit(130)`。`read_raw` 切终端后武装、`Restore::drop` 里先解除。
- 剩下说不清的：第一次 Ctrl+C 在终端切到 `~ISIG` 之前那一瞬由信号处理立标志，现在第一片 `poll` 前就看得到 → 收尾；切好之后 Ctrl+C 是字节 0x03，照旧。
- 测：`read_raw_line` 假源停止 → `Interrupted`、缓冲区交回（读秘密那边清零）；`poll` 那一层靠管道（`libc::pipe`）当 fd 测：另一线程立标志 → 100 ms 级返回；`arm` / `disarm` 只测状态位（真发信号会把测试进程 `_exit`）。

## #100 login：内核故障后空转到 30 秒才报 `disconnected`（`src/atg-cli/src/login.rs` `wait_line`）
- 现状：`wait_auth` / `wait_auth_update` 在内核已故障时立刻回 `None`，`wait_line` 不睡，空转到期限再报 `stalled` → `disconnected`。原版立即报。
- 修法：一片回 `None` 之后看 `b.status().lines.core == Faulted` → 立刻 `Err(Fail::Core(CoreError::Disconnected))`。`stalled` 判法不变。
- 测：已有 `core_fault_mid_login_is_disconnected` 加计时断言（远小于 `STEP_TIMEOUT`，如 < 2 秒）；再补一条「等授权线途中崩溃」（`stop_once_sent` 同款：等假后端记下请求再 `crash`）。

## #40 退格删掉的密码字节没清零（`tty.rs` 退格分支 + `src/atg-core/src/secret.rs`）
- 现状：退格 `truncate` 后被删字节留在容量区；`Secret::drop` 只清 `0..len`。Ctrl+U 那支已手动清零。
- 修法（两处都做，各管一层）：
  1. `Secret::drop` 按**容量**清零：`spare_capacity_mut()` 里的 `MaybeUninit<u8>` 也 `write_volatile(0)`——以后任何 `truncate` / `pop` 进来的都兜住。
  2. 退格分支 `truncate` 之前把 `buf[start..]` 逐字节 `write_volatile(0)`（Ctrl+U 那支改成同一个小函数 `wipe`），读的过程中缓冲区里就不留删掉的字。
- 测：`secret` 单测里对 `Vec` 先 push 再 `truncate`、交给 `Secret`、丢掉前用裸指针读不了已释放内存——改测 `wipe` 与「drop 前把容量区清零」的函数本身（抽 `fn zero_all(v: &mut Vec<u8>)`，测调用后 `spare_capacity` 全 0）；退格测：读「ab 退格」后 `buf.as_ptr()` 的第 2 个字节（容量内）为 0。

## 顺序与名额
#40（core secret + tty 退格）→ #100（login）→ #98（tty / signals）→ #97（core config）；编译一次领一次名额。自审 atg-review → codex 045 → 记录 `docs/reviews/045_*`。

## 要大总管 / 人定
- 无。#98 的「`poll` 切片、不改 `SA_RESTART`」与「第二次信号先放回终端」属技术细节，按上面做；有异议再改。

## 第四批进度与实际做法（2026-11-14）
- 大总管已提交 c281455。实现与上面设计的出入（以这里为准）：临时文件名 `<pid>.<序号>`（同进程多线程不重名，设计写的纳秒会撞）；`read_raw_line` 沿用 `Read`，源交出 `ErrorKind::Interrupted` 即「停止标志立起」（`Polled` 文档写明约定）；清零正本 `atg_core::secret::wipe`，`Secret::drop` 用私有 `wipe_all` 按容量清，tty 的 `wipe_from` 调 `wipe`；#100 测试是两条新测（等之前已故障、等的途中故障），没改旧测。
- atg-review：P1×1 属实——macOS 的 `poll` 不支持 `/dev/tty`（一调就 `POLLNVAL`、不等；本机伪终端实测，`select` 按期等满 300 ms），切片形同虚设；改 `select`，补真终端测试（无控制终端时跳过）。P2×5（「内核」两义、`is_fresh` 定义与 `config-privacy.md` 落盘清单 / 第 2 小节 / 变更记录、`cli.md` #98 一句 + 变更记录、本节交接）、P3×10 照改（登记恢复信息挪到切终端之前、TCSANOW 理由、SA_RESTART 写在 `install`、pid 复用、`set_len` 理由、清零只留一份……）。
- 下一步：领名额复跑 → 请大总管提交 → codex 045（范围 `origin/fix/62-64-95...HEAD`，只看第四批）。

# 发送闸一组修法设计（#77 + #80 + #83 + #109）· 2026-11-16 · 只设计，未动代码
codex 048。行号按 7f69de0。直发是唯一方式（正式服务器全走 DirectMode）。

## #77 查重与登记分两步（`gate/check.rs` `issue`:93 查、`before_submit`:147 记）
- 现状：`issue` 开凭据时 `check_duplicate`（读 send-guard.json），之后 `verify` → `preview_check`（await）→ `before_submit` 才 `record(Submitted)`。中间同进程两条同内容发送都能过查重
- 修法：`GuardStore` 加 `claim(now, chat, fp, again) -> Result<(), Claim>`：一次 load → 不是 again 就查命中 → push Submitted → save；`before_submit` 在已持有的 `facts` 锁里调它（进程内原子；跨进程有档案锁，同档案只一个进程）。`issue` 里的查重留着当提前拒（人少等），以 `claim` 为准。`again` 要从凭据带到交出：`ConfirmToken` 加 `again`，`verify` 返回 `Verified { mode, again }`，`before_submit(&Verified, plan)`
- 测：同一 Gate 两份同内容计划都拿到凭据，按序 before_submit：第二份报 `duplicate_recent`；带 again 的放行
- 动的文件：`gate.rs`（GuardStore::claim、ConfirmToken）、`gate/check.rs`（issue / verify / before_submit / check_duplicate 共用命中判定）、`send/text.rs`、`send/resending.rs`、`send.rs`（签名跟着改）、`gate/tests.rs`

## #80 交出前就超时也报结果未知、拦 24 小时（`send.rs` `submit_within`:195）
- 现状：期限包住「验凭据 → 预览复核（await，可能慢）→ before_submit → 交给 TDLib」全程；到点一律 `after_outcome(Unknown)` → 防重复补一条 Unknown（24 小时）+ 报 `send_outcome_unknown`
- 修法：`send_text` / `resend` 收一个 `&AtomicBool handed`，`before_submit` 成功、调 `send_limits::submit` 之前置真（tdlib-rs 首次 poll 就同步交出，置在这里偏保守）。`submit_within` 到点：`handed` 为假 → 不记账、不写留痕，报 `network`（可重跑，凭据已吃掉要重新 prepare）；为真 → 照旧结果未知
- 契约：`actions.md` 第一节第 5 小节、`cli.md` send 一行、`errors.md` 补一句「交出前到点 → network，不登记防重复」——**改了一种情况的错误码，报大总管定是否属技术细节**
- 测：假后端预览请求挂住（`FakeBackend::HANG` 同款）→ `network`、guard 无记录、留痕无行；交出后挂住 → 仍 `send_outcome_unknown`

## #83 批量重发留痕按请求记（`gate/check.rs` `before_submit` / `after_outcome`、`send/resending.rs`）
- 现状：`before_submit` 一次请求写一行 `submitted`（temp 0）；`finish` 按条写结果行（只有 TDLib 给了新 id 的条）；不能重发（应答 null）的条不写；请求级失败 / 超时只写一行 temp 0
- 修法：`Gated` 加 `fn items(&self) -> Vec<i64>`（文字发送空表；重发为各条旧 id）。`DirectLogLine` 加 `old_id: Option<i64>`（skip_none）。交出前每条一行 submitted；结果每条一行：有新 id 的照 finish；null 的写 `failed` + `error:"not_resendable"`（或 unknown，待定，倾向 failed——TDLib 明确说不能重发=没发）；请求级失败 / 超时每条一行
- ⚠ **和 msg2 撞**：msg2 要在 `DirectLogLine` 加 `target` 字段。我加 `old_id`，同一个结构体两个新字段，建议 msg2 先合、我在它之上加；或我顺带加，请大总管定
- 契约：`actions.md`:180、`config-privacy.md`:25「每条直发两行」改「重发按条记」
- 测：重发三条（一条 null）→ 留痕 3 行 submitted + 3 行结果，old_id 对得上

## #109 pending cancel 交出后超时报 network
- **代码不在本分支、也不在 main**：`pending` 是 #57（OPEN，msg1 在做）。#109 只能在 #57 合并后、或由 msg1 在 #57 里一并改。建议：交给 msg1 在 #57 里顺手修（口径照 `send_outcome_unknown`：撤销交出后到点报「结果未知」，提示去会话里看），或 #57 合后我再做。请大总管定

## 顺序
#77 → #80（都改 submit 链的签名，一起改一次）→ #83（等和 msg2 对齐）→ #109（看定）。一个分支一个 PR（大总管开分支，从 main）

## 发送闸组：大总管定（2026-11-16）与测试计划 / 契约改法（等 #9 #57 合 main 后开新分支再动代码）
- 定：#83 在 msg2 的 `target` 字段之上加 `old_id`；#80 报 `network` 属技术细节；#109 交 msg1（#57 里修），本组不做
- **测试计划**（先写会红的，都在 `gate/tests.rs`、`send/tests/*.rs`，假后端）
  - #77：同一 Gate、两份同内容文字计划先后 `issue` 都拿到凭据（查重时都还没登记）→ 第一份 `send_text` 过；第二份在 `before_submit` 报 `send_refused(duplicate_recent)`，TDLib 只收到一次请求，`send-guard.json` 一条 submitted。带 `again` 的第二份放行、记两条。修前：第二份也交出（红）
  - #80 ①预览请求挂住（`FakeBackend::HANG`）→ 期限到报 `network`；`send-guard.json` 无记录、`direct-send.log` 无行；随后不带 `--again` 重跑能发。修前：`send_outcome_unknown` + 24 小时拦（红）②交出请求挂住 → 仍 `send_outcome_unknown(timeout)`、拦 24 小时（回归）③重发路径同样两条
  - #83：重发 3 条、TDLib 应答 `[m, null, m]` → 留痕 3 行 submitted（各带 old_id）+ 3 行结果（中间那条 failed / `not_resendable`），temp/final id 对得上；请求级失败（`submit_failed`）与交出后超时 → 每条一行 unknown。修前：1 行 submitted + 2 行结果（红）
- **契约改法**（同一 PR，变更记录各一行「手下 fix，属技术细节 / 大总管定」）
  - `actions.md` 第一节第 5 小节「重复提交拦截」：查重与登记在交出前一步做完（凭据开出时先查一次只为早拒，以交出前那次为准）；「交出前就到点」：没交出、不登记、报 `network`，可重跑
  - `actions.md`:180「本地留痕」：重发按条记，每条带 `old_id`；TDLib 说不能重发的那条记 `failed`、错误码 `not_resendable`
  - `config-privacy.md`:25：「每条直发两行」后补「重发按条记」
  - `cli.md` send / resend 行：到点分两种——交出前到点 → `network`（退出码同 network，可重跑）；交出后 → `send_outcome_unknown`（不变）
  - `errors.md` `send_outcome_unknown` 一段补「只在已交出后」；`not_resendable` 只出现在留痕的 `error` 里，不是对外错误码——若 errors.md 要求留痕错误码也登记，就登记在「`direct-send.log` 的 error 取值」处（开工时核）
