//! 内容树（AST）：一切要被读出来或打印出来的东西，都先变成这里的一棵有类型的树。
//!
//! 规矩（详见 `specs/content-tree.md`）：
//! - **树只放数据，不放怎么读**。读法（`speech`）、CLI 的 JSON、GUI、无障碍名称都是渲染器
//! - **每个对应 TDLib 开放集合的枚举都有一个「未知 / 泛化」变体**，带原类型名 `td_type`，绝不崩
//! - 枚举**不加** `#[non_exhaustive]`：同一 workspace 里的渲染器要在加新变体时编不过，被编译器提醒
//! - 时间在结构体里存 Unix 秒；JSON 里写成 RFC3339 UTC
//!
//! 分两层：
//! - [`Content`]（正文 + 附件）只取决于消息本身，收到消息时建一次，存进 model；`updateMessageContent` 整块替换
//! - [`Doc`] 是渲染时从 model + store 现投影出来的（名字、已读状态都可能晚到），不存

pub mod text;
mod time;

use serde::Serialize;

/// JSON 外壳的版本号（外壳与树共用一个）。只在破坏性变动时加一：改名、删字段、改语义。
/// 新增可选字段、新增节点种类不加——使用方要忽略不认识的字段，把不认识的 `type` 当未知处理。
pub const SCHEMA_V: u32 = 1;

/// Unix 秒。JSON 里写成 RFC3339 UTC。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Time(pub i64);

/// 消息身份。TDLib 的消息 id 只在所属会话内唯一，所以一律用这一对。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct MsgId {
    pub chat_id: i64,
    pub message_id: i64,
}

// ───────────────────────── 顶层：一份「要读 / 要打印」的东西 ─────────────────────────

// 绝大多数 Doc 就是 Message，装箱只会给每条消息多一次分配
#[allow(clippy::large_enum_variant)]
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Doc {
    /// 普通消息（含带媒体的）
    Message(Message),
    /// 系统消息（进群、改名、置顶……）
    Service(Service),
    /// 原位留着的「已撤回」占位（拍板 7）
    Tombstone(Tombstone),
    /// 会话列表的一项
    ChatItem(ChatItem),
    /// 新消息提醒（`notify` 策略产出）
    Alert(Alert),
    /// 用户状态：断网、连接中、同步中、已连接、未登录……（T10）
    Status(Status),
    /// 不认识的顶层对象
    Unknown(Unknown),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unknown {
    pub td_type: String,
}

// ───────────────────────── 消息 ─────────────────────────

#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Message {
    pub id: MsgId,
    pub head: Head,
    pub content: Content,
    pub tail: Tail,
}

#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Head {
    pub sender: Sender,
    pub date: Time,
    pub outgoing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<ReplyTo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forward: Option<Forward>,
    /// 相册：同一相册的几条消息 id 相同。v0 不建父节点，渲染时合并
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_id: Option<String>,
    /// 所在话题。回复、发送时要沿用，否则话题群里可能发错地方
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<Topic>,
    /// 消息在频道里（投影时按会话种类填）。读法据此分开「频道自己发的」和「匿名管理员」。为假时不输出（新增可选字段，`v` 不加一；存下来的树里一律为假，投影时才填）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub in_channel: bool,
}

/// `MessageTopic`（`td_api.tl:3422-3431`）
#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Topic {
    Thread {
        thread_id: i64,
    },
    /// 论坛话题；名字投影时能查到才有
    Forum {
        topic_id: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    DirectMessages {
        topic_id: i64,
    },
    SavedMessages {
        topic_id: i64,
    },
    Unknown {
        td_type: String,
    },
}

/// 发送人。名字在投影时从用户 / 会话缓存查；查不到是 `None`，读法给兜底说法。
#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Sender {
    User {
        user_id: i64,
        name: Option<String>,
        /// 已注销的账号（`userTypeDeleted`，投影时填）。为假时不输出（新增可选字段，`v` 不加一）
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        deleted: bool,
    },
    Chat {
        chat_id: i64,
        title: Option<String>,
    },
    Unknown {
        td_type: String,
    },
}

#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReplyTo {
    /// 回复一条消息。`target.chat_id` 可能是别的会话；被回复的那条不一定在 store 里
    Message {
        target: MsgId,
        /// 被回复那条的发送人（投影时能查到才有）
        #[serde(skip_serializing_if = "Option::is_none")]
        sender: Option<Sender>,
        /// 只引了一段时的那段（TDLib `quote`）
        #[serde(skip_serializing_if = "Option::is_none")]
        quote: Option<Vec<Span>>,
    },
    Unknown {
        td_type: String,
    },
}

