# 契约 · CLI（`atg-cli`）v0

> 2026-10-25 · 手下 infra 起草 · 外壳已与 `tree` 对齐（2026-10-25 回信同意）· 状态：**草稿**；末尾「待人拍板」C1、C2 **已拍板**（`docs/plan/003_decisions-b2.md`），正文已同步
> 管：子命令与参数风格、每行 JSON 的外壳、错误输出与退出码、流式输出、给 AI 的自描述
> 不管：外壳里 `data` 的内容结构（归 `tree`：`specs/content-tree.md`）；动作语义（`actions.md`）；错误种类（`errors.md`）
> 依据：`.42cog/intent.md`「AI 友好」①命令行优先；`docs/plan/001_arch-stack.md` 第一节第 2 小节（`atg-cli` 职责）

## 一、定了什么

### 1. 三条总规矩

1. **标准输出只放 JSON 行**（JSON Lines：一行一个 JSON 对象，UTF-8，无颜色码、无进度条）。给人看的话（提示、进度、确认问句）一律走**标准错误**或 `/dev/tty`，机器不解析它
2. **每条命令以恰好一行 `result` 或 `error` 结尾**。读的一方看到它就知道结束了，不用猜
3. **退出码和最后一行一致**：`result` → 0；`error` → 第 4 小节的码

**例外只有三个**：`schema`（输出一个 JSON 文档，无结尾行）· `--format text`（给人看，不是 JSON）· `--help`（给人看）。`version` 不是例外：一行 `kind:"version"` + `result`。

### 2. 子命令

风格：`atg-cli <名词或动词> [位置参数] [--长选项]`。id 一律用数字；选项只用长形式（`--limit`），常用的给短形式（`-n`）。会话 id 处可以写 `saved`，表示自己的收藏夹（闸照样按用户 id 判定，`actions.md` 第一节第 4 小节）。

