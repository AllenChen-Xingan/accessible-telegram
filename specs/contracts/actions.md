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

记法：`名字(参数) -> 返回`。返回里的 `ChatItem`、`Message`、`UserRef`、`MsgId`（消息身份，`{chat_id, message_id}` 一个对象）是 `tree` 定的内容节点，正式名以 `specs/content-tree.md` 第三节第 7 小节为准（群不是单独类型，是 `ChatItem` 的 `ChatKind`）；不属于内容树的 `SendPlan`、`DeletePlan`、`ReactPlan`、`AddedReactions`、`PendingMessage`、`Page`、`MessageProps` 在本文第一节第 6 小节定义。`Ack` = 成功无返回值。所有动作都可能返回 `disconnected`、`network`、`rate_limited`、`internal`，下表「错误」列只写**额外**的。

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
| `auth_submit_phone(phone: String)` | `Ack` | `auth_phone_invalid` `auth_phone_not_for_profile` `auth_wrong_state` | L0 | `setAuthenticationPhoneNumber`（`td_api.tl:12192`）。先在本地查形状与档案（测试档案只收 `99966` 开头，正式档案不收），不合格不发给 TDLib。等号码、等验证码、等邮箱、等密码、等注册这几步都收（TDLib 允许中途换号重来） |
| `auth_submit_code(code: String)` | `Ack` | `auth_code_invalid` `auth_code_expired` `auth_wrong_state` | L0 | `checkAuthenticationCode`（`:12222`） |
| `auth_resend_code()` | `Ack` | `auth_wrong_state` | L0 | `resendAuthenticationCode`（`:12215`） |
| `auth_submit_email(address: String)` | `Ack` | `invalid_input` `auth_wrong_state` | L0 | `setAuthenticationEmailAddress`（`:12210`） |
| `auth_submit_email_code(code: String)` | `Ack` | 同验证码 | L0 | `checkAuthenticationEmailCode`（`:12219`） |
| `auth_submit_password(pw: Secret)` | `Ack` | `auth_password_invalid` `auth_wrong_state` | L0 | `checkAuthenticationPassword`（`:12261`）；`Secret` 用完即清零，Debug 不打印 |
| `logout()` | `Ack` | — | L0（本机）\* | `logOut`（`:12292`）；\* 不对外，但会让本机丢掉登录，须人当场确认（同 L2 的确认步骤，不过发送闸） |

实现上这一组的输入（号码、验证码、邮箱、密码）一律用 `Secret` 装（`Debug` 打不出、用完清零），比上表写的 `String` 更严。提交成功只表示 TDLib 收下了；下一步看下一条授权状态更新——同一步可能重新下发（换号重交、重发验证码后仍是 `wait_code`），所以不能只比步骤名。

注册新账号（`registerUser`）、找回密码第一阶段不做（例外：**测试档案上**测试代码可以注册，因为测试服务器会定期清空账号，见 `testing.md` 第 5 小节；`atg-cli` 的 `login` / `auth *` 在测试档案上遇到注册步骤会自动注册，名字写死 `Atg Test`，`names.txt` 建好后改取它）；二维码登录（`auth_qr`，`requestQrCodeAuthentication` `:12228`）以后再说，对读屏用户不友好。不做的：落到 `unsupported` 步骤（`errors.md`）。

**会话（T02、T08、T09）**

| 动作 | 返回 | 额外错误 | 等级 | TDLib |
|---|---|---|---|---|
| `chats_load(list: ChatListKind, limit: u32)` | `Ack`；会话经 `updateNewChat` / `updateChatPosition` 等进 Store | — | L0 | `loadChats`（`:12502`）；没有更多时 TDLib 回 404，内核吞掉、标「已到底」 |
| `chat_get(chat: ChatId)` | `Ack`；会话经 `updateNewChat` 先进 Store | `not_found` | L0 | `getChat`（`:12413`）；本地没有这个私聊（如收藏夹）时改用 `createPrivateChat(force=false)`（`:14288`）取已有的，**不新建会话** |
| `mute(chat: ChatId, on: bool)` | `Ack` | `not_found` | L0 | `setChatNotificationSettings`（`:14493`）；从第二批提到 MVP：吵的群会打断读屏 |
| `search_chats(query: String, limit: u32)` | `Vec<ChatItem>` | — | L0 | `searchChats`（`:12523`，本地）；不够再 `searchChatsOnServer`（`:12529`） |
| Store：`chat_list(list)` | 当前排好序的 `Vec<ChatItem>` | — | — | 纯查询 |
| Store：`next_unread_chat(after: ChatId)` | `Option<ChatId>` | — | — | 纯查询，T09；到底不回绕，返回空，界面读「没有更多未读」（A3） |
| Store：`pending_sends()` | `Vec<MsgId>`（按会话 id、消息 id 排） | — | — | 纯查询，T04；store 里活着、还在「待发」的自己的消息（见「发送」表下「待发出的消息」）。待发的不参与删旧、也不占每会话的保留配额 |

`ChatListKind`：`main` · `archive`（`td_api.tl:3970,3973`）。文件夹第一阶段不做。

**消息（T03、T05、T06、T07）**

| 动作 | 返回 | 额外错误 | 等级 | TDLib |
|---|---|---|---|---|
| `chat_open(chat: ChatId)` | `Ack` | `not_found` | L0 | `openChat`（`:14188`）；让 TDLib 推这个会话的实时更新（超级群 / 频道只有打开的会话才推）。**`chat_open` 本身绝不隐式调 `viewMessages`**（频道浏览量也靠它涨）；GUI 进会话时另行**显式**调 `mark_read` 标到最新一条（F1）；必须与 `chat_close` 配对 |
| `chat_close(chat: ChatId)` | `Ack` | — | L0 | `closeChat`（`:14191`） |
| `forum_topic_get(chat: ChatId, topic_id: i64)` | 话题的「已读到」位置 + 通知设置 | `not_found` `invalid_input`（话题 id 超出 int32，本地先拒） | L0 | `getForumTopic`（`td_api.tl:13632`），只读、不标已读。话题没加载过时 TDLib 不推它的 `updateForumTopic`，服务器推来的设置变更也丢掉（TDLib 1.8.69 `ForumTopicManager.cpp:659-662`），所以 `notify` 判到一个还没设置的话题时请调用方取一次；应答进 store 时只在还没有这个话题时收，不盖掉之后推来的更新 |
| `history(chat: ChatId, from: MessageId, offset: i32, limit: u32, local_only: bool)` | `Vec<Message>`（从新到旧） | `not_found` · `invalid_input`（`limit` 不在 1–100、`offset` 不在 -99–0、`offset` 为负时 `limit < -offset`：本地先拒，不发） | L0 | `getChatHistory`（`:12758`），参数同 TDLib（`from` 为 0 = 最新一条起；`offset` 为 0 时**不含** `from` 那条、往旧取（TDLib 1.8.69 `OrderedMessage.cpp` `get_history`）；负 `offset` 含它、另取比它新的，往新翻页用（CLI 那层去掉 X 本身）。应答为空 = TDLib 已经自己从本地库 / 服务器拉过、确实没有了；不空但短 = 它只给了内存里连着的那段，再要能拿到更多）；带请求代次（架构方案第一节第 3 小节第 6 条）。F2 落点、循环补满、从旧到新排在 CLI 里做 |
| `message_get(id: MsgId)` | `Message` | `not_found` | L0 | `getMessage`（`:12418`）；`MsgId` 即 `tree` 的 `MsgId` |
| `replied_message_get(id: MsgId)` | `Option<Message>` | — | L0 | `getRepliedMessage`（`:12436`）；T06「回复谁」 |
| `message_properties(id: MsgId)` | `MessageProps`（能回复 / 能转发 / 能编辑……） | `not_found` | L0 | `getMessageProperties`（`:12457`）；决定菜单里列什么 |
| `user_get(user: UserId)` · `group_get(id)` | `UserRef` / `ChatItem` | `not_found` | L0 | `getUser`（`:12391`）· `getBasicGroup` / `getSupergroup`（`:12397,12404`）；读法查不到名字时补取 |
| `search_messages(scope: Option<ChatId>, query: String, limit: u32, cursor: Option<Cursor>)` | `Page<Message>` | — | L0 | 有会话用 `searchChatMessages`（`:12793`），否则 `searchMessages`（`:12806`） |
| `file_download(file: FileRef)` | `Downloaded{path, size}`（同步等下完：`downloadFile` `synchronous=true`） | `not_found{what:"file"}` · `internal{what:"download_incomplete"}` | L0 | `downloadFile`（`:15001`）；对外不可见，前提是不调 `mark_listened`。**只有 TDLib 报 `is_downloading_completed` 才交路径**（exp007：没下完的文件解码器会静默解出半截）。文件句柄是内容树附件里的 `FileRef`（`specs/content-tree.md` 第三节第 3 小节），v0 只有语音带。T07 语音时长、文件名在消息节点里就有，不必下载 |
| `mark_listened(id: MsgId)` | `Ack` | `not_found` | **L1**（与已读同一道把关，A1）；阅后即焚类 **L2** | `openMessageContent`（`:14205`）；对方会看到「已听 / 已看」，阅后即焚一打开就开始倒计时、不可逆。GUI 播放即标（同官方客户端，与 F1 同一取向）；CLI 只在 `play --mark-listened` 时标；阅后即焚按 L2 要确认 |
| `mark_read(chat: ChatId, up_to: MessageId)` | `Ack` | — | **L1** | `viewMessages`（`:14199`，`force_read`）。⚠️ **不是按条标**：TDLib 取传入的最大消息 id，把普通会话的已读位置推进到那里（`MessagesManager.cpp:16961` `read_dialog_inbox`），此前的消息全部连带变成已读。所以参数是「标到哪条为止」。F1 人定：GUI 进会话就标到最新一条（同官方客户端，不确认）；CLI 的 `history` / `show` 不标 |
| `message_links(id: MsgId)`（**不是 `Backend` 动作**：`Backend` 动作是真后端、假后端各实现一份的那种；这个是在它们之上拼出来的） | `Vec<Link>`（链接文字 + 地址 + 是否隐藏 `hidden`） | `not_found` | L0 | `message_get`（`getMessage`）取这条，再用 `tree::text::links_marked` 从内容树取；现在 CLI 的 `links` 这样拼，GUI 照同样两步。隐藏的链接调用方默认遮住（`cli.md` `links`） |
| `open_link(url)` · `open_file(file: FileId)` | `Ack` | `not_found` `invalid_input` | **L1 外部** | 用系统接口（`NSWorkspace`）打开，**不经 shell**。链接只许 `http` / `https`，其余协议（`file:`、`tg:`、`mailto:`、自定义协议……）拒绝；打开前把完整网址（域名单独读出）给人看并确认。文件只许常见的只读类型（图片、音频、视频、PDF、纯文本），可执行文件、`.app`、`.command`、脚本、安装包一律拒绝，改为「在访达中显示」。打开文件前须已下载 |

