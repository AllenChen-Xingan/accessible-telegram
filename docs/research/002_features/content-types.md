# 内容类型清单（给 `tree` 的输入）

> 2026-10-25 · 手下 api 起草 · v0.2（已过三视角自审与 codex 评审 003；批次与 `decision.md` 对齐）
> 行号全部指 `resources/td/td/generate/scheme/td_api.tl`（TDLib **1.8.69**，`resources/td` 提交 `ea3f69c`）。下文简写为 `tl:行号`。
> 用途：内容树（AST）节点种类要覆盖到哪。**批次**一栏对应 `decision.md` 的切分：**M** = MVP 必须有专门节点 · **2** = 第二批 · **泛** = 先落到「泛化系统消息」或「泛化附件」节点（读出类型名 + 能拿到的一句话）· **未** = 落「未知」节点。
> 原则：**一个都不许崩**。没有专门节点的子类，至少要能读出「这是什么类型」——`messageUnsupported`（`tl:6279`）和 tdlib-rs 解析不了的类型一样落「未知」。

## 一、消息外壳 `message`（`tl:3598`，字段说明 `tl:3550-3597`）

树里「消息头 / 尾」要放的字段。

| 字段 | 类型（行号） | 批次 | 读屏要点 |
|---|---|---|---|
| `id` + `chat_id` | `tl:3598` | M | 身份 = `(chat_id, id)`；id 只在会话内唯一 |
| `sender_id` | `MessageSender`：`messageSenderUser` `tl:3249` · `messageSenderChat` `tl:3252` | M | 发送人：用户 or 以频道/群身份发；名字渲染时查缓存 |
| `is_outgoing` | `tl:3598` | M | 「我」 |
| `date` · `edit_date` | `tl:3598` | M | 时间；`edit_date≠0` → 「已编辑」 |
| `sending_state` | `messageSendingStatePending` `tl:3455` · `messageSendingStateFailed` `tl:3465` | M | 发送中 / 发送失败（失败原因在 `error`） |
| `reply_to` | `messageReplyToMessage` `tl:3505`（含引用 `quote`、跨会话回复）· `messageReplyToStory` `tl:3508` | M（消息）/ 泛（故事） | 「回复 X：……」；被回复的那条可能要另取 |
| `forward_info` | `messageForwardInfo` `tl:3375`；`origin` = `messageOriginUser` `tl:3305` · `messageOriginHiddenUser` `tl:3308` · `messageOriginChat` `tl:3313` · `messageOriginChannel` `tl:3319` | M | 「转发自 X」；隐藏用户只有名字字符串 |
| `interaction_info` | `messageInteractionInfo` `tl:3410`：浏览数、转发数、`reply_info`（`messageReplyInfo` `tl:3388`，评论数）、`reactions`（`messageReactions` `tl:3403` → `messageReaction` `tl:3396`） | M（表情回应）/ 2（浏览数、评论数） | T06「表情回应」 |
| 表情回应类型 | `reactionTypeEmoji` `tl:3335` · `reactionTypeCustomEmoji` `tl:3338` · `reactionTypePaid` `tl:3341` | M（emoji）/ 泛（自定义、付费） | 自定义表情只有 id，读「自定义表情」 |
| `unread_reactions` | `unreadReaction` `tl:3416` | 2 | 别人对我消息的新回应 |
| `contains_unread_mention` | `tl:3598` | 2 | 「提到你」 |
| `is_pinned` | `tl:3598` | 2 | 「已置顶」 |
| `reply_markup` | `replyMarkupInlineKeyboard` `tl:4314`（按钮 `inlineKeyboardButtonType*` `tl:4230-4266`）· `replyMarkupShowKeyboard` `tl:4309` · `replyMarkupRemoveKeyboard` `tl:4294` · `replyMarkupForceReply` `tl:4299` | 2（读出与按下） | T06「按钮」 |
| `media_album_id` | `tl:3598` | 2 | 相册：多条消息合成一组，读「相册，N 张」 |
| `self_destruct_type` | `messageSelfDestructTypeTimer` `tl:6514` · `…Immediately` `tl:6517` | 2 | 阅后即焚 |
| `topic_id` | `MessageTopic` `tl:3598` | M（带上话题名，发送时要用）/ 2（话题列表导航） | 论坛话题群里不带它可能发错地方 |
| `via_bot_user_id` · `author_signature` | `tl:3598` | 2 | 「经由 @bot」「署名」 |
| `ephemeral_content` | `ephemeralMessageContent` `tl:3547`（字段说明 `tl:3594`：存在时**必须替代**普通内容显示）；更新 `updateMessageEphemeralContent` `tl:11223` | M | **建树前先选有效内容**：有 `ephemeral_content` 就用它的 `content` 和 `reply_markup`，否则用 `content`。忽略它会读错正文（codex 评审 003 P1） |
| `restriction_info` | `restrictionInfo` `tl:2738`（`restriction_reason`、`has_sensitive_content`；字段说明 `tl:3591`：**必须应用于**消息内容） | M | 有限制时内容节点换成「受限」节点，读出原因（如「此内容在你所在地区不可用」「敏感内容」），不把受限内容当普通消息读（codex 评审 003 P1） |
| `fact_check` · `effect_id` · `paid_message_star_count` · `suggested_post_info` | `tl:3598` | 未（先不建节点，字段忽略） | 以后再说 |

