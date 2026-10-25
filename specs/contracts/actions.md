# 契约 · 核心动作清单（`Backend` 意图级操作）v0

> 2026-10-25 · 手下 infra 起草 · 状态：**草稿**；动作名与分级已与 `api` 的 MVP 初版对齐（`docs/research/002_features/inventory.md`「附：意图级动作」）；末尾「待人拍板」**已拍板**（`docs/plan/003_decisions-b2.md`），正文已同步
> 管：`atg-core` 对 CLI / GUI 暴露哪些动作，每个动作的参数、返回、错误、**对外等级**
> 不管：返回值里内容节点长什么样（归 `tree`：`specs/content-tree.md`）；错误种类（`errors.md`）；CLI 怎么调（`cli.md`）
> 依据：`docs/plan/001_arch-stack.md` 第一节第 2 小节第 6 条（`Backend` 只暴露意图级操作）、第四节第 1 小节（发送闸）、第七节第 9、10 条（人已拍板）

## 一、定了什么

### 1. 两类接口，分清楚

| | 是什么 | 例子 | 谁实现 |
|---|---|---|---|
| **`Backend` 动作** | 要跟 TDLib（或假后端）打交道的操作，异步，可能失败 | 登录下一步、拉历史、发送 | 真后端一份、假后端一份 |
| **`Store` 查询** | 只读本地状态（reducer 的结果），同步纯函数，不联网 | 会话列表当前顺序、下一个有未读的会话 | 只有一份，与后端无关 |

T09「跳到下一个有未读的会话」是 `Store` 查询，不是 `Backend` 动作。界面上的读法、焦点都不在这里。

### 2. 对外等级（每个动作必标一个）

| 等级 | 含义 | 闸 |
|---|---|---|
| **L0 内部** | 只读，或只影响本机；别人看不到 | 无 |
| **L1 对外可见·轻** | 别人能看到痕迹，但不发内容：标记已读、「正在输入」、在线状态、「已听」语音；**写草稿**特殊，按 L2 同级把关（见 `draft_set`） | **人已定（F1、A2）：和官方客户端一样**——GUI 进会话就把已读标到最新一条、界面开着就在线、打字时发「正在输入」；F1 原话「AI 经界面读时同样会标」。CLI 永不标已读、不设在线、不发正在输入。（AI 按同一取向推，可推翻：GUI 播放语音即标已听；CLI 只在 `play --mark-listened` 时标已听。）不弹确认；入口检查见第 4 小节第 9 条 |
| **L1 外部** | 不对 Telegram 里的人可见，但会**离开本应用**：打开链接（对方网站看得到你的 IP 和访问时间）、用别的应用打开附件（可能触发那个应用的动作） | 见 `open_link` / `open_file`：协议与类型白名单 + 当场确认 |
| **L2 对外·不可逆** | 发内容或改别人能看到的东西：发送、回复、转发、编辑、删除、表情回应 | **发送闸 + 确认凭据**（第 4 小节） |

`.42cog/real.md`「底线红线」点名的「发送、转发、删除」都是 L2；「标记已读」（L1）已由人从开发测试红线里去掉（N3）。

**L2 一律不自动重试**：内核的通用重试（网络、限流）只用于 L0；L2 请求失败就把错误交回调用方，由人决定是否 `resend`（重新确认）。否则一次网络抖动会让对方收到两条。

**L2 的「排队」只有一种，而且要说在确认里**：本项目自己不排队（连接不是 `ready` 时提交直接拒绝，`network`）；但请求一旦交给 TDLib，TDLib 会把它存进本地库，断网就等网络恢复再发，重启后也会接着发——凭据的 60 秒只管「交给 TDLib」这一刻，管不住 TDLib 之后什么时候真正发出。所以：确认时写明「若网络中断，恢复后仍会发出」；`status` 列出待发消息并能撤销（见「待发出的消息」）。

### 3. 动作表

记法：`名字(参数) -> 返回`。返回里的 `ChatItem`、`Message`、`UserRef`、`MsgId`（消息身份，`{chat_id, message_id}` 一个对象）是 `tree` 定的内容节点，正式名以 `specs/content-tree.md` 第三节第 7 小节为准（群不是单独类型，是 `ChatItem` 的 `ChatKind`）；不属于内容树的 `SendPlan`、`PendingMessage`、`Page`、`MessageProps` 在本文第一节第 6 小节定义。`Ack` = 成功无返回值。所有动作都可能返回 `disconnected`、`network`、`rate_limited`、`internal`，下表「错误」列只写**额外**的。

**生命周期**