**发送（T04）**

| 动作 | 返回 | 额外错误 | 等级 | TDLib |
|---|---|---|---|---|
| `send_text_prepare(target: ChatId, draft: Draft)` | `SendPlan`：目标、内容全文、回复的那条、**最终话题**、内容摘要值 | `not_found` `forbidden` `invalid_input`（带 `why`）`send_refused`（`reply_dropped` / `topic_changed`） | L0 | 只查不发；给确认步骤看。**话题规则**（`decision.md`、`002_roadmap.md` M2.2）：话题群里回复某条，话题取被回复那条的；话题群里不回复任何一条的新消息，第一阶段拒发（`invalid_input`，`why:"forum_needs_reply"`，「话题群里请先选一条消息回复」）。被回复那条不能回复（`getMessageProperties.can_be_replied` 为假）→ `invalid_input`（`why:"reply_not_allowed"`）。最终话题进计划展示、进摘要、发送时校验。**预览比对**（003 发6）：组好请求后调一次 `sendMessage` 的 `only_preview`（TDLib 走完会话、权限、内容校验，返回假消息、不发），预览里的回复对象、话题与请求逐项比对，不一致就拒——TDLib 遇到被回复那条找不到 / 不能回复 / 不在同一话题时**不报错、悄悄去掉回复照常发**（`MessagesManager.cpp:21144-21170`）。摘要仍按我们交给 TDLib 的请求算，不按预览算 |
| `send_text(plan: SendPlan, token: ConfirmToken)` | `PendingMessage`（临时 id）；最终结果经事件 `send_succeeded` / `send_failed`，或临时 id 出现在 `messages_deleted` 里（结果未知） | `send_refused` `forbidden` | **L2** | `sendMessage`（`:13143`）；成败分别是 `updateMessageSendSucceeded` / `updateMessageSendFailed`（`:11208,11214`）。请求固定：`inputMessageText.clear_draft = false`（传 true 会顺手清掉人在手机上的云端草稿，003 发2）；`options.sending_id` 带一个本次随机的正数（TDLib 原样放回待发消息的状态里，早到的「待发」新消息能认出是自己这次的，003 发7）。闸验过凭据后、**紧挨着交出前再预览比对一次**（准备到交出之间最长 60 秒、还等了人确认，被回复那条可能被删、被挪话题），不一致 `send_refused`（`reply_dropped` / `topic_changed`），不发；`send_succeeded` 到了再核一遍，不一致在结尾行标明（`reply_kept:false`，事后兜底）。**等结果有 30 秒上限**（单个请求本身没有超时，003 发9；交出本身——验凭据、复核、等 TDLib 回交出的应答——也算在这 30 秒里，迟迟不回同样按结果未知报，`temp_message_id` 为 0）：到点按 `send_outcome_unknown` 报；晚到的应答与结果照常进 store，不丢。结果可能比 `sendMessage` 的应答先到（003 发4）：事件一直在流里排着，应答回来后从头看 |
| `resend_prepare(chat: ChatId, ids: Vec<MessageId>)` | `ResendPlan`：同一会话里这几条，**一份计划、一张凭据**（每条写明会不会去掉回复关系） | `not_found` `invalid_input` `rate_limited` | L0 | 只查不发。按 `messageSendingStateFailed`（`:3458-3465`）逐条检查：`can_retry` 为假 → 拒；`retry_after` 没到 → `rate_limited`；`need_drop_reply` → 计划里写明「重发时会去掉回复关系」（`resendMessages` 自动去掉，`:3462`）；`need_another_sender` / `need_another_reply_quote` → 第一阶段拒；`required_paid_message_star_count > 0` → **一律拒**（付费发送不做） |
| `resend(plan: ResendPlan, token: ConfirmToken)` | 每条一项：`PendingMessage` 或该条的错误 | `send_refused` | **L2** | `resendMessages`（`:13195`）；凭据绑交出的 `resendMessages` 请求（会话 + 这几条 id）、每条重发后的回复关系与**每条的正文**；`plan` 里每条给出正文与回复关系（重发后仍保留的回复对象也进凭据摘要）；重发不进直发的重复提交拦截（第 5 小节）；TDLib 对不能重发的那条返回 null（`:13190`），逐条报告，不当整体失败；交出前重取每条的失败态、正文与回复关系，与人确认过的计划比，变了拒 `send_refused(content_mismatch)`、什么都不交出 |
| `draft_set(chat: ChatId, text: String, token: ConfirmToken)` · `draft_clear(chat, token)`（第二批） | `Ack` | `send_refused` | **L1，按 L2 同级把关** | `setChatDraftMessage`（`:14488`）；不发给对方，但会同步到你手机的输入框——AI 写进去、人在手机上顺手一按就发了；清掉则会毁掉你手写的草稿。所以与发送用**同一道闸、同一种凭据**（绑会话 + 内容摘要；测试服务器 + 收藏夹免确认）。GUI 输入框里的草稿默认**只存本机、不同步到云端**——「人在打字」同样证明不了是人（computer use 也能打字） |
| `typing(chat: ChatId)` | `Ack` | — | **L1** | `sendChatAction`（`:14159`）；A2 人定进 MVP：GUI 输入框里打字时发（同官方客户端，`002_roadmap.md` M4.1）；CLI 不发 |
| `delete_prepare(chat: ChatId, ids: Vec<MessageId>)` | `DeletePlan`（删哪几条、给所有人） | `not_found` `forbidden` | L0 | 只查不发；逐条按 `message_properties` 的「能否删除（双方）」检查，不能的拒。⚠️ 收藏夹里的消息不能给所有人删（`MessagesManager.cpp:8346`），必拒 |
| `pending_cancel_prepare(chat: ChatId, ids: Vec<MessageId>)` | `DeletePlan`（撤哪几条、每条正文） | `not_found` · `invalid_input`：`why:"not_pending"`（不是自己的待发消息）；id 为空或超过 100 条时不带 `why` | L0 | 只查不删；逐条 `getMessage` 查「自己发的、还在待发」（见下「待发出的消息」）。临时 id 只在同一进程里有效（续发会重编），CLI 在同一进程里从 Store 的 `pending_sends` 取这个会话的全部待发再调它 |
| `pending_cancel(plan, token: ConfirmToken)` | `Ack`；撤没撤下经事件（临时 id 出现在 `messages_deleted` = 撤销了；先来 `send_succeeded` = 没赶上，随后正式 id 也被删 = 只从自己这边删掉了） | `send_refused` `invalid_input`（`not_pending`：交出前再查，已发出或正文变了） | **L2** | `deleteMessages`（`:13226`）**`revoke=false`**（见下「待发出的消息」）；凭据绑会话 + 这几条 id + 每条正文；只是尽力撤销 |
| `delete(plan, token: ConfirmToken)` | `Ack`；结果经 `messages_deleted` | `send_refused` `forbidden` | **L2** | `deleteMessages`（`:13226`）`revoke=true`。**S1 人定进 MVP：只做「给所有人」**；只删自己这边（`revoke=false`）留第二批。凭据绑会话 + 消息 id 列表 |

