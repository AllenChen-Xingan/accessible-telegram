# 现状页 · 手下 send2（issue #52 接手：发送闸 + `atg-cli send` 收尾）

> 批次 b18 · 交代 MAIN `_tmp/20261113-b18/send2.md` · 工作目录 `atg-wt/52-send`（分支 `feat/52-send`，我不碰 git）
> 前任现状页：`state/handoff/send.md`（B17-2 机制、冒烟命令、8 轮自审、待人定 4 条）

## 做到哪

- [x] 报到（2026-11-13）
- [x] 1. 解冲突（9 个文件，冲突标记清零）
  - Rust：`backend.rs` / `fake.rs` / `td.rs` 的 `use crate::model` 取并集（`FailedInfo`、`ChatTarget`、`Me` + `ForumTopicState`）；`Backend` trait 两组方法都留（send 的 `gate`/`me`/`chat_target`/`can_reply`/`failed_info`/`submit` + watch 的 `chat_open`/`chat_close`/`forum_topic_get`）；`args.rs` 取 send 的 `Send(SendArgs)` + watch 的 `Watch(WatchArgs)`；`tests/cli.rs` 的 `DONE` 加 `watch`、`send`、`resend`，`PLACEHOLDERS` 去掉两者
  - 契约：对照合并基础版本，每行都是只一边改过，取改过的那份；`actions.md` `chat_action` 取 main、`messages_deleted`/`send_succeeded` 取 send；`cli.md` `send` 取 send、`watch` 取 main；`config-privacy.md` 断开行取 send（150 秒 + 发送类报结果未知）、流式订阅一节取 main、能转的列表合并（加 `watch`（流式））；`errors.md` `disconnected` 以 send 为底，补上 main 的两条 watch 说明；修订记录两边都留
  - **统一数字**：main 侧新写的「watch 不用 90 秒帧超时」改成 150 秒（代码 `FRAME_TIMEOUT` 已是 150）；修订记录里的 90 是历史，不动
  - 抽读自动合并的 `forward.rs` / `serve.rs`：watch 的流式转发与 send 的 `forward` 并存，`request()` 抽出共用，语义没坏
- [x] 全量（名额 2026-11-13）：fmt 过（补格式化 localauth.rs 刻度尺测试）· clippy -D warnings 过 · test 368 过 / 0 败 / 1 忽略 · check-no-debug 过；已报大总管提交合并
- [x] 修前任遗漏：`examples/live_wait_phone.rs` 缺 `TdConfig.gate`，`--features tdlib` 编不过 → 补上（非直发）；之后 tdlib 特性 clippy 过
- ⚠️ 环境阻塞：`_build/20261025-171733/tdlib` 的 libtdjson 链 Homebrew openssl@3，本机只剩 openssl@4 → tdlib 特性测试与冒烟 1–3 / B 都起不来；已报大总管待定（A 装回 openssl@3 · B 重编 TDLib）
- [x] 2. 第 9 轮自审（atg-review，3 视角，2026-11-13）：P0 0 · P1 2 · P2 6（去重后 5）· P3 5
  - P1① `src/atg-cli/README.md` 停在第 6 块，还说直发「没接上」→ 补 history/links、send/resend、watch、serve，细节指向 `cli.md`
  - P1② 确认框只数换行符、不管系统折行，表头一折正文先被截 → `gate::fits_confirm_box` 改按视觉行（每行 40 列，全角 2 列，6 个视觉行，120 字），加回归测试 `confirm_box_counts_wrapped_lines_not_newlines`；契约 `actions.md` 同步。三个数仍是估计，冒烟 B 的刻度尺（每行 60 字）能同时量出列宽和行数
  - P2 修：`args.rs` 全局选项（`--profile` / `--tz` / `--redact` 实际作用范围）、`serve` 帮助补 send/resend/watch、`--text-file` 写明 Telegram 4096 字上限；`main.rs` / `forward.rs` / `serve.rs` 头注释补 `watch`；`tests/cli.rs` DONE 补 `chats`、`serve`，新测试 `command_tables_cover_every_subcommand`（两张表 = `--help` 全部子命令）
  - P3 修：`lib.rs` 不再把 `notify` 标成占位。没修（属 #53 的代码，交大总管开 issue）：`notify.rs` F6/N5 代号没出处、Unigram 没说明是什么、`link.rs:5` 说两条状态线与 `status.rs` 三条不一致、`speech` 帮助里「树」「读法」没解释
  - 自审没挑出、我自己发现的：#52 新文件超共同规矩第 8 条 600 行——`atg-core/src/gate.rs` 1176、`atg-core/src/send.rs` 1490、`atg-cli/src/send.rs` 870（全仓已有多个超限文件，且没有行数棘轮脚本）→ 报大总管，由 arch 线定拆不拆、何时拆
  - [x] 修完验证（名额 2026-11-13）：fmt · clippy · test 370 过 / 0 败 / 1 忽略 · check-no-debug 全过；已报大总管