| 动作 | 返回 | 额外错误 | 等级 | 说明 |
|---|---|---|---|---|
| `start(profile: Profile)` | `Ack` | `profile_busy` | L0 | 起 TDLib、收线程；读配置与钥匙串（`config-privacy.md`） |
| `subscribe()` | `EventStream`（`CoreEvent` 带递增 `seq`） | — | L0 | 一个后端只许一个订阅者（CLI 循环或 GUI 的 Store） |
| `status()` | `UserStatus` + 三条线原值 | — | L0 | `errors.md` 第一节第 3 小节 |
| `shutdown()` | `Ack` | — | L0 | 发 `close`（`td_api.tl:12295`），等 `authorizationStateClosed`（带超时） |
| `set_online(on: bool)` | `Ack` | — | **L1** | `setOption`（`:16805`）的 `online` 选项（td_api.tl 里没写，说明见 core.telegram.org/tdlib/options；默认 false）；别人会看到「在线」。**启动时显式设 false**，不靠默认值；GUI 界面开着时设 true（A2 人定：同官方客户端），「开着」具体指窗口打开、在前台还是未闲置，GUI 阶段对照官方客户端实测再定；CLI 永远不设 |
| `proxy_add(proxy)` · `proxy_enable(id)` | `AddedProxy` / `Ack` | `invalid_input` | L0 | `addProxy` / `enableProxy`（`:17576,17586`）；A5：人不用开梯子，**不进 MVP**，放后面版本（`002_roadmap.md` M6）；名字先占住 |
| `me()` | `UserRef` + `saved_chat_id` | `auth_required` | L0 | `getMe`（`:12388`）；**每次进入 `authorizationStateReady` 都重算**（测试服务器清空后重注册，id 会变），用于判定收藏夹 |

**登录（T01）**

| 动作 | 返回 | 额外错误 | 等级 | TDLib |
|---|---|---|---|---|
| `auth_state()` | `AuthStep` + 本步提示（如密码提示语、验证码发到哪） | — | L0 | `updateAuthorizationState` 缓存。提示语可能就是密码的变体，验证码去向带打码手机号：**只给面向人的界面 / 终端，不进 CLI 的 JSON**（JSON 只给步骤名） |
| `auth_submit_phone(phone: String)` | `Ack` | `auth_phone_invalid` `auth_wrong_state` | L0 | `setAuthenticationPhoneNumber`（`td_api.tl:12192`） |
| `auth_submit_code(code: String)` | `Ack` | `auth_code_invalid` `auth_code_expired` `auth_wrong_state` | L0 | `checkAuthenticationCode`（`:12222`） |
| `auth_resend_code()` | `Ack` | `auth_wrong_state` | L0 | `resendAuthenticationCode`（`:12215`） |
| `auth_submit_email(address: String)` | `Ack` | `invalid_input` `auth_wrong_state` | L0 | `setAuthenticationEmailAddress`（`:12210`） |
| `auth_submit_email_code(code: String)` | `Ack` | 同验证码 | L0 | `checkAuthenticationEmailCode`（`:12219`） |
| `auth_submit_password(pw: Secret)` | `Ack` | `auth_password_invalid` `auth_wrong_state` | L0 | `checkAuthenticationPassword`（`:12261`）；`Secret` 用完即清零，Debug 不打印 |
| `logout()` | `Ack` | — | L0（本机）\* | `logOut`（`:12292`）；\* 不对外，但会让本机丢掉登录，须人当场确认（同 L2 的确认步骤，不过发送闸） |

注册新账号（`registerUser`）、找回密码第一阶段不做（例外：**测试档案上**测试代码可以注册，因为测试服务器会定期清空账号，见 `testing.md` 第 5 小节）；二维码登录（`auth_qr`，`requestQrCodeAuthentication` `:12228`）以后再说，对读屏用户不友好。不做的：落到 `unsupported` 步骤（`errors.md`）。

**会话（T02、T08、T09）**

| 动作 | 返回 | 额外错误 | 等级 | TDLib |
|---|---|---|---|---|
| `chats_load(list: ChatListKind, limit: u32)` | `Ack`；会话经 `updateNewChat` / `updateChatPosition` 等进 Store | — | L0 | `loadChats`（`:12502`）；没有更多时 TDLib 回 404，内核吞掉、标「已到底」 |
| `chat_get(chat: ChatId)` | `ChatItem` | `not_found` | L0 | `getChat`（`:12413`） |
| `mute(chat: ChatId, on: bool)` | `Ack` | `not_found` | L0 | `setChatNotificationSettings`（`:14493`）；从第二批提到 MVP：吵的群会打断读屏 |
| `search_chats(query: String, limit: u32)` | `Vec<ChatItem>` | — | L0 | `searchChats`（`:12523`，本地）；不够再 `searchChatsOnServer`（`:12529`） |
| Store：`chat_list(list)` | 当前排好序的 `Vec<ChatItem>` | — | — | 纯查询 |
| Store：`next_unread_chat(after: ChatId)` | `Option<ChatId>` | — | — | 纯查询，T09；到底不回绕，返回空，界面读「没有更多未读」（A3） |

