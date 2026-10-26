# 交接 · 手下 speech · issue #19（第一刀第 6 块：speech v0 + 快照）

> 2026-10-26 · 交代 `_tmp/20261026-b15/speech.md`（在 MAIN）· 大总管 `telegram-accessibility-1a` · 分支 `feat/19-speech`
> 状态：**可以收尾**（2026-10-26：读法按 007 人定、五轮自审 P0/P1 清零、235 过）。待 codex 补审（#32）；`--level` 形状 #45；不认识的更新 #44

## 一句话

`atg-core::speech` 做成渲染器 v0，按 `specs/content-tree.md` 第九节与 style 三份稿（#20，`specs/style/{chat-item,message,service}.md`，在分支 `docs/20-style-v0`）的默认正例：R1「隐藏内容，按 D 展开」（键由调用方给；CLI、会话项不读键）· R3 只读钟点 · R7「林小雨撤回了一条消息，19:36」（自己读「我」，记不到读「未知发送人」，没记到时间就不读时间，会话项不带时间）· 系统消息里的自己读「我」、频道里读「频道名 / 频道头像」· 没做读法的系统消息读「暂不支持的消息」。`atg-cli speech --fixture PATH [--level brief|chat_brief] [--tz]`。

## 大总管授权 2026-10-26

1. `specs/contracts/cli.md`：`speech` 那一行改成 `--fixture PATH [--level brief|chat_brief]`；`--tz` 写法登记。修订记录已写
2. 读法夹具整文件移到 `src/atg-core/fixtures/speech/`（`basic.json`、`entities.json` 内容未改，纯移动，**提交时请单独一步**）；`specs/contracts/testing.md` 加 `speech/` 一行。修订记录已写
3. 已注销：model `User.deleted`、`Chat.peer_user`；`ChatItem.peer_deleted`。**#18 的 `basic_chats_json` 快照逐字节未变**（第一轮已验证，本轮再验）
4. 收藏夹：`ChatItem.saved_messages`；自己的 id 来自新事件 `my_id_changed`（`updateOption my_id` 进共用 `tdjson::event`，正式路径与夹具同一入口；`actions.md` 第一节第 7 小节已加一行）
5. `specs/content-tree.md`：`chat_item` 两字段、`tombstone.outgoing`、`service.outgoing / in_channel`、`UserRef.is_me / deleted`，一条修订记录
6. 新加的可选字段**全部为假不输出**，`v` 不加一

## TDLib 核实

- 私聊标题 = 对方「名 姓」（`DialogManager.cpp:2062` → `UserManager.cpp:5708`）；注销账号无名、无号 → 标题空；`user.type` = `userTypeDeleted`（`UserManager.cpp:10495`）
- `my_id`：登录时 `AuthManager.cpp:348` `set_option_integer("my_id", …)`，TDLib 自带 `cli.cpp:311` 也靠这条认自己

## 和 style 三份稿对齐（2026-10-26 版）

style 标「现状」的 9 处（加上 service 后来补的「移出了我」）全部改成正例：

| 稿 | 例 | 原来 | 现在 |
|---|---|---|---|
| message | M13–M16 | 隐藏内容没有展开键 | 带「按 D 展开」（`SpeechCtx.expand_key`） |
| message | M17 | 剧透 + 18+ 读两遍「隐藏内容」 | 只读一次 |
| message | M18–M21 | 「已撤回的消息」 | R7 句式；自己读「我」；没时间不读时间（V15 A） |
| chat-item | C7 | 「未命名会话」 | 「已注销的账号」 |
| chat-item | C12 A/B/C | 「已撤回的消息」 | 「撤回了一条消息」（私聊省名，V3 A）/「周大山撤回了…」/「我撤回了…」 |
| service | S7 | 频道里读「群名」 | 「频道名」「频道头像」（`Service.in_channel`） |
| service | S11 | 「我自己把群名改为…」 | 「我把群名改为…」（`Service.outgoing`） |
| service | S12 | 「周大山添加了我自己」「移出了我自己」 | 「添加了我」「移出了我」（`UserRef.is_me`，V17 A） |
| service | S15 | 「一条系统消息」 | 「暂不支持的消息」（V19 B，规约先于代码） |

待人定题一律按稿里「默认」：V1 A · V2 A · V3 A · **V4 A**（私聊空标题一律「已注销的账号」，不看 `peer_deleted`）· V5 A · **V6 C**（收藏夹照读自己的名字，`saved_messages` 数据已有、读法暂不用）· V8–V13 A · V14 B · V15 A · V16–V18 A · V19 B · V21–V23 A。第一轮我按建议选了 V6 A，本轮按大总管「默认为准」改回 C。

夹具：`chat_items.json` 覆盖 C1–C15（含 C12 的 A/B/C 三种）；`cases.json` 覆盖 M1.5 情形 + M17/M19/M21 + M28 + S7/S11/S12/S14/S15。

## 数据缺口（大总管列的 ④–⑧）

- ④ `UserRef.deleted` ✓ · ⑤ 自己读「我」：撤回 `Tombstone.outgoing`、系统消息 `Service.outgoing` + `UserRef.is_me` ✓ · ⑥ `SpeechCtx.expand_key` ✓ · ⑦ 系统消息带会话类型：做了便宜版 `Service.in_channel`（只区分频道）✓
- ⑧ 用户名、`author_signature`：**没做**（要改 model 的 User、tree 的 Sender / Head，牵动面大），大总管开 issue