**转发（#10）**

| 动作 | 返回 | 额外错误 | 等级 | TDLib |
|---|---|---|---|---|
| `forward_prepare(to: ChatId, from: ChatId, ids: Vec<i64>)` | `ForwardPlan`：目标与来源会话（类型、名字、id）、每条的 id 与纯文字正文（带附件、系统消息为空）、发起方、开法、摘要 | `not_found` `invalid_input`（`why`：`forward_too_many` 一次超过 20 条，不截断、不分批；`not_forwardable` 有一条不能转，如来源会话禁止转发，整批不转）`send_refused(test_dc_saved_only)` | L0 | 只查不发：`me` → 目标会话 → 来源会话 → 逐条 `getMessage`（在不在）+ `getMessageProperties.can_be_forwarded` → 组请求、算摘要。id 去重、按从小到大排（TDLib 要严格递增）。目标是论坛群时不带话题，落进「General」（计划行写明）。只做普通转发（带原发送人），不做复制（`send_copy`，会隐去原作者）；20 条与「不做复制」是人定前的默认（2026-11-17，待人定） |
| `forward(plan, token)` | 每条一项：`PendingMessage`（目标会话里的临时 id）或「转不了」（TDLib 应答里是 null） | `send_refused` `forbidden` `send_outcome_unknown` | **L2，直发** | `forwardMessages`（`:13180`，`options` 为空即默认、`send_copy=false`、`remove_caption=false`）。和其他 L2 一样直发（#113）。**交出前复核**：逐条重取，和计划比（内容、临时内容、受限、敏感、编辑时间、发送人）；准备到交出之间被编辑、删掉、变得不能转的，`send_refused(content_mismatch)`、什么都不交出。每条之后照 `send` 等结果（30 秒，连同交出），到点 `send_outcome_unknown`。**进重复提交拦截**（第 5 小节）：同一批转两次对方就收到两份。**留痕**另记来源会话 `from_chat_id`（只记 id） |

**表情回应（#9）**

| 动作 | 返回 | 额外错误 | 等级 | TDLib |
|---|---|---|---|---|
| `react_prepare(id: MsgId, emoji: String, remove: bool, replace: bool)` | `ReactPlan`：目标会话、哪一条、哪个回应、加还是去、可能挤掉的我原先点的回应、这份名单拿不拿得准（`replaces_known`）、开法、摘要 | `not_found` `invalid_input`（`why`：`reaction_invalid` 不是一个 emoji；`reaction_not_chosen` 要去掉的我没点过；`reaction_would_replace` 这条上我已点了别的回应、没给 `replace`——每条能点几个有上限，超了 TDLib 自动去掉最早的，直发没人当场看着，所以默认拒；`reaction_state_unknown` 我点过哪些拿不准、没给 `replace`：本机库里是缓存，拿得准要调用方等到补发结束（启动或重连后数据追平，即 `sync_done`，本节第 7 小节），且只有私聊、加密聊天信本机（TDLib 只给它们推回应变化，`MessagesManager.cpp` 里普通群、超级群、频道都靠打开时轮询），其余另向服务器取一页「谁点了」（`getMessageAddedReactions`）、一页取全才算）`send_refused(test_dc_saved_only)` | L0 | 只查不发：`me` → 目标会话 → `getMessage`（这条在不在、我点过哪些）→ 组请求、算摘要。去掉（`remove`）判我点没点过只看本机缓存，任何会话都不另问服务器：判错只会误报 `reaction_not_chosen`，不会动到别的回应。v0 只收 emoji（`reactionTypeEmoji`）；自定义表情不做，**付费星星（`reactionTypePaid`，要花钱）不做** |
| `react(plan, token)` | `Ack` | `send_refused` `forbidden` `invalid_input`（这个会话不许这个回应，TDLib 400）`send_outcome_unknown` | **L2，直发** | `addMessageReaction`（`:13748`，`is_big=false` 不放大动画、`update_recent_reactions=false` 不改本机「最近用过的回应」列表——少动别处）/ `removeMessageReaction`（`:13754`）。和其他 L2 一样直发（#113），闸开 `direct_mode` 凭据（测试服务器收藏夹 `test_exempt`）。**交出前复核**：加回应交出前按准备时同样的条件再判一次会不会挤掉、拿不拿得准（计划可能是早先准备的，期间又点了别的、或断过线）——「补发完了」要准备时完了且现在连接线仍是 `ready`；没给 `replace` 的照旧拒，给了的不拒、留痕按新算的记。同一后端同一条消息同时只许一个回应在交出途中，第二个报 `reaction_state_unknown`；交出后没等到应答就被放弃（到点）的，本进程里这条之后一律按拿不准拒，`replace` 才放行。其余照 L2：凭据绑会话 + 请求摘要（哪条、哪个回应、加或去）、60 秒一次性、受限与限流照管、不自动重试、测试服务器只许收藏夹；交出连同应答 30 秒上限，到点 `send_outcome_unknown`（回应可能已点上）。**留痕**：照直发写 `direct-send.log`（`kind` 为 `react` / `unreact`，不记 emoji）；另记 `reaction`：点在哪条消息上（`message_id`）、按最坏情况可能挤掉几个原先的回应（`may_replace`，事前估的上限、不是实际挤掉的数，只记个数；拿不准还用 `replace` 放行的记 `null`；去掉回应恒为 0）；不进重复提交拦截（同一个回应点两次不会多出一个） |
| `added_reactions(id: MsgId, kind: Option<ReactionKind>, offset: String, limit: u32)` | `AddedReactions`：总数 + 这一页（谁、哪个回应、是不是我、时间）+ 下一页的 `next_offset`（空 = 没有更多） | `not_found` `invalid_input` | L0 | `getMessageAddedReactions`（`:13802`），只读；`limit` 1–100（调用方先查）。点回应的人经 `updateUser` 先进事件流，名字投影时从 store 填（`tree` 的 `reactor` 节点） |

