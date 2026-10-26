# 内容树（AST）规约 v0

> 2026-10-25 · 手下 tree 起草 · 状态：**草稿**；末尾「待人拍板」R1–R7 **已拍板**（`docs/plan/003_decisions-b2.md`），正文已同步；快照尚未按拍板改（S0 代码暂停，清单见 `state/coord.md`）
> 代码：`src/atg-core/src/tree/`（节点类型 `mod.rs` · 实体建树 `text.rs`）· 读法 `speech.rs` · JSON `json.rs` · 夹具输入 `tdjson.rs`
> 依据：`td_api.tl`（TDLib 1.8.69，下文写 `tl:行号`）· 内容类型清单 `docs/research/002_features/content-types.md`（api 手下）· 架构方案 `docs/plan/001_arch-stack.md`

## 术语（读到不懂回这里）

- **AST / 内容树**：把一条消息拆成有类型的零件（发送人、正文、粗体、图片……）、按包含关系排成的一棵树
- **TDLib**：Telegram 官方的客户端库，本项目从它拿数据；`td_type` = TDLib 里的原类型名，如 `messageDice`
- **枚举 / 变体**：Rust 里「几选一」的类型，每一种选项叫一个变体；**开放集合** = TDLib 以后还会加新选项的那类
- **渲染器**：把树变成某种输出的函数（读屏的一句话、一行 JSON、界面）
- **model / store / reducer**：本项目自己的数据类型 / 全部状态 / 「旧状态 + 一件事 → 新状态」的函数（架构方案术语表）
- **投影**：从 model 和 store 里现取数据、拼成一棵树的那个函数
- **夹具**：测试用的手写假数据；**快照**：存下来的期望输出，变了测试就报
- **UTF-16 码元 / 代理对**：TDLib 量文字位置的单位。中文一个字 1 个码元，大多数表情 2 个（这 2 个合称代理对）；**字素簇** = 人眼看着是「一个字」的一串字符，如 👨‍👩‍👧
- **int53**：不超过 2^53 的整数，JavaScript 能精确表示；更大的要写成字符串

## 〇、一句话

**一切要被读出来或打印出来的东西（消息、系统消息、会话项、新消息提醒、连接状态……），都先变成一棵有类型的树；读法、CLI 的 JSON、GUI、无障碍名称都只是这棵树的渲染器。树只放数据，不放怎么读。不认识的东西变成「未知」节点，不崩。**

人的原话（意图正本）：「每一个或者一类东西都用ast 来标记…这样架构上改起来会比较容易」。

## 一、三条硬规矩

1. **树只放数据。** 树里没有任何读法字符串、按键提示、界面文字。「按 M 收听」的 M 归 GUI，「已撤回的消息」这句话归读法
2. **每个对应 TDLib「开放集合」的枚举都有兜底变体，并带原类型名 `td_type`**：消息内容、系统消息、文本实体、发送人、转发来源、回复对象、话题、表情回应、会话类型、顶层对象。（用户状态 `UserStatus` 不是 TDLib 的集合，是本项目合成的封闭值，没有兜底变体；TDLib 不认识的连接状态在合成之前就处理掉，见第三节第 6 小节）TDLib 升级会加新子类（`MessageContent` 现有 108 个），兜底变体是升级不崩的关键
3. **文字一个字都不丢。** 建树遇到不合规的实体只拆实体、不删文字：`plain_text(build(文本, 实体)) == 文本` 对任何输入成立（`text.rs` 的 `never_loses_text_on_messy_input` 用 500 组乱实体测住）

## 二、两层：存的 `Content`，现算的 `Doc`

| 层 | 是什么 | 取决于 | 什么时候建 | 存不存 |
|---|---|---|---|---|
| `Content` | 正文行内树 + 附件 | **只取决于这条消息本身** | 收到消息时建一次 | **存进 model**，`updateMessageContent` 时整块替换 |
| `Doc` | 头（发送人名字、回复谁、转发自谁）+ `Content` + 尾（已编辑、表情回应、已读） | 消息 + 用户缓存 + 会话的已读位置…… | **渲染时现投影** | 不存 |

**为什么这样分**（回答「树是存进 store 的，还是渲染时现算的」）：