## 文件

- `src/atg-core/src/`：`speech.rs` · `model.rs` · `tdjson.rs` · `tree/mod.rs` · `project.rs` · `store/mod.rs`
- `src/atg-core/fixtures/`：`speech/{basic,entities}.json`（移来）· `speech/{cases,chat_items}.json`（新）· `names.txt`（新）
- `src/atg-core/tests/`：`snapshots.rs` · `coverage.rs` · `reducer.rs` · `privacy.rs` · 快照
- `src/atg-cli/`：`src/speech.rs`（新）· `src/args.rs` · `src/main.rs` · `tests/cli.rs` · `README.md`
- `specs/`：`contracts/cli.md` · `contracts/testing.md` · `contracts/actions.md` · `content-tree.md`

## 验证

- 第一轮（名额 2026-10-26）：`cargo test --workspace` 231 过 0 败 · fmt · clippy（默认 + `tdlib-api`）· `check-no-debug` 全过
- 第二轮（名额 2026-10-26）：231 过 0 败 · 全部检查过
- 第三轮（自审修后，名额 2026-10-26）：231 过 0 败；快照只多一行「陈一一通过邀请链接加入了频道」
- 第四轮（按 007 人定改读法后）：233 过；快照差异逐行对过 007，已报大总管。#18 的 `basic_chats_json` 多一行 `"in_channel": true`（新可选字段在频道里为真，大总管接受，PR 里写一句）
- 第五轮（第三、四轮自审修后，名额 2026-10-26）：**234 过 0 败** · fmt · clippy（默认 + `tdlib-api`）0 警告 · `check-no-debug` 全过。另用二进制实测夏令时：纽约时区 EDT / EST 两侧当地 19:36 都读「19:36」；固定 `--tz -04:00` 按固定偏移读 19:36 / 20:36

## 自审

- 第一轮（`atg-review`，3 视角）：P1×2（收藏夹正式路径接不上 → 改成 `my_id_changed`；注释指向本分支没有的 style 稿 → 注明分支）· P2×3（「默认 A」没解释、tombstone 字段未登记、README 过期）· P3×7，全部已改
- 第二轮：**P0 / P1 零**。P2×4：已修 3（注释写明各 V 题选项与改法、频道里加入 / 退出也读「频道」、`UserRef.deleted` 暂不读的原因）；1 条（不认识的 TDLib 更新在 `brief` 里读成「暂不支持的消息」，World 原有设计）大总管选 C，开 **#44** 留后续。P3 多数已修
- 第三轮（按 007 改后）：P1×2——注释指向本分支没有的 007 / style 稿（注明位置）；快照与代码不一致（逗号改动后没重跑，复测已对上）。P2×2 已修（README 写清比对前提；新增测试 `fixture_names_come_from_names_txt`，夹具列表按目录取）
- 第四轮（短）：P1×2 同一件事——007 与 style 三份稿还没提交，几十处「Vn 人定」暂时指向不存在的文件。**大总管认定属于依赖提交（由大总管提交），不算本块未清**。P2 已修：夏令时（`SpeechCtx.offset_at`，每个钟点按自己的时间点取偏移）、`speech --fake` 改报 usage、代号表补「定标 / 实听 / V1–V23」。1 条（`--level` 混了层级和对象）选 C 开 **#45**
- 第五轮（只审 `offset_at`，单视角）：**没有 P0/P1**。逐项核过：跨夏令时按每条自己的时间点取偏移；三处钟点（消息、系统消息、撤回）都走 `hm()`；极端时间值不 panic；unsafe 块正确。P2×2 大总管定「现在修」，**已修**（`result.data` 加 `tz`，跟系统时区时 `utc_offset_s` 为 null，`cli.md` 已改；单测 `offset_at_is_per_timestamp`），复测 **235 过 0 败**，全部检查过。原题：① 跟系统时区时 `result.data.utc_offset_s` 只对「此刻」成立，跨夏令时的行对不上——建议改成 `null` 并加 `tz: "system" | "fixed"`（改 `cli.md`）；② `offset_at` 这条路没有自动化测试——建议注入假偏移函数的单元测试（以 2026-11-01 为界 -4h / -5h，两条都读 19:36）。P3：`localtime_r` 失败回落 UTC 只在年份越界时发生；GUI 多线程时别在运行中改 `TZ`
- 按停止线收工；待 codex 补审（#32）

## 要人定的事

1. **合并后的表情紧挨文字时要不要加逗号**（V12 的细节，style 稿没写）：「好的👍👍👍」读成 A「好的，3 个👍」（现默认，协调会话定）或 B 紧挨文字的不合并「好的👍👍👍」。代价：A 多一个停顿；B 那种情况下会念三遍。建议 A
2. **单处遮挡的按键提示也放末尾**（V13 推及，style 与 speech 形状一致）：「图片，隐藏内容，山顶的风景，按 D 展开，19:33」。待人确认

读法 V1–V23 已人定（`docs/plan/007_decisions-b15.md`），代码与快照已按 007 改。

## 下一步

等大总管验收；验收后在这里写「已收尾」。之后 style 人改定 V 题时回填快照（改 `speech.rs` 里对应注释标的那一处）