## 二、文本与实体 `formattedText`（`tl:122`）· `textEntity`（`tl:113`）

**嵌套规则原文在 `tl:118-121`**：实体可嵌套但**不许交叉**；`Pre` / `Code` / `PreCode` / `DateTime` 里不能再有实体；`BlockQuote` 不能套 `BlockQuote`；`Bold` / `Italic` / `Underline` / `Strikethrough` / `Spoiler` 可以包也可以被包；其余实体互不包含。偏移与长度都是 **UTF-16 码元**（`tl:110-111`）。

| 实体（`TextEntityType`） | 行号 | 批次 | 读屏要点 |
|---|---|---|---|
| `Mention`（@用户名） | `tl:6309` | M | 读原文即可 |
| `MentionName`（无用户名的提及，带 user_id） | `tl:6369` | M | 文本是名字；可跳转 |
| `Hashtag` · `Cashtag` | `tl:6312` · `tl:6315` | M | 读原文 |
| `BotCommand` | `tl:6318` | 2 | |
| `Url` · `TextUrl`（文字链接，带隐藏 url） | `tl:6321` · `tl:6366` | M | **TextUrl 的 url 不在文本里**——详情层要能读出 |
| `EmailAddress` · `PhoneNumber` · `BankCardNumber` | `tl:6324` · `tl:6327` · `tl:6330` | M（前两个）/ 2 | |
| `TonAddress` | `tl:6333` | 泛 | |
| `Bold` · `Italic` · `Underline` · `Strikethrough` | `tl:6336-6345` | M | 简短读法默认不读格式；节点要有 |
| `Spoiler`（剧透） | `tl:6348` | M | **默认不读内容**，读「隐藏内容，按 D 展开」（R1 人定，`specs/content-tree.md` 第九节） |
| `Code` · `Pre` · `PreCode`（带语言） | `tl:6351` · `tl:6354` · `tl:6357` | M | 代码块要能整段读 |
| `BlockQuote` · `ExpandableBlockQuote` | `tl:6360` · `tl:6363` | M | 「引用：……」 |
| `CustomEmoji`（文本里是替代 emoji） | `tl:6373` | M | 读替代 emoji 字符 |
| `MediaTimestamp`（跳到媒体某秒） | `tl:6378` | 2 | |
| `DateTime`（本地化时间） | `tl:6383` | 2 | |

## 三、消息内容 `MessageContent`（`tl:5689-6279`）

### 3.1 普通内容

