//! `model`：本项目自己的数据类型（会话、消息、用户……）与喂给 [`crate::store`] 的事件 [`CoreEvent`]。
//!
//! 规矩（`docs/plan/001_arch-stack.md` 第一节第 2、3 小节；`specs/content-tree.md` 第二节）：
//! - 消息身份一律用 [`MsgId`]（会话 id + 消息 id）：TDLib 的消息 id 只在所属会话内唯一
//! - 消息正文直接用内容树的 [`Content`]（系统消息用 [`Action`]），不另定义「格式化文本」
//! - **存下来的东西只放 id，名字一律投影时填**（[`crate::project`]）：发送人、转发来源、系统消息里提到的人
//! - 不认识的 TDLib 类型落带 `td_type` 的变体，不崩
//!
//! 这里的类型不带任何 tdlib-rs 类型。映射（TDLib JSON → model）只有一份，在 [`crate::tdjson`]：
//! 真实后端（`td`）把 tdlib-rs 的更新序列化回 TDLib JSON 交给它，夹具也走它。**别在 `td` 里另写映射**

pub use crate::tree::{
    Action, Attachment, ChatKind, Content, MsgId, Reaction, Span, Time, Topic, UserStatus,
};

pub type ChatId = i64;
pub type UserId = i64;

// ───────────────────────── 用户 ─────────────────────────

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub struct User {
    pub id: UserId,
    pub first_name: String,
    pub last_name: String,
}

impl User {
    /// 「名 姓」，缺哪个省哪个
    pub fn name(&self) -> String {
        match (self.first_name.is_empty(), self.last_name.is_empty()) {
            (false, false) => format!("{} {}", self.first_name, self.last_name),
            (false, true) => self.first_name.clone(),
            (true, false) => self.last_name.clone(),
            (true, true) => String::new(),
        }
    }
}

// ───────────────────────── 消息 ─────────────────────────

/// 发送人（只有 id，名字投影时查）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SenderId {
    User(UserId),
    Chat(ChatId),
    Unknown { td_type: String },
}

/// 转发来源（只有 id；隐藏账号的用户只有一个名字字符串，TDLib 就只给这个）
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub enum OriginId {
    User(UserId),
    HiddenUser { name: String },
    Chat(ChatId),
    Channel(ChatId),
    Unknown { td_type: String },
}

#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub enum ReplyRef {
    Message {
        target: MsgId,
        quote: Option<Vec<Span>>,
        /// TDLib 附带的被回复那条的来源；被回复那条不在 store 里时拿它兜底发送人
        origin: Option<OriginId>,
    },
    Unknown {
        td_type: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForwardRef {
    pub origin: OriginId,
    pub date: Time,
}

/// 一条消息的「内容」：普通内容或系统消息。两者都只取决于消息本身，存进 model
#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub enum Body {
    Content(Content),
    Service(Action),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendingState {
    /// 已在服务器上（自己发的，读没读由会话的 `last_read_outbox` 判）
    Sent,
    Pending,
    Failed,
}

#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub struct Message {
    pub id: MsgId,
    pub sender: SenderId,
    pub date: Time,
    pub outgoing: bool,
    pub reply_to: Option<ReplyRef>,
    pub forward: Option<ForwardRef>,
    pub album_id: Option<String>,
    pub topic: Option<Topic>,
    /// 原内容。受限时这里放的已是受限节点，原文不留（见 [`Message::enforce`]）
    pub content: Body,
    /// 临时内容（`ephemeral_content`，`td_api.tl:3594`「必须代替原内容显示」）
    pub ephemeral: Option<Body>,
    /// `restriction_info.restriction_reason` 非空时的原因
    pub restriction: Option<String>,
    /// `restriction_info.has_sensitive_content`
    pub sensitive: bool,
    /// 只由 `message_edited` 改（不拿它比内容新旧）
    pub edited: Option<Time>,
    pub reactions: Vec<Reaction>,
    pub sending: SendingState,
}

impl Message {
    /// 有效内容：有临时内容就用它
    pub fn effective(&self) -> &Body {
        self.ephemeral.as_ref().unwrap_or(&self.content)
    }

