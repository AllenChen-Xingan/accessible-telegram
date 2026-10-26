//! 投影：model + store → 内容树 [`Doc`]。纯函数，渲染时现算、不存（`specs/content-tree.md` 第二节）。
//!
//! 名字（发送人、转发来源、系统消息里提到的人、回复对象的发送人）、已读状态、有效静音都在这里现取，
//! 所以比消息晚到的 `updateUser`、`updateChatReadOutbox` 下次投影就能用上。
//! 查不到的名字留 `None`，兜底说法归读法。

use crate::model::{
    Body, ChatList, Message as MMessage, NotifyScope, OriginId, ReplyRef, SenderId, SendingState,
};
use crate::store::{Entry, Store};
use crate::tree::*;

/// 一条消息的投影：普通消息、系统消息，或撤回后的原位占位。消息表里没有就是 `None`
pub fn message(store: &Store, id: MsgId) -> Option<Doc> {
    Some(match store.entry(id)? {
        Entry::Live(m) => live(store, m),
        Entry::Tombstone(t) => Doc::Tombstone(Tombstone {
            id,
            sender: t.sender.as_ref().map(|s| sender(store, s)),
            date: t.date,
        }),
    })
}

/// 当前窗口里的消息，从旧到新。从上次带过来的撤回占位只显示落在历史应答确认的加载边界之内的
/// （`Window::loaded_from`；还没有应答时不显示）
pub fn window(store: &Store) -> Vec<Doc> {
    let Some(w) = store.window() else {
        return vec![];
    };
    w.ids
        .iter()
        .filter(|m| !w.carried.contains(m) || w.loaded_from.is_some_and(|b| **m >= b))
        .filter_map(|&message_id| {
            message(
                store,
                MsgId {
                    chat_id: w.chat_id,
                    message_id,
                },
            )
        })
        .collect()
}

/// 某个会话列表，按 store 维护的顺序
pub fn chat_list(store: &Store, list: &ChatList) -> Vec<Doc> {
    store
        .chat_list(list)
        .into_iter()
        .filter_map(|id| chat_item(store, id, list))
        .map(Doc::ChatItem)
        .collect()
}

/// 会话项。`list` 决定「置顶」看哪个列表（TDLib 的置顶属于具体列表）
pub fn chat_item(store: &Store, chat_id: i64, list: &ChatList) -> Option<ChatItem> {
    let c = store.chat(chat_id)?;
    let inherited = c.notify.use_default_mute_for;
    let mute_for = if inherited {
        // 没收到范围设置时按不静音算（与原型一致）
        NotifyScope::of(&c.kind)
            .and_then(|s| store.scope_mute(s))
            .unwrap_or(0)
    } else {
        c.notify.mute_for
    };
    let last = c.last_message.and_then(|message_id| {
        let id = MsgId {
            chat_id,
            message_id,
        };
        Some(Box::new(match message(store, id)? {
            Doc::Message(m) => ChatLast::Message(m),
            Doc::Service(s) => ChatLast::Service(s),
            Doc::Tombstone(t) => ChatLast::Tombstone(t),
            // message() 只产出上面三种
            Doc::ChatItem(_) | Doc::Alert(_) | Doc::Status(_) | Doc::Unknown(_) => return None,
        }))
    });
    Some(ChatItem {
        chat_id,
        title: c.title.clone(),
        kind: c.kind.clone(),
        unread_count: c.unread_count,
        unread_mention_count: c.unread_mention_count,
        muted: mute_for > 0,
        mute_inherited: inherited,
        pinned: c.positions.iter().any(|p| &p.list == list && p.is_pinned),
        last,
    })
}

// ───── 内部 ─────

fn live(store: &Store, m: &MMessage) -> Doc {
    let sender_doc = sender(store, &m.sender);
    let content = match m.effective() {
        Body::Service(a) => {
            return Doc::Service(Service {
                id: m.id,
                date: m.date,
                actor: sender_doc,
                action: action(store, a),
            });
        }
        Body::Content(c) => c.clone(),
    };
    let delivery = m.outgoing.then(|| match m.sending {
        SendingState::Pending => Delivery::Pending,
        SendingState::Failed => Delivery::Failed,
        SendingState::Sent => {
            let read_upto = store.chat(m.id.chat_id).map_or(0, |c| c.last_read_outbox);
            if read_upto >= m.id.message_id {
                Delivery::Read
            } else {
                Delivery::Sent
            }
        }
    });
    Doc::Message(Message {
        id: m.id,
        head: Head {
            sender: sender_doc,
            date: m.date,
            outgoing: m.outgoing,
            reply_to: m.reply_to.as_ref().map(|r| reply(store, r)),
            forward: m.forward.as_ref().map(|f| Forward {
                origin: origin(store, &f.origin),
                date: f.date,
            }),
            album_id: m.album_id.clone(),
            topic: m.topic.clone(),
        },
        content,
        tail: Tail {
            edited: m.edited,
            reactions: m.reactions.clone(),
            delivery,
        },
    })
}