| 子类 | 行号 | 批次 | 关键字段 / 读屏要点 |
|---|---|---|---|
| `messageText` | `tl:5689` | M | `text` + `link_preview`（收到的链接预览读标题 / 站点名：2；发送前取预览：以后） |
| `messagePhoto` | `tl:5722` | M | `caption`、`has_spoiler`、`is_secret`；无替代文本，读「图片」+ 说明文字 |
| `messageVideo` | `tl:5737` | M | 时长、`caption` |
| `messageVoiceNote` | `tl:5749` | M | 时长、`is_listened`、`caption`；语音转文字见 inventory |
| `messageVideoNote`（圆视频） | `tl:5743` | M | 时长、`is_viewed` |
| `messageDocument` | `tl:5706` | M | 文件名、大小（树里是 `document.size`，字节；TDLib 给不出时为 0，`specs/content-tree.md` 第三节第 3 小节）、`caption` |
| `messageAudio` | `tl:5703` | M | 标题、演唱者、时长 |
| `messageAnimation`（GIF） | `tl:5700` | M | `caption` |
| `messageSticker` | `tl:5725` | M | 贴纸对应的 emoji |
| `messageAnimatedEmoji` | `tl:5778` | M | 单个 emoji 消息：读 emoji |
| `messageLocation` · `messageVenue` · `messageLiveLocation` | `tl:5769` · `tl:5772` · `tl:5766` | 2 | 「位置」/ 地点名 / 「实时位置，剩 N 分钟」 |
| `messageContact` | `tl:5775` | 2 | 名字 + 电话 |
| `messagePoll` | `tl:5796` | 2 | 问题、选项、是否已投；投票见 inventory |
| `messageChecklist` | `tl:5813` | 2 | 清单 |
| `messageDice` · `messageStakeDice` | `tl:5786` · `tl:5804` | 泛 | 「骰子，点数 N」 |
| `messageCall` · `messageGroupCall` | `tl:5832` · `tl:5843` | 2 | 「语音通话，3 分钟 / 未接」——只读记录，不做通话 |
| `messageExpiredPhoto` / `Video` / `VideoNote` / `VoiceNote` | `tl:5752-5761` | 2 | 「已过期的图片」 |
| `messageStory` | `tl:5810` | 泛 | 不做 Stories |
| `messagePaidMedia` | `tl:5713` | 泛 | |
| `messageGame` · `messageInvoice` | `tl:5789` · `tl:5825` | 泛 | 不做支付 / 机器人开发 |
| `messageRichMessage` | `tl:5692` | 泛 | 新类型，内容是块结构，先读出纯文本兜底 |
| `messageUnsupported` | `tl:6279` | 未 | 「此消息类型暂不支持」 |

### 3.2 系统消息（服务消息）

**MVP 只给常见的几种专门节点，其余统一落「泛化系统消息」节点**：读「系统消息：<类型名的中文说法>」+ 能拿到的主语（谁）。

| 子类 | 行号 | 批次 |
|---|---|---|
| `messageChatAddMembers` · `messageChatJoinByLink` · `messageChatJoinByRequest` · `messageChatDeleteMember` | `tl:5904` · `5907` · `5910` · `5916` | M |
| `messageChatChangeTitle` · `messageChatChangePhoto` · `messageChatDeletePhoto` | `tl:5880` · `5883` · `5886` | M |
| `messagePinMessage` | `tl:5931` | M |
| `messageBasicGroupChatCreate` · `messageSupergroupChatCreate` | `tl:5874` · `5877` | M |
| `messageContactRegistered` | `tl:6249` | M |
| `messageScreenshotTaken` | `tl:5934` | 2 |
| `messageChatSetMessageAutoDeleteTime` | `tl:5948` | 2 |
| `messageChatUpgradeTo` / `From` | `tl:5925` · `5928` | 2 |
| `messageForumTopicCreated` / `Edited` / `IsClosedToggled` / `IsHiddenToggled` | `tl:5957-5969` | 2 |
| `messageVideoChat*` · `messageInviteVideoChatParticipants` | `tl:5848-5859` | 2 |
| `messageGameScore` · `messageCustomServiceAction` | `tl:5985` · `5979` | 2 |
| `messageChatSetBackground` · `messageChatSetTheme` · `messageChatBoost` · `messageChatOwnerLeft/Changed` · `messageChatHasProtectedContent*` · `messageChatJoinFromCommunity` · `messageChatAddedToCommunity/RemovedFromCommunity` · `messagePollOption*` · `messageSuggestProfilePhoto` · `messageSuggestBirthdate` · `messageProximityAlertTriggered` | `tl:5865-5976`、`6276` | 泛 |
| **其余所有** `= MessageContent;` 子类（支付、礼物、星星、TON、抽奖、付费消息、建议帖、机器人相关、清单任务变化等，以 `grep -n "= MessageContent;"` 为准，不再列区间） | — | 泛 |