| 子命令 | 作用 | 对应动作 | 任务 |
|---|---|---|---|
| `login` | 给人用的交互登录：手机号 → 验证码 → 二步验证密码；输入从 `/dev/tty` 读，不回显 | `auth_*` | T01 |
| `auth state` · `auth phone` · `auth code` · `auth password` · `auth email` · `auth email-code` · `auth resend` | 分步登录，给 AI 和脚本：每步输出一行 `kind:"auth"`（只有 `AuthStep`，不带提示语）。**秘密值不进命令行参数**：从 `/dev/tty` 读；只有 `test` 档案上可从 `ATG_TEST_PHONE` 取号、按测试号规则推出验证码（`testing.md` 第一节第 5 小节） | `auth_*` | T01 |
| `logout` | 登出本机（须终端确认） | `logout` | — |
| `me` | 自己是谁 + `saved_chat_id` | `me` | — |
| `pending [cancel CHAT_ID MSG_ID]` | 列出待发出的消息；`cancel` 撤销一条（`actions.md`「待发出的消息」） | `status` `pending_cancel` | T04 |
| `status` | 内核 / 授权 / 连接三条线、合成后的用户状态、**待发出的消息** | `status` | T10 |
| `chats [--list main\|archive] [-n N] [--unread] [--cursor C]` | 列会话，一行一个 | `chats_load` + Store | T02 |
| `next-unread [--after CHAT_ID]` | 下一个有未读的会话；没有就零行 + `result{count:0}`，退出码 0（「没有」不算错） | Store | T09 |
| `history CHAT_ID [-n N] [--from MSG_ID] [--local]` | 从新到旧列消息，一行一条。`--from` **不含**起点那条；CLI 内部循环补满 N 条（TDLib 首次常只回 1 条）。**不标已读** | `history` | T03 |
| `links CHAT_ID MSG_ID` | 列出这条消息里的链接（文字 + 地址） | `message_links` | T06 |
| `download CHAT_ID MSG_ID` | 下载这条消息的附件，输出进度事件与本地路径 | `file_download` | T07 |
| `open (--link URL \| --file CHAT_ID MSG_ID)` | 用系统打开；协议与类型白名单，先输出 `plan` 再确认（`actions.md` `open_link` / `open_file`） | `open_link` `open_file` | T06 T07 |
| `mute CHAT_ID (on\|off)` | 静音 / 取消静音 | `mute` | T05 |
| `delete CHAT_ID MSG_ID... [--dry-run]` | 删除消息，**给所有人**（S1 人定进 MVP；只删自己这边第二批）：先逐条输出 `plan`（删哪条 + 简短读法），按闸确认后输出结果；`--dry-run` 只输出 `plan` | `delete_prepare` `delete` | T04 |
| `resend CHAT_ID MSG_ID...` | 重发失败的消息：先逐条输出 `plan`（写明会不会去掉回复关系），确认后逐条输出结果 | `resend_prepare` `resend` | T04 |
| `show CHAT_ID MSG_ID` | 一条消息的全部细节（含被回复的那条），`speech` 带 `detail`。**不标已读** | `message_get` `replied_message_get` | T06 T07 |
| `search chats QUERY [-n N]` / `search messages QUERY [--chat CHAT_ID] [-n N] [--cursor C]` | 搜索 | `search_*` | T08 |
| `send CHAT_ID (--text T \| --text-file PATH) [--reply-to MSG_ID] [--dry-run]` | 先输出一行 `plan`，再按闸要求确认，**等到 `send_succeeded` / `send_failed` 才输出结尾行**；30 秒内都没等到 → `error(send_outcome_unknown)` 带临时 id，**不许重发**，用 `pending` 查；`--dry-run` 只输出 `plan` | `send_text_prepare` `send_text` | T04 |
| `watch [--chat CHAT_ID]` | 订阅更新，持续输出事件，Ctrl+C 结束 | `subscribe` | T05 T10 |
| `draft set CHAT_ID (--text T \| --text-file PATH)` · `draft clear CHAT_ID`（第二批） | 写 / 清草稿；先输出 `plan`，确认规则与 `send` 相同 | `draft_set` `draft_clear` | T04 |
| `play CHAT_ID MSG_ID [--mark-listened]`（F3 人定进 MVP；声音从新建的 `atg-audio` 出，F8） | 前台播放语音。标准输入每行一条控制命令：`pause` · `resume` · `seek +N` / `seek -N`（秒）· `speed X` · `stop`；标准输出每次状态变化一行 `kind:"playback"`（`state` · `position_s` · `duration_s` · `speed`）；播完或 `stop` 输出 `result`（`reason:"finished"\|"stopped"`），退出码 0；Ctrl+C 按 130。默认不标已听，`--mark-listened` 走 `mark_listened` 的把关（A1） | `file_download` `mark_listened` | T07 |
| `speech --scenario NAME [--chat CHAT_ID] [--level brief\|detail\|chat_brief]` | 只打印读法，用夹具场景 | 无（直接调 `speech`） | 定标 |
| `schema [COMMAND]` | 机读自描述（第 6 小节） | — | — |
| `version` | 版本、TDLib 版本、外壳版本 `v` | — | — |

**全局选项**（放在子命令前后都可以）：

| 选项 | 作用 |
|---|---|
| `--test-dc` | 等于「选 `test` 档案，并断言它是测试服务器档案」；断言不过就拒绝启动（`config-privacy.md` 第一节第 2 小节） |
| `--profile NAME` | 选本地档案，默认 `main`。档案是哪种服务器由档案自己的标记决定，不由选项决定 |
| `--fake SCENARIO` | 用假后端，读 `src/atg-core/fixtures/scenarios/<SCENARIO>/`（也接受路径），不联网（`testing.md`） |
| `--format json\|text` | 默认 `json`；`text` 给人看，每行一句读法（`speech.brief`） |
| `--redact` | 打码输出，用于贴进 issue / 发给别人：**白名单**——只留 `type`、计数、时间、状态、错误码；正文、说明、名字、用户名、手机号、文件名、链接、`speech` 全部换成「类型 + 长度」；id 换成**每次随机加盐**的哈希（同一次输出内一致，跨次不可对照、也无法穷举反推）。日志打码另有脚本，见 `config-privacy.md` |
| `--unsafe-direct-send` | **直发模式**（001 第七节第 11 条）：本次运行 L2 免确认（范围见 `actions.md` 第一节第 5 小节，D1），到进程退出为止。只认这个参数（不读环境变量、配置文件）；第一行 `status` 带 `direct_send:true`，标准错误打醒目提示；每条直发写 `direct-send.log`。测试档案仍只许发收藏夹。细则只在 `actions.md` 第一节第 5 小节 |
| `--wait` | 遇到 L0 的限流就等够再继续（默认不等，立刻返回 `rate_limited`，`errors.md` 第一节第 5 小节） |
| `--tz OFFSET` | 读法里时间按哪个时区读，默认取系统时区 |

