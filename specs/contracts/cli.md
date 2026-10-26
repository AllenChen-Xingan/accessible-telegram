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

**例外只有三个**：`schema`（输出一个 JSON 文档，无结尾行）· `--format text`（给人看，不是 JSON）· `--help`（给人看）。另：`serve` 被第二次 Ctrl+C / SIGTERM 强行打断时立刻以 130 退出，不出结尾行。`version` 不是例外：一行 `kind:"version"` + `result`。

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
| `chats [--list main\|archive] [-n N] [--unread] [--cursor C] [--reveal]` | 列会话，一行一个；会话项最后一条里的隐藏内容默认遮住（规则同 `history`，正本 `content-tree.md` 第八节），`--reveal` 才出原文；加 `--redact` 报 `not_implemented`（会话名和最后一条是原文，打码还没做）；`-n` 默认 100；会话行的 `speech.brief` 即会话项读法（`chat_brief`） | `chats_load` + Store | T02 |
| `next-unread [--after CHAT_ID]` | 下一个有未读的会话；没有就零行 + `result{count:0}`，退出码 0（「没有」不算错） | Store | T09 |
| `history CHAT_ID [-n N] [--before MSG_ID \| --after MSG_ID] [--detail] [--full] [--reveal] [--local]` | 列消息，一行一条，**从旧到新**（阅读顺序）。都不给 `--before` / `--after` 时按 F2 落点（「进会话落在第一条未读」，人定，`docs/plan/003_decisions-b2.md`）：有未读就从第一条未读（已读位置之后第一条别人发的）起往新取 N 条；没有未读取最新 N 条；会话从没读过（已读位置为 0）也取最新 N 条——要落到第一条未读就得从整段历史最旧的一条起，把整个会话拉一遍，太慢，这是取舍不是 bug。`--before X`（别名 `--from`）取比 X 旧的、`--after X` 取比 X 新的，都**不含** X。`-n` 默认 20（1–1000）。CLI 内部循环补满 N 条（TDLib 首次常只回 1 条）；只有空应答（或往新翻到了最后一条）才算到头；TDLib 不空却没给新的（往新翻时只拿回起点那一条，即比它新的还没进内存；往旧翻正常不会，万一应答里只有起点或更新的也算）就等一下再要（150 ms、300 ms），连着 3 次还是这样就停：这一页可能不满 N 条，`result.data.partial` 为真（稍后重试同一条命令即可）；本页拿到了消息时，**停住的那一侧**（往旧是 `more`、往新是 `more_newer`）照为真、照给游标，另一侧照常判断；**一条都没拿到**时没有游标可给，两侧都为假，只看 `partial`。「起点」指每次向 TDLib 要的那一处（第一次是 X，之后是上一批最边上那条）；「还没进内存」指 TDLib 自己的本地缓存里还没有、它正在后台拉，所以稍后重试有用。加 `--redact` 报 `not_implemented`（打码还没做，不悄悄输出原文）。`--detail` 另带 `speech.detail`（日期、回复谁、转发自、已编辑、表情回应、话题名、送达状态；句子是 AI 草稿，要等人开着读屏听过才定，措辞会变，别依赖）；`--full` 另带 `speech.full`（全文，保留换行与标点）；隐藏内容一律只读「隐藏内容」：剧透的字在读法和 `data` 里都遮住（`specs/content-tree.md` 第八节）；18+ 遮挡的媒体只读「图片，隐藏内容」这类说法，**说明文字照读、`data` 里照常输出**（同 GUI）；`--reveal` 才读、才出原文——相当于 GUI 里人按了展开隐藏内容的键（R1，`content-tree.md` 第九节）。文字模式一行一条；加 `--full` 时全文保留换行，一条可能占几行（JSON 模式换行在字符串里，不受影响）。`--local` 只取本地已有的。**不标已读** | `history` | T03 |
| `links CHAT_ID MSG_ID [--reveal]` | 列出这条消息里的链接，一行一个 `kind:"link"`；落在剧透里、或链接文字里套着剧透的链接 `hidden:true`（同 `content-tree.md` 第八节的遮挡规则），文字读「隐藏内容」、地址为 `null`，`--reveal` 才给出。`links` 不分页（一条消息的链接一次出完），`result.data` 不带 `more` / `next_cursor`（字段见下方结尾行一节）。加 `--redact` 报 `not_implemented` | `message_links` | T06 |
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
| `speech --fixture PATH [--level brief\|chat_brief] [--reveal]` | 只打印读法：夹具文件（TDLib 更新对象的 JSON 数组，按顺序喂进 store，读法夹具在 `src/atg-core/fixtures/speech/`）→ 树 → 读法，不联网、不碰档案。`brief`（默认）按到达顺序每条消息 / 系统消息 / 撤回占位 / 状态一行；`chat_brief` 主列表每个会话一行（`kind:"chat"`）。读法都在 `speech.brief`；CLI 没有按键，不读「按 D 展开」「按 M 收听」。`result.data` 带 `level`、`count`、`tz`、`utc_offset_s`：给了 `--tz` 时 `tz:"fixed"`、`utc_offset_s` 是那个固定偏移（秒）；跟系统时区时 `tz:"system"`、`utc_offset_s` 为 `null`（每个钟点按它自己的时间点取偏移，夏令时前后不同，没有对所有行都成立的一个值）。隐藏内容默认遮住（规则同上，`content-tree.md` 第八节），`--reveal` 才出原文。读不了、不是 JSON 数组 → `usage`。以后有场景目录再加 `--scenario NAME`；`detail` 归 `show` / `history --detail`（M1.6） | 无（直接调 `speech`） | 定标 |
| `schema [COMMAND]` | 机读自描述（第 6 小节） | — | — |
| `serve` | 常驻当执行方：占着档案、开本机接口，替后起的 `atg-cli` 执行转来的命令；起好输出一行 `status`（另带 `listening:true`），Ctrl+C / SIGTERM 后先做完手上的请求，输出 `result`（`reason:"interrupted"` · `served`：执行了几个请求 · `refused`：拒绝了几个非本用户的连接），退出码 0（同 `watch`：被打断就是正常结束）；再按一次立刻以 130 退出、不出结尾行。没有登录入口：要登录先停掉它。细则 `config-privacy.md` 第一节第 2 小节 | — | — |
| `version` | 版本、TDLib 版本、外壳版本 `v`。`atg-cli --version` 是它的别名，输出相同；不能和别的子命令同用（`usage`） | — | — |

