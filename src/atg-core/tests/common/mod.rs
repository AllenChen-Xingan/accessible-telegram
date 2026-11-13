//! 集成测试共用的「造假数据」助手（Cargo 约定：`tests/common/mod.rs` 不当成单独的测试）。
//! 现在只有 `reducer.rs` 用；以后别的集成测试也从这里取，不各自另造（arch 010 R3 testkit 的第一步）。
//! 全是手写的假事件，值都是编的。

use atg_core::model::*;
use atg_core::store::{HistoryTicket, Input, Store};
use atg_core::tree::{self, Span};

pub const ME: i64 = 1;
pub const LIN: i64 = 7;
pub const CHAT: i64 = 101;
pub const GROUP: i64 = -1001;

pub fn id(chat_id: i64, message_id: i64) -> MsgId {
    MsgId {
        chat_id,
        message_id,
    }
}

pub fn text(t: &str) -> Body {
    Body::Content(Content {
        body: vec![Span::Text { text: t.into() }],
        attachment: None,
        sensitive: false,
    })
}

pub fn voice() -> Body {
    Body::Content(Content {
        body: vec![],
        attachment: Some(Attachment::Voice {
            duration: 3,
            listened: false,
            file: atg_core::tree::FileRef { id: 7 },
        }),
        sensitive: false,
    })
}

pub fn msg(chat_id: i64, message_id: i64, body: Body) -> Message {
    Message {
        id: id(chat_id, message_id),
        sender: SenderId::User(LIN),
        date: Time(1_792_755_420),
        outgoing: false,
        reply_to: None,
        forward: None,
        album_id: None,
        topic: None,
        content: body,
        ephemeral: None,
        restriction: None,
        sensitive: false,
        edited: None,
        reactions: vec![],
        sending: SendingState::Sent,
        mentions_me: false,
        self_destruct: false,
    }
}

pub fn chat(chat_id: i64, order: i64) -> Chat {
    Chat {
        id: chat_id,
        title: format!("会话{chat_id}"),
        kind: ChatKind::Private,
        positions: vec![pos(ChatList::Main, order)],
        unread_count: 0,
        unread_mention_count: 0,
        last_read_inbox: 0,
        last_read_outbox: 0,
        notify: ChatNotify::default(),
        peer_user: None,
        last_message: None,
    }
}

pub fn pos(list: ChatList, order: i64) -> ChatPosition {
    ChatPosition {
        list,
        order,
        is_pinned: false,
    }
}

/// 按顺序喂一串事件，序号从 1 起
pub fn feed(events: impl IntoIterator<Item = CoreEvent>) -> Store {
    let mut s = Store::new();
    for e in events {
        let seq = s.last_seq() + 1;
        s.apply(Input::Update { seq, event: e });
    }
    s
}

pub fn push(s: &mut Store, e: CoreEvent) {
    let seq = s.last_seq() + 1;
    s.apply(Input::Update { seq, event: e });
}

pub fn new(m: Message) -> CoreEvent {
    CoreEvent::NewMessage(m)
}

pub fn deleted(chat_id: i64, ids: &[i64], is_permanent: bool, from_cache: bool) -> CoreEvent {
    CoreEvent::MessagesDeleted {
        chat_id,
        message_ids: ids.to_vec(),
        is_permanent,
        from_cache,
    }
}

pub fn live_text(s: &Store, i: MsgId) -> Option<String> {
    match s.message(i)?.effective() {
        Body::Content(c) => Some(tree::text::plain_text(&c.body)),
        Body::Service(_) => None,
    }
}

pub fn window_ids(s: &Store) -> Vec<i64> {
    s.window()
        .map(|w| w.ids.iter().copied().collect())
        .unwrap_or_default()
}

pub fn loaded(ticket: HistoryTicket, messages: Vec<Message>) -> Input {
    Input::HistoryLoaded { ticket, messages }
}
