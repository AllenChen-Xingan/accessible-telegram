# 功能盘点

> 2026-10-25 · 手下 api 起草 · 行号指 `resources/td/td/generate/scheme/td_api.tl`（TDLib 1.8.69），简写 `tl:行号`
> 列：功能 · TDLib 函数 / 更新 · 读屏价值 · 难度（S 一两个函数无状态 / M 要维护状态或多步 / L 要大块界面或复杂交互）· 依赖 · 对应任务 · 批次建议（MVP / 二 / 以后 / 不做）
> 批次的最终切分与理由见 `decision.md`；本表的「批次建议」是盘点时的初判。

> 三组由子代理分头 grep 起草、api 合并；抽查 15 处行号全对。**组间有重叠**（如搜索、投票、置顶消息、`openChat` 在两组都出现），批次建议偶有不一致——**以 `decision.md` 的切分为准**。

## A · 登录与账号 · 会话列表与文件夹 · 通知与未读 · 联系人 · 设置与隐私


### ① 登录与账号

| 功能 | TDLib 函数 / 更新 | 读屏价值 | 难度 | 依赖 | 对应 T | 批次建议 |
|---|---|---|---|---|---|---|
| 登录状态机驱动 | `updateAuthorizationState` tl:11193 · `getAuthorizationState` tl:12167 · `setTdlibParameters` tl:12185 | 高：每一步都要朗读「现在要输入什么」 | M：状态机多分支 | 无 | T01 | MVP |
| 手机号 + 验证码登录 | `setAuthenticationPhoneNumber` tl:12192 · `checkAuthenticationCode` tl:12222 · `resendAuthenticationCode` tl:12215 | 高：最基本入口，纯文本框读屏友好 | S：几个调用 | 状态机 | T01 | MVP |
| 二步验证密码 | `checkAuthenticationPassword` tl:12261 · 找回（只在等密码这一步可用）：`requestAuthenticationPasswordRecovery` tl:12264 → `checkAuthenticationPasswordRecoveryCode` tl:12268 → `recoverAuthenticationPassword` tl:12274（codex 评审 003 改正：原写的 `requestPasswordRecovery` tl:12361 是登录后用的） | 高：开了两步验证的人不做就进不去 | S | 状态机 | T01 | MVP（找回走二） |
| 邮箱登录 / 邮箱验证码 | `setAuthenticationEmailAddress` tl:12210 · `checkAuthenticationEmailCode` tl:12219 | 中：服务器有时强制要邮箱 | S | 状态机 | T01 | MVP（状态机里出现就得接） |
| 二维码登录 | `requestQrCodeAuthentication` tl:12228 · `confirmQrCodeAuthentication` tl:12303 | 低：盲人扫码难；但「本机已登录去确认别的设备」有用 | M：要渲染二维码图 | 状态机 | T01 | 以后 |
| 注册新账号 | `registerUser` tl:12253 | 低：多数用户在手机已注册 | S | 状态机 | T01 | 二 |
| 登出 | `logOut` tl:12292 | 中：换号、排障必需 | S | 状态机 | T01 | MVP |
| 多账号 | 无专门函数：每账号一个 TDLib client + 独立 database_directory（`setTdlibParameters` tl:12185） | 中：一部分人有工作号 | M：多实例状态管理与切换 | 登录全套 | — | 二 |
| 活跃会话管理 | `getActiveSessions` tl:16190 · `terminateSession` tl:16193 · `terminateAllOtherSessions` tl:16196 · `setInactiveSessionTtl` tl:16213 | 中：安全相关，列表好读 | S | 登录 | — | 二 |
| 二步验证密码设置 / 登录邮箱 | `getPasswordState` tl:12316 · `setPassword` tl:12324 · `setLoginEmailAddress` tl:12333 | 低：官方手机端可做 | M：多步确认 | 登录 | — | 以后 |
| 删号 | `deleteAccount` tl:16818 · `setAccountTtl` tl:16809 | 低 | S | — | — | 不做（人定，且不可逆） |

### ② 会话列表与文件夹

