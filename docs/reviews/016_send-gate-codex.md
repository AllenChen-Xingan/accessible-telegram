# 评审 016 · #52 发送闸 + `atg-cli send` / `resend`（codex · gpt-6-astra · read-only · 一级评审）

> 对象：`git diff main`（`feat/52-send` 工作区：#52 全部改动 + 自审第 9–12 轮修复）。**最新一轮：第 5 轮（2026-11-13）· 总评「修改」· 待第 6 轮**。
> 累计处理：P1×14、P2×7 逐条回源码核实，属实 21、驳回 0、无法核实 0。新出的要人定的事：无。

## 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/reviews-016/send-gate-codex-1.md`（不入库；提示词、补丁、日志同目录）。`--type diff --target main`，effort high，重点见 `_tmp/reviews-016/focus.md`。隐私扫描拦下 10 处 参数 token 的类型标注 `ConfirmToken`（类型标注，误报），人确认后 `--force-secrets` 放行（2026-11-13）；大总管代跑。行号指评审时的版本。
本轮 P0/P1：5 条 · 停止线：未满足 · 留作 issue 的 P2：#6（大总管开 issue）

- **P1｜`src/atg-core/src/gate.rs:653` 回拨时钟会解除防重复**（第 1 轮 #1）
  - 核实：属实。`GuardStore::live` 丢掉 `at > now + 60` 的记录，注释还写着「丢掉只会少拦」——少拦恰恰就是漏拦：结果未知的那条在时钟回拨后能再发一次
  - 处理：比「现在」还晚的记录照样算数，等时钟追上再过期（`e.at > now || now - e.at < hold`）；测试 `clock_turned_back_does_not_lift_the_guard`；`actions.md` 第一节第 5 小节写明
- **P1｜`gate.rs:668` 读失败会清空其他发送的保护**（第 1 轮 #2）
  - 核实：属实。`record` / `settle` 用 `load().unwrap_or_default()`，读错（含文件坏了）当空表再整体重写；`--again` 跳过前置的 `hit`，读错不会先暴露
  - 处理：两处改为 `load()?`，读不了就不写，`before_submit` 据此拒直发（`send_guard_unwritable`）；文件坏了要人删掉才恢复；测试 `unreadable_guard_file_is_never_overwritten`（坏文件原样不动）；契约同步
- **P1｜`src/atg-core/src/send.rs:605` 30 秒上限不覆盖交出应答**（第 1 轮 #3）
  - 核实：属实。`send_and_wait` / `resend_and_wait` 用不设时限的 `block_on` 等交出，之后才开始计 30 秒
  - 处理：新增 `submit_within`：交出（验凭据 → 复核 → 等 TDLib 回交出的应答）用 `link::block_on_timeout` 计入同一上限，余下的时间才给等结果；到点按结果未知记账、报 `send_outcome_unknown`（`why:"timeout"`，`temp_message_id` 0）。没加单测：假后端的交出不会挂住，构造挂起要改假后端，留给第 2 轮看是否必要；契约 `actions.md` `send_text` 行写明
- **P1｜`send.rs:720` 重发未确认实际回复关系**（第 1 轮 #4）
  - 核实：属实。`ResendItem` 只有 `drops_reply` 与正文；重发后保留的回复对象人看不到，摘要也不绑
  - 处理：`ResendItem.reply`（`ResendReply`：不是回复或会被去掉 / 回复某条（可在别的会话）/ 认不出）；确认框每条写「（回复 id M 那条）」，`plan` 行每条带 `reply_to`，`resend_digest` 绑上；测试 `resend_confirm_shows_and_binds_the_reply_target`；契约同步。**话题没加**：重发的是同一条消息，TDLib 把它留在原话题，不会落到新地方；回复对象同理也不变，加它是为了让人看得到