- 全存：名字、已读状态会比消息晚到（`updateUser`、`updateChatReadOutbox`），存下来的树会过时；每来一条用户更新就要重建一批树。和架构方案第三节「在渲染时算」相冲突
- 全现算：每次渲染都重新解析实体。代价其实很小（虚拟化列表每帧只渲染约 12 项），但 model 就得另存一份 TDLib 形状的「纯文本 + 实体」，等于两种正文表示并存
- **建议：分层。** model 里消息的正文字段**就是** `Content`，不再另定义一个「格式化文本」类型——唯一编码，也让 reducer 的「按字段合并」天然成立（内容只认 `updateMessageContent`，它换的正是 `Content`）。`Doc` 由投影纯函数现算（`src/atg-core/src/project.rs`，拍板 006 乙9）：`message(&store, 消息身份)`（一条消息 / 系统消息 / 撤回占位）、`window(&store)`（当前窗口，从旧到新）、`chat_list(&store, 列表)`、`chat_item(&store, 会话 id, 列表)`（「置顶」按传入的列表看）；以后量出来慢再加缓存，缓存键带上名字和已读位置的版本号

**哪些更新能改 `Content` 的哪个字段**（reducer 照这张表，其余更新一律不碰 `Content`）：

| 更新 | 改什么 |
|---|---|
| `updateMessageContent` `tl:11217` | 整块替换（按「有效内容」规则重建） |
| `updateMessageEphemeralContent` `tl:11223` | 整块替换：临时内容出现或消失，有效内容随之变 |
| `updateMessageContentOpened` `tl:11244` | 只改 `voice.listened` / `video_note.viewed` 这类「已听 / 已看」标记 |

**系统消息怎么存**：`Action` 同样只取决于消息本身，和 `Content` 一样存进 model；但里面提到的用户（`UserRef`）在 model 里只存 id、`name` 留空，名字到投影时才填。规则统一成一句：**存下来的树只放 id，名字一律投影时填。**

原型里 `tdjson::World::docs()` 就是这样做的：事件按顺序收下，**调 `docs()` 时才投影**，所以夹具里最后才到的用户名也能查到（`basic.json` 的「陈一一」）。

## 三、节点种类 v0

### 1. 顶层 `Doc`（JSON 判别字段 `type`）

| `type` | 是什么 | 来源 |
|---|---|---|
| `message` | 普通消息（含带媒体的） | `message` `tl:3598` |
| `service` | 系统消息 | 系统类 `MessageContent` |
| `tombstone` | 原位「已撤回」占位（拍板 7）：`id` + 撤回前记下的 `sender`、`date`（R7 读法要用，store 在删除前留下这两项） | `updateDeleteMessages` `tl:11523`，判定只在一处：`specs/contracts/actions.md` 事件 `messages_deleted`（只认永久且非缓存的删除） |
| `chat_item` | 会话列表一项 | `chat` `tl:4080` |
| `alert` | 新消息提醒：总数 + 各会话条数 | 本项目 `notify` 策略 |
| `status` | 用户状态（T10）：内核、授权、连接三条线合成的一个值 | 合成规则归 `specs/contracts/errors.md` 第一节第 3 小节；连接一线来自 `updateConnectionState` `tl:11818` |
| `unknown` | 不认识的顶层对象 | 任何别的 |

**以后加**（api 清单「五」）：**未读分隔**（api 升为 M：进会话的落点要用，下一个加）、日期分隔、登录步骤、发送结果、服务通知。分隔不是 TDLib 对象，由投影按 `chat.last_read_inbox_message_id` 和日期插进消息序列，形状预定为 `divider{kind: unread | date, …}`，自己没有消息身份，不进焦点顺序还是进，交 GUI 阶段定。登录步骤与发送结果和 infra 的 CLI 契约（`prompt` / `result`）交界，等那份定了再加。

### 2. 消息 `Message`

```
Message
├── id: {chat_id, message_id}
├── head
│   ├── sender: user{user_id, name?} | chat{chat_id, title?} | unknown{td_type}
│   ├── date
│   ├── outgoing
│   ├── reply_to?: message{target, sender?, quote?} | unknown{td_type}
│   ├── forward?: {origin, date}，origin = user | hidden_user{name} | chat | channel | unknown
│   ├── album_id?
│   └── topic?: thread | forum{topic_id, name?} | direct_messages | saved_messages | unknown   ← 回复、发送时要沿用
├── content
│   ├── body: [Span]          ← 文本消息的正文，或媒体的说明文字
│   └── attachment?: Attachment
└── tail
    ├── edited?: 时间
    ├── reactions: [{kind: emoji | custom_emoji | paid | unknown, count, chosen}]
    └── delivery?: pending | failed | sent | read   ← 只有自己发的才有
```