> 共 **108** 个 `MessageContent` 子类（`grep -c "= MessageContent;"`）：按上两表实数，**M 专门节点 21 个 · 第二批 26 个 · 泛化约 60 个 · 未知 1 个**。tree 只需保证**泛化节点带上原类型名**，读法层就能逐步加中文说法而不改树。

## 四、会话列表项 `chat`（`tl:4080`）

| 字段 | 行号 | 批次 | 读屏要点（T02「名字、未读数、最后一条摘要」） |
|---|---|---|---|
| `type` | `chatTypePrivate` `tl:3887` · `chatTypeBasicGroup` `tl:3890` · `chatTypeSupergroup` `tl:3893`（`is_channel` 区分频道）· `chatTypeSecret` `tl:3896` | M | 「私聊 / 群 / 频道 / 加密聊天」，简短读法里可省私聊 |
| `title` | `tl:4080` | M | |
| `unread_count` · `unread_mention_count` · `unread_reaction_count` · `is_marked_as_unread` | `tl:4080` | M（前两项）/ 2 | 「3 条未读，1 条提到你」 |
| `last_message` | `message`，`tl:4080` | M | 摘要 = 最后一条的 `brief` 去时间 |
| `positions` | `chatPosition` `tl:3996`（`is_pinned`、`order`、所在 `ChatList`：`chatListMain` `tl:3970` · `chatListArchive` `tl:3973` · `chatListFolder` `tl:3976`） | M | 「置顶」；排序 |
| `notification_settings` | `chatNotificationSettings` `tl:3811`；`use_default_mute_for` 为真时要**继承**范围设置（私聊 / 群 / 频道，`scopeNotificationSettings` `tl:3823`；论坛话题继承所在论坛，字段说明 `tl:3795`）——读 `getScopeNotificationSettings` `tl:14672`，跟 `updateScopeNotificationSettings` `tl:11489` | M | 「已静音」读的是**有效静音**（会话自身或继承来的），只看会话自身会误报（codex 评审 003 P1） |
| `draft_message` | `tl:4080` | 2 | 「草稿：……」 |
| 正在输入 | `updateChatAction` `tl:11530`；`chatActionTyping` 等 `tl:6960-7002` | 2 | 「X 正在输入」——播报策略另定，别打断 |
| 对方在线状态 | `userStatus*` `tl:7008-7026`；`updateUserStatus` `tl:11553` | 2 | 只在私聊详情里读 |
| `view_as_topics` / 论坛 | `tl:4080` | 2 | |

## 五、不是消息、也要进树的东西

