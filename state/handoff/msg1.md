# 现状页 · msg1

## 已收尾（2026-11-17）
b20 这条线全部做完：#57 pending / pending cancel、#88、#109（PR #134，已合）；#8 A 段删除（PR #140，已合）；#8 B 段编辑（最新 PR，Closes #8，一级评审待人本人审）。
- 留下的 issue：#127（#88 余下 P3）、#132（文字格式结尾句改到树上，含 delete / edit）、#138（删除复查同一秒换图认不出）、#147（编辑后链接预览按默认重新生成等）、另一条「三个 L2 动作交出前复查到内容变了的码不统一 + delete 结果未知 temp_message_id 恒 0」
- 待人看（大总管攒着）：只许删 / 改自己发的；带格式照改标 `formatting_dropped`；频道管理员改别人帖子不开；#88 Q1（正常关闭时还没回来的结果照删标记）
- 人的收藏夹里留着实测消息：「atg 实验 编辑 1–3 改后」3 条（没有经闸的「只删自己这边」，没清，大总管已转告人）
- 工作副本：`atg-wt/msg1`（feat/57-pending，已合）、`atg-wt/msg1-8`（feat/8-edit-delete，已合）、`atg-wt/msg1-edit`（feat/8-edit，PR 中），都由大总管处理；三处 `.env.local` 是指向 MAIN 的符号链接
- 经验：评审一律按提交范围取 diff（共同规矩第 17 条）；codex 的 target 用不会变的写法（如 `<基点>..HEAD`），否则换范围时身份对不上；解冲突挪了代码要先编译再报

## b20 接手（2026-11-16）
交代：MAIN `_tmp/20261116-b20/msg1.md`。大总管 `allenchen-37`。顺序：#57 pending 收尾（029）→ #88 待发标记提前删（先写红测试）→ #8 编辑 / 删除自己刚发的（030、031，直发不确认）。
- **做到哪**：合并已提交 bbe2f20（大总管）。排队等编译名额（第二位）。#88：红测试已写 `src/atg-core/tests/send_limits.rs` 末尾两条（`late_limit_with_a_full_disk_still_stops_the_next_process` 现应红；`accepted_send_left_open_at_a_clean_close_leaves_no_manual_work` 现应绿、守 Q1=b）；单测三条（新 API `hold` / `await_outcome` / `outcome` / `closed`）暂放 scratchpad `88-unit-tests.rs`，红跑之后再放回 `send_limits.rs`。大总管定：#8 直发；`actions.md:294` 是历史记录不改；Q1 先按 b
- **#57 实测通过（2026-11-17，大总管给名额）**：正式档案 main 收藏夹、直发，「atg 实验 待发撤销 1–3」三轮：发出到交出删除 4–6 毫秒，三轮下落都是 `cancelled`；收藏夹最近 20 条里留下 0 条；档案里 send-pending / flood-until / account-limited 都没留。顺带改：`exp_real.rs` 出错时打印错误码（原来只说「中途出错」）；本工作副本补了 `.env.local` 符号链接（指 MAIN，同其他工作副本，被 git 忽略）。可写 Closes #57。下一步：PR 合后从 main 新开分支做 #8；两条旧 P2 = #132
- **codex 029 收敛（2026-11-17）**：第 5、6 轮（新身份第 3、4 轮）都 P0–P3 全 0，满足停止线。#57 + #109 + #88 这一块评审完。第 5、6 轮记录待提交。下一步：报大总管开 PR（`Refs #57` 还是 `Closes #57`、#8 在不在同一分支，由大总管定）→ #8
- **codex 029 第 4 轮（新身份第 2 轮）**：P1×1 属实——关闭后写盘入口没封全（晚到的 `note` 写限制文件、`begin` 写标记）→ 关闭后 `note` 只记内存、`begin` 拒；单测 `nothing_is_written_after_close`；记录已写。名额验证：全量 521 过、TDLib 494 过、clippy 两种净、fmt / 行数过；提交信息 `_tmp/msg1/commit-b20-3.md`。下一轮 focus 要写进共同规矩第 16 条（同进程构造时序竞态最高 P2）
- **第三次名额（b20，2026-11-17）**：全量 520 过 0 败；开 TDLib 493 过 0 败；clippy 两种净；fmt / 空白 / 行数（--tighten 后 0 报错）过。修了自己引入的两处：新单测临时目录名与旧测试撞名（`early` → `outcome-first`）、clippy `match_like_matches_macro`；`fake.rs` 超基线 → `play_send` 挪进 `fake/submit.rs`（顺带成为假后端唯一登记处）。提交信息 `_tmp/msg1/commit-b20-2.md`。下一步：提交后 codex 029 新身份第 2 轮
- **atg-review（#88 #109）**：P0×1（同 codex #1）、P2×9、P3×10，已合并处理：登记只留每个后端一处、删 `Sealed::is_preview`；补测试 2 条（成败先到、只认永久删除）；文档（正常关闭定义、断网可留很久、Q1 改指 issue #88、errors.md 期限与 temp=0）。**未改的 P3**（报大总管开 issue）：`jam` 两份且依赖 `write_atomic` 临时文件名；`cancel_unknown` 留痕没测；「守卫」「交出」等自造词无定义。记录已写进 029 第 3 轮。Q1 题面大总管已评论到 #88；余下 P3 开了 #127。排名额第二（fix 之后）
- **codex 029 第 3 轮（pending-57-b20 第 1 轮，gpt-6-astra，read-only，medium）**：P1×2 P2×1，全部属实，已改（未编译）：
  - #1 正常关闭误删上个进程留下的标记 → `inherited`，本进程从不自动删；测试 `inherited_mark_survives_a_clean_close`
  - #2 关闭不等交出线程、放锁后还碰标记 → `closed(wait)` 先等在途归零（Condvar），放锁之前调，之后 `closed` 标志封住登记与删文件；测试 `close_waits_for_the_submit_thread_then_keeps_hands_off`
  - #3 撤销交出时断线报成失败 → `delete::cancel` 复用 `send::submit_failed`，CLI 换撤销说明；测试 `cancel_disconnected_after_submit_is_outcome_unknown`
  - `send_limits.rs` 测试挪进 `send_limits/tests.rs`（635 行超 600）；契约 config-privacy / cli / errors 同步
