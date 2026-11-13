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
- [ ] 2. 第 9 轮自审
- [ ] 3. 冒烟 1–3 命令核好交人跑
- [ ] 4. 冒烟 B（Touch ID）量确认框
- [ ] 5. codex 评审 016
- [ ] 6. 回报

## 要人定的事

（接前任 4 条，收尾时整理）