`ChatListKind`：`main` · `archive`（`td_api.tl:3970,3973`）。文件夹第一阶段不做。

**消息（T03、T05、T06、T07）**

| 动作 | 返回 | 额外错误 | 等级 | TDLib |
|---|---|---|---|---|
| `chat_open(chat: ChatId)` | `Ack` | `not_found` | L0 | `openChat`（`:14188`）；让 TDLib 推这个会话的实时更新（超级群 / 频道只有打开的会话才推）。**`chat_open` 本身绝不隐式调 `viewMessages`**（频道浏览量也靠它涨）；GUI 进会话时另行**显式**调 `mark_read` 标到最新一条（F1）；必须与 `chat_close` 配对 |
| `chat_close(chat: ChatId)` | `Ack` | — | L0 | `closeChat`（`:14191`） |
| `history(chat: ChatId, from: Option<MessageId>, limit: u32, local_only: bool)` | `Vec<Message>`（从新到旧） | `not_found` | L0 | `getChatHistory`（`:12758`）；带请求代次（架构方案第一节第 3 小节第 6 条） |
| `message_get(id: MsgId)` | `Message` | `not_found` | L0 | `getMessage`（`:12418`）；`MsgId` 即 `tree` 的 `MsgId` |
| `replied_message_get(id: MsgId)` | `Option<Message>` | — | L0 | `getRepliedMessage`（`:12436`）；T06「回复谁」 |
| `message_properties(id: MsgId)` | `MessageProps`（能回复 / 能转发 / 能编辑……） | `not_found` | L0 | `getMessageProperties`（`:12457`）；决定菜单里列什么 |
| `user_get(user: UserId)` · `group_get(id)` | `UserRef` / `ChatItem` | `not_found` | L0 | `getUser`（`:12391`）· `getBasicGroup` / `getSupergroup`（`:12397,12404`）；读法查不到名字时补取 |
| `search_messages(scope: Option<ChatId>, query: String, limit: u32, cursor: Option<Cursor>)` | `Page<Message>` | — | L0 | 有会话用 `searchChatMessages`（`:12793`），否则 `searchMessages`（`:12806`） |
| `file_download(file: FileId)` | `Ack`；进度经 `updateFile` | `not_found` | L0 | `downloadFile`（`:15001`）；对外不可见，前提是不调 `mark_listened`。T07 语音时长、文件名在消息节点里就有，不必下载 |
| `mark_listened(id: MsgId)` | `Ack` | `not_found` | **L1**（与已读同一道把关，A1）；阅后即焚类 **L2** | `openMessageContent`（`:14205`）；对方会看到「已听 / 已看」，阅后即焚一打开就开始倒计时、不可逆。GUI 播放即标（同官方客户端，与 F1 同一取向）；CLI 只在 `play --mark-listened` 时标；阅后即焚按 L2 要确认 |
| `mark_read(chat: ChatId, up_to: MessageId)` | `Ack` | — | **L1** | `viewMessages`（`:14199`，`force_read`）。⚠️ **不是按条标**：TDLib 取传入的最大消息 id，把普通会话的已读位置推进到那里（`MessagesManager.cpp:16961` `read_dialog_inbox`），此前的消息全部连带变成已读。所以参数是「标到哪条为止」。F1 人定：GUI 进会话就标到最新一条（同官方客户端，不确认）；CLI 的 `history` / `show` 不标 |
| `message_links(id: MsgId)` | `Vec<Link>`（链接文字 + 地址） | `not_found` | L0 | 从内容树取，不联网 |
| `open_link(url)` · `open_file(file: FileId)` | `Ack` | `not_found` `invalid_input` | **L1 外部** | 用系统接口（`NSWorkspace`）打开，**不经 shell**。链接只许 `http` / `https`，其余协议（`file:`、`tg:`、`mailto:`、自定义协议……）拒绝；打开前把完整网址（域名单独读出）给人看并确认。文件只许常见的只读类型（图片、音频、视频、PDF、纯文本），可执行文件、`.app`、`.command`、脚本、安装包一律拒绝，改为「在访达中显示」。打开文件前须已下载 |

**发送（T04）**

