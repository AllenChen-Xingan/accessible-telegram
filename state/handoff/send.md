# 现状页 · 手下 send（issue #52：M2.1 发送闸 + M2.2 `atg-cli send`）

> 批次 b17 · 交代 `_tmp/20261026-b17/send.md` · 工作目录 `atg-wt/52-send`（分支 `feat/52-send`，我不碰 git）

## 手上是什么

M2.1 发送闸（`atg-core::gate`）+ M2.2 `atg-cli send` / `resend`，接进 `serve` 转发；契约按 003 第 1、2、3、7、8 条改。

## 做到哪

- [x] 读完：common 规矩、交代、调研 003（README + decision）、路线图 M2.1/M2.2、001 拍板 9–11、契约 actions/cli/errors/config-privacy、现有代码（td / fake / link / pump / serve / forward / chats）
- [x] B17-2 机制设计（下节），已报大总管
- [x] `atg-core::gate`：凭据、摘要、三种开法、验凭据、防重复记录（大总管同意，0600 原子写、密钥带标签派生）
- [x] `atg-core::send`：准备（两次预览比对）、交出、等结果（三种结尾）、重发；`localauth`（Touch ID / 系统密码，60 秒）
- [x] Backend 加 `gate` / `me` / `chat_target` / `can_reply` / `failed_info` / `submit`（只收 crate 内才能造的 `Sealed`），真 + 假
- [x] `atg-cli send` / `resend`、serve 转发；转发断在半路按结果未知报；两帧间隔 90 → 150 秒
- [x] 契约改动（003 的 1、2、3、7、8 + B17-2；修订记录「按 003 大总管采纳 2026-10-26」）
- [x] 测试 / clippy / fmt / check-no-debug 全过（2026-10-26 第 3 轮名额）
- [ ] 自审两轮无 P0/P1（第 1 轮在跑）；待 codex 补审（#32）
- [ ] 收藏夹真发冒烟（经大总管和人同意）

## B17-2 重复提交拦截：机制（待大总管看）

人已选 A（拦下）。设计：

1. **管谁**：只管 `direct_mode` 凭据（直发模式，含转给以直发启动的执行方的命令）。`test_exempt`（测试服务器 + 收藏夹）不拦（决定原文）；`human_auth` 每次人按 Touch ID，本身就是确认，不拦
2. **比什么**：凭据摘要（`actions.md` 第一节第 4 小节第 5 条：请求字节 + 选项 + 回复目标 + 最终话题 + 自己账号 id + 服务器类别）再过一道**带密钥的 HMAC-SHA256** 得指纹。密钥不新建钥匙串条目：由本地库钥匙（已在钥匙串）派生 `HMAC(库钥匙, "atg.send-guard.v1")`，只在内存里。所以同目标 + 同内容 + 同回复 + 同话题 = 同指纹；短句也反推不回内容
3. **记在哪**：档案目录 `send-guard.json`（0600，跨进程可见；同一档案同时只有一个进程，靠档案锁，不用另加锁）。每条：时间（Unix 秒）· `chat_id` · 指纹 · 状态（`submitted` / `succeeded` / `unknown` / `failed`）。**不记内容、不记名字**
4. **何时记**：交给 TDLib **之前**写 `submitted`（写不进去就拒直发，同 `direct-send.log` 的规矩）；等到结果后改成对应状态。进程中途死掉，`submitted` 留着，照样算数（结果确实未知）
5. **何时拦**：同 `chat_id` + 同指纹、10 分钟内、上次状态是 `submitted` / `succeeded` / `unknown` → `send_refused(duplicate_recent)`，`error.data` 带 `last_at`（RFC3339）与 `last_outcome`；说明「N 秒前已向同一会话提交过同样的内容（结果：已发出 / 未知），确要再发一次加 --again」。上次是 `failed`（服务器明确没收）不拦——失败的该用 `resend`
6. **放行**：`send --again`（只这一次；经本机接口转发时随参数过去）。放行的那次照样记一条
7. **到期**：每次写时删掉 10 分钟以前的；文件不会越积越大，同一句话 10 分钟后自然能再发
8. **为什么 10 分钟**：要长过「结果未知」的 30 秒（AI 等满 30 秒后重跑正好落在窗口里）；也要短到不至于让人觉得「同一句话被永远拦死」。改它只动一个常量
9. **`resend` 不进这套**：重发对象是某条失败消息的 id，TDLib 重发后旧 id 就没了，同一条重发不了两次