    /// 把受限、敏感两条规则落到存下来的内容上。store 每次写入内容后都调它，所以幂等。
    /// - 受限（原因非空）：原内容、临时内容整块换成 `restricted{reason}`，原文不留——
    ///   系统消息也一样（改名、建群带着原标题）
    /// - 只有敏感标记：照常存，`Content.sensitive = true`
    pub fn enforce(&mut self) {
        let sensitive = self.sensitive;
        let restricted = self.restriction.as_ref().map(|reason| Content {
            body: vec![],
            attachment: Some(Attachment::Restricted {
                reason: reason.clone(),
            }),
            sensitive,
        });
        for body in std::iter::once(&mut self.content).chain(self.ephemeral.as_mut()) {
            match (&restricted, &mut *body) {
                (Some(r), _) => *body = Body::Content(r.clone()),
                (None, Body::Content(c)) => c.sensitive = sensitive,
                (None, Body::Service(_)) => {}
            }
        }
    }

    /// `updateMessageContentOpened`：只改原内容的「已听 / 已看」。临时内容不动——TDLib 只改
    /// `m->content`（`MessagesManager.cpp:9300-9312`），临时内容的状态随它自己的更新维护
    pub fn mark_opened(&mut self) {
        self.content.mark_opened();
    }
}

impl Body {
    /// 把这份内容里的语音标成已听、圆视频标成已看；别的内容不变
    pub fn mark_opened(&mut self) {
        let Body::Content(Content {
            attachment: Some(a),
            ..
        }) = self
        else {
            return;
        };
        match a {
            Attachment::Voice { listened, .. } => *listened = true,
            Attachment::VideoNote { viewed, .. } => *viewed = true,
            Attachment::Photo { .. }
            | Attachment::Video { .. }
            | Attachment::Audio { .. }
            | Attachment::Animation { .. }
            | Attachment::Document { .. }
            | Attachment::Sticker { .. }
            | Attachment::AnimatedEmoji { .. }
            | Attachment::Location
            | Attachment::Venue { .. }
            | Attachment::Contact { .. }
            | Attachment::Poll { .. }
            | Attachment::Call { .. }
            | Attachment::Expired { .. }
            | Attachment::Restricted { .. }
            | Attachment::Other { .. }
            | Attachment::Unknown { .. } => {}
        }
    }
}

// ───────────────────────── 会话 ─────────────────────────

/// 会话列表的种类（`ChatList`，`td_api.tl`）
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChatList {
    Main,
    Archive,
    Folder(i32),
    Unknown { td_type: String },
}

/// 会话在某个列表里的位置。TDLib 规则：按 (order, 会话 id) 从大到小排；order 为 0 = 不在这个列表里
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatPosition {
    pub list: ChatList,
    pub order: i64,
    pub is_pinned: bool,
}

/// 会话自己的通知设置（只取静音相关的两项）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChatNotify {
    /// 跟随范围设置
    pub use_default_mute_for: bool,
    /// 还要静音多少秒；0 = 不静音
    pub mute_for: i64,
}

/// 通知范围（`NotificationSettingsScope`）：会话「跟随默认」时取所属范围的设置
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotifyScope {
    Private,
    Group,
    Channel,
}

impl NotifyScope {
    /// 会话种类 → 所属范围；不认识的种类没有范围
    pub fn of(kind: &ChatKind) -> Option<Self> {
        match kind {
            ChatKind::Private | ChatKind::Secret => Some(Self::Private),
            ChatKind::Group | ChatKind::Supergroup => Some(Self::Group),
            ChatKind::Channel => Some(Self::Channel),
            ChatKind::Unknown { .. } => None,
        }
    }
}

#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub struct Chat {
    pub id: ChatId,
    pub title: String,
    pub kind: ChatKind,
    pub positions: Vec<ChatPosition>,
    pub unread_count: u32,
    pub unread_mention_count: u32,
    pub last_read_inbox: i64,
    pub last_read_outbox: i64,
    pub notify: ChatNotify,
    /// 最后一条的消息 id（消息本身存在 store 的消息表里）。由 store 维护，映射时留空
    pub last_message: Option<i64>,
}

// ───────────────────────── 事件 ─────────────────────────