**待发出的消息**（MVP 必做）：Store 查询 `pending_sends` 列出（Store 只给 `MsgId`，`date` 由调用方从 Store 里那条消息取，合起来就是 `PendingMessage`；`status()` 也该带，`status` 命令还没做）「待发出」的消息（会话 id、临时消息 id、**交给 TDLib 的时间**——消息的 `date`）；`pending_cancel_prepare` → 闸 → `pending_cancel(plan, token)` 删除待发消息（词：**撤销** = 在本机停掉还没发出的请求、删掉这条待发消息；**撤回** = 消息已在服务器上，给会话里所有人删掉，对方设备上也没了，但对方可能已看到或收到过通知），**只是尽力撤销**：删除只停本机的网络请求（`MessagesManager.cpp:21330-21345`），请求已发出就照样送达，TDLib 收到成功应答时自己以 `revoke=true` 在服务器上撤回（`:27848-27851` → `:8484-8490`，对方可能已看到）；会话变得不可访问时连撤回都没有，消息留在对方那边（`:8463-8473`）。所以它不是「结果未知」时的补救，`plan` 行的说明里也不许写成「可以撤销」（003 发3、建议第 8 条）。**等级 L2**，与 `delete` 同一道闸、同一种凭据（大总管定 2026-11-14：赶上已送达会给所有人撤回，属对外影响）：准备时逐条查「自己发的、还在待发、纯文字」（否则 `invalid_input`，`why:"not_pending"` / `"not_text"`；带附件的给不出全文、绑不进凭据），凭据绑会话 + 这几条 id + 每条正文，交出前再查一遍（准备到交出之间发出了，再删就成了给所有人删除——拒，`not_pending`）；`plan` 行的 `note_zh` 写明「已送达的会给所有人撤回」（直发、不确认，#113）。**交出时用 `revoke=false`**（自审第 3 轮 P1）：TDLib 删除时先把临时 id 换成它发出后的正式 id（`get_persistent_message_id`，`:8415`、`:21075-21087`），交出前的复查与 TDLib 处理删除之间隔着一次往返，恰好在这段空隙里发出的那条会被当成正式消息删——`revoke=true` 就成了给所有人删除。`revoke=false` 对还在待发的消息没有区别（只停本机请求），TDLib 事后的自动撤回固定用 `true`、不受影响；赶上恰好发出的只删掉自己这边，对方那边留着。撤没撤下看之后的事件：临时 id 出现在**永久**的 `messages_deleted` 里（`is_permanent` 且不是 `from_cache`）= 撤销了（仍是尽力；清缓存、暂时不可访问的不算）；先来了它的 `send_succeeded` = 没赶上、已发出，随后（约 1 秒内）它的正式 id 也出现在 `messages_deleted` 里 = 被换成正式 id、只从自己这边删掉了，否则消息留着。
⚠️ **续发**（TDLib 1.8.69 源码）：开着消息库时，上次没发完的消息在 TDLib **启动重放本地日志时当场续发**（`:35925-35930`），任何一条命令（含 `pending` 本身）一启动就续发，「抢在续发之前撤销」做不到，只能赶在网络请求真正发出前；续发时**临时 id 与 `date` 都重编**（`:35851-35856`，`sending_id` 不落盘），所以上一个进程报 `send_outcome_unknown` 时给的 `temp_message_id` 与这次列出的对不上，原始确认时间也拿不到（大总管定 2026-11-14：清单里的时间改为 `date`）；待发超过 24 小时的不再续发、直接变失败（`MAX_RESEND_DELAY`，`MessagesManager.h:1708`；收藏夹除外）。实际撤销效果在服务器上实测，列为 M2.1 验收项。

`Draft` 第一阶段只有：纯文本 + 可选「回复哪条」（`inputMessageText` `:6680`、`inputMessageReplyToMessage` `:3518`）。Markdown 转实体（`parseMarkdown` `:13836`）第二批。

**第二批留位**（`api` 已列，名字先占住）：

| 动作 | 等级 | 说明 |
|---|---|---|
| `edit_text` | L2 | `editMessageText`（`:13252`） |
| `delete(ids, revoke=false)` | L0 但本机不可逆 | 只删自己这边；要人当场确认（给所有人的已进 MVP，见上表） |
| `send_file` · `send_voice` | L2 | `sendMessage` + `inputMessageDocument` / `inputMessageVoiceNote`（`:6700,6738`） |
| `speech_to_text` | L0 | **本机转写**（#59 取代 F4）：`atg-audio` 的 `stt` 起本机辅助程序、用 Apple 端侧模型（SpeechAnalyzer），音频经管道喂、不落盘、不外发（以 Apple 端侧说明为据，**断网未实测**，待人验证）；不调 `recognizeSpeech`（`:13121`，那是 Telegram 服务器识别、要 Premium）。CLI `transcribe`；结果只在内存（转写表），不标已听 |
| `mark_unread` | L0 | `toggleChatIsMarkedAsUnread`（`:14514`） |

所有 L2 一律走同一道闸。

### 4. 发送闸与确认凭据

**威胁边界（先说清楚防什么）**：闸防的是「人或 AI **走正规接口**时出错或越权」——AI 调 CLI、AI 用无障碍树点按钮、脚本误发。闸**防不了**一个能改本仓代码、自己编程序的 AI（它可以绕开 `atg-core` 直接调 TDLib）。P4 人定**开发版也能开正式档案**，人已知这一点；剩下的只有改闸的 PR 人本人看（第 7 条）和 `CLAUDE.md` 第五节「测试绝不向真实联系人发消息」。

**直发模式**（001 第七节第 11 条，人已定）是人**有意**打开的口子，不算漏洞：见第 5 小节。

1. **凭据怎么开**。`ConfirmToken` 只能由 `atg-core::gate` 开出，构造函数不公开。开出方式：

   | 方式 | 适用 | 怎么确认 |
   |---|---|---|
   | `test_exempt` | 「测试服务器 + 收藏夹」（001 第七节第 10 条，人已定），以及第 2 条的第二个测试号私聊（N7） | 闸自动开，写死 |
   | `direct_mode` | 正式服务器上的**一切目标**，含自己的收藏夹（CLI 和 GUI 都一样；第 5 小节） | 不确认 |

   Touch ID / 系统密码（原 `human_auth`）已整个删除（人定 2026-11-16「touch id 这个功能彻底关掉以及完全不用」，#113）

   不做终端确认（`/dev/tty`）：它拦不住 `tmux send-keys`，也拦不住 `script` 造出来的伪终端，做了也是假确认（codex 评审 004 的 P0）。正式服务器一律直发，靠防重复、留痕、限流与 `account_limited` 停发兜底（第 5 小节）。

2. **测试服务器上只许发收藏夹**（架构方案第四节第 1 小节）：测试服务器上发往收藏夹以外的会话，有什么凭据都拒绝（`send_refused(test_dc_saved_only)`）。测试号谁都能登，别人可能往里发消息、拉群。
   **唯一例外（N7 人定）**：与**第二个自己控制的测试号**的私聊——可以互发，也可以把发给它的消息给所有人删（收藏夹里删不了给所有人，`MessagesManager.cpp:8346`）。判定只认：本进程实际的 `use_test_dc` 为真 **且** 目标是与该号的私聊；该号的用户 id 在每次进入 `authorizationStateReady` 时按 `.env.local` 的 `ATG_TEST_PHONE_2`（也须 `99966` 开头）重查，不认会话名字。这个私聊里闸同样自动开 `test_exempt` 凭据（AI 推：例外是为了让自动化测试能跑，可推翻）

3. **`test_exempt` 的判定**只认：本进程启动时**实际**传给 `setTdlibParameters` 的 `use_test_dc`（由闸独家持有，不认后端自报）**且**当前登录号以 `99966` 开头**且**目标是 `me()` 的收藏夹。每次发送前重查，不只在开凭据时查。

4. **`plan` 给人看什么**（终端 `plan` 行、界面都一样）：目标会话的**类型 + 名字 + id**（如「私聊 · 张三 · 777」，改名叫「收藏夹」的会话骗不过去）· 内容全文 · 回复的那条 · **发起方**（界面 / 命令行，命令行注明「可能是 AI」）。显示前把 ANSI 控制序列、双向控制字符、零宽字符转义成可见形式（它们能改写终端显示，VoiceOver 也不读）——范围是 Unicode 的 Default_Ignorable_Code_Point 全集加格式类与行 / 段分隔符，含标签字符（U+E0000–E007F，能藏一整段只有程序读得出的文字）与变体选择符；宁可多转不可漏（`gate::visible`）。（原「Touch ID 弹框的说明区与放不下就拒 `too_long_to_confirm`」一段随 Touch ID 一起删除，#113）

5. **摘要算什么**：最终交给 TDLib 的那份请求的字节（文本**不做** Unicode 规范化）+ 所有发送选项 + 回复目标 + **最终话题** + 自己的账号 id + 服务器类别，取 SHA-256。

6. **凭据的性质**（写死，改它的 PR 人本人看）：不实现 `Clone` / `Copy` / `Serialize` / `Deserialize`；`send_text` 按值吃掉它；有效期 60 秒，用单调时钟；用过的随机数在进程内登记；**绝不跨进程传递**。CLI 经本机接口由别的进程执行时（P1 人定一开始就做，`config-privacy.md` 第一节第 2 小节），由**执行方**的闸重新开凭据，确认说明里标明「来自命令行，可能是 AI」（与第 4 条同一句）。

7. **只有闸能发**：`atg-core` 以外的代码不得直接调 tdlib-rs 的发送类函数（`send_message`、`resend_messages`、`add_message_reaction`、`remove_message_reaction`、`delete_messages`、`send_chat_action`、`set_option`（`online`）、`view_messages`、`set_chat_draft_message`、`open_message_content`……）。tdlib-rs 只在 `atg-core::td` 里引入；CI 用一个扫描脚本拦（`testing.md` 第一节第 4 小节）。改到闸、对外等级、这条清单的 PR 属一级评审，人本人看。