- 名字是 `Option`：查不到就是空，读法给兜底说法（「未知发送人」），**树里不写兜底文字**
- **建树前先选有效内容**：消息带临时内容（`ephemeral_content`，`tl:3547`、`:3594`「必须代替原内容显示」）就用它，否则用 `content`；更新 `updateMessageEphemeralContent`（`tl:11223`）归 reducer
- **受限内容**（`restriction_info`，`tl:2738`、`:3591`「必须执行」）：原因非空 → 原内容**不进树**，附件换成 `restricted{reason}`、正文为空；只有「敏感」标记 → 照常建，`content.sensitive = true`，读法标「隐藏」
- `reply_to.target.chat_id` 可能是别的会话、也可能是 0（TDLib：被回复的那条在未知会话）；被回复那条不在 store 里时，发送人先用 TDLib 附带的 `origin` 兜底（api 提醒 2）
- 被回复那条在 store 里且受限 → 投影时 `reply_to.quote` 不出（引用的那段就是它的原文，拍板 006 乙11）；被回复那条换过 id 的，`target` 投影成现在的 id
- **相册**（`media_album_id`）v0 不建父节点：每张仍是一条消息、一个焦点身份，头里带 `album_id`，要合并由渲染器做（api 提醒 4）

### 3. 附件 `Attachment`（一条消息至多一个，`tl:5689-5843`）

| `type` | 字段 | 来源 |
|---|---|---|
| `photo` | spoiler | `messagePhoto` `tl:5722` |
| `video` | duration, spoiler | `messageVideo` `tl:5737` |
| `voice` | duration, listened | `messageVoiceNote` `tl:5749` |
| `video_note` | duration, viewed | `messageVideoNote` `tl:5743` |
| `audio` | duration, title, performer | `messageAudio` `tl:5703` |
| `animation` | duration | `messageAnimation` `tl:5700` |
| `document` | file_name, size（字节，不知道是 0） | `messageDocument` `tl:5706` |
| `sticker` | emoji | `messageSticker` `tl:5725` |
| `animated_emoji` | emoji | `messageAnimatedEmoji` `tl:5778` |
| `location` · `venue` · `contact` · `poll` · `call` | 见代码 | `tl:5769` `5772` `5775` `5796` `5832` |
| `expired` | what: photo / video / video_note / voice | `messageExpired*` `tl:5752-5761` |
| `restricted` | reason | 消息的 `restriction_info` 原因非空（不是内容类型） |
| `other` | td_type | **认识、但 v0 没做专门节点**的内容（骰子、Stories、游戏……清单在 `tdjson.rs` 的 `OTHER_CONTENT`） |
| `unknown` | td_type | `messageUnsupported` `tl:6279`，以及完全不认识的类型 |

`other` 与 `unknown` 分开，是为了让读法区分「我们知道这是什么，只是还没做」和「连我们都不认识」。

### 4. 系统消息 `Service`：`{id, date, actor, action}`

`action` 的 `type`：`members_added{users}` · `joined_by_link` · `joined_by_request` · `member_removed{user}`（`user` 与 actor 同人即自己退出）· `title_changed{title}` · `photo_changed` · `photo_deleted` · `pinned{pinned: 消息身份}` · `group_created{title}` · `contact_registered` · `other{td_type}`（认识但没做专门节点的，清单 `OTHER_SERVICE`）。

不认识的内容类型一律落附件 `unknown`，**不猜它是不是系统消息**。

### 5. 正文行内树 `Span`（`tl:113-122`，实体种类 `tl:6309-6383`）