| 动作 | 返回 | 额外错误 | 等级 | TDLib |
|---|---|---|---|---|
| `send_text_prepare(target: ChatId, draft: Draft)` | `SendPlan`：目标、内容全文、回复的那条、**最终话题**、内容摘要值 | `not_found` `forbidden` `invalid_input` | L0 | 只查不发；给确认步骤看。**话题规则**（`decision.md`、`002_roadmap.md` M2.2）：话题群里回复某条，话题取被回复那条的；话题群里不回复任何一条的新消息，第一阶段拒发（`invalid_input`，原因「话题群里请先选一条消息回复」）。最终话题进计划展示、进摘要、发送时校验 |
| `send_text(plan: SendPlan, token: ConfirmToken)` | `PendingMessage`（临时 id）；最终结果经事件 `send_succeeded` / `send_failed` | `send_refused` `forbidden` | **L2** | `sendMessage`（`:13143`）；成败分别是 `updateMessageSendSucceeded` / `updateMessageSendFailed`（`:11208,11214`） |
| `resend_prepare(chat: ChatId, ids: Vec<MessageId>)` | `SendPlan`（每条一份） | `not_found` `invalid_input` `rate_limited` | L0 | 只查不发。按 `messageSendingStateFailed`（`:3458-3465`）逐条检查：`can_retry` 为假 → 拒；`retry_after` 没到 → `rate_limited`；`need_drop_reply` → 计划里写明「重发时会去掉回复关系」（`resendMessages` 自动去掉，`:3462`）；`need_another_sender` / `need_another_reply_quote` → 第一阶段拒；`required_paid_message_star_count > 0` → **一律拒**（付费发送不做） |
| `resend(plans, token: ConfirmToken)` | 每条一项：`PendingMessage` 或该条的错误 | `send_refused` | **L2** | `resendMessages`（`:13195`）；凭据绑**重发后的实际内容与回复关系**；TDLib 对不能重发的那条返回 null（`:13190`），逐条报告，不当整体失败 |
| `draft_set(chat: ChatId, text: String, token: ConfirmToken)` · `draft_clear(chat, token)`（第二批） | `Ack` | `send_refused` | **L1，按 L2 同级把关** | `setChatDraftMessage`（`:14488`）；不发给对方，但会同步到你手机的输入框——AI 写进去、人在手机上顺手一按就发了；清掉则会毁掉你手写的草稿。所以与发送用**同一道闸、同一种凭据**（绑会话 + 内容摘要；测试服务器 + 收藏夹免确认）。GUI 输入框里的草稿默认**只存本机、不同步到云端**——「人在打字」同样证明不了是人（computer use 也能打字） |
| `typing(chat: ChatId)` | `Ack` | — | **L1** | `sendChatAction`（`:14159`）；A2 人定进 MVP：GUI 输入框里打字时发（同官方客户端，`002_roadmap.md` M4.1）；CLI 不发 |
| `delete_prepare(chat: ChatId, ids: Vec<MessageId>)` | `SendPlan`（删哪几条、给所有人） | `not_found` `forbidden` | L0 | 只查不发；逐条按 `message_properties` 的「能否删除（双方）」检查，不能的拒。⚠️ 收藏夹里的消息不能给所有人删（`MessagesManager.cpp:8346`），必拒 |
| `delete(plan, token: ConfirmToken)` | `Ack`；结果经 `messages_deleted` | `send_refused` `forbidden` | **L2** | `deleteMessages`（`:13226`）`revoke=true`。**S1 人定进 MVP：只做「给所有人」**；只删自己这边（`revoke=false`）留第二批。凭据绑会话 + 消息 id 列表 |

**待发出的消息**（MVP 必做）：`status()` 列出「待发出」的消息（会话 id、临时消息 id、确认时间）；`pending_cancel(MsgId)` 删除待发消息以取消发送（`deleteMessages` 对未发出消息的效果**接口注释没写，要实测**，列为 M2.1 验收项）；启动时先列出上次留下的待发消息并告诉人。能否抢在 TDLib 重启后自动补发之前撤销，**未核**。

`Draft` 第一阶段只有：纯文本 + 可选「回复哪条」（`inputMessageText` `:6680`、`inputMessageReplyToMessage` `:3518`）。Markdown 转实体（`parseMarkdown` `:13836`）第二批。

**第二批留位**（`api` 已列，名字先占住）：

| 动作 | 等级 | 说明 |
|---|---|---|
| `edit_text` | L2 | `editMessageText`（`:13252`） |
| `delete(ids, revoke=false)` | L0 但本机不可逆 | 只删自己这边；要人当场确认（给所有人的已进 MVP，见上表） |
| `forward` | L2 | `forwardMessages`（`:13180`） |
| `react` | L2 | `addMessageReaction`（`:13748`） |
| `send_file` · `send_voice` | L2 | `sendMessage` + `inputMessageDocument` / `inputMessageVoiceNote`（`:6700,6738`） |
| `speech_to_text` | L0 | `recognizeSpeech`（`:13121`） |
| `mark_unread` | L0 | `toggleChatIsMarkedAsUnread`（`:14514`） |

所有 L2 一律走同一道闸。

### 4. 发送闸与确认凭据