**全局选项**（放在子命令前后都可以）：

| 选项 | 作用 |
|---|---|
| `--test-dc` | 等于「选 `test` 档案，并断言它是测试服务器档案」；断言不过就拒绝启动（`config-privacy.md` 第一节第 2 小节） |
| `--profile NAME` | 选本地档案，默认 `main`。档案是哪种服务器由档案自己的标记决定，不由选项决定 |
| `--fake SCENARIO` | 用假后端，读 `src/atg-core/fixtures/scenarios/<SCENARIO>/`（也接受路径），不联网（`testing.md`）。**场景目录以后做**（#18 人定）；现在一律 `not_implemented` |
| `--format json\|text` | 默认 `json`；`text` 给人看，每行一句读法（`speech.brief`） |
| `--redact` | 打码输出，用于贴进 issue / 发给别人：**白名单**——只留 `type`、计数、时间、状态、错误码；正文、说明、名字、用户名、手机号、文件名、链接、`speech` 全部换成「类型 + 长度」；id 换成**每次随机加盐**的哈希（同一次输出内一致，跨次不可对照、也无法穷举反推）。日志打码另有脚本，见 `config-privacy.md` |
| `--unsafe-direct-send` | **直发模式**（001 第七节第 11 条）：本次运行 L2 免确认（范围见 `actions.md` 第一节第 5 小节，D1），到进程退出为止。只认这个参数（不读环境变量、配置文件）；第一行 `status` 带 `direct_send:true`，标准错误打醒目提示；每条直发写 `direct-send.log`。测试档案仍只许发收藏夹。细则只在 `actions.md` 第一节第 5 小节 |
| `--wait` | 遇到 L0 的限流就等够再继续（默认不等，立刻返回 `rate_limited`，`errors.md` 第一节第 5 小节） |
| `--tz OFFSET` | 读法里时间按哪个时区读，默认取系统时区。写法：`+08:00`、`-05:30`、`+0800`、`+8`、`Z`、`UTC`（±14 小时内）；不认得 → `usage` |

