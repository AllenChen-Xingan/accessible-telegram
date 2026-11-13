# 评审 016 · #52 发送闸 + `atg-cli send` / `resend`（codex · gpt-6-astra · read-only · 一级评审）

> 对象：`git diff main`（`feat/52-send` 工作区：#52 全部改动 + 自审第 9–12 轮修复）。**最新一轮：第 2 轮（2026-11-13）· 总评「修改」· 待第 3 轮**。
> 累计处理：P1×8、P2×3 逐条回源码核实，属实 11、驳回 0、无法核实 0。新出的要人定的事：无。

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