- **P1｜`gate.rs:497` 留痕不可写仍可能直发**（第 1 轮 #5）
  - 核实：属实。`before_submit` 只 `open_append`，磁盘满时照样打得开；重发不写防重复记录，于是先发、后写日志失败
  - 处理：交出前真写一行 `result:"submitted"`，写不进拒（`direct_log_unwritable`）；等到结果后照旧一行。留痕由每条一行变两行，测试两处随之改；`actions.md` 第一节第 5 小节、`config-privacy.md` 落盘清单同步
- **P2｜`gate.rs:425` 防重复检查与登记分离**（第 1 轮 #6）
  - 核实：属实（`issue` 查、`before_submit` 记，中间可再开一张同意图凭据）；现在唯一能并发的入口 `serve` 串行处理请求，触发不了
  - 处理：留 issue（大总管开），以后 GUI 接入、或 serve 并行化之前要做
- **P2｜`send.rs:67` `Sealed` 没有限制为 `send` 独家构造**（第 1 轮 #7，构造绕过）
  - 核实：属实（字段 `pub(crate)`，crate 内别的模块能造）
  - 处理：字段私有，后端经 `Sealed::into_kind()` 取（`td.rs`、`fake.rs` 两处）

### 引用抽查

- codex 列的「不符」：`backend.rs:143`「只能由 crate::send 造」未被字段可见性保证——即第 1 轮 #7，已改
- codex 列的「核实属实」：3 条
- 无法核实：无

### 是否再审

有 P1 → 改完开第 2 轮（`--previous` 第 1 轮原文、`--response` 本记录）。验证：fmt · clippy -D warnings · test 375 过 / 0 败 / 1 忽略（名额 2026-11-13）。

## 第 2 轮（2026-11-13）· 总评「修改」

原文：`_tmp/reviews-016/send-gate-codex-2.md`（不入库）。带第 1 轮原文与本记录；gpt-6-astra，effort high；大总管代跑。
上一轮处理的验证：codex 抽查 7 条全部「核实属实」（时钟回拨、读错传播、交出前写留痕、`Sealed` 私有、交出计入超时、执行方的闸、#6 留待后续）。
本轮 P0/P1：3 条 · 停止线：未满足 · 留作 issue 的 P2：#4

- **P1｜`gate.rs:204` 行首空格被计为零宽**（第 2 轮 #1）
  - 核实：属实。`wrapped` 在 `cur == 0` 时对空串不加空格宽度，行首一串空格（及只由空格组成的行）算成 0 列；完整确认说明加 50 个空格能少算一行
  - 处理：逐个空格计 1 列，行满就折、并把它算进新行；测试 `every_space_takes_a_column`（含完整说明的边界例）；`actions.md` 第 4 条写明
- **P1｜`send.rs:904` 重发仍缺提交前复核**（第 2 轮 #2）
  - 核实：属实。`resend` 只验凭据对旧计划的摘要，随后按 id 交出；准备到交出之间那条被改、被别处重发、回复关系变了，旧凭据照样能用
  - 处理：准备时逐条查的那段抽成 `load_items`，交出前再取一遍与计划的 `items` 比，不一样（或取不到）拒 `content_mismatch`，什么都不交出；测试 `resend_rechecks_before_submitting`（变了被拒、没交出；没变则真的交出一次）；`actions.md` `resend` 行同步
- **P1｜`src/atg-cli/src/send.rs:511` 批量重发的文字输出隐藏「结果未知」**（第 2 轮 #3）
  - 核实：属实。结尾行取第一条没成功的，失败在前时未知项只出现在 JSON `items` 里
  - 处理：有结果未知的优先报它（`send_outcome_unknown`，退出码 10）；说明逐项计数（成功 / 失败 / 结果未知），点名未知那几条的 id「不要再重发」；测试 `mixed_resend_reports_unknown_first`；`cli.md` `resend` 行同步
- **P2｜`send.rs:640` 交出前超时也按结果未知报**（第 2 轮 #4）
  - 核实：属实（复核预览还没回、真正的交出没调用时到点，也记「未知」拦 24 小时）。偏保守，不会误发
  - 处理：留 issue（大总管开）

### 引用抽查

- codex 列的「核实属实」：7 条；「不符」：无