| 功能 | TDLib 函数 / 更新 | 读屏价值 | 难度 | 依赖 | 对应 T | 批次建议 |
|---|---|---|---|---|---|---|
| 加载会话列表并按序维护 | `loadChats` tl:12502 · `updateNewChat` tl:11290 · `updateChatPosition` tl:11319 · `updateChatLastMessage` tl:11314 · `updateChatAddedToList` tl:11322 · `updateChatRemovedFromList` tl:11325 | 高：日常的主页面 | M：本地要按 position.order 维护有序表；`getChats` tl:12507 注明仅供参考 | 登录 | T02 | MVP |
| 打开 / 关闭会话（影响更新推送） | `openChat` tl:14188 · `closeChat` tl:14191 | 高：超级群和频道只有打开的会话才收全更新（tl:14186）；标已读不依赖它，关着也能用 `viewMessages` 的 `force_read`（tl:14198） | S | 会话列表 | T03 | MVP |
| 置顶 / 调置顶顺序 | `toggleChatIsPinned` tl:14691 · `setPinnedChats` tl:14694 | 中：常用会话放顶部，减少方向键次数 | S | 会话列表 | T02 | 二 |
| 归档 / 归档列表 | `addChatToList` tl:14334 + `chatListArchive` tl:3973 · `getArchiveChatListSettings` tl:14415 · `setArchiveChatListSettings` tl:14418 | 中：清掉噪音群对读屏很值 | S | 会话列表 | T02 | 二（只读归档列表可进 MVP） |
| 文件夹浏览 | `updateChatFolders` tl:11425 · `chatListFolder` tl:3976 · `getChatFolder` tl:14337 · `getChatFolderChatCount` tl:14357 | 中：已在手机建好文件夹的人能直接用 | M：多列表切换 | 会话列表 | T02 | 二 |
| 文件夹增删改排序 | `createChatFolder` tl:14341 · `editChatFolder` tl:14344 · `deleteChatFolder` tl:14349 · `reorderChatFolders` tl:14362 · `getRecommendedChatFolders` tl:14368 · `getChatListsToAddChat` tl:14329 | 低：可在手机做 | L：编辑界面复杂 | 文件夹浏览 | — | 以后 |
| 标记未读 | `toggleChatIsMarkedAsUnread` tl:14514 · `updateChatIsMarkedAsUnread` tl:11407 | 中：「稍后再读」的书签 | S | 会话列表 | T09 | 二 |
| 静音 / 取消静音 | `setChatNotificationSettings` tl:14493 · `updateChatNotificationSettings` tl:11367 | 高：吵群打断读屏，静音是刚需 | S | 会话列表 | T05 | MVP |
| 搜会话 | `searchChats` tl:12523 · `searchChatsOnServer` tl:12529 | 高：几百个会话方向键走不完 | S | 会话列表 | T08 | MVP |
| 论坛话题列表 | `getForumTopics` tl:13654 · `getForumTopic` tl:13632 · `getForumTopicHistory` tl:13642 · `updateForumTopic` tl:11484 · `updateForumTopicInfo` tl:11471 · `toggleChatViewAsTopics` tl:14508 | 中：话题群会把消息分散到各话题 | M：会话下再套一层列表 | 会话列表、消息读取 | T02/T03 | 二 |
| 话题置顶 / 话题通知 | `toggleForumTopicIsPinned` tl:13678 · `setPinnedForumTopics` tl:13683 · `setForumTopicNotificationSettings` tl:13660 | 低 | S | 论坛话题 | — | 以后 |
| 收藏夹 Saved Messages 话题 | `loadSavedMessagesTopics` tl:12681 · `updateSavedMessagesTopic` tl:11435 · `updateSavedMessagesTopicCount` tl:11438 · `getSavedMessagesTopicHistory` tl:12689 · `toggleSavedMessagesTopicIsPinned` tl:12708 · `setPinnedSavedMessagesTopics` tl:12711 | 中：收藏夹也是测试发信目标（CLAUDE.md 五） | M | 会话列表 | T03 | 二（收藏夹本身作为普通会话进 MVP） |

### ③ 通知与未读

| 功能 | TDLib 函数 / 更新 | 读屏价值 | 难度 | 依赖 | 对应 T | 批次建议 |
|---|---|---|---|---|---|---|
| 每会话未读数 | `updateChatReadInbox` tl:11331（含 unread_count） | 高：列表朗读的核心字段 | S：随会话对象维护 | 会话列表 | T02 | MVP |
| 总未读数（徽标 / 播报） | `updateUnreadMessageCount` tl:11710 · `updateUnreadChatCount` tl:11719 | 中：一句话知道「有多少待读」 | S | 会话列表 | T02/T05 | MVP |
| 标记已读（读到哪算哪） | `viewMessages` tl:14199 · `readChatList` tl:14697（整列表全读） | 高：逐条读完要同步已读 | S；`readChatList` 是批量不可逆 | openChat | T03 | MVP（`readChatList` 走二，需确认） |
| 对方已读回执（我的消息是否被读） | `updateChatReadOutbox` tl:11334 · `getMessageReadDate` tl:12473 · `getMessageViewers` tl:12479 | 中：发完想知道对方看没看 | S | 消息读取 | T06 | 二 |
| @提及未读 | `updateChatUnreadMentionCount` tl:11384 · `updateMessageMentionRead` tl:11250 · `readAllChatMentions` tl:14276 · `readAllForumTopicMentions` tl:13694 | 高：大群里只关心被点名的那条 | M：跳到提及要搜消息过滤器 | 会话列表、消息读取 | T09 | 二 |
| 表情回应未读 | `updateChatUnreadReactionCount` tl:11387 · `updateMessageUnreadReactions` tl:11257 · `readAllChatReactions` tl:14279 | 中 | M | 同上 | T06/T09 | 二 |
| 跳到下一个有未读的会话 | 未找到专门函数：客户端按本地有序列表 + unread_count / is_marked_as_unread 自算 | 高：盲人最省事的「下一条」 | S：纯本地计算 | 会话列表、未读数 | T09 | MVP |
| 系统通知（TDLib 通知 API） | `updateNotification` tl:11495 · `updateNotificationGroup` tl:11506 · `updateActiveNotifications` tl:11511 · `removeNotification` tl:12988 · `removeNotificationGroup` tl:12993 | 高：后台来消息要能被 VoiceOver 播报而不打断 | M：分组状态 + 对接 macOS 通知/播报节流 | 会话列表 | T05 | MVP（可先用 updateNewMessage 自做，见注意 1） |
| 全局通知设置（私聊/群/频道） | `getScopeNotificationSettings` tl:14672 · `updateScopeNotificationSettings` tl:11489 · `setScopeNotificationSettings` tl:14677 | 高：会话设置 `use_default_mute_for` 为真时要继承它（tl:3795），不读就会误判静音、播报已静音的消息 | S（读）/ M（编辑界面） | 会话列表 | T05 | **MVP（读取与有效静音计算）**/ 二（编辑）——codex 评审 003 改 |
| 通知例外列表 | `getChatNotificationSettingsExceptions` tl:14669 | 低 | S | 全局通知设置 | — | 以后 |

### ④ 联系人