| 类 | `type` | 里面能有什么 |
|---|---|---|
| 纯文字 | `text` | — |
| 叶子 | `code` · `pre{language?}` · `date_time{unix_time}` | **只有文字**（`text` 字段，不带 `children`） |
| 格式 | `bold` · `italic` · `underline` · `strikethrough` · `spoiler` | 任何东西 |
| 引用块 | `quote{expandable}` | 除了引用块以外的任何东西 |
| 其余实体 | `link{url}` · `url` · `mention` · `mention_name{user_id}` · `hashtag` · `cashtag` · `bot_command` · `email` · `phone` · `bank_card` · `ton_address` · `custom_emoji{custom_emoji_id}` · `media_timestamp{seconds}` · `unknown{td_type}` | **只有格式** |

- `link` 的地址、`mention_name` 的用户 id 不在文本里，必须是节点属性（api 提醒 3）
- `Pre` 与 `PreCode` 合成一种 `pre`，有语言就带 `language`
- `BlockQuote` 与 `ExpandableBlockQuote` 合成 `quote{expandable}`
- 引用块和代码块是「块级」的，但 TDLib 允许粗体包住引用块，所以**不硬分块级 / 行内两层**，统一放在一棵行内树里，由渲染器决定换不换行

### 6. 会话项 `ChatItem` · 提醒 `Alert` · 连接 `Connection`

- `chat_item`：`chat_id, title, kind(private / group / supergroup / channel / secret / unknown), unread_count, unread_mention_count, muted, mute_inherited, pinned, last?`。`muted` 是**有效静音**：会话设了 `use_default_mute_for`（`tl:3811`）就继承范围设置（`scopeNotificationSettings` `tl:3823`，论坛话题继承所在论坛 `tl:3795`），由投影算好放进来，`mute_inherited` 记它是不是继承的。原型没有范围设置，继承时按不静音算`last` 是一份 `message` / `service` / `tombstone`（Rust 里是窄枚举 `ChatLast`，只许这三种）
- `alert`：`total, chats[{chat_id, title?, count}]`。只有数据，「按 M 收听」不在树里
- `status`：`status` = `fault` / `stopping` / `signed_out` / `signing_in` / `unsupported_step` / `offline` / `connecting` / `syncing` / `online`（`stopping` 由 infra 新增：内核或授权在关闭中 / 已关闭，是人自己关的，不播报；合成规则见 `errors.md` 第一节第 3 小节）。**读法只读合成后的这个值，不直接读 TDLib 的连接状态**；三线合成函数归 infra（`errors.md`），不认识的连接状态在合成时按「正在连接」处理，所以这里不需要 `unknown`。原型 `tdjson` 只按连接一线简化合成

### 7. 正式类型名（给别的规约引用）

Rust 类型名 ↔ JSON `type`：`Doc`（顶层，`type` 见第 1 小节）· `Message` ↔ `message` · `Service` ↔ `service` · `Tombstone` ↔ `tombstone` · `ChatItem` ↔ `chat_item` · `Alert` ↔ `alert` · `Status` ↔ `status`（值是 `UserStatus`）· `Unknown` ↔ `unknown`；零件：`MsgId`（消息身份）· `Sender` · `UserRef`（系统消息里提到的用户）· `Content` · `Attachment` · `Span` · `Action` · `ChatKind`。树里没有「Group」「User」「ChatRef」这样的顶层类型：群是 `ChatKind::group` / `supergroup`。

## 四、建树规则：「纯文本 + 实体」→ 行内树

输入是 TDLib 的 `formattedText`：一段纯文本 + 一串实体，每个实体是「从第几个 UTF-16 码元开始、长几个码元、什么种类」（`tl:110-111`）。实现 `text::build`。

