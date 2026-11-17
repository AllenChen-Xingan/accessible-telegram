//! TDLib JSON → [`CoreEvent`] 的映射（正式路径与夹具共用；入口 [`classify`]），外加夹具专用的 [`World`]。
//! 夹具（fixture）= 手写的假 TDLib 数据，测试用。
//!
//! **正式路径也用这里的映射**：`td` 把 tdlib-rs 的 `Update` 原样序列化回 TDLib JSON，交给 [`classify`]
//! （第一刀第 3 块定，免得维护两份映射）。所以快照测的就是正式的映射、reducer 和投影；
//! 只有 [`World`]（夹具按顺序编序号喂 store）是夹具专用的。
//!
//! 夹具 = 一个 JSON 数组，元素是 TDLib 的更新对象，按顺序喂进来（`updateUser`、`updateNewChat`、
//! `updateNewMessage`……），序号从 1 起递增。**建树在 [`World::docs`] 被调用时才做**——
//! 所以用户信息比消息晚到也能查到名字。
//!
//! 一律用 `serde_json::Value` 逐字段取，字段缺了取默认值，不认识的类型落「未知」，绝不 panic。

use crate::error::map_td_error;
use crate::model::{
    Body, Chat, ChatActionKind, ChatList, ChatNotify, ChatPosition, CoreEvent, ForumTopicState,
    ForwardRef, Message, NotifyScope, OriginId, ReplyRef, ScopeNotify, SenderId, SendingState,
    User,
};
use crate::project;
use crate::status::{AuthStep, CoreState, Lines, NetState};
use crate::store::{Input, Store};
use crate::tree::text::{self, Entity, EntityKind};
use crate::tree::*;
use serde_json::Value;

mod file;
mod react;
use react::reactions;
pub use react::{added_reactions, reaction_type};
// 消息的映射（`message`、正文、发送者、带格式文字）在 `message.rs`
mod message;
use message::{body, ephemeral, sender};
pub use message::{formatted, message};

/// 按到达顺序记下的「会产出 Doc 的东西」
#[allow(clippy::large_enum_variant)]
enum Item {
    /// 一条消息：到 `docs()` 时才投影（撤回的投影成原位占位）
    Msg(MsgId),
    /// 连接状态、不认识的更新：到达时就定了，不依赖晚到的数据
    Doc(Doc),
}

/// 一份夹具喂完之后的「世界」：reducer 的状态 + 按到达顺序的条目
pub struct World {
    store: Store,
    order: Vec<Item>,
    /// 三条线。夹具假定内核在跑、已登录（原型就是这样），连接 / 授权更新照正式规则合成状态
    lines: Lines,
}

impl Default for World {
    fn default() -> Self {
        Self {
            store: Store::default(),
            order: Vec::new(),
            lines: Lines {
                core: CoreState::Running,
                auth: AuthStep::Ready,
                net: NetState::Connecting,
            },
        }
    }
}

impl World {
    pub fn from_json(src: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(src).map_err(|e| format!("夹具不是合法 JSON：{e}"))?;
        let Value::Array(items) = v else {
            return Err("夹具顶层必须是数组".into());
        };
        let mut w = World::default();
        for it in &items {
            w.apply(it);
        }
        Ok(w)
    }