## 人要跑的命令（直发冒烟 1–3，人在 Claude Code 里用「! 」前缀亲手跑）

在本会话或任一会话里，把下面每条整行复制，前面加 `! ` 回车。三条都在 MAIN 目录下运行，编的是 `atg-wt/52-send` 这份代码（`--manifest-path`），签名与 TDLib 库目录由 MAIN 的 `scripts/dev-sign-run.sh` 自动处理（cargo 的 runner，不用手设 DYLD_*）。档案用默认的正式档案 main；只发收藏夹（`saved`），内容是编的。

1. 直发一条到收藏夹，预期退出码 0，最后一行 `result` 里的 `message_id` 记下来给第 2 条用：

```
cd "/Users/allenchen/xingan-chen/B-MyCreate/5-App/telegram accessibility " && LOCAL_TDLIB_PATH="/Users/allenchen/xingan-chen/B-MyCreate/5-App/telegram accessibility /_build/20261025-171733/tdlib" cargo run -q --manifest-path "/Users/allenchen/xingan-chen/B-MyCreate/5-App/atg-wt/52-send/Cargo.toml" -p atg-cli --features tdlib -- --unsafe-direct-send send saved --text "atg #52 冒烟测试一（编的测试文字）"; echo "退出码 $?"
```

2. 回复第 1 条，预期退出码 0、`result` 里 `reply_kept:true`。把 `第1条的message_id` 换成第 1 条结果里的数字：

```
cd "/Users/allenchen/xingan-chen/B-MyCreate/5-App/telegram accessibility " && LOCAL_TDLIB_PATH="/Users/allenchen/xingan-chen/B-MyCreate/5-App/telegram accessibility /_build/20261025-171733/tdlib" cargo run -q --manifest-path "/Users/allenchen/xingan-chen/B-MyCreate/5-App/atg-wt/52-send/Cargo.toml" -p atg-cli --features tdlib -- --unsafe-direct-send send saved --reply-to 第1条的message_id --text "atg #52 冒烟测试二：回复上一条"; echo "退出码 $?"
```

3. 10 分钟内把第 1 条原样再发一次，**预期被拦下、什么都不发**：退出码 8，`error` 是 `send_refused`，`reason:"duplicate_recent"`，说明「N 秒前已向同一会话提交过同样的内容（结果：已发出）……确要再发一次加 --again」（**不要加 --again**）：

```
cd "/Users/allenchen/xingan-chen/B-MyCreate/5-App/telegram accessibility " && LOCAL_TDLIB_PATH="/Users/allenchen/xingan-chen/B-MyCreate/5-App/telegram accessibility /_build/20261025-171733/tdlib" cargo run -q --manifest-path "/Users/allenchen/xingan-chen/B-MyCreate/5-App/atg-wt/52-send/Cargo.toml" -p atg-cli --features tdlib -- --unsafe-direct-send send saved --text "atg #52 冒烟测试一（编的测试文字）"; echo "退出码 $?"
```

注意：每条开头标准错误会有一行直发模式提示（正常）；跑时别有别的进程占着 main 档案（GUI / `serve` 开着会转发给它，那边若不是直发启动就会改成弹 Touch ID）。三条都发生在收藏夹，事后人可在 Telegram 里手动删掉这两条测试消息。跑完只需告诉我三个退出码和第 3 条的 `reason`。

## 要人定的事（攒着，收尾时一起报）