- [x] 第 10 轮自审（2026-11-13）：P0 0 · P1 1 · P2 4 · P3 7
  - P1：视觉行把拉丁字母算 1 列、假定任意处断行；框是比例字体、按词折 → 空格 1 列、其余一律 2 列，模拟按词折行（长词才按字断），加测试；刻度尺加两行大写 M、注释写清段→列换算；`actions.md` 同步
  - P2：README 发消息段改成指针（原来抄了 10 分钟 / 30 秒 / 退出码，还漏了 24 小时）；`main.rs` `no_redact` 文档；刻度尺单位
  - P3 修：常量三个都标「估计值待实测」并解释「冒烟 B」「自审第 N 轮」；`confirm_body` 注明「行」是原文行；README `--fake`。没修：`gate.rs` 用「最终 id」、`send.rs` 用「正式 id」两种叫法；README 前后体例不一
  - 按架构守则第 6 条（#60）：第 9 轮补的命令清单（README、serve 帮助、main/forward/serve 头注释、--redact/--tz 帮助）全改为指向 `cli.md` / `config-privacy.md` 的指针
  - [x] 修完验证（名额 2026-11-13）：fmt · clippy · test 370 过 / 0 败 / 1 忽略 · check-no-debug 全过
- [x] 第 11 轮自审（2026-11-13，带明确 target）：P0 0 · P1 2 · P2 5（1 条不成立）· P3 5
  - P1①：默认模式下正文只能很短，帮助与契约没说 → `send --text` 帮助、`cli.md` send 行、README 写明
  - P1②：制表符没转义，框里 tab 远宽于 2 列，能把后半句挤出框 → `gate::confirm_visible`（tab → ⇥），`fits_confirm_box` 见残留 tab 一律拒，加测试 `tabs_never_reach_the_confirm_box`
  - 顺带：`too_long_to_confirm` 提示原写「或用直发模式」，等于教 AI 自己绕过人的确认 → 改为只叫拆短（已报大总管，列进待人定）
  - P2：README / `main.rs` 的指针改指 `tests/cli.rs` 的 DONE / PLACEHOLDERS（cli.md 表里没有「已做」列）；cli.md send/resend 行补 `--redact`；「--again 没说」不成立（`out.rs:137` 已补）
  - P3：修订记录挪到末尾按时间排；刻度尺换算口径；写明按词折行量不到。没修：`Global` 的 tz/profile 范围 cli.md 全局表里也没写（只给含义指针）
  - [x] 修完验证（名额 2026-11-13）：fmt · clippy · test 371 过 / 0 败 / 1 忽略 · check-no-debug 全过。大总管同意安全措辞改动；第 12 轮起收紧口径（只有发错 / 误发 / 漏拦 / 泄露 / 崩溃算 P1），第 12 轮无 P0/P1 即停自审