/// 收线程把 TDLib 更新映射成的事件（`specs/contracts/actions.md` 第一节第 7 小节，取 store 要的那些）。
/// 带序号的外壳在 [`crate::store::Input::Update`]。
#[allow(clippy::large_enum_variant)]
#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub enum CoreEvent {
    /// `updateNewMessage`
    NewMessage(Message),
    /// `updateMessageContent`：整块替换原内容
    MessageContentChanged { id: MsgId, content: Body },
    /// `updateMessageEphemeralContent`：临时内容出现（`Some`）或消失（`None`）
    MessageEphemeralChanged { id: MsgId, ephemeral: Option<Body> },
    /// `updateMessageEdited`：只带编辑时间
    MessageEdited { id: MsgId, edit_date: Time },
    /// `updateMessageContentOpened`：已听 / 已看
    MessageContentOpened { id: MsgId },
    /// `updateMessageInteractionInfo`：表情回应
    MessageInteractionChanged { id: MsgId, reactions: Vec<Reaction> },
    /// `updateDeleteMessages`。只有 `is_permanent && !from_cache` 才是撤回
    MessagesDeleted {
        chat_id: ChatId,
        message_ids: Vec<i64>,
        is_permanent: bool,
        from_cache: bool,
    },
    /// `updateMessageSendSucceeded`：临时 id → 服务器 id（`message` 是发出后的那份）
    SendSucceeded { old_id: MsgId, message: Message },
    /// `updateMessageSendFailed`：同样换 id（TDLib 给失败的那条也分新 id）
    SendFailed {
        old_id: MsgId,
        message: Message,
        /// 失败原因，已过 [`crate::error::map_td_error`]（store 不用）
        error: crate::error::CoreError,
        /// 能否重发（`messageSendingStateFailed.can_retry`，`actions.md` 事件 `send_failed`）
        can_retry: bool,
    },
    /// `updateNewChat`
    ChatAdded {
        chat: Chat,
        last_message: Option<Message>,
    },
    /// `updateChatPosition`：替换这个列表里的位置；order 为 0 = 移出该列表
    ChatPositionChanged {
        chat_id: ChatId,
        position: ChatPosition,
    },
    /// `updateChatLastMessage`。`positions` 为 `Some` 时整体替换会话的全部位置
    /// （TDLib 总会带；`None` 只给缺字段的夹具用：不动位置）
    ChatLastMessageChanged {
        chat_id: ChatId,
        last_message: Option<Message>,
        positions: Option<Vec<ChatPosition>>,
    },
    /// `updateChatDraftMessage`：只取它带的位置，整体替换（TDLib 可能用它代替 `updateChatPosition`）。
    /// 草稿本身不收（第一阶段不显示草稿；它是内容）
    ChatDraftChanged {
        chat_id: ChatId,
        positions: Vec<ChatPosition>,
    },
    /// `updateChatReadInbox`
    ChatReadInbox {
        chat_id: ChatId,
        last_read_inbox_message_id: i64,
        unread_count: u32,
    },
    /// `updateChatReadOutbox`
    ChatReadOutbox {
        chat_id: ChatId,
        last_read_outbox_message_id: i64,
    },
    /// `updateChatUnreadMentionCount`
    ChatMentionCountChanged { chat_id: ChatId, count: u32 },
    /// `updateChatTitle`
    ChatTitleChanged { chat_id: ChatId, title: String },
    /// `updateChatNotificationSettings`
    ChatNotificationChanged { chat_id: ChatId, notify: ChatNotify },
    /// `updateScopeNotificationSettings`（只取 `mute_for`）
    ScopeNotificationChanged { scope: NotifyScope, mute_for: i64 },
    /// `updateUser`
    UserChanged(User),
    /// 合成后的用户状态（`crate::status::user_status`，`errors.md` 第一节第 3 小节）
    UserStatusChanged(UserStatus),
    /// 授权线变了（`auth_step_changed`；不带提示语）。由收线程从 `updateAuthorizationState` 合成
    AuthStepChanged(crate::status::AuthStep),
    /// 连接线变了（`net_state_changed`）。由收线程从 `updateConnectionState` 合成
    NetStateChanged(crate::status::NetState),
    /// 内核线变了（`core_state_changed`）：收线程监督产出
    CoreStateChanged(crate::status::CoreState),
    /// 不认识的更新：只有类型名
    Unknown { td_type: String },
}