| 东西 | 来源 | 批次 |
|---|---|---|
| **新消息提醒**（「N 条新消息，按 M 收听」） | 本项目 `notify` 策略产出，非 TDLib 类型 | M |
| **连接状态**（T10） | `updateConnectionState` `tl:11818`；`connectionStateWaitingForNetwork` `tl:10657` · `ConnectingToProxy` `10660` · `Connecting` `10663` · `Updating` `10666` · `Ready` `10669` | M |
| **登录步骤**（T01） | `updateAuthorizationState` `tl:11193`；`authorizationStateWaitPhoneNumber` `tl:210` · `WaitCode` `233` · `WaitPassword` `250` · `WaitEmailAddress` `222` · `WaitEmailCode` `229` · `WaitRegistration` `242` · `WaitOtherDeviceConfirmation` `237`（二维码）· `Ready` `253` · `LoggingOut` `256` · `Closing` `259` · `Closed` `263` · `WaitPremiumPurchase` `217` | M |
| **发送结果**（「已发送 / 发送失败」） | `updateMessageSendSucceeded` `tl:11208` · `updateMessageSendFailed` `tl:11214` | M |
| **撤回占位**（001 拍板 7：原位留「已撤回」；读法 R7「林小雨撤回了一条消息，19:36」） | `updateDeleteMessages` `tl:11523`；哪些算撤回见 `specs/contracts/actions.md` 事件 `messages_deleted` | M |
| **日期分隔**（「今天 / 昨天」） | 本项目按 `date` 算 | 2 |
| **未读分隔**（「以下为未读」；进会话落点也靠它，见 `decision.md` F2） | `chat.last_read_inbox_message_id` | M |
| **服务通知**（Telegram 官方弹窗） | `updateServiceNotification` `tl:11587` | 2 |

## 六、会改树的更新（store 要接的）

`updateNewMessage` `tl:11196` · `updateMessageContent` `11217` · `updateMessageEphemeralContent` `11223` · `updateMessageEdited` `11230` · `updateMessageInteractionInfo` `11239`（表情回应变化）· `updateMessageIsPinned` `11233` · `updateMessageContentOpened` `11244`（语音已听）· `updateMessageMentionRead` `11250` · `updateMessageUnreadReactions` `11257` · `updateMessageSendSucceeded/Failed` `11208/11214`（**临时 id 换正式 id**——身份会变，树的稳定 id 要处理）· `updateDeleteMessages` `11523` · `updateNewChat` `11290` · `updateChatTitle` `11293` · `updateChatLastMessage` `11314` · `updateChatPosition` `11319` · `updateChatReadInbox/Outbox` `11331/11334` · `updateChatNotificationSettings` `11367` · `updateScopeNotificationSettings` `11489`（有效静音要重算）· `updateChatDraftMessage` `11349` · `updateChatUnreadMentionCount` `11384` · `updateUser` `11556`（名字晚到）· `updateFile` `11596`（下载进度）· `updatePoll` `12059`。

## 七、给 tree 的几个提醒

1. **发送成功会换 id**：`updateMessageSendSucceeded` 带 `old_message_id`（`tl:11208`）。焦点锚点、元素 id 都要跟着换，否则「刚发的那条」焦点会丢
2. **`messageReplyToMessage` 可指向别的会话**（`chat_id` 字段，`tl:3505`），被回复的那条不一定在 store 里；它自带 `origin` 和 `content`（跨会话或已删除时 TDLib 给的副本）、`quote`（只引了一段），渲染时可先用这些兜底
3. **`TextUrl` 的链接地址不在文本里**，`MentionName` 的 user_id 也不在——节点必须带这些属性，否则详情层读不到
4. **相册是多条消息**（`media_album_id`），不是一条消息里的多张图——是建一个「相册」父节点还是渲染时合并，请 tree 定
5. **108 个内容子类会随 TDLib 升级增加**（TDLib 小版本升级常加新子类；tdlib-rs 落后时还会碰到它不认识的类型）：泛化节点带原类型名，是升级不崩的关键

## 修订记录

- 2026-10-25 v0.1 / v0.2（api），三视角自审与 codex 评审 003
- 2026-10-25 按 b2 拍板同步（sync）：剧透读法按 R1；`messageDocument` 对齐 tree 已加的 `size`；撤回占位判定与 R7 读法