8. 验凭据：目标对、摘要对、60 秒内、没用过、第 2、3 条成立；任一不符即 `send_refused`，不发。
9. **L1 也在入口把关**：`mark_read`、`mark_listened`、`set_online`、`typing`、`draft_set` 都只从 `atg-core` 入口走。F1、A2 人定后，`mark_read` / `set_online` / `typing` 不再一律拒、不确认，谁调谁负责（GUI 按官方客户端的时机调；CLI 不调）；`mark_listened` 照同一取向（AI 推，可推翻：GUI 播放即标，CLI 只在 `play --mark-listened` 时）。开发 / 测试时在正式档案上发这些 L1 也允许（N3 人定，`.42cog/real.md`「不替我对外发消息」已去掉「标记已读」）；`draft_set` 仍按 L2 同级把关；阅后即焚的 `mark_listened` 按 L2。`send_refused(l1_disabled)` 保留，供以后若加整类关闭的开关时用（第一阶段没有这个开关）。

### 5. 直发模式（001 第七节第 11 条，人已定 2026-10-25；人定 2026-11-16 为唯一方式）

人原话见 `docs/plan/001_arch-stack.md` 第七节第 11 行。下面只写它在契约里怎么落：

- **怎么进入**：不用进入——**直发是唯一方式**（人定 2026-11-16「我完全不需要 touch id」#111、「touch id 这个功能彻底关掉以及完全不用」#113）。`--unsafe-direct-send` 留着只为旧脚本不报错，加不加一样（帮助里不显示）
- **管多久**：一直如此，没有开关。测试服务器只许发收藏夹（第一节第 4 小节第 2 条）照旧
- **免什么**（D1 人定：全部免确认）：**一切 L2**——`send_text`、`resend`、`delete`（给所有人），`react` / 去掉回应，以及将来的 `send_file` / `send_voice` / `forward` / `edit_text`——**所有会话**免确认，闸直接开 `direct_mode` 凭据。`draft_set`（按 L2 同级）与阅后即焚的 `mark_listened`（按 L2）同样免（N6 人定：一切 L2 都免确认）
- **不免什么**（这些不是「确认」，是防误发与防封号，不增加摩擦）：摘要与目标校验照做（防「确认 A 发 B」那类程序错误）；L2 不自动重试；限流与 `account_limited` 照旧；**重复提交拦截**（下一条）。`logout` 的当场确认不是 L2，照旧
- **重复提交拦截**（`docs/research/003_send-receive-prior-art/decision.md` B17-2，人选 A「拦下」）：一次性凭据只防同一张凭据被重放，直发模式下闸每次自动开新凭据，双击、界面重复触发、AI 在「结果未知」后重跑都会发出第二条，撤不回。所以：
  - **管谁**：只管 `direct_mode` 凭据（含转给以直发启动的执行方的命令）。`test_exempt` 不拦（回归测试要反复发固定文本）
  - **比什么**：指纹 = HMAC-SHA256（密钥，会话 id ‖ 意图摘要）。意图摘要同第 4 小节第 5 条的摘要，只是把每次都变的 `sending_id` 记成 0——同目标 + 同内容 + 同回复 + 同话题 = 同指纹。密钥由本地库钥匙**带上下文标签**派生（`HMAC(库钥匙, "atg.send-guard.v1" ‖ 档案名)`），只在内存里，不新建钥匙串条目；短句也反推不回内容
  - **记在哪**：档案目录 `send-guard.json`（`config-privacy.md` 第一节第 1 小节）。交给 TDLib **之前**写「已提交」（写不进去就拒直发，同留痕），等到结果改成「已发出 / 失败 / 未知」；进程中途死掉，「已提交」照样算数。记录文件读不了（含文件坏了）就不写、拒直发——拿空表覆盖会抹掉别的发送的记录；坏了要人删掉它才恢复
  - **何时拦**：同会话同指纹、上次是「已发出」且在 **10 分钟内**，或上次是「未知 / 已提交（没收尾）」且在 **24 小时内** → `send_refused(duplicate_recent)`，`error.data` 带 `last_at`、`last_ago_s`、`last_outcome`，说明写清「几秒前、什么结果」。上次是「失败」（服务器明确没收）不拦——失败的用 `resend`。10 分钟要长过「结果未知」的 30 秒，又不把同一句话长久拦死。结果未知的另留 24 小时：那条可能还在 TDLib 的本地待发队列里，下次任何一条命令启动 TDLib 都会自动续发（003 发8），断网几小时后才发出也有可能，10 分钟后放行就会发两条（自审第 2 轮）；仍有期限，不永远拦（B17-2「到期作废」）。每次写时删掉过期的。时间比「现在」还晚的记录（本机时钟被往回拨过）照样算数，等时钟追上再过期——丢掉就等于回拨时钟能解除拦截
  - **放行**：`send --again`（只这一次；经本机接口转发时随参数过去）。放行的那次照样记一条
  - `react` 也不进这套：同一个回应点两次 Telegram 那边不会多出一个
  - `forward` **进**这套：指纹 = 目标 + 来源会话 + 这几条 id（请求本身）；同一批转给同一会话两次对方收到两份。`forward --again` 放行
  - `resend` 不进这套：重发对象是某条失败消息的 id，重发后旧 id 就没了，同一条重发不了两次
- **测试档案**：仍然**只许发收藏夹**（`test_dc_saved_only`），外加第 4 小节第 2 条的第二个测试号私聊（N7）。测试服务器上的号谁都能登，可能是真人；`CLAUDE.md` 第五节「测试绝不向真实联系人发消息」
- **启动时不提示**：直发是唯一方式，CLI 不出开场 `status` 行与醒目提示，GUI 标题不加「直发模式」、不播报（原规定随 #113 删除）
- **本地留痕**：每条直发在档案目录追加一行到 `direct-send.log`（JSON 行；交给 TDLib 之前先写一行结果为 `submitted` 的，写不进去就拒，等到结果后再写一行）：时间（Unix 秒）· 进程种类（CLI / GUI）· 进程号 · `chat_id` · 消息 id（临时与最终）· 内容类型（文字 / 文件 / 表情回应……）· 表情回应另有 `reaction`（`message_id` · `may_replace` 个数，拿不准为 `null`）· 转发另有 `from_chat_id`（来源会话 id）· 结果（`submitted` / `succeeded` / `failed` / `unknown`，失败带错误码）。**不记内容、不记名字**。保留 30 天（写的时候发现头一行已过 30 天就整理一遍）
- **建议**（都不增加摩擦）：`plan` 行照常输出（AI 和人事后都能看到发了什么）；退出时汇总「本次直发 N 条」；`status` 显示本次已直发条数；`direct-send.log` 写不进去就拒绝直发（留痕是条件，不是可选）
- 人已明确知道并接受：AI 能直接发；收到的消息里藏的指令可能诱导 AI 发错。本契约**不**为它加确认、白名单之类的门槛

### 6. 不属于内容树的类型

| 类型 | 字段 |
|---|---|
| `SendPlan` | 目标（类型、名字、id）· 内容全文 · 回复的那条（`MsgId` + 简短读法）· 发起方 · 摘要值（删除用 `DeletePlan`，见下一行） |
| `DeletePlan` | 目标（同 `SendPlan`）· 每条 `temp_message_id` 与能给人确认的正文（不是纯文字为空）· `hidden_chars` · 发起方 · 确认方式 · 摘要 · `note_zh`（「只是尽力撤销」）。`action` 现在只有 `pending_cancel`（交出 `revoke=false`）；M2.7 再加「给所有人」（`revoke=true`）一项；`docs/plan/002_roadmap.md` M2.7「删除（给所有人）」也用它（那时 id 叫 `message_id`） |
| `ForwardPlan` | 目标与来源会话（类型、名字、id）· 每条 `message_id` 与纯文字正文（不是纯文字为空）· `hidden_chars` · `topic`（恒为空：论坛群落「General」）· 发起方 · 开法 · 摘要 |
| `ReactPlan` | 目标（类型、名字、id）· 哪一条 · 哪个回应（`ReactionKind`）· 加还是去 · 可能挤掉的我原先点的回应 · 这份名单拿不拿得准（`replaces_known`）· 发起方 · 开法 · 摘要值 |
| `AddedReactions` | `total` · 这一页每项（回应种类 · 谁（发送人 id）· 是不是我 · 时间）· `next_offset`（空 = 没有更多） |
| `PendingMessage` | `chat_id` · `temp_message_id`（临时 id）· `date`（交给 TDLib 或续发的时间；续发时与临时 id 一起重编） |
| `Page<T>` | `items` · `next_cursor`（无更多时为空）· `total`（TDLib 给得出时才有，否则空） |
| `MessageProps` | 能否回复 / 转发 / 编辑 / 删除（仅自己）/ 删除（双方）· 能否收听 |