1. **偏移换算**：Rust 字符串按 UTF-8 存，先扫一遍文本，记下每个字符边界的「UTF-16 位置 ↔ 字节下标」，再二分查。中文一个字 1 个码元；`👍` 2 个码元、4 个字节；`👨‍👩‍👧` 8 个码元
2. **落在代理对中间**（只可能是坏数据）：起点往前取整、终点往后取整，记 `Snapped`
3. **防越界**：偏移为负或长度 ≤ 0 → 丢弃；起点在文本之外 → 丢弃；终点超出 → 截到末尾（`Clamped`）；`offset + length` 用饱和加法，不溢出
4. **谁在外**：按 起点升序 → 终点降序（长的在外）→ 类（引用块 > 其余实体 > 格式 > 叶子）→ 种类本身含参数（格式按 粗 > 斜 > 下划线 > 删除线 > 剧透）→ 输入顺序。前四项都相同的两个实体内容完全一样，所以**输出与输入顺序无关**：同范围的粗体和斜体永远粗体在外，同范围的网址和 @提及也总是同一个在外
5. **完全相同**的两个实体（同范围同种类）只留一个（`DroppedDuplicate`）
6. **交叉**（TDLib 说不许，但要防）：在外层的边界处切成两段，里面一段进外层，剩下一段留在外面（`Split`）。**例外：外层是格式、交叉的是链接一类的实体时，切格式、不切链接**（切开的网址就坏了，`links()` 会给出半截地址）
7. **嵌套规则**（`tl:118-121`）按「**最近一层非格式祖先**」判（格式层是透明的：`link > bold > hashtag` 里 hashtag 的祖先算 link）：
   - 祖先是叶子 → 里面什么实体都不许
   - 祖先是引用块 → 不许引用块
   - 祖先是其余实体 → 只许格式
   - 违反的那层**拆掉、文字留着**（`Unwrapped`），里面合法的东西照常建（`link > mention > bold`：mention 拆掉，bold 留下）
8. **相邻文字合并**，空文字不出节点
9. 所有修补都记成 `Repair`，**只记种类名和原因，不记文字**，可以安全进日志（架构方案第四节「日志里不出现聊天内容」）
10. **嵌套深度上限** 32 层（`MAX_DEPTH`），再深的拆平、文字留着（`TooDeep`），防恶意输入把栈撑爆

已知局限：只保证不切开 UTF-16 代理对，**不保证不切开字素簇**（实体边界可能落在 `👨‍👩‍👧` 的两个人之间）。TDLib 自己发来的实体不会这样；要防就得引一个字素切分库，v0 不引。

## 五、正例与反例

**正例**（`text.rs` 单测 + `tests/fixtures/*.json` 快照）：

| 输入 | 树 |
|---|---|
| `ab斜体cd`，粗体 0..6，斜体 2..4 | `bold[ "ab", italic["斜体"], "cd" ]` |
| `👍好的`，粗体 2..3 | `"👍", bold["好"], "的"` |
| `@lin_xy 看下路线图 #爬山`，提及、文字链接、话题标签 | `mention["@lin_xy"], " 看下", link{url}["路线图"], " ", hashtag["#爬山"]` |

**反例**（容易建错的输入，每条都有单测或夹具）：

| 输入 | 容易建错成 | 正确 |
|---|---|---|
| `👍好的`，粗体偏移 2 | 把偏移当字符数，粗体落到「的」 | 偏移按 UTF-16，粗体落在「好」 |
| `👍好的`，粗体偏移 2 | 把偏移当字节数，切在 `👍` 的字节中间而 panic | 换算成字节下标再切 |
| `👨‍👩‍👧到齐了`，下划线偏移 8 | 把表情当 1 个或 3 个码元 | ZWJ 序列 = 3×2 + 2×1 = 8 个码元 |
| `👍短`，下划线 1..2 | panic 或切出半个表情 | 往外取整到整个 `👍` |
| 粗体 0..4 与斜体 2..6 交叉 | 栈式建树时丢掉斜体后半，或 panic | `bold["一二", italic["三四"]], italic["五六"]` |
| 代码里套粗体 | 在代码节点里出粗体 | 代码只有文字，粗体拆掉 |
| 引用块套引用块 | 两层引用 | 一层，内层拆掉文字留着 |
| 链接里套 @提及 和粗体 | 两个都留，或两个都丢 | 提及拆掉，粗体留下 |
| 同范围的斜体和粗体，输入顺序不同 | 输出跟着输入顺序变，快照不稳 | 永远粗体在外 |
| 偏移 −1、长度 0、起点越界、长度 `i64::MAX` | panic 或整条消息丢掉 | 丢弃或截断该实体，文字完整 |
| 空文本带一个粗体 | panic | 空树 |
| 不认识的实体 `textEntityTypeSparkle` | 整条消息解析失败 | `unknown{td_type}`，文字照留 |
| 不认识的内容 `messageFancyFutureThing` | 解析失败、消息消失 | 附件 `unknown{td_type}`，消息还在列表里 |
| 用户信息比消息晚到 | 名字永远是「未知用户」 | 渲染时现投影，名字能查到 |