- **评审在跑**：codex 029 第 3 轮（run-round3.sh，大总管准 `--force-secrets`，共同规矩第 15 条）与 atg-review（#88 #109，`git diff bbe2f20 3bdc42b`）并行后台跑，结果合着处理
- **已提交 3bdc42b（大总管）**。codex 029 第 3 轮：脚本 `_tmp/codex-029/run-round3.sh`，被隐私扫描拦（9 处 `token: ConfirmToken` 误报，要人确认 `--force-secrets`），已报大总管；因本机 `main` 陈旧改用 `origin/main...HEAD`、新身份 `pending-57-b20`，停止线重新计数。建议先对 #88/#109 跑 atg-review，等大总管回
- **第二次名额结果（b20）**：#57 收尾 + #109 + #88 全部验证：build 过；全量 515 过 0 败 5 忽略（#88 红测试已转绿）；开 TDLib 特性 488 过 0 败；clippy -D warnings 两种都净；tdlib 编译过；`--tighten` 后行数检查 0 报错；rustfmt 过（顺带格式化了合并时改的 `serve/l2.rs`、`session.rs`）。提交一笔，信息见 `_tmp/msg1/commits-b20.md`，交大总管提交。下一步：codex 029 第 3 轮（脚本交大总管）→ #8
- **#88 已实现**：`send_limits.rs` 加 `hold` / `await_outcome` / `outcome` / `observe` / `closed`，标记盖到「在途交出 = 0 且待回成败为空且无未写入限制」；`early` 集让成败先于应答到、或登记两次都不卡；`link::Shared::event` 先 note 再 observe；`td.rs::detached` 线程自领守卫、应答成功当场登记 id（`td/requests.rs::sent` / `resent`）；td / fake 正常关闭调 `closed`（Q1=b）；单测 3 条放回 `send_limits.rs`；契约 config-privacy 同步。红测试第一次跑：`late_limit_with_a_full_disk_still_stops_the_next_process` 红（下一个进程放行），符合预期
- 第一次名额结果（b20）：build 过、clippy 净（tdlib 特性下 main 的 `examples/live_wait_phone.rs:27` 多余 `self` 报错，已报大总管）、tdlib 编译过、全量 512 过 0 败；`td.rs` 解析挪进 `td/requests.rs`（847→832）
- **#109（并入 #57，大总管 2026-11-16）**：`pending cancel` 交出到点没回 → `send_outcome_unknown`（`timeout` / 故障时 `disconnected`，退出码 10，专用说明 `CANCEL_UNKNOWN_ZH`，留痕记未知）；`execute_cancel_within` 让单测给短期限；测试 `cancel_submit_timeout_is_outcome_unknown`；契约 cli / errors 同步。未编译
- **合 main 预计冲突**：core `send.rs`（main 拆成子模块，分支改了约 100 行）、`confirm_box.rs`（main 删、分支改：修改/删除冲突）；还有 fake / td / store/mod / error / lib、CLI send / serve / session / args / main / forward / deadlines、契约 actions / cli / errors / config-privacy、reviews/README、文件行数基线
- **Touch ID 删除波及我的代码**：pending 撤销确认框文字（confirm_box）、`delete.rs` 与其测试里的 HumanAuth / 确认文字、`serve/l2.rs`、`pending.rs`、`exp_real.rs`、CLI session / send 里的 Authenticator 接线 → 全改成直发
- **下一步**：合完 → 去掉确认、编译测试（领名额）→ codex 029 第 3 轮 → #88
- **要人定的事**：见下「要人定的事（b20）」
- **#88 计划（读了 main 的 `send_limits.rs`、`td.rs::detached`、`link::Shared::event`，未动代码）**
  - 病根两处：①调用方 30 秒等不及丢掉等待 → `Pending` 守卫 drop、标记删了，可 `detached` 线程还在等应答，晚到的受限写盘失败时 `mark_pending` 也写不进（磁盘满）→ 重启放行；②交出成功（进 TDLib 待发队列）后，真正的成败靠 `updateMessageSendFailed` 晚到，那时标记早删了
  - 修法 A（治①）：`detached` 线程开跑前自己领一个守卫（inflight+1），登记完应答再放；调用方先放也删不掉标记
  - 修法 B（治②）：交出成功回来的临时 id 各领一个守卫，存进 `SendLimits`；`Shared::event` 收到该 id 的 SendSucceeded / SendFailed（先 note 再放）才放；临时 id 换正式 id 要跟上
  - 红测试：A——`SendLimits` 层模拟「调用方守卫先放、晚到受限写盘失败、线程守卫后放」→ 重启后 `check` 应拒（现在会放行）；B——假后端：交出成功 → 堵住限制文件 → 推 SendFailed(受限) → 新进程 `load` 应拒
  - 待定 Q1（见下）