`send` 的文本不许从标准输入读：标准输入可能是 AI 拼进去的管道，内容由 `--text` / `--text-file` 明确给出。怎么确认（Touch ID、测试豁免、直发模式）由闸定，见 `actions.md` 第一节第 4、5 小节。`history` / `show` 永不标已读。

**界面开着时**（P1 人定一开始就做共用）：档案被 GUI（或另一个 CLI）占着时，`atg-cli` 自动经本机接口把命令转给它执行，输出与直跑完全一样，调用方不用改任何参数（例外：直发参数听执行方的——执行方不是直发启动就照常确认，CLI 的 `--unsafe-direct-send` 忽略并在标准错误说明，N1 人定）；`login` / `auth *` 不转发（秘密不过接口）。细则只在 `config-privacy.md` 第一节第 2 小节。连不上才报 `profile_busy`（退出码 9）。

**什么时候算数据齐了**：`chats`、`next-unread` 读的是 Store，Store 靠 `chats_load` 之后陆续到的更新填满。这两条命令等到 `chats_load` 完成**且**收到 `sync_done`（超时 10 秒）才输出；`result.data.synced` 说明是不是等到了，`false` 时未读数可能是旧的。

### 3. 每行 JSON 的外壳

```json
{"v":1,"kind":"message","at":"2026-10-25T08:30:00.123Z","seq":1042,"data":{…},"speech":{"brief":"…"}}
```

| 字段 | 必有 | 说明 |
|---|---|---|
| `v` | ✅ | 整数，**外壳与内容树共用这一个版本号**（与 `tree` 约定）。破坏性变动才加一；新增可选字段、新增节点种类不算 |
| `kind` | ✅ | 见下表 |
| `at` | ✅ | 这一行**输出的时间**，RFC3339 UTC，毫秒，以 `Z` 结尾 |
| `seq` | 仅 `event` | 收线程给更新编的递增序号（架构方案第一节第 3 小节第 3 条），断了能看出来 |
| `data` | ✅ | 见下表 |
| `speech` | 可选 | 读法：`brief` / `detail` / `chat_brief`，由 `atg-core::speech` 生成，与 GUI 读的同一句 |

| `kind` | `data` 是什么 |
|---|---|
| `chat` · `message` · `user` | `tree` 的内容节点：`chat` = `ChatItem`；`message` = `Message` / `Service` / `Tombstone`（靠节点自己的 `type` 区分）；`user` = `UserRef` |
| `auth` | `{"step":"<AuthStep>"}` |
| `status` | 三条线原值 + `user_status` + `pending_sends` + `direct_send`（布尔）+ 本次已直发条数 |
| `event` | `{"type":"<事件名>", …}`，事件名与字段只登记在 `actions.md` 第一节第 7 小节 |
| `plan` | `SendPlan`（`actions.md` 第一节第 6 小节） |
| `version` | 各版本号 |
| `playback` | `play` 的状态：`state`（`playing` / `paused` / `stopped` / `finished`）· `position_s` · `duration_s` · `speed` |
| `result` | 见下 |
| `error` | 字段只登记在 `errors.md` 第一节第 1 小节「`error.data` 的字段」；CLI 另有两个自有码 `usage`、`interrupted` |