- [x] 第 12 轮自审（收紧口径，2026-11-13）：P0 0 · P1 1 · P2 8 · P3 7
  - P1（漏拦）：比汉字宽好几倍的字形（U+FDFD、楔形文字）按 2 列估，能把正文挤出框 → `gate::width_known` 白名单，白名单外每字按一整行；测试 `glyphs_of_unknown_width_take_a_whole_line`；`actions.md` 同步
  - P2 修：默认模式弹框前标准错误提示 `HUMAN_AUTH_PROMPT_ZH`（测试断言）；`actions.md` resend 行「id N 那条」
  - **P2 留给大总管开 issue**：`status` 行字段名 `direct_sent` 没进 cli.md；防重复两个时限在 actions/cli/errors/config-privacy 四处各写一遍（正本应只在 actions.md 第一节第 5 小节）；4096 字上限没进 cli.md / errors.md（`invalid_input` 的 why）；`account-limited` 提示没给完整路径、没说怎么「处理好」；「架构守则」指针等 arch 合入才有文件
  - 顺带修：`errors.md` 与 `error.rs` 注释里 `too_long_to_confirm` 仍写「或用直发模式」→ 改掉（与大总管已同意的安全措辞同一问题）
  - [x] 修完验证（名额 2026-11-13）：fmt · clippy · test 372 过 / 0 败 / 1 忽略 · check-no-debug 全过
  - **大总管定：自审到此停止**（确认框宽度这类估计问题靠冒烟 B 量真值），停止线改按 codex 轮次算；剩余 P2 由大总管开 issue
- [x] 3. 冒烟 1–3 命令核好交人跑（下方「冒烟 1–3」节，已报大总管，等人跑）
- [ ] 4. 冒烟 B（Touch ID）量确认框
- [ ] 5. codex 评审 016（记录 `docs/reviews/016_send-gate-codex.md`；脚本由人 / 大总管跑，本会话分类器拦 `--force-secrets`）
  - 第 1 轮（2026-11-13，gpt-6-astra，high）：P1×5 · P2×2，全部属实。修：防重复记录时钟回拨仍拦、读不了不覆盖；交出计入 30 秒；重发确认框与摘要带回复对象；留痕交出前先写 `submitted`；`Sealed` 字段私有。#6（查重与登记分离）留 issue。验证 test 375 过
  - 第 2 轮：P1×3 · P2×1，全属实。修：空格逐个计宽；重发交出前复核；批量重发结果未知优先报。P2 #4 留 issue。test 380 过
  - 第 3 轮：P1×2 · P2×1，全属实。修：长词逐字排；晚到的账号受限经 `Shared::event` 点亮闸的信号；文字计划行带回复 / 话题。test 382 过
  - 第 4 轮：脚本 `_tmp/reviews-016/run-round4.sh` 已交大总管，等结果（P1 走势 5 → 3 → 2）
- [ ] 6. 回报

## 要人定的事

- ✅ 人定（2026-11-13）：阿拉伯文 / 泰文等宽度估不准的字符，长正文在默认模式下被拒——接受，宁可多拒

（接前任 4 条，收尾时整理）

## 冒烟 1–3 结果（2026-11-13，人授权、大总管代跑，正式服务器 main 档案，只发收藏夹）

| 条 | 预期 | 实际 | 核对 |
|---|---|---|---|
| 1 直发 | 退出码 0 | 0，1330 ms | ✅ |
| 2 回复第 1 条 | 0、`reply_kept:true` | `result` 成功、`reply_kept:true`，606 ms（zsh 下没打印退出码，以结果行为准） | ✅ |
| 3 原样重发 | 8、`send_refused` / `duplicate_recent`，什么都没发 | 8、`duplicate_recent`，「23 秒前……（结果：已发出）……确要再发一次加 --again」，`last_outcome:succeeded` | ✅ |