| 功能 | TDLib 函数 / 更新 | 读屏价值 | 难度 | 依赖 | 对应 T | 批次建议 |
|---|---|---|---|---|---|---|
| 联系人列表 / 搜索 | `getContacts` tl:15550 · `searchContacts` tl:15555 · `getUserFullInfo` tl:12394 | 中：找人开私聊，平时多从会话列表进 | S | 登录 | T08 | 二 |
| 加 / 删联系人、按手机号找人 | `addContact` tl:15543 · `removeContacts` tl:15558 · `searchUserByPhoneNumber` tl:15614 · `sharePhoneNumber` tl:15618 | 低 | S；属对外动作，需确认 | 联系人列表 | — | 以后 |
| 导入通讯录 | `importContacts` tl:15547 · `changeImportedContacts` tl:15566 · `clearImportedContacts` tl:15569 · `getImportedContactCount` tl:15561 | 低：涉隐私 | M | — | — | 以后 |
| 屏蔽 / 解除屏蔽、屏蔽列表 | `setMessageSenderBlockList` tl:15522 · `getBlockedMessageSenders` tl:15535 | 中：防骚扰，屏蔽很需要 | S | 会话列表 | — | 二 |
| 亲密好友名单 | `setCloseFriends` tl:15572 | 低：主要服务 Stories | S | — | — | 不做（随 Stories） |

### ⑤ 设置与隐私

| 功能 | TDLib 函数 / 更新 | 读屏价值 | 难度 | 依赖 | 对应 T | 批次建议 |
|---|---|---|---|---|---|---|
| 当前用户信息 | `getMe` tl:12388 · `updateUser` tl:11556 | 中：确认登录的是哪个号 | S | 登录 | T01 | MVP |
| 在线状态 / 对方在线 | `setOption` tl:16805（"online" 选项）· `updateUserStatus` tl:11553 | 中：设 online 才会正常收推送且表明在线；对方状态可读给人听 | S | 登录 | T02 | MVP（设 online）/ 二（读对方） |
| 隐私规则 | `getUserPrivacySettingRules` tl:16763 · `setUserPrivacySettingRules` tl:16760 · 规则类型 tl:9641–9722 | 中 | M：规则组合 + 例外用户列表的界面 | 联系人 | — | 以后 |
| 已读时间 / 陌生人来信隐私 | `getReadDatePrivacySettings` tl:16769 · `setReadDatePrivacySettings` tl:16766 · `getNewChatPrivacySettings` tl:16775 · `setNewChatPrivacySettings` tl:16772 | 低 | S | — | — | 以后 |
| 改名 / 简介 / 用户名 | `setName` tl:15873 · `setBio` tl:15876 · `setUsername` tl:15880 · `setBirthdate` tl:15891 | 低：一次性设置 | S | 登录 | — | 二 |
| 改头像 / 删头像 | `setProfilePhoto` tl:15851 · `deleteProfilePhoto` tl:15854 | 低：选图对读屏不友好 | M：文件选择 | 文件上传 | — | 以后 |
| 界面语言（服务器语言包） | `getLocalizationTargetInfo` tl:16701 · `getLanguagePackInfo` tl:16705 · `synchronizeLanguagePack` tl:16715 · `getLanguagePackStrings` tl:16710 · `setOption` "language_pack_id" | 低：本客户端界面自有中文字符串 | M | — | — | 以后（先不接 Telegram 语言包） |
| 代理 | `addProxy` tl:17576 · `editProxy` tl:17583 · `enableProxy` tl:17586 · `disableProxy` tl:17589 · `removeProxy` tl:17592 · `getProxies` tl:17595 · `pingProxy` tl:17599 · `testProxy` tl:17687 | 高：大陆用户无代理连不上 | M：增改与测速界面 | 无 | T10 | MVP（至少能加一个 SOCKS5/MTProto 并启用） |
| 连接状态播报 | `updateConnectionState` tl:11818 · 五种状态 tl:10657–10669 | 高：断网 / 连接中要读出来 | S | 无 | T10 | MVP |
| 网络类型 / 流量统计 | `setNetworkType` tl:17157 · `getNetworkStatistics` tl:17160 | 低 | S | — | T10 | 以后 |
| 存储与缓存 | `getStorageStatisticsFast` tl:17136 · `getStorageStatistics` tl:17133 · `optimizeStorage` tl:17151 | 低：偶尔清理 | S；清理是破坏性动作需确认 | — | — | 二 |
| 自动下载设置 | `getAutoDownloadSettingsPresets` tl:17169 · `setAutoDownloadSettings` tl:17172 | 低 | S | 文件下载 | T07 | 以后 |

### 注意（A）

1. **通知 API 有开关**：updateNotification 系列需设选项 `notification_group_count_max` > 0 才会推（据官方 options 文档；td_api.tl 内未 grep 到该选项名）。MVP 也可直接用 updateNewMessage + 静音设置自做播报，二选一需人定。
2. **会话排序必须自维护**：`getChats` 注释明说「仅供参考」，正确做法是 loadChats + 处理 updateChatPosition 本地排序；这是 T02/T09 的地基。
3. **进出会话成对调用 openChat / closeChat**：超级群和频道只有打开的会话才收全更新（tl:14186）。已读不靠它：会话关着时 `viewMessages` 带 `force_read=true` 也能标（tl:14198；codex 评审 003 改准）。
4. **对外/不可逆动作需人确认**：terminateSession、readChatList、屏蔽、删联系人、optimizeStorage、deleteAccount。
5. **多账号**无 TL 函数，是多 client 实例的架构问题，需与架构方案 001 对齐后再定批次。

## B · 读消息 · 发 / 回复 / 转发 / 编辑 / 删除 · 表情回应 · 投票与按钮