## 六、身份

- **顶层**：消息、系统消息、占位一律用 `(chat_id, message_id)`（架构方案第一节第 3 小节「身份」）；会话项用 `chat_id`。树里的 id 只是 model 的一份拷贝，不另起编号
- **发送成功换 id**（`updateMessageSendSucceeded` 带 `old_message_id`，`tl:11208`）：由 store 改映射、迁移焦点锚点；树下次投影自然用新 id。⚠️ GUI 的元素 id 若直接用消息 id，换 id 那一帧无障碍节点身份就断了，VoiceOver 可能重读或丢焦点。建议 GUI 阶段给列表项另用一个「本次运行内稳定的键」：store 记住旧 id → 新 id 的映射，元素 id 沿用最初的那个。要实测，记为 GUI 阶段的待办
- **子节点要不要稳定 id**：**不存，现算**。子节点的 id = 顶层 id + 从根到它的下标路径，如 `(101, 1001)/body/2/0`、`(101, 1001)/attachment`。理由：
  - 子节点只在正文变了时才会变结构，而正文一变（`updateMessageContent`）界面那一块本来就该换新节点
  - GPUI 的无障碍节点 id 本来就是「从根到它的元素 id 路径」哈希出来的（`gpui/src/element.rs:230-254`，架构方案第二节第 1 小节），用下标路径作元素 id 正好对得上
  - 存 id 要给每个节点加字段，JSON 变吵，还得保证唯一
- 什么时候需要：详情层要让链接、提及能逐个按（`link`、`mention_name`）时。v0 列表项只用顶层 id

## 七、渲染器

渲染器 = 「树 → 某种输出」的纯函数。现在有两个：`speech::brief`（读法）、`json::value`（CLI 的 JSON 行）。以后有 GUI 视图、无障碍名称（取 `speech` 的结果）、`speech::detail`。

**怎么遍历**：对每个枚举直接写 `match`，**不写 `_ =>`**；不另造访问者（visitor）抽象——Rust 的穷尽匹配本身就是最好的访问者。共用的遍历两个，都在 `text.rs`：`plain_text`（还原原文，给「阅读视图 / 复制全文」）、`links`（按出现顺序列出所有链接：文本里的网址 + 文字链接的隐藏地址和文字，给「列出本条链接」）。

api 清单 v0.1 列的读法要求——连续同一发送人、重名、以频道 / 匿名管理员发言、名字未到、连续相同 emoji 合并——**都是渲染器的事**（原列的「简短读法里表情回应只读总数」「长消息截断」已被人定的读法原则取代：表情回应进 `detail`，不截断，见第九节），树里的数据已齐（发送人带 id、名字可空、表情回应带种类和次数）。「连续同一发送人」要看前一条，读法的输入届时从「一个 Doc」扩成「Doc + 前一条的发送人」，树不变。

**加一种新节点，谁来提醒**：

1. **编译器**：所有枚举都**不加** `#[non_exhaustive]`（加了反而逼下游写 `_ =>`，就提醒不到了）。`atg-cli`、`atg-gui` 在同一个 workspace，新变体一加，每个没处理它的渲染器都编不过
2. **lint**：渲染器模块顶上 `#![deny(clippy::wildcard_enum_match_arm)]`，防有人图省事写 `_ =>`（`speech.rs`、`tests/coverage.rs` 已加）。CI 的 clippy 用 `-D warnings`
3. **测试**：`tests/coverage.rs` 里每个枚举一个「变体 → 名字」的穷尽 `match` 加一张 `ALL_*` 清单；夹具里缺哪种就失败。于是加新节点必须补一条夹具，它就会出现在**所有**渲染器的快照里，PR 的快照差异一眼可见
4. **JSON** 用 `serde` 派生，新节点自动有 JSON 形状；但它的形状变化同样落在快照里

## 八、JSON 与版本