## #8 B 段编辑（工作副本 `atg-wt/msg1-edit`，分支 `feat/8-edit`，从 main 2309789；A 段 #140 已合）
- **2026-11-17 代码 + 契约已写，未编译，等名额**。大总管定：只许改自己发的（频道管理员改别人帖子进待人清单）；带格式照改、计划标 `formatting_dropped`（进待人清单）；附件说明第二批（`not_text`）；交出 10 秒；`DeleteRefused` 改名 `ActionRefused`（`RefusedWhy` 加 `NotEditable`；删除 / 编辑共用，说明改成两用）
  - core：`edit.rs`（`EditPlan`、`prepare_edit`、`edit_text`；复查比原文 + 整份内容 + 编辑时间）；`InvalidSendWhy::{EditNotText(not_text), Unchanged}`；共享文件（大总管准）：`send.rs` 加 `SealedKind::Edit` + `Sealed::edit`、应答复用 `Answer::Message`；`td/requests.rs::edit`；`td/real/submit.rs` 一臂；`fake/submit.rs` 一臂 + `fake/edit.rs`；`fake/call.rs` 加 `Call::Edit`；测试 `edit/tests.rs` 10 条
  - CLI：`edit.rs`（plan / 回显改后那条 / result；交出 10 秒到点报未知）；`send.rs` 的 `check_len` / `read_text_file` / `echo` 改 `pub(crate)` 共用；args / main / session / forward / serve / serve/l2 / tests/cli.rs 接线；测试 `edit/tests.rs` 6 条
  - 契约：actions（两行 + `EditPlan` + 第 4 小节第 7 条清单补 `edit_message_text`——**这条清单改了，PR 属一级评审，人本人看**）· cli · errors · config-privacy
  - 自审 atg-review（按 `2309789..1fca933`）：P2×1（链接预览按默认重新生成）+ P3×3 开 #147，P3×9 改在 `bde20a0`
  - **codex 031 收敛（2026-11-17）**：第 1 轮 P2×2（实测失败仍通过、交出报错留痕不记 id）改在 `c28d1d1`（`send::submit_failed_for`，大总管准）；第 2 轮起换身份 `edit-8b-r`（target `2309789..HEAD`），第 2、3 轮通过。复验 635 / 636 过、clippy 两种净
  - 下一步：提交评审记录 → 真账号只在收藏夹跑 `edit/exp_real.rs`（「atg 实验 编辑 N」三轮）→ PR（契约第 4 小节第 7 条清单改了，**一级评审、人本人看**）