`send` 的文本不许从标准输入读：标准输入可能是 AI 拼进去的管道，内容由 `--text` / `--text-file` 明确给出。怎么确认（Touch ID、测试豁免、直发模式）由闸定，见 `actions.md` 第一节第 4、5 小节。`history` / `show` 永不标已读。

**界面开着时**（P1 人定一开始就做共用）：档案被 GUI（或另一个 CLI）占着时，`atg-cli` 自动经本机接口把命令转给它执行，输出与直跑一样（只有执行方当场生成的时间字段不同），调用方不用改任何参数（例外：直发参数听执行方的——执行方不是直发启动就照常确认，CLI 的 `--unsafe-direct-send` 忽略并在标准错误说明，N1 人定）；`login` / `logout` / `auth *` 不转发（登录交秘密，登出改执行方自己的状态），**`auth state` 除外**（只读一个步骤名，不带秘密）。执行方是 GUI 或常驻的 `atg-cli serve`（Ctrl+C 结束）；协议与细则只在 `config-privacy.md` 第一节第 2 小节。连不上才报 `profile_busy`（退出码 9）。

**什么时候算数据齐了**：`chats`、`next-unread` 读的是 Store，Store 靠 `chats_load` 之后陆续到的更新填满。这两条命令等到 `chats_load` 完成**且**收到 `sync_done`（超时 10 秒）才输出；`result.data.synced` 说明是不是等到了，`false` 时未读数可能是旧的。**`sync_done` 的暂行定义**（#18，事件本身随 `watch` 落地前）：启动或重连后，连接线到 `ready`——服务器把离线期间漏掉的更新补送完，TDLib 把连接状态从 `updating` 报成 `ready`。`chats` 的加载另有 10 秒上限（断网时本地库不够一页，`loadChats` 会转去问服务器、一直没有应答），到点照出已有的，`result.data.load_timed_out` 为真、`more` 为真，且不再另等补发。`history` 也按上面同一个判据、同样 10 秒上限等补发完再定未读落点，`result.data.synced` 同义。

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
| `speech` | 可选 | 读法：`brief` / `detail` / `full` / `chat_brief`，由 `atg-core::speech` 生成，与 GUI 读的同一句 |

| `kind` | `data` 是什么 |
|---|---|
| `chat` · `message` · `user` | `tree` 的内容节点：`chat` = `ChatItem`；`message` = `Message` / `Service` / `Tombstone`（靠节点自己的 `type` 区分）；`user` = `UserRef` |
| `auth` | `{"step":"<AuthStep>"}` |
| `status` | 三条线原值 + `user_status` + `pending_sends` + `direct_send`（布尔）+ 本次已直发条数；`serve` 起好那一行另带 `listening:true` |
| `event` | `{"type":"<事件名>", …}`，事件名与字段只登记在 `actions.md` 第一节第 7 小节 |
| `plan` | `SendPlan`（`actions.md` 第一节第 6 小节） |
| `version` | 各版本号 |
| `link` | `links` 的一行：`index`（从 1 起）· `text` · `url`（隐藏时 `null`）· `hidden`（链接落在剧透里，或链接文字里套着剧透） |
| `playback` | `play` 的状态：`state`（`playing` / `paused` / `stopped` / `finished`）· `position_s` · `duration_s` · `speed` |
| `result` | 见下 |
| `error` | 字段只登记在 `errors.md` 第一节第 1 小节「`error.data` 的字段」；CLI 另有三个自有码 `usage`、`interrupted`、`not_implemented` |