**威胁边界（先说清楚防什么）**：闸防的是「人或 AI **走正规接口**时出错或越权」——AI 调 CLI、AI 用无障碍树点按钮、脚本误发。闸**防不了**一个能改本仓代码、自己编程序的 AI（它可以绕开 `atg-core` 直接调 TDLib）。P4 人定**开发版也能开正式档案**，人已知这一点；剩下的只有改闸的 PR 人本人看（第 7 条）和 `CLAUDE.md` 第五节「测试绝不向真实联系人发消息」。

**直发模式**（001 第七节第 11 条，人已定）是人**有意**打开的口子，不算漏洞：见第 5 小节。

1. **凭据怎么开**。`ConfirmToken` 只能由 `atg-core::gate` 开出，构造函数不公开。开出方式：

   | 方式 | 适用 | 怎么确认 |
   |---|---|---|
   | `test_exempt` | 「测试服务器 + 收藏夹」（001 第七节第 10 条，人已定），以及第 2 条的第二个测试号私聊（N7） | 闸自动开，写死 |
   | `human_auth` | 正式服务器上的**一切目标**，含自己的收藏夹（CLI 和 GUI 都一样） | Touch ID / 系统密码（macOS 本地认证） |
   | `direct_mode` | 以直发参数启动的这一次运行（第 5 小节） | 不确认 |

   理由：终端确认（`/dev/tty`）拦不住 `tmux send-keys`，也拦不住 `script` 造出来的伪终端，AI 能无人在场完成真实发送；拍板 10 只给「测试服务器 + 收藏夹」免确认，正式服务器的收藏夹不在其内（codex 评审 004 的 P0）。所以默认模式下终端确认**不再开凭据**，正式服务器一律要 AI 拿不到的东西（第七节第 9 条的理由推广到 CLI 与 GUI 全部发送：界面分不清按键的是人还是 AI，这是推断，不是原话）。

2. **测试服务器上只许发收藏夹**（架构方案第四节第 1 小节）：测试服务器上发往收藏夹以外的会话，有什么凭据都拒绝（`send_refused(test_dc_saved_only)`）。测试号谁都能登，别人可能往里发消息、拉群。
   **唯一例外（N7 人定）**：与**第二个自己控制的测试号**的私聊——可以互发，也可以把发给它的消息给所有人删（收藏夹里删不了给所有人，`MessagesManager.cpp:8346`）。判定只认：本进程实际的 `use_test_dc` 为真 **且** 目标是与该号的私聊；该号的用户 id 在每次进入 `authorizationStateReady` 时按 `.env.local` 的 `ATG_TEST_PHONE_2`（也须 `99966` 开头）重查，不认会话名字。这个私聊里闸同样自动开 `test_exempt` 凭据（AI 推：例外是为了让自动化测试能跑，可推翻）

3. **`test_exempt` 的判定**只认：本进程启动时**实际**传给 `setTdlibParameters` 的 `use_test_dc`（由闸独家持有，不认后端自报）**且**当前登录号以 `99966` 开头**且**目标是 `me()` 的收藏夹。每次发送前重查，不只在开凭据时查。

4. **确认时给人看什么**（终端、界面、Touch ID 弹框说明文字都一样）：目标会话的**类型 + 名字 + id**（如「私聊 · 张三 · 777」，改名叫「收藏夹」的会话骗不过去）· 内容全文 · 回复的那条 · **发起方**（界面 / 命令行，命令行注明「可能是 AI」）。显示前把 ANSI 控制序列、双向控制字符、零宽字符转义成可见形式（它们能改写终端显示，VoiceOver 也不读）。每次发送新建一次认证，不复用。

5. **摘要算什么**：最终交给 TDLib 的那份请求的字节（文本**不做** Unicode 规范化）+ 所有发送选项 + 回复目标 + **最终话题** + 自己的账号 id + 服务器类别，取 SHA-256。

6. **凭据的性质**（写死，改它的 PR 人本人看）：不实现 `Clone` / `Copy` / `Serialize` / `Deserialize`；`send_text` 按值吃掉它；有效期 60 秒，用单调时钟；用过的随机数在进程内登记；**绝不跨进程传递**。CLI 经本机接口由别的进程执行时（P1 人定一开始就做，`config-privacy.md` 第一节第 2 小节），由**执行方**的闸重新开凭据，确认说明里标明「来自命令行，可能是 AI」（与第 4 条同一句）。

7. **只有闸能发**：`atg-core` 以外的代码不得直接调 tdlib-rs 的发送类函数（`send_message`、`delete_messages`、`send_chat_action`、`set_option`（`online`）、`view_messages`、`set_chat_draft_message`、`open_message_content`……）。tdlib-rs 只在 `atg-core::td` 里引入；CI 用一个扫描脚本拦（`testing.md` 第一节第 4 小节）。改到闸、对外等级、这条清单的 PR 属一级评审，人本人看。