#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Forward {
    pub origin: Origin,
    pub date: Time,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Origin {
    User {
        user_id: i64,
        name: Option<String>,
    },
    /// 隐藏了账号的用户：只有一个名字字符串
    HiddenUser {
        name: String,
    },
    Chat {
        chat_id: i64,
        title: Option<String>,
    },
    Channel {
        chat_id: i64,
        title: Option<String>,
    },
    Unknown {
        td_type: String,
    },
}

/// 正文 + 附件。只取决于消息本身，可以存进 model。
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Content {
    /// 文本消息的正文，或媒体的说明文字；没有就是空
    pub body: Vec<Span>,
    /// TDLib 一条消息至多一个媒体（相册是多条消息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment: Option<Attachment>,
    /// 媒体要按 18+ 遮挡（`restrictionInfo.has_sensitive_content`，`td_api.tl:2738`）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub sensitive: bool,
}

#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Attachment {
    Photo {
        spoiler: bool,
    },
    Video {
        duration: u32,
        spoiler: bool,
    },
    /// 语音
    Voice {
        duration: u32,
        listened: bool,
    },
    /// 圆视频
    VideoNote {
        duration: u32,
        viewed: bool,
    },
    Audio {
        duration: u32,
        title: String,
        performer: String,
    },
    /// GIF
    Animation {
        duration: u32,
    },
    Document {
        file_name: String,
        /// 字节；不知道是 0
        size: u64,
    },
    Sticker {
        emoji: String,
    },
    /// 单个 emoji 的大表情消息
    AnimatedEmoji {
        emoji: String,
    },
    Location,
    Venue {
        title: String,
    },
    Contact {
        name: String,
    },
    Poll {
        question: Vec<Span>,
    },
    Call {
        video: bool,
        duration: u32,
        missed: bool,
    },
    /// 阅后即焚已过期的图片 / 视频……
    Expired {
        what: Expirable,
    },
    /// 内容受限（`restrictionInfo.restriction_reason` 非空，`td_api.tl:2738`）：
    /// 原内容不进树，正文为空，只留原因
    Restricted {
        reason: String,
    },
    /// 认识但还没做专门节点的内容（第二批、不做的那些）：读出类型
    Other {
        td_type: String,
    },
    /// `messageUnsupported`，或不认识的类型
    Unknown {
        td_type: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Expirable {
    Photo,
    Video,
    VideoNote,
    Voice,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited: Option<Time>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<Reaction>,
    /// 只有自己发的才有
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery: Option<Delivery>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Reaction {
    pub kind: ReactionKind,
    pub count: u32,
    /// 我自己点了没有
    pub chosen: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReactionKind {
    Emoji {
        emoji: String,
    },
    /// int64，JSON 里写字符串
    CustomEmoji {
        custom_emoji_id: String,
    },
    Paid,
    Unknown {
        td_type: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Pending,
    Failed,
    Sent,
    Read,
}

// ───────────────────────── 正文：实体建成的行内树 ─────────────────────────

/// 正文的一段。由 [`text::build`] 从「纯文本 + UTF-16 实体」建成。
///
/// 三类：只有文字的叶子（`Text` `Code` `Pre` `DateTime`）· 格式（粗斜下删剧透，可嵌任何东西）·
/// 其余实体（带 `children`，里面只许有格式）。
#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Span {
    Text {
        text: String,
    },
    // 叶子：里面不许有别的实体
    Code {
        text: String,
    },
    Pre {
        #[serde(skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        text: String,
    },
    DateTime {
        unix_time: Time,
        text: String,
    },
    // 格式
    Bold {
        children: Vec<Span>,
    },
    Italic {
        children: Vec<Span>,
    },
    Underline {
        children: Vec<Span>,
    },
    Strikethrough {
        children: Vec<Span>,
    },
    Spoiler {
        children: Vec<Span>,
    },
    // 引用块：不许套引用块
    Quote {
        expandable: bool,
        children: Vec<Span>,
    },
    // 其余实体
    /// 文字链接：地址不在文本里
    Link {
        url: String,
        children: Vec<Span>,
    },
    /// 文本里直接写出来的网址
    Url {
        children: Vec<Span>,
    },
    /// @用户名
    Mention {
        children: Vec<Span>,
    },
    /// 没有用户名的提及：user_id 不在文本里
    MentionName {
        user_id: i64,
        children: Vec<Span>,
    },
    Hashtag {
        children: Vec<Span>,
    },
    Cashtag {
        children: Vec<Span>,
    },
    BotCommand {
        children: Vec<Span>,
    },
    Email {
        children: Vec<Span>,
    },
    Phone {
        children: Vec<Span>,
    },
    BankCard {
        children: Vec<Span>,
    },
    TonAddress {
        children: Vec<Span>,
    },
    /// 自定义表情：文本里是替代 emoji
    CustomEmoji {
        custom_emoji_id: String,
        children: Vec<Span>,
    },
    MediaTimestamp {
        seconds: u32,
        children: Vec<Span>,
    },
    /// 不认识的实体：文字照留
    Unknown {
        td_type: String,
        children: Vec<Span>,
    },
}

// ───────────────────────── 系统消息 ─────────────────────────

#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Service {
    pub id: MsgId,
    pub date: Time,
    /// 谁做的
    pub actor: Sender,
    pub action: Action,
    /// 自己做的（读「我」）。为假时不输出（新增可选字段，`v` 不加一）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub outgoing: bool,
    /// 发生在频道里（读「频道名」「频道头像」而不是「群名」「群头像」）。投影时按会话种类填；为假时不输出（新增可选字段，`v` 不加一；存下来的树里一律为假，投影时才填）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub in_channel: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct UserRef {
    pub user_id: i64,
    pub name: Option<String>,
    /// 就是自己（投影时按 `my_id` 填）。为假时不输出（新增可选字段，`v` 不加一；存下来的树里一律为假，投影时才填）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_me: bool,
    /// 已注销的账号（`userTypeDeleted`，投影时填）。为假时不输出（新增可选字段，`v` 不加一；存下来的树里一律为假，投影时才填）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
}

#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    MembersAdded {
        users: Vec<UserRef>,
    },
    JoinedByLink,
    JoinedByRequest,
    /// 被移出或自己退出（`user` 与 actor 相同就是自己退出）
    MemberRemoved {
        user: UserRef,
    },
    TitleChanged {
        title: String,
    },
    PhotoChanged,
    PhotoDeleted,
    /// `pinned` 是被置顶的那条；`content` 是它的有效内容（投影时从 store 查，查不到为空；
    /// 存下来的树里一律为空——同「名字投影时填」）。为空时不输出（新增可选字段，`v` 不加一）
    Pinned {
        pinned: MsgId,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<Box<Content>>,
    },
    GroupCreated {
        title: String,
    },
    ContactRegistered,
    /// 认识但还没做专门节点的系统消息：读出类型
    Other {
        td_type: String,
    },
}

