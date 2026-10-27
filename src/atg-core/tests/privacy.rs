//! 暗号测试：把一个暗号塞进每种事件的每个文字字段，整条喂进 store、再投影，
//! 然后对事件、输入、整个 Store、投影出的树逐个 `{:?}`——暗号一次都不许出现
//! （`specs/contracts/config-privacy.md` 验收判据「`{:?}` 整个消息也漏不出内容」）。
//! 集成测试编的是不带 `cfg(test)` 的库，所以测的正是下游拿到的脱敏 `Debug`。

use atg_core::model::*;
use atg_core::project;
use atg_core::store::{Input, Store};
use atg_core::tree::{self, Doc, Span};

const SECRET: &str = "ZQX暗号";

fn s(tag: &str) -> String {
    format!("{SECRET}-{tag}")
}

fn spans() -> Vec<Span> {
    vec![
        Span::Text { text: s("text") },
        Span::Link {
            url: format!("https://{SECRET}.example/"),
            children: vec![Span::Text { text: s("link") }],
        },
        Span::Pre {
            language: Some(s("lang")),
            text: s("pre"),
        },
    ]
}

fn content(attachment: Option<Attachment>) -> Body {
    Body::Content(Content {
        body: spans(),
        attachment,
        sensitive: false,
    })
}

fn attachments() -> Vec<Attachment> {
    vec![
        Attachment::Document {
            file_name: s("file"),
            size: 1,
        },
        Attachment::Audio {
            duration: 1,
            title: s("title"),
            performer: s("performer"),
        },
        Attachment::Venue { title: s("venue") },
        Attachment::Contact { name: s("contact") },
        Attachment::Poll { question: spans() },
        Attachment::Sticker { emoji: s("emoji") },
    ]
}

fn message(message_id: i64, body: Body) -> Message {
    Message {
        id: MsgId {
            chat_id: 101,
            message_id,
        },
        sender: SenderId::User(7),
        date: Time(1),
        outgoing: false,
        reply_to: Some(ReplyRef::Message {
            target: MsgId {
                chat_id: 101,
                message_id: 1,
            },
            quote: Some(spans()),
            origin: Some(OriginId::HiddenUser { name: s("hidden") }),
        }),
        forward: Some(ForwardRef {
            origin: OriginId::HiddenUser { name: s("fwd") },
            date: Time(1),
        }),
        album_id: None,
        topic: Some(Topic::Forum {
            topic_id: 1,
            name: Some(s("topic")),
        }),
        content: body,
        ephemeral: Some(content(None)),
        restriction: None,
        sensitive: false,
        edited: None,
        reactions: reactions(),
        sending: SendingState::Sent,
        mentions_me: false,
    }
}

fn reactions() -> Vec<tree::Reaction> {
    vec![
        tree::Reaction {
            kind: tree::ReactionKind::Emoji { emoji: s("react") },
            count: 1,
            chosen: false,
        },
        tree::Reaction {
            kind: tree::ReactionKind::CustomEmoji {
                custom_emoji_id: s("custom"),
            },
            count: 1,
            chosen: false,
        },
    ]
}