fn user_name(store: &Store, id: i64) -> Option<String> {
    store.user(id).map(|u| u.name())
}

fn chat_title(store: &Store, id: i64) -> Option<String> {
    store.chat(id).map(|c| c.title.clone())
}

fn sender(store: &Store, s: &SenderId) -> Sender {
    match s {
        SenderId::User(user_id) => Sender::User {
            user_id: *user_id,
            name: user_name(store, *user_id),
        },
        SenderId::Chat(chat_id) => Sender::Chat {
            chat_id: *chat_id,
            title: chat_title(store, *chat_id),
        },
        SenderId::Unknown { td_type } => Sender::Unknown {
            td_type: td_type.clone(),
        },
    }
}

fn origin(store: &Store, o: &OriginId) -> Origin {
    match o {
        OriginId::User(user_id) => Origin::User {
            user_id: *user_id,
            name: user_name(store, *user_id),
        },
        OriginId::HiddenUser { name } => Origin::HiddenUser { name: name.clone() },
        OriginId::Chat(chat_id) => Origin::Chat {
            chat_id: *chat_id,
            title: chat_title(store, *chat_id),
        },
        OriginId::Channel(chat_id) => Origin::Channel {
            chat_id: *chat_id,
            title: chat_title(store, *chat_id),
        },
        OriginId::Unknown { td_type } => Origin::Unknown {
            td_type: td_type.clone(),
        },
    }
}

/// 被回复那条的发送人：store 里有那条（含撤回后记下的）就用它的，没有就用 TDLib 附带的来源兜底
fn reply(store: &Store, r: &ReplyRef) -> ReplyTo {
    match r {
        ReplyRef::Message {
            target,
            quote,
            origin: fallback,
        } => {
            // 被回复那条可能已换 id：按现在的 id 查、也按现在的 id 输出
            let target = &store.current_id(*target);
            let from_store = match store.entry(*target) {
                Some(Entry::Live(t)) => Some(sender(store, &t.sender)),
                Some(Entry::Tombstone(t)) => t.sender.as_ref().map(|s| sender(store, s)),
                None => None,
            };
            // 被回复那条受限：引用的那段就是它的原文，不出（拍板 006 乙11）。
            // 限制状态 store 单独记着，被删旧、被撤回后也认得（存的时候也已清掉）
            let restricted = store.is_restricted(*target);
            let quote = if restricted { None } else { quote.clone() };
            let sender = from_store.or_else(|| {
                fallback.as_ref().and_then(|o| match origin(store, o) {
                    Origin::User { user_id, name } => Some(Sender::User { user_id, name }),
                    Origin::Chat { chat_id, title } | Origin::Channel { chat_id, title } => {
                        Some(Sender::Chat { chat_id, title })
                    }
                    Origin::HiddenUser { .. } | Origin::Unknown { .. } => None,
                })
            });
            ReplyTo::Message {
                target: *target,
                sender,
                quote,
            }
        }
        ReplyRef::Unknown { td_type } => ReplyTo::Unknown {
            td_type: td_type.clone(),
        },
    }
}

/// 系统消息里提到的人：存的只有 id，名字这里填
fn action(store: &Store, a: &Action) -> Action {
    let fill = |u: &UserRef| UserRef {
        user_id: u.user_id,
        name: user_name(store, u.user_id),
    };
    match a {
        Action::MembersAdded { users } => Action::MembersAdded {
            users: users.iter().map(fill).collect(),
        },
        Action::MemberRemoved { user } => Action::MemberRemoved { user: fill(user) },
        // 被置顶那条可能已换 id
        Action::Pinned { pinned } => Action::Pinned {
            pinned: store.current_id(*pinned),
        },
        Action::JoinedByLink
        | Action::JoinedByRequest
        | Action::TitleChanged { .. }
        | Action::PhotoChanged
        | Action::PhotoDeleted
        | Action::GroupCreated { .. }
        | Action::ContactRegistered
        | Action::Other { .. } => a.clone(),
    }
}