### 7. 事件（`subscribe` 推出来的东西）

事件名用 snake_case，CLI 原样输出（`cli.md` 第一节第 5 小节）。第一阶段：

事件名的**唯一登记处**就是这张表。`atg-core` 的 `CoreEvent`（`src/atg-core/src/model.rs`）就是这些对外事件，变体名与事件名一一对应（拍板 006 乙6）；store 消费的事件以本表为准，表外的不许出现在 `CoreEvent` 里：

| 事件 | 带什么 |
|---|---|
| `new_message` | 完整消息节点 |
| `message_content_changed` | `chat_id` `message_id` + 新内容节点（替代内容、投票结果、语音转文字等原内容的变化都走它，store 整体替换原内容，不做局部合并；临时内容另走下一行） |
| `message_ephemeral_changed` | `chat_id` `message_id` + 新的临时内容节点，或空（临时内容消失）（`updateMessageEphemeralContent` `td_api.tl:11223`）。有临时内容时显示它、不显示原内容（`content-tree.md` 第三节第 2 小节） |
| `message_edited` | `chat_id` `message_id` + 编辑时间（`updateMessageEdited` `:11230`）。**字段更新里编辑时间只认它**；整条消息（新消息、历史）自带的 `edit_date` 照收；**0 = 没有编辑时间**（拍板 006 乙8） |
| `chat_action` | `chat_id` + 话题 + 发送人 + 动作种类（`updateChatAction` `td_api.tl:11530`；第一阶段只认「正在输入」`chatActionTyping` `:6960` 与「停了」`chatActionCancel` `:7002`，其余落 `other{td_type}`）。**以 `(chat_id, 话题, 发送人)` 为键**；过期靠 TDLib 推的取消（5.5 秒没续、或对方发出消息，TDLib 自己推 `chatActionCancel`），本地不计时（调研 `docs/research/003_send-receive-prior-art/README.md` 第一节的条目「收5」）。播不播由 `notify` 判（F6：只在当前打开的私聊、停手后播一次「对方正在输入」；N5：停手 = 人自己停下，不在朗读、不在按键） |
| `messages_deleted` | ⚠️ 自己的**待发消息**的临时 id 出现在这里，是 `send` 的第三种结尾（TDLib 注明有些待发消息会被不可恢复地删掉、这时不来失败，`td_api.tl:11210`）：**不等于没发出**（发送途中被删，服务器的成功应答照样会回来，`MessagesManager.cpp:27848-27851`），按「结果未知」报（003 发3）。`chat_id` + `message_ids` + `is_permanent` + `from_cache`（`updateDeleteMessages`，`td_api.tl:11523`）。**只有 `is_permanent` 为真且 `from_cache` 为假才是撤回**、才留「已撤回」占位；`from_cache`（只是清缓存）和非永久（暂时不可访问）都不许显示成撤回。另两条（拍板 006 乙7）：**待发送 / 发送失败的消息被删不是撤回**（自己取消，或 TDLib 重发时先删掉失败那条，`MessagesManager.cpp:25221-25223`）；**没见过原消息的删除待应答判定**——先留占位，后到的历史应答说它没发出去就拿掉，说它已发出就补记发送人和时间 |
| `send_succeeded` | `chat_id` + 临时 id + **发出后的整条消息**（含最终 `message_id`；`updateMessageSendSucceeded` `:11208` 就带整条，内容可能被服务器改过）。换 id **原地改**：store 里同一条消息换身份、索引重键，稳定键不变，焦点不丢（003 发5，Unigram 同法） |
| `send_failed` | `chat_id` + 临时 id + 失败后的整条消息（TDLib 也给它新 id，`:11214`）+ 错误（`errors.md` 的 `code`）+ 能否 `resend` |
| `chat_added` | 会话节点 |
| `chat_position_changed` · `chat_last_message_changed` · `chat_read_inbox` · `chat_title_changed` | `chat_id` + 变了的字段。`chat_last_message_changed` 带整条最后一条消息（可空）和全部位置；最后一条未知期间新消息可以只随它到达、不带 `new_message`（`:11312`） |
| `chat_draft_changed` | `chat_id` + 全部位置（`updateChatDraftMessage` `:11349`；TDLib 可能用它代替 `updateChatPosition`）。草稿本身不带——第一阶段不显示草稿，它是内容 |
| `chat_mention_count_changed` | `chat_id` + 未读提及数。来源两个：`updateChatUnreadMentionCount` `:11384`，和 `updateMessageMentionRead` `:11250`（消息已通知过客户端时 TDLib 只发后者） |
| `chat_read_outbox` | `chat_id` + 对方已读到的消息 id（`updateChatReadOutbox` `:11334`；「已读」状态要它） |
| `message_interaction_changed` | `chat_id` `message_id` + 回应、浏览数、回复数（`updateMessageInteractionInfo` `:11239`） |
| `message_unread_reactions` | `chat_id` `message_id` + 未读回应（`updateMessageUnreadReactions` `:11257`；会话的未读回应数 `updateChatUnreadReactionCount` `:11387` 并入 `chat_read_inbox` 一类，写明字段） |
| `message_content_opened` | `chat_id` `message_id`（`updateMessageContentOpened` `:11244`：已听 / 已看）。只改原内容，临时内容不动（TDLib 只改 `m->content`，`MessagesManager.cpp:9300-9312`） |
| `message_pinned_changed` | `chat_id` `message_id` + 是否置顶（`updateMessageIsPinned` `:11233`） |
| `chat_notification_changed` | `chat_id` + 会话自己的通知设置：**静音**（`mute_for`）、**提及通知**（`disable_mention_notifications`）、**置顶通知**（`disable_pinned_message_notifications`）三项，各带「跟随上一层」开关（`updateChatNotificationSettings` `:11367`，`chatNotificationSettings` `:3811`） |
| `scope_notification_changed` | 范围（私聊 / 群 / 频道）+ 这一档的静音、提及通知、置顶通知三项（`updateScopeNotificationSettings` `:11489`，`scopeNotificationSettings` `:3823`） |
| `forum_topic_changed` | `chat_id` + 论坛话题 id + 话题的「已读到」位置 + 话题自己的通知设置（也是静音 / 提及 / 置顶三项，形状同会话那份；`updateForumTopic` `:11484`）。**有效值**（静音、提及、置顶各自）按「话题 → 会话 → 私聊 / 群 / 频道三档」逐层取：哪层设成「跟随上一层」就往上找（同一份调研的「收1」；`002_roadmap.md` M2.3）。**话题静音按绝对到期时间判**：TDLib 话题静音到期不再推更新（TDLib 1.8.69 源码 `resources/td/td/telegram/ForumTopicManager.cpp:796` 的 TODO），所以收线程给它记下收到的时刻，store 拿它把剩余秒数换成绝对到期时刻存下（store 自己不碰时钟；常驻执行方上中途接入的订阅者也从 store 拿得到）；会话与范围的到期 TDLib 自己推，按剩余秒数 > 0 判（同上「收2」） |
| `user_changed` | 用户节点（含是否已注销：`user.type` 为 `userTypeDeleted`） |
| `my_id_changed` | `user_id`：自己的用户 id（`updateOption` 的 `my_id`，登录后 TDLib 自己推，`AuthManager.cpp:348`；0 = 没有）。投影据此认出收藏夹、把系统消息里的自己读成「我」（`content-tree.md` 第三节第 4、6 小节） |
| `file_progress` | `file_id` + 已下 / 总字节 |
| `auth_step_changed` | `AuthStep`（不带提示语，理由同 `auth_state`） |
| `net_state_changed` · `core_state_changed` | 新值 |
| `user_status_changed` | 合成后的 `UserStatus`（`errors.md` 第一节第 3 小节），读的一方不必自己合成 |
| `sync_done` | — （启动或重连补发结束）。由收线程合成：连接线变到 `ready` 时紧跟 `net_state_changed` 推一条（定义见 `cli.md`「什么时候算数据齐了」：#18 暂定，#53 起由收线程合成） |
| `coalesced` | `dropped`：合并掉的中间态条数（**v0 没做**：`watch` 不合并，不出这个事件；`cli.md` 第一节第 5 小节） |
| `unknown_update` | 只有 `td_type`，不带内容（与 `tree` 的「未知」节点同理） |