### ⑥ 读、发与互动

| 功能 | TDLib 函数 / 更新（`tl:行号`） | 读屏价值 | 难度 | 依赖 | 对应 T | 批次建议 |
|---|---|---|---|---|---|---|
| 打开/关闭会话 | `openChat` tl:14188 · `closeChat` tl:14191 | 高：超级群和频道不打开就收不全实时更新（tl:14186）；已读另靠 `viewMessages`，关着时用 `force_read`（tl:14198） | S：成对调用，记住当前会话 | 会话列表 | T03 | MVP |
| 拉历史（逐条读） | `getChatHistory` tl:12758 · `getMessage` tl:12418 · `getMessages` tl:12451 · `updateNewMessage` tl:11196 | 高：读消息的核心 | M：分页（from_message_id/offset）+ 本地缓存与新消息合并；首次常只返回 1 条需循环 | 打开会话 | T03 | MVP |
| 标已读 | `viewMessages` tl:14199 · `updateChatReadInbox` tl:11331 · `updateChatReadOutbox` tl:11334 | 高：未读数要对，对方要看到已读 | S：读到哪条就报哪条 | 打开会话 | T03/T09 | MVP |
| 消息变化（编辑/删除/互动） | `updateMessageContent` tl:11217 · `updateMessageEdited` tl:11230 · `updateDeleteMessages` tl:11523 · `updateMessageInteractionInfo` tl:11239 | 高：读屏要播「已编辑」「已删除」 | M：按 id 改本地状态并决定是否播报 | 拉历史 | T06 | MVP |
| 取被回复的那条 | `getRepliedMessage` tl:12436 · 字段 `messageReplyToMessage` tl:3505（含 quote） | 高：不知道回的是哪句就听不懂 | S：一次调用；摘要念被回复者+前几字 | 拉历史 | T06 | MVP |
| 转发来源 | 字段 `messageForwardInfo` tl:3375 | 高：「转发自 X」是理解前提 | S：纯读字段 | 拉历史 | T06 | MVP |
| 消息可做什么 | `getMessageProperties` tl:12457（`messageProperties` tl:6863） | 中：决定菜单里列哪些动作，避免读屏用户点了才报错 | S：一次调用 | 拉历史 | T04/T06 | MVP |
| 置顶消息 | `getChatPinnedMessage` tl:12440 · `searchChatMessages` tl:12793 + `searchMessagesFilterPinned` tl:6930 · `pinChatMessage` tl:14562 · `unpinChatMessage` tl:14567 | 中：群里置顶常是规则/公告；读为主，置顶操作低 | S：读一条；全列表走搜索 | 拉历史 | T03 | 二（读）/ 以后（置顶/取消） |
| 话题 / 评论线程 | `getMessageThread` tl:12468 · `getMessageThreadHistory` tl:12768 · `getForumTopics` tl:13654 · `getForumTopicHistory` tl:13642 | 中：论坛群、频道评论区要能进 | L：多一层导航（话题列表→话题内消息），回复要带 topic | 拉历史、会话列表 | T03 | 二 |
| 按日期跳转 | `getChatMessageByDate` tl:12898 · `getChatMessageCalendar` tl:12918 | 中：长会话里「跳到上周三」对读屏比滚动省事得多 | M：输入日期 + 以该条为锚重新拉历史 | 拉历史 | T03 | 二 |
| 相册（多图一组） | 字段 `message.media_album_id` tl:3589（无专用函数） | 中：要合并念成「相册 4 张」而非 4 条 | M：客户端按 album_id 聚合相邻消息 | 拉历史、媒体描述 | T07 | 二 |
| 打开媒体内容 | `openMessageContent` tl:14205 · `updateMessageContentOpened` tl:11244 | 中：语音/阅后即焚要报「已打开」才计已听 | S：一次调用；但前置是下载/播放 | 文件下载、媒体播放 | T07 | 二 |
| 翻译 | `translateMessageText` tl:13073 · `translateText` tl:13060 · `summarizeMessage` tl:13087 | 低：日常非必需；摘要对长消息有用 | S：一次调用 | 拉历史 | — | 以后 |
| 发文字消息 | `sendMessage` tl:13143 · `inputMessageText` tl:6680 · `updateMessageSendSucceeded` tl:11208 | 高：回消息的核心 | M：临时 id → 成功后换正式 id，要跟踪 | 打开会话 | T04 | MVP |
| 回复 / 引用回复 | `inputMessageReplyToMessage` tl:3518（`quote` 用 `inputTextQuote` tl:3488）· `inputMessageReplyToExternalMessage` tl:3526（跨会话） | 高：群聊里不回复就乱 | S：发送时带参数；引用要选文字片段，界面 M | 发文字 | T04 | MVP（回复）/ 二（引用、跨会话） |
| 草稿 | `setChatDraftMessage` tl:14488 · `draftMessage` tl:3881 · `updateChatDraftMessage` tl:11349 | 中：切会话不丢字、与手机同步 | S：离开会话时存，进来时恢复 | 发文字 | T04 | 二 |
| 编辑文字 / 说明 | `editMessageText` tl:13252 · `editMessageCaption` tl:13284 | 中：改错字常用 | M：要把原文放回输入框，读屏要报「编辑模式」 | 发文字、消息可做什么 | T04 | 二 |
| 删除（给自己/给所有人） | `deleteMessages` tl:13226（`revoke` 决定是否对所有人） | 中：不常用但必须可及 | S：一次调用；须确认对话框（对外不可逆） | 消息可做什么 | T04 | 二 |
| 转发 | `forwardMessages` tl:13180 | 中：常用分享手段 | M：要选目标会话（复用会话搜索） | 会话列表、搜索 | T04 | 二 |
| 重发失败 | `resendMessages` tl:13195 · `updateMessageSendFailed` tl:11214 | 高：断网时发失败读屏用户看不见红叹号，必须播报并可重试 | S：一次调用 | 发文字、网络状态 | T04/T10 | MVP |
| 定时 / 静音发送 | `messageSendOptions` tl:6533（`disable_notification`、`scheduling_state`）· `messageSchedulingStateSendAtDate` tl:6500 · `getChatScheduledMessages` tl:12936 | 低：偶用 | S（静音）/ M（定时要日期输入+列表） | 发文字 | T04 | 以后 |
| 正在输入 | `sendChatAction` tl:14159 · `chatActionTyping` tl:6960 · `updateChatAction` tl:11530 | 低：发出去有礼貌；收到的「对方正在输入」播报会打断，默认不播 | S：定时发；收到只存状态 | 打开会话 | T05 | 二（发）/ 收到默认静默 |
| Markdown 解析 | `parseTextEntities` tl:13832（`textParseModeMarkdown` tl:10828 / `textParseModeHTML` tl:10831）· `parseMarkdown` tl:13836 · `formattedText` tl:122 | 中：纯键盘发粗体/代码靠标记；AI/CLI 也好用 | S：可同步调用 | 发文字 | T04 | 二 |
| 链接预览 | `getLinkPreview` tl:15840 · `inputMessageText.link_preview_options` tl:6680 | 低：发送前预览对读屏意义小；收到的预览念标题即可 | S | 发文字 | T04/T07 | 以后 |
| 表情回应：看 | 字段 `messageReactions` tl:3403 / `messageInteractionInfo` tl:3410 · `getMessageAddedReactions` tl:13802 · `updateMessageUnreadReactions` tl:11257 · `readAllChatReactions` tl:14279 | 高：「👍 3」是重要反馈；谁回应的按需查 | S（念计数）/ M（谁回应要分页列表） | 拉历史 | T06 | MVP（计数）/ 二（谁回应） |
| 表情回应：加/删 | `getMessageAvailableReactions` tl:13737 · `addMessageReaction` tl:13748 · `removeMessageReaction` tl:13754 · `setMessageReactions` tl:13794 | 中：低成本的回复方式 | M：可用列表可变，要可键盘选的菜单 | 看回应 | T04/T06 | 二 |
| 投票 | `setPollAnswer` tl:13890 · `getPollVoters` tl:13900 · `stopPoll` tl:13912 | 中：群里常见；读屏要能念选项与百分比 | M：单/多选、匿名、测验模式 | 拉历史 | T06 | 二（念+投）/ 以后（查投票人、stopPoll） |
| 内联按钮回调 | `replyMarkupInlineKeyboard` tl:4314 · `inlineKeyboardButtonTypeCallback` tl:4242 · `inlineKeyboardButtonTypeUrl` tl:4230 · `getCallbackQueryAnswer` tl:14098 · `callbackQueryPayloadData` tl:8369 | 中：机器人消息（验证码、菜单）不可点就卡住 | M：按钮网格要做成可导航列表；502 超时；答复要播报 | 拉历史 | T03/T04 | 二 |
| 已读时间 / 群内已读者 | `getMessageReadDate` tl:12473 · `getMessageViewers` tl:12479 | 低：偶查 | S | 消息可做什么 | — | 以后 |
| 未读 @ 提及 | `readAllChatMentions` tl:14276 · 字段 `message.contains_unread_mention` tl:3598 | 中：群里被 @ 要能跳过去 | S | 拉历史 | T09 | 二 |