### 是否再审

有 P1 → 改完开第 3 轮。

## 第 3 轮（2026-11-13）· 总评「修改」

原文：`_tmp/reviews-016/send-gate-codex-3.md`（不入库）。带第 2 轮原文与本记录；gpt-6-astra，effort high；大总管代跑。
上一轮处理的验证：codex 抽查 6 条全部「核实属实」（空格计宽、重发复核、未知优先、前两轮修复、执行方的闸）。
本轮 P0/P1：2 条 · 停止线：未满足 · 留作 issue 的 P2：无（#3 已修）

- **P1｜`gate.rs:221` 长词折行把字形宽度当成可拆分的总量**（第 3 轮 #1）
  - 核实：属实。比一行长的词用「总宽 ÷ 列宽」算行数，`A﷽B`（2 + 40 + 2 列）算成 2 行，逐字排是 3 行
  - 处理：比一行长的词从新行起逐字排，放不下整个字就换行（字不拆）；测试 `wide_glyph_inside_a_long_word_gets_its_own_line`；`actions.md` 第 4 条写明
- **P1｜`gate.rs:548` 晚到的账号受限事件不触发停发**（第 3 轮 #2）
  - 核实：属实。受限只在 `after_outcome` 记；等结果超时之后才到的 `SendFailed(AccountLimited)` 只经 Feed 进 store，下一次不同内容照样过闸
  - 处理：闸新增「账号受限」信号（`Gate::limited_signal`），后端建好时交给 `link::Shared`（td、fake 两处）；所有事件的唯一入口 `Shared::event` 见到受限失败就点亮；闸在 `mode_for` 看到信号即 `mark_limited`（本进程停 L2 并写 `account-limited`）。测试 `late_account_limited_signal_stops_sending`（点亮后下一次开凭据被拒、文件写出）；没做经假后端推真实失败事件的端到端测试（构造受限错误的 TDLib 消息要改假后端），入口那几行逻辑简单
- **P2｜`src/atg-cli/src/send.rs:423` 文字计划漏了回复与话题**（第 3 轮 #3）
  - 核实：属实
  - 处理：`send` 文字计划行加最终话题；`resend` 文字计划每条带「（回复 id M 那条）」并改称「id N 那条」（`ResendReply::zh` 改为公开，与确认框同一份）。没补文字输出断言

### 引用抽查

- codex 列的「核实属实」：6 条；「不符」：无

### 是否再审

有 P1 → 改完开第 4 轮。

## 第 4 轮（2026-11-13）· 总评「修改」

原文：`_tmp/reviews-016/send-gate-codex-4.md`（不入库）。带第 3 轮原文与本记录；gpt-6-astra，effort high；大总管代跑。
上一轮处理的验证：codex 抽查 5 条全部「核实属实」。
判级口径（大总管定 2026-11-13）：只有会导致发错、误发、漏拦、泄露、崩溃的才认 P1。
本轮 P0/P1：2 条（都认）· 停止线：未满足 · 留作 issue 的 P2：#4

- **P1｜`gate.rs:439` 账号受限仍会漏持久化**（第 4 轮 #1）
  - 核实：属实，认 P1（漏拦：收到受限后进程直接退出，重启恢复发送）。**与第 3 轮 #2 同类第二次出现**（受限只记在内存 → 只在下一次判开法时才落盘），按 skill 改结构，不补个例
  - 处理：新模块 `src/atg-core/src/send_limits.rs`（`SendLimits`，闸与收线程共用一份 `Arc`）：账号受限、限流冷却不论从哪条路得知（交出口当场回错、失败事件、超时后才到的失败事件、重发准备时的 `retry_after`）都经 `note` **当场落盘**；闸判开法只问 `check`；新进程 `load` 读回；后端起来前收到的先记内存、知道档案目录时补写。原 gate.rs 里的受限代码挪过去（gate.rs 在超长基线里，只许变小：1402 → 1386 行）。测试：`send_limits` 3 条（收到即写且下一个进程读回、目录未知时先记后补写、限流跨进程且到点失效）+ gate 2 条