注：跑的是第 12 轮修复后、codex 016 第 1 轮修复前的代码；第 1 轮修复动了留痕写入时机与防重复读写，冒烟 B 时顺带再看一眼 `direct-send.log` 是两行。

## 冒烟 1–3：人要跑的命令（2026-11-13 核过）

核对项：`send` 参数 `--text` / `--reply-to` / `--again` 与全局 `--unsafe-direct-send` 与 `src/atg-cli/src/args.rs` 一致；`--manifest-path` 指本 worktree；在 MAIN 目录运行用的是 MAIN 的 `.cargo/config.toml` runner（`scripts/dev-sign-run.sh`，自动签名 + 带 TDLib 库目录）；TDLib 目录 `_build/20261025-171733/tdlib` 依赖的 openssl@3 大总管已装回。

**前提**：先确认没有 GUI / `serve` 占着 main 档案。只发收藏夹（`saved`），内容是编的。每条整行复制，前面加 `! ` 回车。

1. 直发一条到收藏夹 → 预期退出码 0；记下最后一行 `result` 的 `message_id`：

```
cd "/Users/allenchen/xingan-chen/B-MyCreate/5-App/telegram accessibility " && LOCAL_TDLIB_PATH="/Users/allenchen/xingan-chen/B-MyCreate/5-App/telegram accessibility /_build/20261025-171733/tdlib" cargo run -q --manifest-path "/Users/allenchen/xingan-chen/B-MyCreate/5-App/atg-wt/52-send/Cargo.toml" -p atg-cli --features tdlib -- --unsafe-direct-send send saved --text "atg #52 冒烟测试一（编的测试文字）"; echo "退出码 $?"
```

2. 回复第 1 条 → 预期退出码 0、`result` 里 `reply_kept:true`（把 `第1条的message_id` 换成数字）：

```
cd "/Users/allenchen/xingan-chen/B-MyCreate/5-App/telegram accessibility " && LOCAL_TDLIB_PATH="/Users/allenchen/xingan-chen/B-MyCreate/5-App/telegram accessibility /_build/20261025-171733/tdlib" cargo run -q --manifest-path "/Users/allenchen/xingan-chen/B-MyCreate/5-App/atg-wt/52-send/Cargo.toml" -p atg-cli --features tdlib -- --unsafe-direct-send send saved --reply-to 第1条的message_id --text "atg #52 冒烟测试二：回复上一条"; echo "退出码 $?"
```

3. 10 分钟内原样重发第 1 条 → **预期被拦、什么都不发**：退出码 8，`error` 为 `send_refused`、`reason:"duplicate_recent"`（**不要加 --again**）：

```
cd "/Users/allenchen/xingan-chen/B-MyCreate/5-App/telegram accessibility " && LOCAL_TDLIB_PATH="/Users/allenchen/xingan-chen/B-MyCreate/5-App/telegram accessibility /_build/20261025-171733/tdlib" cargo run -q --manifest-path "/Users/allenchen/xingan-chen/B-MyCreate/5-App/atg-wt/52-send/Cargo.toml" -p atg-cli --features tdlib -- --unsafe-direct-send send saved --text "atg #52 冒烟测试一（编的测试文字）"; echo "退出码 $?"
```

注意：第一次跑要编译 tdlib 特性（重活，人跑时大总管那边别同时发名额）；每条开头标准错误有一行直发提示（正常）。跑完把三个退出码和第 3 条的 `reason` 告诉 send2 即可；两条测试消息事后可在 Telegram 里手动删。

## 大总管已定（2026-11-13）

- 超 600 行：#52 先合，`gate.rs` / core `send.rs` / cli `send.rs` 进基线，arch 第二段最先拆；我现在不拆，**PR 描述里写一句**
- #53 那 4 条 P3 由大总管转 arch
- 第 10 轮自审照跑（只读）；名额留给人跑冒烟