- 外壳（与 infra 2026-10-25 对齐，**以 infra 的 CLI 契约为准**）：`{"v":1,"kind":"message|chat|event|…","data":<树>,"speech":{"brief":"…"}}`，`at` / `seq` 由 CLI 循环填。读法在外壳的 `speech` 里，不进树
- 判别字段一律叫 `type`，值 snake_case；树里别的字段不叫 `type`（所以表情回应的种类叫 `kind`）
- **消息身份一律嵌套成一个对象** `{"chat_id": …, "message_id": …}`，不平铺、不拼字符串：顶层 `message` / `service` / `tombstone` 的 `id`、回复的 `target`、置顶的 `pinned` 都是这个形状
- id：会话、消息、用户 id 都是 int53，用数字；超过 2^53 的 int64（`custom_emoji_id`、`album_id`）写字符串
- 时间：结构体里存 Unix 秒，JSON 里写 RFC3339 UTC（如 `2026-10-23T11:30:00Z`）；读法按调用方给的时区偏移转本地钟点
- 空的可选字段不输出（`reply_to`、`forward`、`attachment`、`edited`、`delivery`、空的 `reactions`）
- **版本**：外壳和树共用一个 `v`（代码里 `tree::SCHEMA_V`）。
  - **不加一**：新增可选字段、新增节点种类、新增 `td_type` 值；**某种内容从 `other` / `unknown` 升级成专门节点**（如骰子从 `attachment.type = "other", td_type = "messageDice"` 变成 `"dice"`）也不加——使用方本来就得把不认识的 `type` 当未知处理
  - **给取值固定的字段加新取值**（如 `status` 加 `stopping`）也**不加一**（拍板 006 乙10）——使用方遇到不认识的值按「未知」处理、读 `speech.brief` 兜底，与「新增节点种类」同理
  - **加一**：改字段名、删字段、改字段含义、改判别值（把已有取值改名或改义）
  - **使用方的义务**（写进 CLI 文档）：忽略不认识的字段；遇到不认识的 `type` 当 `unknown` 处理、读 `speech.brief` 兜底。这样加节点不破坏老的使用方

## 九、读法 v0（`brief`；**原则人定 2026-10-25，具体说法待人实听定标**）

**说法的正本只有一处：快照文件**（`src/atg-core/tests/snapshots/*brief*.snap`）；人定标后搬进 `specs/style/`（架构方案第三节）。这一节只记规则，不抄句子；别的规约（如 infra 的 `errors.md`）要引用说法时，指到这里。形状照架构方案第三节：消息 =「发送人：内容，时间」，不读位置。

**原则（人定，`docs/plan/003_decisions-b2.md`「读法」）**：读出声的始终是 VoiceOver，本项目只决定每项的名称文字。**名称（`brief`）只放主要内容：谁发的、说了什么、几点**；回复谁、转发自谁、已编辑、表情回应、日期等一律进 `detail`，人按详情键展开。**不截断、不设时长上限**——听够了人自己用 Control 打断或按 ↓ 走。人的原话：「为啥要读完a / 这个读完是啥意思 / 我觉得直接我自己用 voice over 读就好呀」「对 / 然后就是消息信息，如果要读这些东西，我可以用专门的详情来展开看」。

要点：

- 自己发的读「我」；名字查不到一律读「未知发送人」；不认识或没做读法的内容一律读「暂不支持的消息」——**英文类型名不读**，只进 JSON
- 格式（粗斜下删）不读；**剧透、18+ 遮挡统一读「隐藏内容，按 D 展开」**（R1 人定），两边用逗号隔开（VoiceOver 不念括号）；键由调用方给（同提醒的「按 M 收听」），没有键的调用方（CLI）只读「隐藏内容」；D 是否就是详情键，GUI 阶段定键位时一并定；链接只读文字不读地址；代码照读文字
- **时间只读钟点**（R3）：日期进 `detail`。**回复谁、转发自谁**（R4）、**已编辑、表情回应**（R5）都不进 `brief`，进 `detail`
- **相册每张一项**（R6），GUI 阶段再看
- **撤回占位**（R7）：句式「〈发送人〉撤回了一条消息，〈钟点〉」，与消息句同形——发送人、时间取撤回前记下的（第三节第 1 小节），发送人沿用「我」「未知发送人」的规则（记不到发送人读「未知发送人撤回了一条消息，〈钟点〉」）。读的是**原作者**——TDLib 不给删除人，管理员删别人的消息也读成原作者撤回，接受；**会话项**里的占位不带时间（同会话项最后一条的规则）。实听再定（N8 人定）
- 一律「类型，内容」用逗号隔开（空格停顿太短）：「图片，山顶的风景」「文件，路线.gpx」「投票，几点出发？」；时长读成「12 秒」「1 分 5 秒」；圆视频叫「圆形视频」，免得和普通视频混
- 正文末尾的句号、逗号去掉（后面还要接「，时间」，免得读成「。，」）；问号、叹号带语气，留着
- 会话项：「名字，N 条未读，K 条提到你，最后一条」，最后一条不读时间、**不截断**（R2 人定，按原则；原 40 字截断撤销，`specs/tasks.md` T02「约 3 秒」判据已删）；私聊里对方发的、频道以自己名义发的，不重复读名字
- 提醒：「2 条新消息，来自林小雨，按 M 收听」（键由调用方给，CLI 不读键）