fn events() -> Vec<CoreEvent> {
    let id = MsgId {
        chat_id: 101,
        message_id: 10,
    };
    let chat = Chat {
        id: 101,
        title: s("chat"),
        kind: ChatKind::Group,
        positions: vec![ChatPosition {
            list: ChatList::Main,
            order: 1,
            is_pinned: false,
        }],
        unread_count: 0,
        unread_mention_count: 0,
        last_read_inbox: 0,
        last_read_outbox: 0,
        notify: ChatNotify::default(),
        peer_user: None,
        last_message: None,
    };
    let mut v = vec![
        CoreEvent::UserChanged(User {
            id: 7,
            first_name: s("first"),
            last_name: s("last"),
            deleted: false,
        }),
        CoreEvent::ChatAdded {
            chat,
            last_message: Some(message(2, content(None))),
        },
        CoreEvent::ChatTitleChanged {
            chat_id: 101,
            title: s("newtitle"),
        },
    ];
    for (k, a) in attachments().into_iter().enumerate() {
        v.push(CoreEvent::NewMessage(message(
            10 + k as i64,
            content(Some(a)),
        )));
    }
    v.extend([
        CoreEvent::NewMessage(message(
            30,
            Body::Service(tree::Action::TitleChanged { title: s("svc") }),
        )),
        CoreEvent::NewMessage(message(
            31,
            Body::Service(tree::Action::GroupCreated {
                title: s("created"),
            }),
        )),
        CoreEvent::MessageContentChanged {
            id,
            content: content(None),
        },
        CoreEvent::MessageEphemeralChanged {
            id,
            ephemeral: Some(content(None)),
        },
        // 消息还没到时的更新：落进孤儿补丁
        CoreEvent::MessageContentChanged {
            id: MsgId {
                chat_id: 101,
                message_id: 999,
            },
            content: content(None),
        },
        // 消息还没到时的回应：落进孤儿补丁
        CoreEvent::MessageInteractionChanged {
            id: MsgId {
                chat_id: 101,
                message_id: 998,
            },
            reactions: reactions(),
        },
        CoreEvent::MessageInteractionChanged {
            id,
            reactions: reactions(),
        },
        CoreEvent::ChatLastMessageChanged {
            chat_id: 101,
            last_message: Some(message(40, content(None))),
            positions: None,
        },
        CoreEvent::SendSucceeded {
            old_id: MsgId {
                chat_id: 101,
                message_id: 1_000_001,
            },
            message: message(41, content(None)),
        },
        CoreEvent::MessagesDeleted {
            chat_id: 101,
            message_ids: vec![11],
            is_permanent: true,
            from_cache: false,
        },
        CoreEvent::Unknown {
            td_type: "updateX".into(),
        },
    ]);
    // 受限消息：限制原因也算内容（store 单独登记了它）
    let mut restricted = message(60, content(None));
    restricted.restriction = Some(s("reason"));
    v.push(CoreEvent::NewMessage(restricted));
    v
}

fn assert_clean(what: &str, dump: String) {
    assert!(!dump.contains(SECRET), "{what} 的 Debug 漏了内容：{dump}");
}

#[test]
fn debug_never_leaks_content() {
    let mut store = Store::new();
    store.apply(Input::OpenChat { chat_id: 101 });
    for (k, e) in events().into_iter().enumerate() {
        assert_clean("CoreEvent", format!("{e:?}"));
        let input = Input::Update {
            seq: k as u64 + 1,
            event: e,
        };
        assert_clean("Input", format!("{input:?}"));
        store.apply(input);
    }
    let t = store.ticket(101);
    let resp = Input::HistoryLoaded {
        ticket: t,
        messages: vec![message(50, content(None))],
    };
    assert_clean("Input::HistoryLoaded", format!("{resp:?}"));
    store.apply(resp);

    // 确认暗号真的进了 store（否则这个测试什么也没测）
    let docs = project::window(&store);
    let json = serde_json::to_string(&docs).unwrap();
    assert!(json.contains(SECRET), "暗号没进投影，测试失效");

    assert_clean("Store", format!("{store:?}"));
    assert_clean("投影 Doc", format!("{docs:?}"));
    for d in &docs {
        assert_clean("Doc", format!("{d:?}"));
        assert_clean("Doc #", format!("{d:#?}"));
    }
    let chats = project::chat_list(&store, &ChatList::Main);
    assert!(serde_json::to_string(&chats).unwrap().contains(SECRET));
    assert_clean("ChatItem", format!("{chats:?}"));
    if let Some(Doc::ChatItem(c)) = chats.first() {
        assert_clean("ChatItem", format!("{c:?}"));
    }
    for m in events() {
        if let CoreEvent::NewMessage(m) = m {
            assert_clean("model::Message", format!("{m:?}"));
            assert_clean("Body", format!("{:?}", m.content));
        }
    }
    assert_clean("Span", format!("{:?}", spans()));
    assert_clean("Attachment", format!("{:?}", attachments()));
    assert_clean("links", format!("{:?}", tree::text::links(&spans())));
}