### 注意（B）
1. **对外动作须人确认**（intent）：发送、删除给所有人、转发、加回应、投票、点回调按钮都是对外动作；CLI 要有确认/`--yes` 设计，测试只发「收藏夹」。
2. `getChatHistory` 首次可能只返回很少几条（本地缓存），必须循环拉到够数，否则「读不全」——这是读屏最易踩的坑。
3. 相册没有专用函数，要客户端按 `media_album_id` 聚合；发相册是 `sendMessageAlbum` tl:13152。
4. 「对方正在输入」「回应到达」都是高频更新，默认不播报，否则违反 T05 不打断；需人定播报策略。
5. 本表只盘了函数；`messageReplyTo` 的 quote、`MessageTopic` 等新式参数在 1.8.69 已变（回复/草稿/输入状态都带 `topic_id`），写 tdlib-rs 调用时以 tl 原文为准。

## C · 媒体与文件 · 语音 · 搜索 · 群与频道 · 其他


### ⑦ 媒体与文件

| 功能 | TDLib 函数 / 更新 | 读屏价值 | 难度 | 依赖 | 对应 T | 批次建议 |
|---|---|---|---|---|---|---|
| 读出媒体描述（类型、文件名、大小、时长、贴纸对应 emoji、说明文字） | `messagePhoto` tl:5722 · `messageDocument` tl:5706 · `messageVideo` tl:5737 · `messageAudio` tl:5703 · `messageSticker` tl:5725 · `messageAnimation` tl:5700 · `sticker`(含 emoji) tl:612 | 高：读屏用户唯一能"看到"媒体的方式 | S：纯读字段拼句子 | 消息读取 | T07 | MVP |
| 下载文件 + 进度 | `downloadFile` tl:15001 · `updateFile` tl:11596 · `cancelDownloadFile` tl:15011 · `getFile` tl:12487 | 高：打开文件/听语音的前提 | M：要跟踪 updateFile 进度状态，读屏需播报进度/完成 | — | T07 | MVP |
| 打开已下载文件（交给系统默认应用 / Finder 显示） | `file`/`localFile.path` tl:326/306 + macOS `open`；`openMessageContent` tl:14205 标记已打开 | 高：文档、PDF 交给 VoiceOver 原生应用读 | S：拿本地路径调用系统 | 下载 | T07 | MVP |
| 存到本地（另存为 / 下载列表） | `addFileToDownloads` tl:15061 · `toggleDownloadIsPaused` tl:15066 · `searchFileDownloads` tl:15086 · `removeFileFromDownloads` tl:15072 · `updateFileDownloads` tl:11614 · `getSuggestedFileName` tl:15014 | 中：另存为用 macOS 保存面板即可，下载列表是锦上添花 | S（复制本地文件）/ M（完整下载管理器） | 下载 | T07 | 另存为：二；下载管理器：以后 |
| 发送图片/文件/视频 | `sendMessage` tl:13143 + `inputMessagePhoto` tl:6716 · `inputMessageDocument` tl:6700 · `inputMessageVideo` tl:6727 · `inputFileLocal` tl:340；多张 `sendMessageAlbum` tl:13152 | 中：读屏用户发文件主要是"文档"，图片次之 | M：选文件（NSOpenPanel 可访问）+ 上传进度 + 对外动作需确认 | 发消息、updateFile | T04 | 发文件：二；相册/视频压缩：以后 |
| 上传进度 / 对方"正在发送文件"状态 | `updateFile` tl:11596 · `sendChatAction` tl:14159 · `chatActionUploadingDocument` tl:6978 · `updateChatAction` tl:11530 | 低：读屏不宜频繁播报 | S | 发文件 | T05 | 二（仅自己上传进度） |
| 缩略图 / 迷你缩略图 | `thumbnail` tl:393 · `minithumbnail` tl:361 · `photoSize` tl:355 | 低：视觉内容；唯一用途是喂给本机图像描述（见注意 2） | S | 下载 | T07 | 以后 |
| 自动下载设置 | `getAutoDownloadSettingsPresets` tl:17169 · `setAutoDownloadSettings` tl:17172 · `autoDownloadSettings` tl:10587 | 中：自动下载语音/小文件能省一步；TDLib 不会自己按此下载，客户端须自行执行策略 | M：客户端要自己实现"收到消息→按规则下载" | 下载 | T07 | 二（先硬编码"语音自动下"） |
| 存储占用 / 清理 | `getStorageStatisticsFast` tl:17136 · `getStorageStatistics` tl:17133 · `optimizeStorage` tl:17151 · `deleteFile` tl:15052 | 低：偶尔用 | S：读数字 / 一键清理 | — | — | 以后 |
| 网络流量统计 | `getNetworkStatistics` tl:17160 | 低 | S | — | — | 不做（非日常） |