## 十、待人拍板

> ✅ **已拍板（2026-10-25）**：结果见 `docs/plan/003_decisions-b2.md`——R1 C · R2 不截断 · R3 只读钟点 · R4、R5 进详情 · R6 每张一项 · R7 B。已落进第九节；下面的题面（含当时的建议）保留作依据。

读法的全部说法都在快照 `src/atg-core/tests/snapshots/*brief*.snap` 里，人可以直接在快照上改字，改定的那版就是期望值（架构方案第六节第 7 块）。下面是最值得先定的几处：

1. **隐藏内容怎么读**：A. 只读「隐藏内容」（现做法）　B. 照读内容　C. 读「隐藏内容，按 D 展开」（多约 1 秒）。建议 A，展开放 `detail`
2. **会话项「最后一条」截多长**：现 40 字，频道那项整句约 50 字、要读 8–10 秒，超过 T02 的约 3 秒。A. 40 字　B. 20 字（约 3–4 秒）　C. 30 字。代价：越短越容易截在半句话上。建议 B 试听
3. **时间怎么读**：现只读「19:30」。A. 只读钟点　B. 今天读钟点，昨天读「昨天 19:30」，更早读「10 月 21 日」（多约 1 秒）　C. 同一分钟里连续几条只在第一条读时间。建议 B，C 等读法能看前一条时再加
4. **回复、转发读不读**：架构方案把「回复谁」「转发自谁」放 `detail`；但真难题 2 说漏了「这是回复谁的」就看不懂（快照里「林小雨：跨群回复」听不出回复谁）。A. 都不读（现做法）　B. 开头加「回复周大山，」「转发自示例频道，」，每条多 4–6 字、约 1 秒。建议先 B 试听
5. **已编辑、表情回应读不读**：现都不读（放 `detail`）。A. 不读　B. 末尾加「已编辑」　C. 末尾加「3 个回应」（只读总数，api 清单 M1.5 的建议）。代价同上，每项约 1 秒。建议 B、C 都先不加，听过回复、转发那条再定
6. **相册**：A. 每张一项（现做法，焦点身份最简单）　B. 合成一项「相册，3 张图片」。建议 A，等 GUI 阶段再看
7. **已撤回的占位读什么**：现在只读「已撤回的消息」，没有发送人和时间，与别的句子形状不同。A. 保持　B. 「林小雨撤回了一条消息，19:36」（要在撤回前记下发送人和时间，store 多存一点）。建议 B

## 修订记录

- 2026-10-25 初稿（tree），收 api v0.1 / v0.2
- 2026-10-25 按 b2 拍板同步（sync）：第九节写入人定读法原则与 R1–R7；`tombstone` 只认 `is_permanent` 且非 `from_cache`，并带撤回前的 `sender`、`date`；`UserStatus` 加 `stopping`（infra 转来）。快照与代码未改（S0 暂停）
- 2026-10-25 按 b3 拍板（`docs/plan/005_decisions-b3.md`）同步（sync）：R7 撤回占位细节按 N8
- 2026-10-25 按 b7 拍板（`docs/plan/006_decisions-b7.md`）同步（model，大总管授权）：第二节投影函数改写成实际的四个（乙9）；第八节补「给取值固定的字段加新取值不加一」（乙10）；第三节第 2 小节受限规则系统消息也适用，原文本就没写例外，无需改（乙12）；第三节第 2 小节补「被回复那条受限则引用不出」（乙11）与回复对象跟随换 id