- 发送状态怎么读（003 第 4 条，读法规约）：先按 003 建议做默认，收尾时列出
- Touch ID 框里的那段话（大总管要求，等人实听）：现为「发给 私聊 · 名字 · id 777（，回复 id N 那条）」/「来自命令行，可能是 AI，网络断了也会在恢复后发出」/「共 N 字 M 行：」+ 正文（⏎ 代表换行）；整段超过 120 字或 4 行就不弹框、拒发——保守估计，要在真弹框上量（冒烟 B 时量）
- 长正文怎么确认（自审第 5 轮）：正式服务器默认模式下正文超过 Touch ID 框能放下的 150 字 · A 拒发（现默认，拆短或用直发模式）· B 弹框前在人看得见的地方显示全文（系统通知 / serve 终端），要再设计 · 建议 A 先用，GUI 的确认步骤（M4.2）再给全文
- 直发防重复的两个时限（10 分钟 / 结果未知 24 小时）：大总管已同意，属 B17-2 的参数，人要不要看一眼

## 自审记录

- 第 1 轮（atg-review，3 视角）：P0 0 · P1 2 · P2 13 · P3 15。P1 都修：① 交出后断线 / 网络错被记成「失败」→ 改按结果未知报、防重复照样拦重跑（加回归测试）；② 重发半进防重复 → `Gated::dedup` 分开，重发不查不记（加测试）。P2 修：重发请求与摘要同一份 JSON、留痕类型随计划、`FailedInfo` 挪到 model、99966 规则只留一份、两处文档注释错位、帮助文字去黑话、N7 注释、术语表「请求凭据 / 确认凭据」、代号给路径、`resend` 契约签名、`saved` 两条命令同一来源。P3 修了大半；没修：`status` 行两处拼（直发条数写死 0，等 `status` 命令做时一起收）