## #8 进度（新工作副本 `atg-wt/msg1-8`，分支 `feat/8-edit-delete`，从 main 030f91b）
- **A 删除（2026-11-17）代码 + 契约已写，未编译，等名额**。大总管定：只删自己发的（报 `forbidden why:not_own`，待人看）；一行 `plan`、`items` 逐条列；文字格式先在 CLI 拼句（#132 补「含 delete」）；受限 / 限流也拦删除（照现状）
  - core：`model::MessageCaps`；`Backend::message_caps`（`can_reply` 成默认方法委托它，`send/text.rs` 不动）；td 走 `td/requests.rs::caps`；假后端 `fake/caps.rs`（`Cap`、`deny`，默认照 TDLib：自己发的、已发出、不在收藏夹）；`error::DeleteRefused{why: saved|not_own|not_revocable}`（码 `forbidden`）、`InvalidSendWhy::Changed`；`delete.rs` 加 `Action`、`prepare_delete`、`delete`，与撤销共用 `prepare` / `submit`（种类不对 `internal{delete_action_mismatch}`）；测试 `delete/tests/revoke.rs` 9 条
  - CLI：`delete.rs`（plan 带每条 `brief`、`--dry-run`、交出 10 秒到点报未知）、args / main / session / forward / serve / serve/l2 接线；测试 `delete/tests.rs` 7 条；`tests/cli.rs` delete 从占位挪到已做；`serve/tests/lifecycle.rs` 占位改用 `show`
  - 契约：cli / actions / errors / config-privacy
  - 自审 atg-review：P1×1（TDLib 对还没发出的说能给所有人删，`:8352-8354`）已修 → `forbidden why:not_sent`、假后端照真 TDLib；P2 码不统一 + P3 `temp_message_id` 恒 0 大总管合开 issue
  - **codex 030 收敛（2026-11-17）**：第 1 轮 P2（附件改说明 / 换图复查不出）已修（存整份内容 + `edit_date`）；第 2 轮 P2（同一秒再换图）待开 issue；两轮无 P0/P1。复验 586 / 587 过、clippy 两种净
  - 下一步：提交 → 真账号只 `delete --dry-run saved …`（验 `forbidden why:saved`，只读）→ PR → B 段编辑（codex 031）

## #8 计划（b20，未动代码）
分两段交付（roadmap 116 行）：先 **A 删除（给所有人，M2.7）**，后 **B 编辑（M5 ①）**；评审 030（删除）、031（编辑）。都是 L2，直发不确认（#113；现行规则以 actions.md 第一节第 5 小节为准，294 行是历史记录不改）。
- A 删除：
  - core：`Backend::can_reply`（`backend.rs:148`、`td.rs:703`、`fake.rs:938`，唯一调用 `send/text.rs:180`）改成 `message_caps(id) -> MessageCaps { can_reply, can_delete_for_all, can_edit }`（同一次 `getMessageProperties`：`can_be_replied` / `can_be_deleted_for_all_users` / `can_be_edited`，唯一正本），fake 用三个集合模拟；`delete.rs` 加 `prepare_delete(chat, ids)`（L0：逐条查自己能删、给所有人；收藏夹必拒，`actions.md:110`）、`DeletePlan` 加 action `delete`（`revoke=true`）、`delete(plan, token)`（验凭据 → 交出前再查 → `send_limits::submit` → 错误走 `send::submit_failed`）；下落看永久的 `messages_deleted`
  - CLI：`atg-cli delete CHAT_ID MSG_ID... [--dry-run]`（`cli.md:37`）：每条一行 `plan` → 结果；转发进 `serve/l2.rs`、`forward::is_l2`
  - 测试：假后端全流程（成功、收藏夹拒、不能删的拒、交出前变了拒、断线报未知）；真账号只许 `--dry-run` 或收藏夹——收藏夹「给所有人删」TDLib 本就不许，正好验拒绝路径（只读查询，不交出）；成功路径要第二个测试号（N7，测试服务器登录不通 #35）→ 只能假后端，报大总管