**内容里的格式**：只登记在 `specs/content-tree.md` 第八节。要点：判别字段 `type`；消息身份是一个对象 `{"chat_id":…,"message_id":…}`；未知节点 `{"type":"unknown","td_type":"…"}`；int53 的 id 写数字，更大的 int64 写字符串；时间 RFC3339 UTC；读的一方忽略不认识的字段、不认识的 `type` 当 `unknown`。

**结尾行**：

```json
{"v":1,"kind":"result","at":"…","data":{"command":"chats","count":37,"elapsed_ms":212,"more":true,"next_cursor":"…","synced":true}}
{"v":1,"kind":"result","at":"…","data":{"command":"send","chat_id":777,"message_id":1048577,"elapsed_ms":640}}
{"v":1,"kind":"error","at":"…","data":{"code":"auth_code_invalid","message_zh":"验证码不对，请重新输入","retryable":false,"td":{"code":400,"message":"PHONE_CODE_INVALID"}}}
```

- 列表类命令的 `result.data` 统一带：`count`（本页条数）· `more` · `next_cursor`（`more` 为真时才有；`history` 的游标就是本页最旧那条的 `message_id`）· `total`（TDLib 给得出才有）
- `send` 的 `result` 带最终 `message_id`：**退出码 0 = 服务器已收下**；`send_failed` 时输出 `error`，`code` 来自失败原因
- 每条命令 `result.data` 的完整字段由 `schema` 给出（第 6 小节）

### 4. 退出码

| 码 | 含义 | 对应 `error.code` |
|---|---|---|
| 0 | 成功 | — |
| 1 | 内部错误 / 未知 | `internal` `unknown` |
| 2 | 用法错误（参数不对） | `usage` |
| 3 | 要登录 / 登录失败 | `auth_*` |
| 4 | 网络 / 连接 | `network` `disconnected` |
| 5 | 限流 / 账号受限 | `rate_limited` `account_limited` |
| 6 | 找不到 | `not_found` |
| 7 | 没权限 / 输入不合法 | `forbidden` `invalid_input` |
| 8 | 发送被拒或人取消 | `send_refused` `cancelled` |
| 9 | 本地档案被占用且本机接口连不上 / 本地库钥匙不对 | `profile_busy` `db_key_invalid` |
| 10 | 发送结果未知（已交出，没等到成败） | `send_outcome_unknown` |
| 130 | Ctrl+C 中断（`watch` 除外） | `interrupted` |

### 5. 流式（`watch`）

- 先输出一行 `status`，然后每个事件一行 `kind:"event"`，带 `seq`
- 启动后补发的积压更新照常输出；补完时输出 `sync_done` 事件，读的一方据此区分「历史补发」和「真正的新消息」
- 合并规则与 GUI 相同（架构方案第一节第 3 小节第 4 条）：积压太多时同一会话的「最后一条 / 未读数」只输出最新一次，并输出 `coalesced` 事件
- `seq` 断了（读的一方发现跳号）：说明有东西没看到，重跑 `chats` / `history` 拿全量
- `watch --chat ID` 会成对调 `chat_open` / `chat_close`（超级群、频道只有打开的会话才推新消息）；不带 `--chat` 时，超级群和频道的新消息**可能收不到**，以 `chat_last_message_changed` 为准（未核 TDLib 实际行为）
- 标准输出被读的一方堵住时不丢事件、不崩；内核侧照常合并
- Ctrl+C：输出 `result`（`data.reason:"interrupted"`）后以 0 退出——对 `watch` 来说被打断就是正常结束

### 6. 给 AI 的自描述

- `atg-cli schema` 输出**一个** JSON 文档：外壳的 JSON Schema、每个子命令的参数 / 输出的 `kind` / **`result.data` 的 Schema** / 可能的 `error.code` / 对外等级（`actions.md`）、**每个事件的 Schema**、退出码表、当前 `v`
- `atg-cli schema send` 只输出这一条
- 这份文档**由代码生成**（参数解析库的定义 + 类型派生的 Schema），并有快照测试；本文与它冲突时，以快照为准、回来改本文
- `--help` 给人看，不保证机读