每个事件带 `seq`。

**不是事件、但也喂给 store 的应答**：历史应答（`history`，带请求凭据）、重发应答 `resend` 逐条的新旧对应（store 里叫 `Input::Resent{old, new}`：新消息沿用旧的稳定键）。它们走请求—应答，不进 `subscribe`。

## 二、为什么

1. **意图级、少而稳**：架构方案定了不照抄 TDLib 几百个函数；CLI 和 GUI 用同一组动作，AI 学一套就够（AI 友好①）
2. **查询和动作分开**：Store 查询不联网、不会失败、能同步调用，GUI 按键响应快（速度很快）；假后端也不用实现它们
3. **每个动作必标对外等级**：对外动作（L2）一律过闸——验目标与摘要、一次性、受限与限流照管、留痕——把它变成表里一列，加新动作时漏不了。（依据 intent「发消息等对外动作一律过闸」；直发不确认是人定 2026-11-16 #113，intent 经 #133 同步）
4. **凭据绑「发给谁 + 发什么」且一次性**：防止确认 A 却发出 B、确认一次发多次
5. **不做终端确认**：`/dev/tty` 挡得住管道，挡不住 `tmux send-keys` 和伪终端，做了也是假确认。正式服务器一律直发（人定 2026-11-16，#113：Touch ID 删除；有留痕、有重复拦截）
7. **L2 不自动重试**：重试一次就可能是对方收到两条
6. **收藏夹按自己的用户 id 判断**：别人可以把会话改名叫「收藏夹」

## 三、正例 / 反例

| | 例子 |
|---|---|
| ✅ | `send_text_prepare(收藏夹, "测试")` → 测试服务器上闸自动开 `test_exempt` 凭据 → `send_text` 成功，收到 `send_succeeded` |
| ✅ | 正式服务器发给收藏夹：CLI 打出计划 → 直接交出，不弹框（`confirm:"direct_mode"`） |
| ✅ | 人用 `atg-cli send 777 --text 好的` → 不确认、直接发；`direct-send.log` 多两行（`submitted` 与结果，只有 id 和类型） |
| ✅ | 测试档案上发给别的会话 → 仍拒（`test_dc_saved_only`） |
| ❌ | `send_text(plan)` 不带凭据也能发 |
| ❌ | 确认时内容是「好的」，之后改成「好的！」用同一张凭据发出 |
| ✅ | GUI 进会话 → `chat_open` → 显式 `mark_read(会话, 最新一条)`（F1，同官方客户端） |
| ✅ | 删除一条给所有人 → 不确认、直接删（D1）；`direct-send.log` 多两行（`submitted` 与结果） |
| ❌ | `chat_open` 自己顺手调 `viewMessages`（标已读必须是显式的 `mark_read`） |
| ❌ | `atg-cli history` 把读到的消息标成已读 |
| ❌ | `Backend` 上出现 `get_chat_history_raw(td_json)` 这种 TDLib 透传口子 |
| ❌ | 用会话标题等于「Saved Messages」来判断收藏夹 |
| ❌ | AI 在 tmux 里跑 `atg-cli send …`，再 `tmux send-keys y` 把确认按掉（不做终端确认，没有可按的；闸开不开凭据与终端按键无关） |
| ❌ | 加环境变量、配置文件或启动参数把发送改回「要确认」或关掉闸的留痕、防重复（直发是唯一方式，没有开关；第 5 小节） |
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
（已废止：#113 起正式服务器一律直发，没有 Touch ID；下面是当时的原文）默认模式保持最严（正式服务器一律 Touch ID）；嫌确认烦，用 `--unsafe-direct-send` 启动。原来的放宽选项不再需要。