8. 验凭据：目标对、摘要对、60 秒内、没用过、第 2、3 条成立；任一不符即 `send_refused`，不发。
9. **L1 也在入口把关**：`mark_read`、`mark_listened`、`set_online`、`typing`、`draft_set` 都只从 `atg-core` 入口走。F1、A2 人定后，`mark_read` / `set_online` / `typing` 不再一律拒、不确认，谁调谁负责（GUI 按官方客户端的时机调；CLI 不调）；`mark_listened` 照同一取向（AI 推，可推翻：GUI 播放即标，CLI 只在 `play --mark-listened` 时）。开发 / 测试时在正式档案上发这些 L1 也允许（N3 人定，`.42cog/real.md`「不替我对外发消息」已去掉「标记已读」）；`draft_set` 仍按 L2 同级把关；阅后即焚的 `mark_listened` 按 L2。`send_refused(l1_disabled)` 保留，供以后若加整类关闭的开关时用（第一阶段没有这个开关）。

### 5. 直发模式（001 第七节第 11 条，人已定 2026-10-25）

人原话见 `docs/plan/001_arch-stack.md` 第七节第 11 行。下面只写它在契约里怎么落：

- **怎么进入**：启动参数 `--unsafe-direct-send`，CLI 与 GUI 同名同义（GUI 是启动参数，不是界面里的开关）。**只认命令行参数**：不读环境变量、不读 `config.toml`，也不传给子进程——这样它一定出现在启动命令里，看得见
- **管多久**：从启动到这个进程退出。没有中途开关；不进入时，001 第七节第 9、10 条和本文第一节第 4 小节的默认闸照旧从严
- **免什么**（D1 人定：全部免确认）：**一切 L2**——`send_text`、`resend`、`delete`（给所有人），以及将来的 `send_file` / `send_voice` / `forward` / `edit_text` / `react`——**所有会话**免确认，闸直接开 `direct_mode` 凭据。`draft_set`（按 L2 同级）与阅后即焚的 `mark_listened`（按 L2）同样免（N6 人定：一切 L2 都免确认）
- **不免什么**（这些不是「确认」，是防误发与防封号，不增加摩擦）：摘要与目标校验照做（防「确认 A 发 B」那类程序错误）；L2 不自动重试；限流与 `account_limited` 照旧。`logout` 的当场确认不是 L2，照旧
- **测试档案**：仍然**只许发收藏夹**（`test_dc_saved_only`），外加第 4 小节第 2 条的第二个测试号私聊（N7）。测试服务器上的号谁都能登，可能是真人；`CLAUDE.md` 第五节「测试绝不向真实联系人发消息」
- **启动时提示**：CLI 在标准错误打一行醒目提示，并在标准输出第一行输出 `kind:"status"`，带 `direct_send: true`；GUI 窗口标题前加「直发模式」，启动时播报一次「直发模式已开启，发送不再确认」，状态常驻在标题里
- **本地留痕**：每条直发在档案目录追加一行到 `direct-send.log`（JSON 行）：时间 · 进程种类（CLI / GUI）· 进程号 · `chat_id` · 消息 id（临时与最终）· 内容类型（文字 / 文件……）· 结果。**不记内容、不记名字**。保留 30 天
- **建议**（都不增加摩擦）：`plan` 行照常输出（AI 和人事后都能看到发了什么）；退出时汇总「本次直发 N 条」；`status` 显示本次已直发条数；`direct-send.log` 写不进去就拒绝直发（留痕是条件，不是可选）
- 人已明确知道并接受：AI 自己也能加这个参数；收到的消息里藏的指令可能诱导 AI 发错。本契约**不**为它加 Touch ID、白名单之类的门槛

### 6. 不属于内容树的类型

| 类型 | 字段 |
|---|---|
| `SendPlan` | 目标（类型、名字、id）· 内容全文 · 回复的那条（`MsgId` + 简短读法）· 发起方 · 摘要值。删除时内容换成：要删的消息 id 与各自简短读法 · 给所有人 |
| `PendingMessage` | `chat_id` · 临时 `message_id` · 确认时间 |
| `Page<T>` | `items` · `next_cursor`（无更多时为空）· `total`（TDLib 给得出时才有，否则空） |
| `MessageProps` | 能否回复 / 转发 / 编辑 / 删除（仅自己）/ 删除（双方）· 能否收听 |

### 7. 事件（`subscribe` 推出来的东西）

事件名用 snake_case，CLI 原样输出（`cli.md` 第一节第 5 小节）。第一阶段：

事件名的**唯一登记处**就是这张表：