### ⑧ 语音

| 功能 | TDLib 函数 / 更新 | 读屏价值 | 难度 | 依赖 | 对应 T | 批次建议 |
|---|---|---|---|---|---|---|
| 读出语音消息描述（时长、是否已听、说明） | `messageVoiceNote`(is_listened) tl:5749 · `voiceNote`(duration) tl:643 | 高：语音在私聊里很常见 | S | 消息读取 | T07 | MVP |
| 播放语音消息 | `downloadFile` tl:15001 → 本地 OGG/Opus 文件 → 播放器 | 高：读屏用户最依赖听 | M：macOS AVFoundation 不原生解 Opus/OGG，需解码库或先转码；播放/暂停/快进键位要和 VoiceOver 不冲突 | 下载 | T07 | MVP（至少能播） |
| 标记已听 | `openMessageContent` tl:14205 → `updateMessageContentOpened` tl:11244 | 中：对方能看到"已听"，自己能分辨哪条没听 | S | 播放 | T06/T07 | MVP（随播放一起） |
| 语音转文字 | `recognizeSpeech` tl:13121 · 结果写进 `voiceNote.speech_recognition_result` 经 `updateMessageContent` tl:11217 推送 · `speechRecognitionResultPending/Text/Error` tl:8008/8011/8014 · 能否用看 `messageProperties.can_recognize_speech` tl:6863（`getMessageProperties` tl:12457）· `rateSpeechRecognition` tl:13127 | 高：让语音变成可逐字浏览/搜索的文字，对读屏极友好 | M：异步多次更新（pending→text），要在消息上挂状态 | 消息读取 | T07 | 二（取决于是否 Premium，见注意 1） |
| 转文字的非 Premium 试用额度 | `updateSpeechRecognitionTrial` tl:11921（每周次数 weekly_count、剩余 left_count、单条最长 max_media_duration 秒、重置时间）· Premium 特性 `premiumFeatureVoiceRecognition` tl:8748 · 超长报 `MSG_VOICE_TOO_LONG` | 中：要告诉用户"本周还剩几次" | S | 转文字 | T07 | 二 |
| 录制并发送语音 | `inputMessageVoiceNote` tl:6738 · `inputVoiceNote`(OGG/Opus 单声道或 MP3/M4A, duration, waveform 5-bit) tl:6467 · `sendMessage` tl:13143 · 录制中状态 `chatActionRecordingVoiceNote` tl:6969 | 中高：打字慢的读屏用户常用语音回复 | L：录音 + 编码 + 波形 + 按住/切换录制的键盘交互 + 试听 + 发前确认 | 发消息、麦克风权限 | T04 | 二 |
| 视频圆消息（video note）描述与转文字 | `videoNote`(含 speech_recognition_result) tl:635 | 中 | S（描述）/ M（播放） | 下载 | T07 | 描述：MVP；播放：以后 |

### ⑨ 搜索