- 第 2 轮：P0 0 · P1 1 · P2 8 · P3 13。P1：结果未知的消息可能还在 TDLib 待发队列里、下次启动自动续发，而防重复 10 分钟就过期 → 结果未知 / 没收尾的记录留 24 小时（大总管同意 2026-10-26），报错文字说清「会自动补发、别重发、去会话看」。P2 全改：发送全程收成 `send_and_wait`、凭条 `Submitted` 必须交回 `finish`、正文 64 KB 三个入口都查、重发结尾 `error` 行与 send 同一条路、核心不再写死 `--again`、术语 / 话题种类 / `sending_id` / 带钥匙指纹的理由补进注释、契约名与代码名对照。P3 修了大半
- 第 3 轮：P0 0 · P1 1 · P2 7 · P3 11。P1：确认前的「看不见的字符转义」漏了标签字符（U+E0000–E007F）、行分隔符、变体选择符等 → 改按 Default_Ignorable 全集 + 格式类 / 分隔符转义，计划行加 `hidden_chars`，契约写明范围。P2 全改：重发全程收进 core（`resend_and_wait`）、`ChatTarget` / `Me` 挪 model、术语表补 TDLib / 直发模式 / 库钥匙 / 凭条 / B17-2 选 A、`resend` 帮助写明用哪个 id、契约 dry-run 与结果未知的查法。P3 修了一部分（main.rs 文档、`DuplicateRecent` 注释、`phone_allowed` 复用、`chat_id` 可 null 写进契约、`resend` items 取值写进契约）
- 第 4 轮：P0 0 · P1 1 · P2 2 · P3 14。P1：Touch ID 弹框把正文原样放进说明（保留换行、不限长），一串空行就能把真正要发的话挤出可见区 → `confirm_body`：先写字数行数、正文压成一段（⏎ 是换行）、超 150 字明说只显示前段，其余信息放内容之前；契约写明。P2：`SendOutcomeUnknown.chat_id` 改 `Option`；64 KB 上限挪进 core（`TEXT_MAX_BYTES`，`invalid_input{why:too_long}`）。P3 修：时钟容差注释、重发拒发几种情形与话题群拒发的理由、收藏夹等式、`visible` 文档、`execute_send` 改名、模块文档补 `resend_and_wait`。没修：status 行两处拼、错误行三处拼、`Me`/`ChatTarget` 的再导出、两个 `target()` 同名不同义
- 第 5 轮：P0 0 · P1 1 · P2 1 · P3 15。P1：超 150 字时弹框叫人「看终端」，可发起方终端多半是 AI 的输出 → 正式服务器默认模式下正文放不下就拒（`too_long_to_confirm`），列进待人定。P2：`resend` CLI 层补 3 个测试（成功、又失败、不是失败的）；假后端发出的消息留进历史。P3 修：L0/L2 释义、随机数登记的意思、摘要为何含账号与服务器、两句「断网也会发」常量放一起、store 释义、lib.rs 分组
- 第 6 轮：P0 0 · P1 1 · P2 2 · P3 13。P1：`resend` 的 Touch ID 框只给 id 不给正文 → 准备时取每条正文，框与计划行给全文，凭据绑正文（`resend_digest`），非纯文字或合计超 150 字的拒。P2：`project_one` 注释分清历史请求凭据与确认凭据；「待发被删」为何按未知写清。P3 修：转给直发执行方的 send/resend 也先出直发提示与 status 行；`history::resolve` 退回私有；lib.rs 分组；`again` 对重发无效的注释；`Fed` 顺序理由；`confirm_body` 去掉走不到的截断支
- 第 7 轮：P0 0 · P1 2 · P2 2 · P3 13。P1①：新加的带文字类型（`ChatTarget`、`Draft`、`TextPlan`、`ResendItem`、`ResendPlan`）派生了完整 `Debug` → 改成只在本 crate 单测里派生、登记 `redact.rs`，`tests/privacy.rs` 加暗号测试。P1②：「放得下」只数正文 → 按整段说明的字数与行数判（`gate::fits_confirm_box`，220 字 / 6 行，估计值待实测），长会话名、重发条数多都拒（加测试）。P2：消息 id 读作「id N 那条」不再说「第 N 条」；结果未知那行的临时 id 字段改名 `temp_message_id`。P3 修：「断网」那句改清楚、`FailedInfo` 各字段注释、目标里的数字标「id」
- 第 8 轮：P0 0 · P1 2 · P2 4 · P3 13。P1①：`account_limited` 只记在内存，下个进程就忘 → 落盘为档案目录的 `account-limited`（契约原已要求），人删文件才恢复（加跨进程测试）。P1②：确认框 220 字 / 6 行没实测、系统还会加前缀 → 收紧到 120 字 / 4 行直到真弹框量过（冒烟 B 时量）。P2：超时把 Touch ID 框收掉（`invalidate`）；64 KB 与 Telegram 4096 字的关系写清；`direct_sent` 接上真计数；`SendParams` 的 Debug 只在单测派生。P3 修：`Waited` 登记脱敏、核心字段改名 `temp_message_id`、单调时钟释义
- 第 9 轮：大总管改了顺序——冒烟先做（量出确认框容量），再跑第 9 轮
- 冒烟（2026-10-26）：第 8 轮修完测试全过；`--features tdlib` 编好、td 单测 2 个过。方案 1（直发发收藏夹）被本会话权限分类器拦下（Safety Bypass Flag），**什么都没发**，没绕；已报大总管，等人选：放行 / 人自己跑 / 先做 B（Touch ID）
- 冒烟：人已同意（经大总管）方案 1、2、3 与 B；顺序：第 3 轮过 → 领名额前置编译 → 1、2、3 → B 前先报大总管「要弹 Touch ID 了」等回「可以」。只报统计

## 已知没做 / 偏差（收尾时报）

- `pending` / `status` 列待发消息、`pending_cancel` 没做（不在本交代）；`send_outcome_unknown` 的说明仍叫人「查看待发消息」
- N7 第二测试号例外没做（大总管同意，M2.7 用）
- 正式服务器 Touch ID 的弹框没有真跑过（要人在场）
- 003 待实测（预览对话题群 / 被删回复的返回、待发被删是否送达、中途退出续发）没跑，要测试服务器
- `direct-send.log` 每条在等到结果后写一行（不是交出时）；进程在等结果途中被杀则缺这一行（防重复记录里有 `submitted`）

## 下一步

收第 1 轮自审结果、修；第 2 轮；然后申请收藏夹真发冒烟。