**D1. 直发模式管不管「改 / 删 / 回应」** —— ✅ 人选 B：全部免确认
（已废止：#113 起一律直发，含表情回应；下面是当时的原文）人原话是「直接发」。本文暂定只管「发出」类（发送、重发，以后的发文件、转发）；编辑、删除（双方）、表情回应照默认规则要 Touch ID。
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
- 2026-10-25 按 b7 拍板（`docs/plan/006_decisions-b7.md`）同步（model，大总管授权）：第一节第 7 小节——写明 `CoreEvent` 即本表（乙6）；补 `message_ephemeral_changed`、`chat_draft_changed`、`chat_mention_count_changed` 三行；`send_succeeded` / `send_failed` 改为带整条消息；`messages_deleted` 补「未发出的不是撤回、没见过的待应答判定」（乙7）；`message_edited` 补编辑时间来源与 0（乙8）；`message_content_opened` 补只改原内容；`chat_last_message_changed` 补「新消息可只随它到达」；末尾补「喂给 store 的应答」
- 2026-10-26 issue #19 speech（大总管授权 2026-10-26）：第一节第 7 小节加 `my_id_changed`；`user_changed` 注明含是否已注销
- 2026-10-25 issue #17 登录（login，大总管授权，人定「全按建议，把现状写进契约」）：`auth_submit_phone` 加 `auth_phone_not_for_profile` 与可收的步骤；登录输入用 `Secret`；提交后按下一条授权更新判断；测试档案自动注册、名字写死 `Atg Test`
- 2026-10-26 issue #21 history（大总管授权 2026-10-26，属技术细节）：`history` 动作参数改成与 `getChatHistory` 同形（加 `offset`，往新翻页要用）；F2 落点与循环补满归 CLI
- 2026-10-26 issue #21 自审后（大总管授权 2026-10-26，属技术细节）：`chat_get` 返回 `Ack` 并登记 `createPrivateChat(force=false)` 后备；`message_links` 登记先 `getMessage` 与 `hidden`；`history` 写明含不含起点
- 2026-10-26 issue #21 第三轮自审后（大总管授权 2026-10-26，属技术细节）：`message_links` 标明是组合、不是 `Backend` 动作；`history` 登记本地参数检查的 `invalid_input`
- 2026-10-26 issue #52 自审第 2 轮后（send，属技术细节，大总管授权；B17-2 的「到期」参数，已报大总管）：结果未知的防重复记录留 24 小时
- 2026-10-26 issue #52 自审第 1 轮后（send，属技术细节，大总管授权）：`resend_prepare` 返回一份 `ResendPlan`、一张凭据（与 `cli.md` 一致），写明凭据绑什么
- 2026-10-26 issue #52 发送（send，按 003 大总管采纳 2026-10-26）：`send_text_prepare` / `send_text` 写明 `clear_draft=false`、`sending_id`、两次预览比对与发出后复核（003 建议第 2 条）、等结果 30 秒上限与晚到不丢（第 3 条）；`send_text` 与 `messages_deleted` 写明「临时 id 被删」是第三种结尾、按结果未知报（第 1 条）；「待发出的消息」里 `pending_cancel` 改称尽力撤销（第 8 条）；第 5 小节加「重复提交拦截」（B17-2 人选 A，机制经大总管同意，第 7 条）；`direct-send.log` 写明何时写、结果取值；`invalid_input` 带 `why`、`send_refused` 加 `reply_dropped` / `topic_changed` / `duplicate_recent`
- 2026-10-26 issue #21 第四轮自审后（大总管授权 2026-10-26，属技术细节）：`message_links` 的说法改清楚（什么叫「不是 `Backend` 动作」、GUI 照同样两步）
- 2026-10-26 issue #52 自审第 3 轮后（send，属技术细节，大总管授权）：第 4 条写明转义范围（自审第 3 轮 P1：标签字符等能把隐藏内容带过人的确认）
- 2026-10-26 issue #52 自审第 8 轮后（send，属技术细节，大总管授权）：确认框上限收紧到 120 字 / 4 行直到实测；直发条数进 `status`
- 2026-10-26 issue #52 自审第 7 轮后（send，属技术细节，大总管授权）：确认框放不下按整段说明的字数与行数判（会话名、重发条数也算），消息 id 读作「id N」
- 2026-10-26 issue #52 自审第 6 轮后（send，属技术细节，大总管授权）：`resend` 的确认给人看每条正文、凭据绑正文
- 2026-10-26 issue #52 自审第 5 轮后（send，大总管授权；默认取法列进待人定）：Touch ID 确认框放不下全文就拒（`too_long_to_confirm`）
- 2026-10-26 issue #52 自审第 4 轮后（send，属技术细节，大总管授权）：第 4 条加 Touch ID 弹框的显示规则（自审第 4 轮 P1）
- 2026-10-26 issue #53 watch（按 003 大总管采纳 2026-10-26，建议第 5、6 条）：第一节第 7 小节——`chat_action` 写明键为 `(chat_id, 话题, 发送人)`、过期靠 TDLib 推取消；`chat_notification_changed` / `scope_notification_changed` 加提及通知、置顶通知两项；新增 `forum_topic_changed`（话题通知设置与已读位置），有效值按「话题 → 会话 → 三档」取、话题静音按绝对到期时间判；`sync_done` 写明由收线程在连接线到 `ready` 时合成；`coalesced` 注明 v0 没做；动作表加 `forum_topic_get`（`getForumTopic`，L0 只读：话题没加载过时 TDLib 不推它的设置）
- 2026-11-13 #4 M2.5（手下 media，大总管定、arch 定 2026-11-13）：`file_download` 参数改 `FileRef`、同步等下完、只在下完时交路径，错误加 `download_incomplete`
- 2026-11-13 issue #52 自审第 9 轮后（属技术细节）：确认框「放得下」改按视觉行判（120 字 / 6 个视觉行 / 每行 40 列，仍待在真弹框上实测，`localauth` 的刻度尺测试）
- 2026-11-13 issue #52 自审第 10 轮后（属技术细节）：视觉行改为除空格外一律 2 列、按词折行（比例字体与长词会让「半角 1 列、任意处断」少算）
- 2026-11-13 issue #52 自审第 11 轮后（属技术细节）：确认框里的制表符换成 ⇥，残留制表符一律算放不下（制表符宽度估不准，能把正文挤出框）；「按词折行」刻度尺量不到，暂按保守假设
- 2026-11-13 issue #52 自审第 12 轮后（属技术细节）：确认框字符宽度改白名单，白名单外的字按一整行算；`resend` 的确认框读作「id N 那条」（与代码一致，第 7 轮已改代码、此处漏改）
- 2026-11-13 issue #52 codex 016 第 1 轮后（属技术细节）：防重复记录读不了就不写、拒直发；时钟回拨后比现在还晚的记录照样拦；`direct-send.log` 交出前先写 `submitted` 一行；交出本身计入 30 秒上限；`resend` 确认框与摘要带每条的回复对象
- 2026-11-13 issue #52 codex 016 第 2 轮后（属技术细节）：确认框估宽时每个空格（行首、连续的也算）占 1 列；`resend` 交出前重取每条失败态、正文、回复关系与计划比对，变了拒 `content_mismatch`
- 2026-11-13 issue #52 codex 016 第 3 轮后（属技术细节）：长词逐字排（宽字形不拆）；等结果超时后才到的 `account_limited` 失败由收线程点亮闸的信号，下一次开凭据即拒并落盘（`errors.md`「收到后停掉全部 L2」原有要求的实现）；文字计划行带回复对象与话题
- 2026-11-13 issue #52 codex 016 第 4 轮后（属技术细节）：发送前查限流截止（`rate_limited`）与账号受限，两者收到即落盘；`Gated` 只许核心内的计划实现
- 2026-11-13 issue #52 codex 016 第 5 轮后（属技术细节）：交出前（所有等待之后）再查一次受限与限流；限制标记读不准就拒发
- 2026-11-13 issue #52 codex 016 第 7 轮后（属技术细节）：确认框估行数时连字符、斜杠、标点等断行机会也算（两种断法取多的）；估算挪到 `atg-core::confirm_box`
- 2026-11-14 #59 3c（手下 media，按 exp008 设计）：`speech_to_text` 改成本机转写（取代 F4 的 `recognizeSpeech`），L0 不变
- 2026-11-14 #57 pending（msg1，大总管定 Q1 / Q2）：「待发出的消息」写明续发时机与 id / 时间重编、`pending_cancel` 按 L2 过闸、`not_pending`、撤没撤下怎么判；动作表加 `pending_cancel_prepare` / `pending_cancel` 两行；自审第 2–4 轮：`pending_cancel` 交出用 `revoke=false`（`get_persistent_message_id` 换 id）、写明撤销 / 撤回词义、`delete_prepare` 返回改 `DeletePlan` 并加 `DeletePlan` 类型行、加 Store `pending_sends` 行、`PendingMessage` 用 `temp_message_id`；`PendingMessage` 的「确认时间」改为 `date`
- 2026-11-16 issue #111 / #113（人定）：先改成默认直发，再把 Touch ID / 系统密码确认整个删除——`human_auth`、确认框估算与 `too_long_to_confirm`、`auth_failed`、`not_interactive` 全部去掉；直发为唯一方式
- 2026-11-16 issue #115（codex 047 补审，只改文字）：删 Touch ID 后的文字残留——第一节第 4 小节理由段改成「不做终端确认、一律直发」；第 5 小节「启动时提示」改为不提示（GUI 也不加「直发模式」、不播报）；验收表去掉 `--unsafe-direct-send` 前提，留痕按 `submitted` + 结果两行；第 4 小节小标题改「`plan` 给人看什么」、验收表 tmux 反例括注改写
- 2026-11-14 #9 表情回应（手下 msg2，B18-2 人定）：动作表加「表情回应」一组（`react_prepare` · `react` · `added_reactions`），第二批留位去掉 `react`；凭据开法加 `unconfirmed`（人定不确认的动作种类，照样留痕）
- 2026-11-14 #9 自审第 1 轮后（手下 msg2，属技术细节）：加回应会挤掉我原先点的别的回应时默认拒（`reaction_would_replace`），`replace` 放行并在计划里列出；第 5 小节写明 `react` 不进重复提交拦截；第 4 小节理由段补 `unconfirmed` 例外；第 6 小节加 `ReactPlan`、`AddedReactions`；第 4 小节第 7 条清单补三个交出函数
- 2026-11-14 #9 自审第 2 轮后（手下 msg2）：表情回应的留痕另记 `target`（哪条消息、挤掉了几个）；`replaces` 统一说「可能」
- 2026-11-14 #9 自审第 3 轮后（手下 msg2，属技术细节）：`react_prepare` 加 `reaction_state_unknown`（缓存可能旧时拒）
- 2026-11-16 #9 合 main 后（手下 msg2；第 4 轮 P1 那道待定题人定 2026-11-16 选 A——「普通群也向服务器取名单，拿不准就拒、要 replace」；另两项是 B「拿不准时改走 Touch ID」随 #113 作废、C「不拦只留痕」；大总管转达，题面在 `state/coord.md` 待定清单）：`unconfirmed` 开法随 Touch ID 删除去掉，`react` 与其他 L2 一样开 `direct_mode`（不进重复提交拦截）；拿不准我已点了哪些：只有私聊、加密聊天信本机（TDLib 只给它们推回应变化），普通群、超级群、频道都向服务器取名单；`replace` 放行拿不准的时计划带 `replaces_known:false`，留痕 `reaction.may_replace` 记 `null`（留痕字段由 `target{message_id, replaced}` 改名 `reaction{message_id, may_replace}`：plan 行的 `target` 是会话，不能同名；`replaced` 读着像实际挤掉的数）
- 2026-11-16 #9 codex 032 第 1 轮后（手下 msg2）：`react` 交出前复核会不会挤掉（#1，P1）、同一条消息串行；第一节第 4 小节理由段与第四节 A4、D1 标明 #113 后已废止（#2）
- 2026-11-16 #9 codex 032 第 2 轮后（手下 msg2）：`react` 交出前复核看当前连接线、`replace` 也刷新留痕、到点放弃的那条消息之后按拿不准拒
- 2026-11-17 #9 codex 032 第 3、4 轮后（手下 msg2）：`react` 放弃了的请求槽位一直留着、断线按结果未知也留；有放弃了的请求时 `replace` 放行但留痕记 `null`；第二节第 3 条依据改为现行的「过闸」，intent 原句与 #113 冲突待人改
- 2026-11-17 #9 codex 032 第 5 轮后（手下 msg2）：第二节第 3 条依据随 #133（intent 改为一律过闸、直发）去掉「待人改」括注；验收表「只认启动参数」改为「没有开关」
- 2026-11-17 #10 转发（手下 msg2；人定前按默认：不做复制、一次最多 20 条、留痕记来源会话）：动作表加「转发」一组（`forward_prepare` · `forward`），第二批留位去掉 `forward`；第 5 小节写明 `forward` 进重复提交拦截、留痕另记 `from_chat_id`；第 6 小节加 `ForwardPlan`