| 功能 | TDLib 函数 / 更新 | 读屏价值 | 难度 | 依赖 | 对应 T | 批次建议 |
|---|---|---|---|---|---|---|
| 搜已知会话（本地，按名字/用户名） | `searchChats` tl:12523（离线，空 query 返回最近找到的 50 个） | 高：读屏用户找会话比逐条翻快得多 | S | 会话列表 | T08 | MVP |
| 搜已知会话（服务器） | `searchChatsOnServer` tl:12529 | 中：本地没命中时补 | S | 同上 | T08 | MVP（本地后自动补） |
| 按用户名找公开会话 / 公开搜索 | `searchPublicChat` tl:12511 · `searchPublicChats` tl:12517 | 中：加新频道/找人用 | S | — | T08 | 二 |
| 本会话内搜消息 | `searchChatMessages` tl:12793（可按发送者、按 filter） | 高：长群聊里定位一条消息 | M：结果要能跳回上下文（getChatHistory 定位）| 消息读取 | T08 | MVP |
| 全局搜消息 | `searchMessages` tl:12806（可按 chat_type_filter、日期；不支持密聊）· 密聊另用 `searchSecretMessages` tl:12814 | 高 | M：分页 offset + 结果要读出"在哪个会话" | 消息读取 | T08 | MVP |
| 按类型过滤 | `searchMessagesFilterEmpty/Photo/Document/VoiceNote/Url/Mention/UnreadMention…` tl:6876/6888/6885/6897/6903/6915/6918；会话类型 `searchMessagesChatTypeFilterPrivate/Group/Channel` tl:6936/6939/6942 | 中高："找某人发过的文件/链接""只看@我的"对读屏很省力 | S：参数而已 | 搜消息 | T08/T09 | 二（UnreadMention 可进 MVP 配合 T09） |
| 搜联系人 | `searchContacts` tl:15555 · `getContacts` tl:15550 | 中：发起新私聊时用 | S | — | T08 | 二 |
| 最近搜索 / 最近打开 | `searchRecentlyFoundChats` tl:12572 · `addRecentlyFoundChat` tl:12576 · `removeRecentlyFoundChat` tl:12579 · `clearRecentlyFoundChats` tl:12582 · `getRecentlyOpenedChats` tl:12585 | 中：快速回到常去的会话 | S | 搜会话 | T08 | 二 |
| 常用联系人（Top chats） | `getTopChats` tl:12561 | 低中 | S | — | T08 | 以后 |
| 按日期跳转 / 数量 / 日历 | `getChatMessageByDate` tl:12898 · `getChatMessageCount` tl:12925 · `getChatMessageCalendar` tl:12918 | 中："跳到昨天"对读屏比滚动强 | S | 消息读取 | T03/T08 | 二（按日期跳）；日历：以后 |
| 搜收藏夹 / 搜自己发出的文件 / 搜通话记录 | `searchSavedMessages` tl:12826 · `searchOutgoingDocumentMessages` tl:12837 · `searchCallMessages` tl:12832 | 低 | S | — | T08 | 以后 |
| 搜贴纸/表情/话题标签/公开帖子 | `searchStickers` tl:15682 · `searchEmojis` tl:15780 · `searchHashtags` tl:15831 · `searchPublicPosts` tl:12847 | 低 | S | — | — | 以后 |

### ⑩ 群与频道（成员端）

| 功能 | TDLib 函数 / 更新 | 读屏价值 | 难度 | 依赖 | 对应 T | 批次建议 |
|---|---|---|---|---|---|---|
| 群/频道信息（简介、人数、类型） | `getChat` tl:12413 · `getBasicGroup` tl:12397 · `getBasicGroupFullInfo` tl:12400 · `getSupergroup` tl:12404 · `getSupergroupFullInfo` tl:12407 | 中：进群先知道是什么群 | S | 会话列表 | T02/T03 | 二 |
| 置顶消息 | `getChatPinnedMessage` tl:12440 · 过滤 `searchMessagesFilterPinned` tl:6930 | 中：群公告常放置顶 | S | 消息读取 | T03 | 二 |
| 看成员 / 搜成员 / 管理员列表 | `getSupergroupMembers` tl:16337 · `searchChatMembers` tl:14637 · `getChatMember` tl:14630 · `getChatAdministrators` tl:14640；基础群成员在 `basicGroupFullInfo` 里 | 低中：偶尔要找某人 | M：大群分页 | 群信息 | — | 以后 |
| 加入 / 退出 | `joinChat` tl:14575 · `leaveChat` tl:14578 | 中：订阅频道、退群 | S：对外动作须人确认 | 搜公开会话 | — | 二 |
| 凭邀请链接加入 | `checkChatInviteLink` tl:15188（先读出群名/人数）· `joinChatByInviteLink` tl:15191 | 中 | S | — | — | 二 |
| 生成/查看邀请链接 | `createChatInviteLink` tl:15120 · `getChatInviteLinks` tl:15161 · `getChatInviteLink` tl:15149 | 低：多为管理员操作 | S | — | — | 以后 |
| 管理后台：改权限、封禁、审批入群、设管理员、建群/频道 | `setChatMemberStatus` tl:14598 · `banChatMember` tl:14613 · `getChatJoinRequests` tl:15199 · `processChatJoinRequest` tl:15205 | — | L | — | — | 不做（第一阶段不做群/频道管理后台） |
| 相似频道推荐 | `getChatSimilarChats` tl:12535 · `getChatSimilarChatCount` tl:12540 | 低 | S | — | — | 不做 |

### ⑪ 其他（简列）