- B 编辑：`edit_text`（`actions.md:124`，`editMessageText`），同一套闸；真账号只在收藏夹试
- 先问大总管：编辑 / 删除的时间窗（Telegram 48 小时编辑限制等）由 TDLib 的属性判，不自己算——这算技术细节，默认照此

### 复核（2026-11-17，大总管已同意计划；按 01d596c = 新 main b7ff490 + PR #134，只读）
计划站得住，锚点都还在：`can_reply` 仍是 `backend.rs:149` / `td.rs:703` / `fake.rs:938` / 唯一调用 `send/text.rs:180`；契约 `actions.md` 的 `delete_prepare` / `delete` 两行、`cli.md:37` 的 `delete` 行、roadmap M2.7 都已是直发口径（#115），不用先改契约。补 5 点：
1. **复用 #57 的骨架**：`delete.rs`（218 行）已有 `DeletePlan`、`prepare_cancel` / `cancel`、`td/requests.rs::delete` 已带 `revoke` 参数、`SealedKind::Delete`、`Answer::Deleted`——A 段是给 `DeletePlan` 加一种动作（`delete`，`revoke=true`），不新建模块；`pending.rs` 的「交出到点 → `send_outcome_unknown`」（#109）同样用到 `delete`
2. **文字格式结尾句**：新命令的统计 / 读法句一律走树 + `speech`（架构守则第 3 条），不学 `send` / `pending` 现在在 CLI 拼的写法（那是 #132）
3. **账号受限 / 限流也拦删除**：`send_limits::submit` 对所有交出一视同仁，删除自己的消息也会被「受限」挡住。契约没单说删除——照现状拦（安全侧），记成技术细节；人若要删除不受限制再改。**大总管定 2026-11-17：照现状**
4. **真账号**：收藏夹给所有人删 TDLib 本就不许（`:8346`），`delete --dry-run saved …` 只验拒绝路径（只读，`delete_prepare` 读出原因拒）；成功路径只有假后端（#35 测试服务器登不上、N7 第二测试号也就没有）
5. **B 编辑**：契约只占了名字（`actions.md`「第二批留位」`edit_text`），开工前要补 `edit_prepare` / `edit_text` 两行 + `cli.md` 一行 + `errors.md`（可能要「不能编辑」的原因）；Store 已处理 `updateMessageContent` / `updateMessageEdited`（`tdjson.rs:225,233`），编辑后的读法不用新做。编辑在收藏夹能真试（自己的消息可编辑）

## 要人定的事（b20）
- **Q1 #88：进程正常退出时还有「交出了、结果没回来」的消息，标记留不留？**
  - 选项 a：留——最稳，下个进程拒发，要人看过账号状态后手动删标记。代价：断网时等超时这种常见情况也会留人工活，和现在「常见情况自己清」的承诺相反
  - 选项 b：删——下个进程启动时 TDLib 续发，失败由它自己登记。代价：续发结果回来之前，下个进程可能已经又发了一条（最坏是多一次被拒，不会发错人）
  - 选项 c：留，但标记里写「正常退出、结果未知」，下个进程看到这种标记先等 TDLib 续发的结果（或等一个期限）再放行。代价：多一个状态、多些代码
  - 建议 c 太重，先 b，并在契约里写明；默认 b

## b18 记录（发送这一侧收尾）

交代：MAIN `_tmp/20261113-b18/msg1.md`。分支 `feat/57-pending`。顺序：#57 pending（评审 029）→ 删除（030）→ 编辑（031）。

## 手上
**#57 `atg-cli pending` + `pending cancel CHAT_ID`**：自审 4 轮，每轮各有 P1，均已修（见下「自审记录」）；第 4 轮修后待编译验证，再跑第 5 轮。全量（第 1 轮修后）487 过、clippy 净、tdlib 特性编译过。实测（大总管已同意：正式档案 main 收藏夹 + 直发，「atg 实验 待发撤销 N」3 次）代码在 `src/atg-cli/src/pending/exp_real.rs`（`#[ignore]` + 环境变量），等名额