**内容里的格式**：只登记在 `specs/content-tree.md` 第八节。要点：判别字段 `type`；消息身份是一个对象 `{"chat_id":…,"message_id":…}`；未知节点 `{"type":"unknown","td_type":"…"}`；int53 的 id 写数字，更大的 int64 写字符串；时间 RFC3339 UTC；读的一方忽略不认识的字段、不认识的 `type` 当 `unknown`。

**结尾行**：

```json
{"v":1,"kind":"result","at":"…","data":{"command":"chats","count":37,"elapsed_ms":212,"more":true,"next_cursor":"…","synced":true}}
{"v":1,"kind":"result","at":"…","data":{"command":"send","chat_id":777,"message_id":1048577,"elapsed_ms":640}}
{"v":1,"kind":"error","at":"…","data":{"code":"auth_code_invalid","message_zh":"验证码不对，请重新输入","retryable":false,"td":{"code":400,"message":"PHONE_CODE_INVALID"}}}
```

- 列表类命令的 `result.data` 统一带：`count`（本页条数）· `more` · `next_cursor`（`more` 为真时才有；`history` 的游标就是本页最旧那条的 `message_id`；`chats` 的游标是本页最后一个会话的 `<order>:<会话 id>`）· `total`（TDLib 给得出才有）
- `history` 的 `result.data` 另带：`chat_id` · `next_cursor` 交给 `--before`（`history` 没有 `--cursor`）· `more_newer` / `newer_cursor`（往新还有；游标 = 本页最新那条的 `message_id`，交给 `--after`）· `first_unread`（**本页里**第一条未读的 `message_id`；为 `null` 有两种情况：会话没有未读，或未读不在这一页——如 `--before` 往旧翻的页）· `landing`（`first_unread` / `latest` / `before` / `after`）· `synced`（同 `chats`：等到补发完了没有，`false` 时未读位置可能是旧的）· `partial`（没拉全就停了，见上）。文字模式在第一条未读上面另起一行「以下为未读」（F2）。`links` 的 `result.data` 带 `chat_id`、`message_id`、`count`
- `chats` 的 `result.data` 另带（可选字段，`v` 不加一）：`list`（`main` / `archive`）· `synced` · `load_timed_out`（加载到 10 秒上限提前停了）· `first_chat_ms`（进程启动到这个列表第一次有会话的毫秒数；列表是空的为 `null`；经本机接口转发时是**执行方**进程，见 `config-privacy.md` 第一节第 2 小节）。会话行的读法在 `speech.brief`（即会话项读法 `chat_brief` 那一句）
- `send` 的 `result` 带最终 `message_id`：**退出码 0 = 服务器已收下**；`send_failed` 时输出 `error`，`code` 来自失败原因
- 每条命令 `result.data` 的完整字段由 `schema` 给出（第 6 小节）

### 4. 退出码