| 事件 | 带什么 |
|---|---|
| `new_message` | 完整消息节点 |
| `message_content_changed` | `chat_id` `message_id` + 新内容节点（替代内容、投票结果、语音转文字等一切内容变化都走它，store 整体替换内容，不做局部合并） |
| `message_edited` | `chat_id` `message_id` + 编辑时间 |
| `chat_action` | `chat_id` + 话题 + 发送人 + 动作种类（`updateChatAction` `td_api.tl:11530`；第一阶段只认「正在输入」`chatActionTyping` `:6960`，其余落 `other{td_type}`）。播不播由 `notify` 判（F6：只在当前打开的私聊、停手后播一次「对方正在输入」） |
| `messages_deleted` | `chat_id` + `message_ids` + `is_permanent` + `from_cache`（`updateDeleteMessages`，`td_api.tl:11523`）。**只有 `is_permanent` 为真且 `from_cache` 为假才是撤回**、才留「已撤回」占位；`from_cache`（只是清缓存）和非永久（暂时不可访问）都不许显示成撤回 |
| `send_succeeded` | `chat_id` + 临时 id + 最终 `message_id` |
| `send_failed` | `chat_id` + 临时 id + 错误（`errors.md` 的 `code`）+ 能否 `resend` |
| `chat_added` | 会话节点 |
| `chat_position_changed` · `chat_last_message_changed` · `chat_read_inbox` · `chat_title_changed` | `chat_id` + 变了的字段 |
| `chat_read_outbox` | `chat_id` + 对方已读到的消息 id（`updateChatReadOutbox` `:11334`；「已读」状态要它） |
| `message_interaction_changed` | `chat_id` `message_id` + 回应、浏览数、回复数（`updateMessageInteractionInfo` `:11239`） |
| `message_unread_reactions` | `chat_id` `message_id` + 未读回应（`updateMessageUnreadReactions` `:11257`；会话的未读回应数 `updateChatUnreadReactionCount` `:11387` 并入 `chat_read_inbox` 一类，写明字段） |
| `message_content_opened` | `chat_id` `message_id`（`updateMessageContentOpened` `:11244`：已听 / 已看） |
| `message_pinned_changed` | `chat_id` `message_id` + 是否置顶（`updateMessageIsPinned` `:11233`） |
| `chat_notification_changed` | `chat_id` + 会话自己的通知设置（`updateChatNotificationSettings` `:11367`） |
| `scope_notification_changed` | 范围（私聊 / 群 / 频道）+ 全局通知设置（`updateScopeNotificationSettings` `:11489`）。**有效静音**由 store 合成：会话设成「跟随默认」时取它所属范围的设置（`002_roadmap.md` M2.3） |
| `user_changed` | 用户节点 |
| `file_progress` | `file_id` + 已下 / 总字节 |
| `auth_step_changed` | `AuthStep`（不带提示语，理由同 `auth_state`） |
| `net_state_changed` · `core_state_changed` | 新值 |
| `user_status_changed` | 合成后的 `UserStatus`（`errors.md` 第一节第 3 小节），读的一方不必自己合成 |
| `sync_done` | — （启动或重连补发结束） |
| `coalesced` | `dropped`：合并掉的中间态条数 |
| `unknown_update` | 只有 `td_type`，不带内容（与 `tree` 的「未知」节点同理） |

每个事件带 `seq`。

## 二、为什么

1. **意图级、少而稳**：架构方案定了不照抄 TDLib 几百个函数；CLI 和 GUI 用同一组动作，AI 学一套就够（AI 友好①）
2. **查询和动作分开**：Store 查询不联网、不会失败、能同步调用，GUI 按键响应快（速度很快）；假后端也不用实现它们
3. **每个动作必标对外等级**：intent「对外动作无论走哪条路都须人确认」，把它变成表里一列，加新动作时漏不了
4. **凭据绑「发给谁 + 发什么」且一次性**：防止确认 A 却发出 B、确认一次发多次
5. **默认模式不认终端确认**：`/dev/tty` 挡得住管道，挡不住 `tmux send-keys` 和伪终端；正式服务器一律要 AI 拿不到的东西（Touch ID）。要省事，人用直发模式（有意、看得见、有留痕）
7. **L2 不自动重试**：重试一次就可能是对方收到两条
6. **收藏夹按自己的用户 id 判断**：别人可以把会话改名叫「收藏夹」

## 三、正例 / 反例