## 已核的事实（TDLib 1.8.69 源码，`resources/td/td/telegram/MessagesManager.cpp`）
1. 开着消息库时，上次留下的待发消息在 TDLib 启动重放 binlog 时**当场续发**（`:35925-35930`），并推 `updateNewMessage`。任何一条命令（含 `pending` 本身）一启动就续发
2. 续发时**临时 id 和 `date` 都重编**（`:35851-35856`），`sending_id` 不落盘
3. 超过 24 小时（`MAX_RESEND_DELAY`，`MessagesManager.h:1708`）的不再续发、直接变失败（非收藏夹）
4. 删除待发消息只停本地请求（`:21330-21345`）；服务器已收到的，成功应答回来时 TDLib 自动 `revoke=true` 删（`:27848-27851` → `:8484-8490`）；会话不可访问时放弃
5. `deleteMessages` 先把临时 id 换成发出后的正式 id（`get_persistent_message_id`，`:8415`、`:21075-21087`；登记在 `:26949`、`:28347`）→ 撤销用 **`revoke=false`**（大总管同意），赶上恰好发出的只删自己这边，CLI 报 `sent_deleted_here`

## 大总管定（2026-11-14）
- Q1 `pending_cancel` 按 L2 过闸（同删除）
- Q2 `PendingMessage` 时间改 `date`，契约写明续发后 id 与时间都会变

## 改了什么（#57）
- core：`store/query.rs`（新：`pending_sends`，迁入 `chat_list` / `next_unread_chat`）· `delete.rs`（新：`DeletePlan`、`prepare_cancel`、`cancel`，以后给所有人删除也放这里）· `send.rs` 交出口加 `SealedKind::Delete` / `Answer::Deleted`；确认文字函数迁到 `confirm_box.rs` · `td.rs` 加 `deleteMessages` 交出，建客户端前两步迁到 `td/real/setup.rs` · `fake/submit.rs`（新：重发迁入 + 删除）· `error.rs` 加 `not_pending`
- CLI：`pending.rs`（新：列表 + 撤销）· `pending/exp_real.rs`（新：实测）· `deadlines::CANCEL_RACE_WAIT` 1 秒· `serve/l2.rs`（新：执行方的发送类分派，send / resend 迁入）· args / main / session / forward 接线 · `deadlines::CANCEL_WAIT` 5 秒
- 契约：actions（待发出的消息、PendingMessage）· cli（pending 行、send 行结果未知）· errors（not_pending）· config-privacy（能转 pending）

## 报大总管 / 要人定的
- 超线文件变小（fake / send / store/mod / td / serve），要跑 `python3 scripts/check-file-size.py --tighten` 收紧基线——`scripts/` 不在我可写范围，请大总管跑或授权
- 架构守则第 3 条：`pending` 文字格式的结尾一句是在 CLI 里拼的统计句（同 send / resend 现状），没有「命令结尾统计」树节点；建议另开 issue 统一处理
- 误操作留痕：跑文件行数检查时用了一次 `git add -N`（只登记意向），发现违规立刻 `git reset` 撤回，工作区恢复原状

## 自审记录（atg-review）
- 第 1 轮 P1×3：一条 `messages_deleted` 只认第一条（`send::verdict` 改返回多条）· 文字格式缺 id · `result` 配退出码 10（改退出码 0 + `all_cancelled`）
- 第 2 轮 P1：跨进程临时 id 重编 → `pending cancel CHAT_ID` 不收 id、同进程取全部待发（大总管同意）
- 第 3 轮 P1：删除被换成正式 id → `revoke=false` + `sent_deleted_here`（大总管同意）
- 第 4 轮 P1：换 id 的删除与发出成功同批到、第二次等待等不到 → 下落按 store 判（`cancel_outcome`，有单测）
- codex 029 第 1 轮（`docs/reviews/029_pending-57-codex.md`）：P1×3 属实已修（待发不参与删旧 + 超 100 整批拒；`sent_deleted_here` 只认墓碑；带附件的整批拒 `not_text`），P2×1 已开 issue #109（撤销交出后超时被报成普通 network）
- 剩余 P2（待开 issue）：确认框只能放三种结局里的两句；`DeletePlan` 等 M2.7 再加 revoke 字段；`serve/l2.rs` 名字与内容（含只读列表）不完全一致；文字格式结尾统计句在 CLI 里拼（架构守则第 3 条，同 send / resend 现状）

## 下一步
人定 A：不再自审。名额：编译 + 测试 + tdlib 编译 + 实测 → 大总管提交 → codex 029 第 2 轮（脚本交大总管代跑）→ 报「可以提交」