#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Tombstone {
    pub id: MsgId,
    /// 撤回前记下的发送人（名字投影时填）；没见过原消息就撤回的为空
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender: Option<Sender>,
    /// 撤回前记下的发送时间（读法 R7「某某撤回了一条消息，19:37」用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<Time>,
    /// 撤回前那条是自己发的（R7 读「我撤回了一条消息」）。为假时不输出（新增可选字段，`v` 不加一）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub outgoing: bool,
}

// ───────────────────────── 会话项 · 提醒 · 连接状态 ─────────────────────────

#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct ChatItem {
    pub chat_id: i64,
    pub title: String,
    pub kind: ChatKind,
    pub unread_count: u32,
    pub unread_mention_count: u32,
    /// 有效静音：会话自己的设置，或 `use_default_mute_for` 时继承的范围设置，由投影算好
    pub muted: bool,
    /// `muted` 是不是继承来的
    pub mute_inherited: bool,
    pub pinned: bool,
    /// 私聊、加密聊天的对方是已注销的账号（`userTypeDeleted`）。为假时不输出（新增可选字段，`v` 不加一）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub peer_deleted: bool,
    /// 收藏夹：和自己的私聊（TDLib 给的标题是自己的名字）。为假时不输出（新增可选字段，`v` 不加一）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub saved_messages: bool,
    /// 最后一条
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<Box<ChatLast>>,
}

/// 会话项的「最后一条」只可能是这三种（JSON `type` 与 [`Doc`] 的同名变体一致）
#[allow(clippy::large_enum_variant)]
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatLast {
    Message(Message),
    Service(Service),
    Tombstone(Tombstone),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatKind {
    Private,
    Group,
    Supergroup,
    Channel,
    Secret,
    Unknown { td_type: String },
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Alert {
    pub total: u32,
    pub chats: Vec<AlertChat>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct AlertChat {
    pub chat_id: i64,
    pub title: Option<String>,
    pub count: u32,
}

/// 用户状态：内核、授权、连接三条线合成的一个值（合成规则归 `specs/contracts/errors.md`
/// 第一节第 3 小节）。读法只读这个，不直接读 TDLib 的连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Status {
    pub status: UserStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Fault,
    /// 内核或授权在关闭中 / 已关闭：人自己关的，不播报（infra 新增，`errors.md` 第一节第 3 小节）
    Stopping,
    SignedOut,
    SigningIn,
    UnsupportedStep,
    Offline,
    Connecting,
    Syncing,
    Online,
}