## 二、为什么

1. **标准输出只放 JSON、结尾必有一行**：AI 一行一行解析，不用猜何时结束，也不会被提示语污染（AI 友好①「结构化输出」）
2. **退出码分档**：脚本不解析 JSON 也能判断大类；码表短，好记
3. **外壳和树共用一个版本号**：两个版本号会出现「外壳 v2 + 树 v1」这类组合，读的一方要处理 N×M 种情况
4. **int53 用数字、更大的用字符串**：TDLib 特意把 id 设计成 int53（JS 也不丢精度）；其余 int64 写数字会被 JS 一类的读方截断
5. **`--format text` 与 `--redact`**：人有时也直接用 CLI；`--redact` 让贴 bug 时不带聊天内容（`.42cog/real.md`「隐私」）
6. **自描述由代码生成并快照**：手写文档必然漂移

## 三、正例 / 反例

| | 例子 |
|---|---|
| ✅ | `atg-cli chats -n 3` → 3 行 `chat` + 1 行 `result`，退出码 0 |
| ✅ | 没登录跑 `history` → 1 行 `error`（`auth_required`），退出码 3 |
| ✅ | `atg-cli --test-dc send saved --text "测试"` → 一行 `plan` → 闸自动豁免 → 等到 `send_succeeded` → `result` 带 `message_id`，退出码 0 |
| ✅ | `atg-cli send 777 --text "测试"`（正式服务器，含发给自己的收藏夹）→ 一行 `plan` → 弹 Touch ID（写明「来自命令行，可能是 AI」）；AI 按不了，人不按就 `send_refused(auth_failed)`，退出码 8 |
| ✅ | `send` 30 秒没等到结果 → `error(send_outcome_unknown)`，退出码 10；AI 跑 `pending` 查，不重发 |
| ❌ | 标准输出里混一行「正在连接……」 |
| ❌ | 出错时只往标准错误打一句话，标准输出什么都没有 |
| ❌ | `echo y \| atg-cli send …`、`script` 伪终端、`tmux send-keys y` 能把发给别人的确认按掉 |
| ❌ | `atg-cli auth password hunter2`（秘密进了命令行参数，会留在 shell 历史和进程列表里） |
| ❌ | `send` 一提交就以 0 退出，消息其实没发出去 |
| ❌ | `chat_id` 写成 `"-1001234567890"` 字符串（它是 int53，用数字） |
| ❌ | 加了一个新字段就把 `v` 从 1 改成 2 |

## 四、⚠️ 待人拍板

> ✅ **已拍板（2026-10-25）**：结果见 `docs/plan/003_decisions-b2.md`——C1 默认 JSON，人加 `--format text`（N4 人定；003 记录里的「`--text`」是简写，`docs/plan/005_decisions-b3.md`）· C2 `atg-cli`。下面的题面保留作依据。

**C1. CLI 默认输出 JSON 还是给人看的文字** —— ✅ 人选：默认 JSON
- A. 默认 JSON，人加 `--format text`：AI 什么都不用加；人每次要多打一个选项
- B. 按标准输出是不是终端自动切换：人直接用顺手，但同一条命令在不同环境输出不同，AI 调试时容易困惑
- 建议：A（命令行优先是给 AI 的）

**C2. 命令名** —— ✅ 人选 A
- A. `atg-cli`（与 crate 同名，暂定）
- B. 装好后另起短名 `atg`
- 建议：先 A，打包时再议

## 修订记录

- 2026-10-25 初稿（infra），外壳与 tree 对齐
- 2026-10-25 按 b2 拍板同步（sync）：加 `delete`（S1）；`play` 进 MVP（F3、F8）；直发模式免一切 L2（D1）；GUI 开着时经本机接口转发（P1）；C1、C2 结果
- 2026-10-25 按 b3 拍板（`docs/plan/005_decisions-b3.md`）同步（sync）：`--format text` 定名（N4）；本机接口下直发参数听执行方（N1）