| | 例子 |
|---|---|
| ✅ | `send_text_prepare(收藏夹, "测试")` → 测试服务器上闸自动开 `test_exempt` 凭据 → `send_text` 成功，收到 `send_succeeded` |
| ✅ | 正式服务器发给收藏夹：CLI 打出计划 → 弹 Touch ID（说明里写「来自命令行，可能是 AI」） |
| ✅ | 人用 `atg-cli --unsafe-direct-send send 777 --text 好的` → 不确认、直接发；`direct-send.log` 多一行（只有 id 和类型） |
| ✅ | `--unsafe-direct-send` 跑在测试档案上发给别的会话 → 仍拒（`test_dc_saved_only`） |
| ❌ | `send_text(plan)` 不带凭据也能发 |
| ❌ | 确认时内容是「好的」，之后改成「好的！」用同一张凭据发出 |
| ✅ | GUI 进会话 → `chat_open` → 显式 `mark_read(会话, 最新一条)`（F1，同官方客户端） |
| ✅ | `--unsafe-direct-send` 下删除一条给所有人 → 不确认、直接删（D1）；`direct-send.log` 多一行 |
| ❌ | `chat_open` 自己顺手调 `viewMessages`（标已读必须是显式的 `mark_read`） |
| ❌ | `atg-cli history` 把读到的消息标成已读 |
| ❌ | `Backend` 上出现 `get_chat_history_raw(td_json)` 这种 TDLib 透传口子 |
| ❌ | 用会话标题等于「Saved Messages」来判断收藏夹 |
| ❌ | AI 在 tmux 里跑 `atg-cli send …`，再 `tmux send-keys y` 把确认按掉（默认模式下终端确认根本不开凭据） |
| ❌ | 用环境变量或配置文件打开直发模式（只认启动参数） |
| ❌ | `mark_read` 只想标第 5 条，结果第 1–4 条也被标了却没告诉人 |
| ❌ | 缓存清理（`from_cache`）被显示成「已撤回」 |
| ❌ | 网络超时后内核自动再调一次 `sendMessage` |
| ❌ | `atg-gui` 里直接 `use tdlib_rs::functions::send_message` |

## 四、⚠️ 待人拍板

> ✅ **已拍板（2026-10-25）**：结果见 `docs/plan/003_decisions-b2.md`——A1（= F1）进会话就标到最新一条 · A2 同官方客户端 · A3 停住说「没有更多未读」· A5 放后面版本 · D1 一切 L2 免确认。正文已同步，下面的题面保留作依据。

**A1. 标记已读 / 已听（L1）怎么处理** —— 正本是 `docs/research/002_features/decision.md` 的 **F1**，只在那里问人一次（含「焦点经过哪条标哪条」选项与「AI 代操作」开关）。本文不受结果影响的部分：CLI 的 `history` / `show` **默认不标已读**。

**A2. 在线状态与「正在输入」（L1）** —— ✅ 人选：和官方客户端一样
TDLib 的 `online` 选项设为真时，别人会看到你「在线」；GUI 输入时可以发「正在输入」。
- A. CLI 永远不设在线、不发正在输入；GUI 仅在前台时设在线，正在输入默认关
- B. 全都不发（隐身式）
- 建议：A；正在输入第一阶段不做

**A3. 「下一个有未读」到底时** —— ✅ 人选 A
- A. 停住并说「没有更多未读」
- B. 回到第一个有未读的会话
- 建议：A（不回绕，和列表两端夹紧一致）

**A4. ~~发给别人一律 Touch ID，要不要放宽~~ —— 已被第七节第 11 条（直发模式）吸收，关闭**
默认模式保持最严（正式服务器一律 Touch ID）；嫌确认烦，用 `--unsafe-direct-send` 启动。原来的放宽选项不再需要。

**D1. 直发模式管不管「改 / 删 / 回应」** —— ✅ 人选 B：全部免确认
人原话是「直接发」。本文暂定只管「发出」类（发送、重发，以后的发文件、转发）；编辑、删除（双方）、表情回应照默认规则要 Touch ID。
- A. 只管「发出」（暂定）
- B. 一切 L2 都免确认
- 建议：A；直发模式下 AI 改错、删错别人看得到的消息，比多发一条更难挽回

**A5. 代理（`proxy_add` / `proxy_enable`）进不进 MVP** —— ✅ 人不用开梯子，放后面版本（正本在此；`decision.md` 只放指针）
- 问你：**你平时开 Telegram 之前要不要先开 VPN / 梯子？是全局模式，还是只设了系统代理？**
- 全局模式（所有流量都走）：TDLib 能直接连上，代理可以不进 MVP
- 只设系统代理：⚠️ TDLib 大概率不读系统代理（未实测），没有代理设置就登不上 → 代理要进 MVP
- 不用梯子：不进 MVP

## 修订记录

- 2026-10-25 初稿（infra）
- 2026-10-25 按 b2 拍板同步（sync）：L1 按 F1 / A2 同官方客户端（GUI 发、CLI 默认不发）；`typing` 进 MVP；`delete`（给所有人）进 MVP（S1）；直发模式免一切 L2（D1）；代理不进 MVP（A5）；威胁边界按 P4 改写；本机接口下凭据由执行方重开（P1）；加 `chat_action` 事件（F6）
- 2026-10-25 按 b3 拍板（`docs/plan/005_decisions-b3.md`）同步（sync）：`test_dc_saved_only` 加第二个测试号例外（N7）；直发也免草稿与阅后即焚（N6）；开发测试时 L1 不再受红线限制（N3）