| 功能 | TDLib 函数 / 更新 | 读屏价值 | 难度 | 依赖 | 对应 T | 批次建议 |
|---|---|---|---|---|---|---|
| 通话（语音/视频/群通话） | `createCall` tl:15240 · `updateCall` tl:11649 · `updateGroupCall` tl:11652 · `updateNewCallSignalingData` tl:11695 | 高（但 TDLib 只给信令，媒体要另接 tgcalls） | L | — | — | 不做（第一阶段明确不做）；**来电提示**可在 T05 里只读出"未接来电"消息 |
| Stories | `getStory` tl:14708 · `postStory` tl:14728 · `updateChatActiveStories` tl:11744 · `updateStoryDeleted` tl:11731 | 低：视觉为主 | L | — | — | 不做（第一阶段明确不做） |
| 贴纸面板 / 表情包管理 | `getInstalledStickerSets` tl:15691 · `searchStickerSet` tl:15716 · `inputMessageSticker` tl:6719 · `getStickerEmojis` tl:15775 · `getSavedAnimations` tl:15812 · `createNewStickerSet` tl:17289 | 低：收到贴纸读 emoji 即可（已归 ①）；发贴纸可"按 emoji 选" | M | — | T07 | 收：MVP（只读 emoji）；发/管理：以后 |
| 投票 | `inputMessagePoll` tl:6793 · `setPollAnswer` tl:13890 | 中：群里常见，读出选项并能投 | M | 消息读取 | T06/T07 | 二（读出）/ 以后（投票） |
| 机器人 / 小程序 / 内联按钮 | `getCallbackQueryAnswer` tl:14098 · `getInlineQueryResults` tl:13978 · `sendInlineQueryResultMessage` tl:13169 · `getWebAppUrl` tl:14050 · `sendWebAppData` tl:14061 · `getBotInfoDescription` tl:16162 | 中：点按钮机器人是普通用户场景；小程序是网页，无障碍不可控 | M（按钮）/ L（小程序） | 消息读取 | — | 按钮读出：二；点按钮：以后；小程序与机器人开发：不做 |
| 支付 / 星星 / 礼物 | `getPaymentForm` tl:16362 · `sendPaymentForm` tl:16377 · `getStarTransactions` tl:17430 · `sendGift` tl:16409 | 低 | L | — | — | 不做（第一阶段明确不做支付） |
| 商业功能（Business） | `getBusinessConnectedBot` tl:15951（`getBusinessInfo` 未找到） | 低 | L | — | — | 不做 |
| 密聊（secret chat） | `createNewSecretChat` tl:14320 · `createSecretChat` tl:14301 · `secretChat` tl:3234 · `secretChatStateReady` tl:3218 · `chatTypeSecret` tl:3896 · `updateSecretChat` tl:11568 · `searchSecretMessages` tl:12814 | 中：少数用户依赖；读屏用户若有密聊而本客户端读不到，会以为消息丢了 | M：TDLib 已全包加密；难点在只在本设备、需显示/读出密钥指纹、自毁计时 | 消息读取 | T02/T03 | **读**：二（至少能列出并读消息，不做会"丢会话"）；**新建**：以后 |
| 联系人管理 | `addContact` tl:15543 · `importContacts` tl:15547 · `createPrivateChat` tl:14288 | 中：能"给某人发起私聊"即可 | S | 搜联系人 | T04 | 二（发起私聊）/ 以后（导入通讯录） |
| 消息链接 | `getMessageLink` tl:13004 · `getMessageLinkInfo` tl:13013 | 中：打开 t.me 链接跳到消息 | S | 消息读取 | T03 | 二 |

### 注意（C）

1. **语音转文字要不要依赖 Premium（需人定）**：非 Premium 有每周试用额度（`updateSpeechRecognitionTrial` tl:11921，weekly_count 可为 0 即无额度），单条有时长上限；超级群可凭 boost 等级对所有人开放（`chatBoostFeatures.min_speech_recognition_boost_level` tl:7501）。若额度不足，可考虑**本机离线转写**（macOS Speech 框架 / whisper）作为后备——需人定隐私与精度取舍。
2. **Telegram 图片没有 alt text**：读屏只能读到"图片 + 说明文字"。要真正"描述图片"只能本机 OCR / 视觉模型（macOS Vision 框架可离线 OCR）——是否做、是否允许外发到云端模型，需人定（铁律 12/13）。
3. **语音收发都绕不开 Opus/OGG**：macOS 原生不支持 OGG 容器；播放与录制都需引入解码/编码库（或用 MP3/M4A 发送——tl:6467 说明允许，但对方客户端显示可能不同）。影响 MVP 的"能播语音"。
4. **自动下载 TDLib 不代劳**：`setAutoDownloadSettings` 只同步设置，真正"收到就下"需客户端在新消息时自己调 `downloadFile`。
5. **密聊判断**：仅本设备可见、与手机不同步；如果用户手机上已有密聊，本客户端本来就看不到（由新设备发起才有）。建议第二批"能读能回已在本端建立的密聊"，新建以后再说。


---

## 附：意图级动作（已移交 infra）

MVP 意图级动作的初版（2026-10-25）已交给 infra，**正本是 `specs/contracts/actions.md`**（动作名、参数、返回、错误、对外分级 L0 / L1 / L2）。这里不再留副本，免得两份不一致。
api 交过去的要点：分级「否 / 轻 / 闸」= L0 / L1 / L2；`next_unread_chat` 是 Store 查询不上 `Backend`；`chat_open` / `chat_close` 必须成对；合并盘点后补的 `set_online` `message_properties` `resend` `mute` `proxy_*`，以及自审后补的 `mark_listened`（`openMessageContent` `tl:14205`，L1）、`message_links`（纯本地，从内容树取，L0）。