| 码 | 含义 | 对应 `error.code` |
|---|---|---|
| 0 | 成功 | — |
| 1 | 内部错误 / 未知 / 子命令还没实现 | `internal` `unknown` `not_implemented` |
| 2 | 用法错误（参数不对） | `usage` |
| 3 | 要登录 / 登录失败 | `auth_*` |
| 4 | 网络 / 连接 | `network` `disconnected` |
| 5 | 限流 / 账号受限 | `rate_limited` `account_limited` |
| 6 | 找不到 | `not_found` |
| 7 | 没权限 / 输入不合法 | `forbidden` `invalid_input` |
| 8 | 发送被拒或人取消 | `send_refused` `cancelled` |
| 9 | 本地档案被占用且本机接口连不上 / 本地库钥匙不对 | `profile_busy` `db_key_invalid` |
| 10 | 发送结果未知（已交出，没等到成败） | `send_outcome_unknown` |
| 130 | Ctrl+C 中断（`watch`、`serve` 除外；`serve` 被第二次打断时是 130，无结尾行） | `interrupted` |

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
- 2026-10-25 issue #14 骨架（skeleton，大总管授权，人选建议）：自有码加 `not_implemented`（退出码 1）；`--version` 登记为 `version` 的别名
- 2026-10-25 issue #18 列会话（chats，大总管授权）：写明 `sync_done` 的暂行定义（连接线到 `ready`）与 `chats` 的加载上限；`chats` 的 `result.data` 登记 `list`、`synced`、`load_timed_out`、`first_chat_ms`，游标形状
- 2026-10-25 issue #18 人定（经大总管）：`chats` 的 `-n` 默认 100；会话行读法键名用 `brief`（即会话项读法）；`--fake` 场景目录以后做
- 2026-10-26 issue #19 speech（大总管授权 2026-10-26）：`speech` 的 `result.data` 加 `tz`（`system` / `fixed`），跟系统时区时 `utc_offset_s` 为 `null`
- 2026-10-26 issue #19 speech（大总管授权 2026-10-26）：`speech` 改成 `--fixture PATH`（TDLib 更新 JSON 数组），`--level` 只留 `brief` / `chat_brief`；`--scenario` 等场景目录、`detail` 归 M1.6
- 2026-10-26 issue #22 本机接口（ipc，大总管授权 2026-10-26，属技术细节；P1 人定原意不变）：加 `serve` 子命令；`auth state` 可经本机接口转发，其余登录命令仍不转；协议只在 `config-privacy.md`
- 2026-10-26 issue #22 自审后（ipc，大总管授权 2026-10-26，属技术细节；P1 人定原意不变）：登记 `serve` 的 `listening`、`reason` / `served` / `refused` 与两次 Ctrl+C 的退出码；退出码 130 行与结尾行例外补 `serve`；`first_chat_ms` 转发时的含义；「完全一样」改为时间字段例外；登出不转的理由
- 2026-10-26 issue #21 history（大总管授权 2026-10-26，属技术细节；F2 人定原意不变）：`history` 改为从旧到新、按 F2 落点，加 `--before`（`--from` 留作别名）/ `--after` / `--detail` / `--full` / `--reveal`，`-n` 默认 20；`result.data` 加 `more_newer`、`newer_cursor`、`first_unread`、`landing`、`synced`；加 `kind:"link"` 与 `links --reveal`
- 2026-10-26 issue #21 自审后（大总管授权 2026-10-26；R1「隐藏内容默认遮住」的延伸）：`data` 里的隐藏内容默认遮住（`content-tree.md` 第八节）；`chats`、`speech` 也认 `--reveal`；外壳 `speech` 登记 `full`；写明从没读过的会话为何取最新、`first_unread` 为 `null` 的两种情况、`next_cursor` 交给 `--before`、`history` 也等补发、文字模式 `--full` 一条可能占几行
- 2026-10-26 issue #21 第二轮自审后（大总管授权 2026-10-26，属技术细节）：`history` 补不满时的语义（`partial`、`more` 照为真）；18+ 遮挡说明文字照出；`--redact` 在 `history` / `links` 报 `not_implemented`；`chats`、`speech` 行写明 `--reveal` 管什么
- 2026-10-26 issue #21 第三轮自审后（大总管授权 2026-10-26，属技术细节）：`chats` 也对 `--redact` 报 `not_implemented`；`links` 写明隐藏范围、不分页、`--redact`；`history` 写明停住的样子与重试次数、间隔
- 2026-10-26 issue #21 第四轮自审后（大总管授权 2026-10-26，属技术细节）：停住且一条没拿到时 `more` 为假、只看 `partial`；`chats` / `speech` 的遮挡说法改为指向 `content-tree.md` 第八节；`links` 的 `result` 字段与 `link` 行的 `hidden` 范围对齐
- 2026-10-26 issue #21 第五、六轮自审后（大总管授权 2026-10-26，属技术细节）：`chats` 行只留指针；`history` 写明往旧也可能停住、停住时只有停住的那一侧照为真、「起点」「进内存」指什么