    /// 一条 TDLib 更新 → [`classify`] → 事件 → reducer（与正式路径同一个入口；授权、连接两条线照
    /// [`crate::status::user_status`] 合成状态，正式路径在 `link` 里做同样的事）
    pub fn apply(&mut self, v: &Value) {
        let ev = match classify(v) {
            Classified::Event(e) => e,
            Classified::Auth(a) => {
                self.lines.auth = a;
                CoreEvent::UserStatusChanged(self.lines.user_status())
            }
            Classified::Net(n) => {
                self.lines.net = n;
                CoreEvent::UserStatusChanged(self.lines.user_status())
            }
        };
        match &ev {
            CoreEvent::NewMessage(m) => self.order.push(Item::Msg(m.id)),
            // 换了 id 的消息原位沿用新 id
            CoreEvent::SendSucceeded { old_id, message }
            | CoreEvent::SendFailed {
                old_id, message, ..
            } => {
                for it in &mut self.order {
                    if let Item::Msg(x) = it
                        && x == old_id
                    {
                        *x = message.id;
                    }
                }
            }
            CoreEvent::UserStatusChanged(s) => {
                self.order
                    .push(Item::Doc(Doc::Status(Status { status: *s })));
            }
            CoreEvent::Unknown { td_type } => self.order.push(Item::Doc(Doc::Unknown(Unknown {
                td_type: td_type.clone(),
            }))),
            _ => {}
        }
        let seq = self.store.last_seq() + 1;
        self.store.apply(Input::Update { seq, event: ev });
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// 按到达顺序投影出所有 Doc（撤回过的消息原位变成占位）
    pub fn docs(&self) -> Vec<Doc> {
        self.order
            .iter()
            .filter_map(|it| match it {
                Item::Msg(id) => project::message(&self.store, *id),
                Item::Doc(d) => Some(d.clone()),
            })
            .collect()
    }

    /// 主会话列表，按 store 维护的顺序
    pub fn chat_items(&self) -> Vec<Doc> {
        project::chat_list(&self.store, &ChatList::Main)
    }
}

// ───────────────────────── 更新 → 事件 ─────────────────────────

/// 收线程看一条更新时的三种去向：授权线、连接线（这两条由收线程自己维护、合成 [`UserStatus`]），
/// 或交给 store 的事件。正式路径（`td`）与假后端都走这里，所以夹具测过的映射就是正式映射
#[allow(clippy::large_enum_variant)]
pub enum Classified {
    Auth(AuthStep),
    Net(NetState),
    Event(CoreEvent),
}

/// 一条 TDLib 更新对象 → 去向。授权、连接两种截走；其余走 [`event`]
pub fn classify(v: &Value) -> Classified {
    match ty(v) {
        "updateAuthorizationState" => {
            Classified::Auth(AuthStep::from_td_type(ty(&v["authorization_state"])))
        }
        "updateConnectionState" => Classified::Net(NetState::from_td_type(ty(&v["state"]))),
        _ => Classified::Event(event(v)),
    }
}

/// 一条 TDLib 更新对象 → 事件。不认识的落 `Unknown`。授权、连接两种更新由 [`classify`] 先截走，
/// 不经这里（经这里会落 `Unknown`）——外面一律调 [`classify`]
pub fn event(v: &Value) -> CoreEvent {
    let chat_id = || i(v, "chat_id");
    let id = || MsgId {
        chat_id: i(v, "chat_id"),
        message_id: i(v, "message_id"),
    };
    match ty(v) {
        // 只认 `my_id`；别的选项不进 store。值是 `optionValueInteger{value}`（int64，JSON 里是字符串）
        "updateOption" if s(v, "name") == "my_id" => CoreEvent::MyIdChanged {
            user_id: i(&v["value"], "value"),
        },
        "updateUser" => {
            let u = &v["user"];
            CoreEvent::UserChanged(User {
                id: i(u, "id"),
                first_name: s(u, "first_name").to_owned(),
                last_name: s(u, "last_name").to_owned(),
                deleted: ty(&u["type"]) == "userTypeDeleted",
            })
        }
        "updateNewChat" => {
            let c = &v["chat"];
            CoreEvent::ChatAdded {
                chat: chat(c),
                last_message: c["last_message"]
                    .is_object()
                    .then(|| message(&c["last_message"])),
            }
        }
        "updateNewMessage" => CoreEvent::NewMessage(message(&v["message"])),
        "updateMessageContent" => CoreEvent::MessageContentChanged {
            id: id(),
            content: body(&v["new_content"], chat_id()),
        },
        "updateMessageEphemeralContent" => CoreEvent::MessageEphemeralChanged {
            id: id(),
            ephemeral: ephemeral(&v["ephemeral_content"], chat_id()),
        },
        "updateMessageEdited" => CoreEvent::MessageEdited {
            id: id(),
            edit_date: Time(i(v, "edit_date")),
        },
        "updateMessageContentOpened" => CoreEvent::MessageContentOpened { id: id() },
        "updateMessageInteractionInfo" => CoreEvent::MessageInteractionChanged {
            id: id(),
            reactions: reactions(&v["interaction_info"]),
        },
        "updateDeleteMessages" => CoreEvent::MessagesDeleted {
            chat_id: chat_id(),
            message_ids: arr(v, "message_ids").iter().map(as_i64).collect(),
            is_permanent: b(v, "is_permanent"),
            from_cache: b(v, "from_cache"),
        },
        "updateMessageSendSucceeded" | "updateMessageSendFailed" => {
            let m = message(&v["message"]);
            let old_id = MsgId {
                chat_id: m.id.chat_id,
                message_id: i(v, "old_message_id"),
            };
            if ty(v) == "updateMessageSendSucceeded" {
                CoreEvent::SendSucceeded { old_id, message: m }
            } else {
                CoreEvent::SendFailed {
                    old_id,
                    message: m,
                    error: map_td_error(
                        i32::try_from(i(&v["error"], "code")).unwrap_or(0),
                        s(&v["error"], "message"),
                    ),
                    can_retry: b(&v["message"]["sending_state"], "can_retry"),
                }
            }
        }
        "updateChatPosition" => CoreEvent::ChatPositionChanged {
            chat_id: chat_id(),
            position: position(&v["position"]),
        },
        "updateChatLastMessage" => CoreEvent::ChatLastMessageChanged {
            chat_id: chat_id(),
            last_message: v["last_message"]
                .is_object()
                .then(|| message(&v["last_message"])),
            // 夹具可能省掉 positions；省掉 = 不动位置
            positions: v["positions"]
                .as_array()
                .map(|ps| ps.iter().map(position).collect()),
        },
        "updateChatDraftMessage" => CoreEvent::ChatDraftChanged {
            chat_id: chat_id(),
            positions: arr(v, "positions").iter().map(position).collect(),
        },
        "updateChatReadInbox" => CoreEvent::ChatReadInbox {
            chat_id: chat_id(),
            last_read_inbox_message_id: i(v, "last_read_inbox_message_id"),
            unread_count: u(v, "unread_count"),
        },
        "updateChatReadOutbox" => CoreEvent::ChatReadOutbox {
            chat_id: chat_id(),
            last_read_outbox_message_id: i(v, "last_read_outbox_message_id"),
        },
        // 提及被读：TDLib 在消息已通知过客户端时只发这条，不另发 updateChatUnreadMentionCount
        "updateMessageMentionRead" => CoreEvent::ChatMentionCountChanged {
            chat_id: chat_id(),
            count: u(v, "unread_mention_count"),
        },
        "updateChatUnreadMentionCount" => CoreEvent::ChatMentionCountChanged {
            chat_id: chat_id(),
            count: u(v, "unread_mention_count"),
        },
        "updateChatTitle" => CoreEvent::ChatTitleChanged {
            chat_id: chat_id(),
            title: s(v, "title").to_owned(),
        },
        "updateChatNotificationSettings" => CoreEvent::ChatNotificationChanged {
            chat_id: chat_id(),
            notify: notify(&v["notification_settings"]),
        },
        "updateScopeNotificationSettings" => match ty(&v["scope"]) {
            "notificationSettingsScopePrivateChats" => scope_event(v, NotifyScope::Private),
            "notificationSettingsScopeGroupChats" => scope_event(v, NotifyScope::Group),
            "notificationSettingsScopeChannelChats" => scope_event(v, NotifyScope::Channel),
            other => CoreEvent::Unknown {
                td_type: format!("updateScopeNotificationSettings/{other}"),
            },
        },
        "updateForumTopic" => CoreEvent::ForumTopicChanged {
            chat_id: chat_id(),
            topic_id: i(v, "forum_topic_id"),
            last_read_inbox: i(v, "last_read_inbox_message_id"),
            notify: notify(&v["notification_settings"]),
            received_ms: None,
        },
        "updateChatAction" => CoreEvent::ChatAction {
            chat_id: chat_id(),
            topic: topic(&v["topic_id"]),
            sender: sender(&v["sender_id"]),
            action: match ty(&v["action"]) {
                "chatActionTyping" => ChatActionKind::Typing,
                "chatActionCancel" => ChatActionKind::Cancel,
                other => ChatActionKind::Other {
                    td_type: other.to_owned(),
                },
            },
        },
        other => CoreEvent::Unknown {
            td_type: other.to_owned(),
        },
    }
}

fn scope_event(v: &Value, scope: NotifyScope) -> CoreEvent {
    let n = &v["notification_settings"];
    CoreEvent::ScopeNotificationChanged {
        scope,
        notify: ScopeNotify {
            mute_for: i(n, "mute_for"),
            disable_mention: b(n, "disable_mention_notifications"),
            disable_pinned: b(n, "disable_pinned_message_notifications"),
        },
    }
}

fn chat(c: &Value) -> Chat {
    let kind = match ty(&c["type"]) {
        "chatTypePrivate" => ChatKind::Private,
        "chatTypeBasicGroup" => ChatKind::Group,
        "chatTypeSupergroup" => {
            if b(&c["type"], "is_channel") {
                ChatKind::Channel
            } else {
                ChatKind::Supergroup
            }
        }
        "chatTypeSecret" => ChatKind::Secret,
        other => ChatKind::Unknown {
            td_type: other.to_owned(),
        },
    };
    Chat {
        id: i(c, "id"),
        title: s(c, "title").to_owned(),
        kind,
        positions: arr(c, "positions")
            .iter()
            .map(position)
            .filter(|p| p.order != 0)
            .collect(),
        unread_count: u(c, "unread_count"),
        unread_mention_count: u(c, "unread_mention_count"),
        last_read_inbox: i(c, "last_read_inbox_message_id"),
        last_read_outbox: i(c, "last_read_outbox_message_id"),
        notify: notify(&c["notification_settings"]),
        peer_user: match ty(&c["type"]) {
            "chatTypePrivate" | "chatTypeSecret" => Some(i(&c["type"], "user_id")),
            _ => None,
        },
        last_message: None,
    }
}

fn notify(n: &Value) -> ChatNotify {
    ChatNotify {
        use_default_mute_for: b(n, "use_default_mute_for"),
        mute_for: i(n, "mute_for"),
        use_default_disable_mention: b(n, "use_default_disable_mention_notifications"),
        disable_mention: b(n, "disable_mention_notifications"),
        use_default_disable_pinned: b(n, "use_default_disable_pinned_message_notifications"),
        disable_pinned: b(n, "disable_pinned_message_notifications"),
    }
}

/// `forumTopic` 对象（`getForumTopic` 的应答）→ store 要的已读位置与通知设置
pub fn forum_topic(t: &Value) -> ForumTopicState {
    ForumTopicState {
        last_read_inbox: i(t, "last_read_inbox_message_id"),
        notify: notify(&t["notification_settings"]),
    }
}

/// `MessageTopic` → 话题；没有 → `None`
fn topic(t: &Value) -> Option<Topic> {
    match ty(t) {
        "" => None,
        "messageTopicThread" => Some(Topic::Thread {
            thread_id: i(t, "message_thread_id"),
        }),
        // 话题名不在消息里，投影时查（v0 还没有话题缓存，留空）
        "messageTopicForum" => Some(Topic::Forum {
            topic_id: i(t, "forum_topic_id"),
            name: None,
        }),
        "messageTopicDirectMessages" => Some(Topic::DirectMessages {
            topic_id: i(t, "direct_messages_chat_topic_id"),
        }),
        "messageTopicSavedMessages" => Some(Topic::SavedMessages {
            topic_id: i(t, "saved_messages_topic_id"),
        }),
        other => Some(Topic::Unknown {
            td_type: other.to_owned(),
        }),
    }
}

fn position(p: &Value) -> ChatPosition {
    let l = &p["list"];
    ChatPosition {
        list: match ty(l) {
            "chatListMain" => ChatList::Main,
            "chatListArchive" => ChatList::Archive,
            "chatListFolder" => {
                ChatList::Folder(i32::try_from(i(l, "chat_folder_id")).unwrap_or(0))
            }
            other => ChatList::Unknown {
                td_type: other.to_owned(),
            },
        },
        order: i(p, "order"),
        is_pinned: b(p, "is_pinned"),
    }
}

// ───── 发送（M2.1 / M2.2）─────

/// TDLib `chat` 对象 → 会话（`getChat` 应答用；与 `updateNewChat` 同一份映射）
pub fn chat_object(c: &Value) -> Chat {
    chat(c)
}

/// 从 TDLib `message` 对象取失败态
pub fn failed_info(m: &Value) -> Option<crate::model::FailedInfo> {
    let st = &m["sending_state"];
    (ty(st) == "messageSendingStateFailed").then(|| crate::model::FailedInfo {
        can_retry: b(st, "can_retry"),
        retry_after: st["retry_after"].as_f64().unwrap_or(0.0),
        need_another_sender: b(st, "need_another_sender"),
        need_another_reply_quote: b(st, "need_another_reply_quote"),
        need_drop_reply: b(st, "need_drop_reply"),
        required_paid_message_star_count: i(st, "required_paid_message_star_count"),
    })
}

// ───── 取字段的小工具：缺了就给默认值 ─────

fn ty(v: &Value) -> &str {
    v["@type"].as_str().unwrap_or("")
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
fn b(v: &Value, k: &str) -> bool {
    v[k].as_bool().unwrap_or(false)
}
/// TDLib JSON 里 int53 是数字，int64 是字符串；两种都认
fn as_i64(v: &Value) -> i64 {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|x| x.parse().ok()))
        .unwrap_or(0)
}
fn i(v: &Value, k: &str) -> i64 {
    as_i64(&v[k])
}
fn u(v: &Value, k: &str) -> u32 {
    u32::try_from(i(v, k).max(0)).unwrap_or(u32::MAX)
}
fn str_or_num(v: &Value) -> String {
    match v {
        Value::String(x) => x.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}
fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v[k].as_array().map_or(&[], Vec::as_slice)
}
fn msg_id(m: &Value) -> MsgId {
    MsgId {
        chat_id: i(m, "chat_id"),
        message_id: i(m, "id"),
    }
}
/// TDLib `file`：`size` 不知道时是 0，再看 `expected_size`
fn file_size(f: &Value) -> u64 {
    let n = i(f, "size");
    let n = if n > 0 { n } else { i(f, "expected_size") };
    u64::try_from(n).unwrap_or(0)
}

fn join_name(first: &str, last: &str) -> String {
    match (first.is_empty(), last.is_empty()) {
        (false, false) => format!("{first} {last}"),
        (false, true) => first.to_owned(),
        (true, false) => last.to_owned(),
        (true, true) => String::new(),
    }
}