- **P1｜`gate.rs:591` 发送限流没有落实等待期限**（第 4 轮 #2）
  - 核实：属实，认 P1（漏拦：`errors.md` 第一节第 5 小节与 `.42cog/real.md`「必须遵守 FLOOD_WAIT」明文要求等待期落盘、跨进程遵守；AI 循环调用会无视等待，可能把账号推到受限）
  - 处理：同上一处实现，落盘文件 `flood-until`（Unix 秒）；闸在冷却中报 `rate_limited`（带还要等的秒数）。`errors.md`、`config-privacy.md` 落盘清单同步
- **P2｜`gate.rs:132` 公开 `Gated` 可分离确认文字与真实内容**（第 4 轮 #3，构造绕过）
  - 核实：属实（crate 外能实现 `Gated`）
  - 处理：顺手修：`Gated: Sealed`，`Sealed` 在私有模块里，只有 crate 内的计划能实现
- **P2｜`send.rs:933` 批量直发重发的留痕与计数按请求而非按条**（第 4 轮 #4）
  - 核实：属实（N 条重发一行 `submitted`、`direct_sent` 只加一，结果逐条写）。不影响拦不拦
  - 处理：留 issue（大总管开）

### 引用抽查

- codex 列的「核实属实」：5 条；「不符」：无

### 是否再审

有 P1 → 改完开第 5 轮。

## 第 5 轮（2026-11-13）· 总评「修改」

原文：`_tmp/reviews-016/send-gate-codex-5.md`（不入库）。带第 4 轮原文与本记录；gpt-6-astra，effort high；大总管代跑。
上一轮处理的验证：codex 抽查 4 条「核实属实」，1 条「不符」——本记录第 4 轮写的「不论从哪条路得知都当场落盘」不成立（预览回的错没登记），即本轮 #1。
本轮 P0/P1：2 条（都认：漏拦）· 停止线：未满足 · 留作 issue 的 P2：#3

- **P1｜`send.rs:388` 发送限制仍漏路径**（第 5 轮 #1）
  - 核实：属实。预览回的 `rate_limited` / `account_limited` 直接返回、没登记；开凭据之后、复核预览期间才到的受限，`before_submit` 不复查，照样交出
  - 处理（按结构，大总管定）：登记只两个入口——**交出口**：预览、发送、重发都只经 `send_limits::submit` 调 `Backend::submit`，回的错在那里登记；**收线程**：`link::Shared::event`。`after_outcome` 里的重复登记删掉（gate 测试改走 `limits().note`）。**交出前最后复查**放在 `Gate::before_submit` 开头——所有 await 之后、交出之前的唯一位置。新集成测试 `src/atg-core/tests/send_limits.rs`：交出口回限流后下一次准备被拒、不再交出；确认后才到的受限被最后复查拦下、没交出。假后端的预览不能报错，交出口那条用真发送回错来测（同一个出口）
- **P1｜`send_limits.rs:53` 限制记录读失败会解除冷却**（第 5 轮 #2）
  - 核实：属实。读错、内容坏了都当成 0；`exists()` 把检查出错当成「不存在」
  - 处理：失败即关——只有 `NotFound` 算没有记录；其余读错、内容坏了记成「读不准」，`check` 拒发 `internal{send_limits_unreadable}`（`errors.md` 登记）；测试 `unreadable_marks_fail_closed`（坏内容、同名目录）
- **P2｜`gate.rs:501` 凭据未绑定签发的闸**（第 5 轮 #3，构造绕过）
  - 核实：属实（经公开的 `FakeBackend::with_gate` 配一个恒通过的认证器，另一个闸开的凭据能被真闸消费）
  - 处理：按威胁模型（构造绕过最高 P2）留 issue；也免得超长基线里的 gate.rs 变长

### 引用抽查

- codex 列的「不符」：1 条（见上，即本轮 #1，已改）；「核实属实」：4 条

### 是否再审

有 P1 → 改完开第 6 轮。

