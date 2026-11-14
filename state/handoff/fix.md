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
