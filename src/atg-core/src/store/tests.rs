// 穷举写入路径的受限原文测试（codex 005 第十轮要求）：受限消息的原文、临时内容、被引用的那段，
// 从**每一条**能写内容的路径喂进来，断言整个 Store 的完整 `{:?}`（单元测试里是派生的完整 Debug，
// 不是脱敏版）里一次都没有。对照组证明 dump 真能看到内容。

use super::*;
use crate::tree::Span;

const SECRET: &str = "ZQX受限原文";
const C: ChatId = 101;
const D: ChatId = -1001;

fn id(m: i64) -> MsgId {
    MsgId {
        chat_id: C,
        message_id: m,
    }
}

fn secret_body() -> Body {
    Body::Content(Content {
        body: vec![Span::Text {
            text: SECRET.into(),
        }],
        attachment: None,
        sensitive: false,
    })
}

fn msg(m: i64) -> Message {
    Message {
        id: id(m),
        sender: SenderId::User(7),
        date: Time(1),
        outgoing: false,
        reply_to: None,
        forward: None,
        album_id: None,
        topic: None,
        content: secret_body(),
        ephemeral: Some(secret_body()),
        restriction: None,
        sensitive: false,
        edited: None,
        reactions: vec![],
        sending: SendingState::Sent,
        mentions_me: false,
        self_destruct: false,
    }
}

/// 不含暗号的普通消息（垫底用，免得把暗号误算成泄漏）
fn plain(m: i64) -> Message {
    let mut x = msg(m);
    x.content = Body::Content(Content {
        body: vec![Span::Text {
            text: "普通".into(),
        }],
        attachment: None,
        sensitive: false,
    });
    x.ephemeral = None;
    x
}

/// 受限的那条（id 5）
fn r() -> Message {
    let mut m = msg(5);
    m.restriction = Some("版权".into());
    m
}

/// 引用 5 的原文的回复（id 6；它自己的正文是普通的，所以正文不用暗号）
fn reply() -> Message {
    let mut m = msg(6);
    m.content = Body::Content(Content {
        body: vec![Span::Text { text: "回".into() }],
        attachment: None,
        sensitive: false,
    });
    m.ephemeral = None;
    m.reply_to = Some(ReplyRef::Message {
        target: id(5),
        quote: Some(vec![Span::Text {
            text: SECRET.into(),
        }]),
        origin: None,
    });
    m
}

struct S(Store);

impl S {
    fn new() -> Self {
        Self(Store::with_config(StoreConfig { keep_per_chat: 1 }))
    }
    fn up(&mut self, e: CoreEvent) -> &mut Self {
        let seq = self.0.last_seq() + 1;
        self.0.apply(Input::Update { seq, event: e });
        self
    }
    fn hist(&mut self, chat: ChatId, ms: Vec<Message>) -> &mut Self {
        let t = self.0.ticket(chat);
        self.0.apply(Input::HistoryLoaded {
            ticket: t,
            messages: ms,
        });
        self
    }
    fn open(&mut self, chat: ChatId) -> &mut Self {
        self.0.apply(Input::OpenChat { chat_id: chat });
        self
    }
    fn content(&mut self, m: i64) -> &mut Self {
        self.up(CoreEvent::MessageContentChanged {
            id: id(m),
            content: secret_body(),
        })
    }
    fn ephemeral(&mut self, m: i64) -> &mut Self {
        self.up(CoreEvent::MessageEphemeralChanged {
            id: id(m),
            ephemeral: Some(secret_body()),
        })
    }
    fn revoke(&mut self, m: i64) -> &mut Self {
        self.up(CoreEvent::MessagesDeleted {
            chat_id: C,
            message_ids: vec![m],
            is_permanent: true,
            from_cache: false,
        })
    }
    fn clean(&self, what: &str) {
        let dump = format!("{:?}", self.0);
        assert!(!dump.contains(SECRET), "{what}：store 里留了受限原文");
    }
}

#[test]
fn control_dump_really_shows_content() {
    let mut s = S::new();
    s.open(C).up(CoreEvent::NewMessage(msg(5)));
    assert!(
        format!("{:?}", s.0).contains(SECRET),
        "对照组：不受限的应能在 dump 里看到"
    );
}

#[test]
fn restricted_text_never_stored_on_any_write_path() {
    type Scenario = (&'static str, fn(&mut S));
    let scenarios: &[Scenario] = &[
        ("新消息", |s| {
            s.open(C).up(CoreEvent::NewMessage(r()));
        }),
        ("历史应答", |s| {
            s.open(C).hist(C, vec![r()]);
        }),
        ("会话的最后一条", |s| {
            s.up(CoreEvent::ChatAdded {
                chat: chat(),
                last_message: Some(r()),
            })
            .up(CoreEvent::ChatLastMessageChanged {
                chat_id: C,
                last_message: Some(r()),
                positions: None,
            });
        }),
        ("发送成功换 id", |s| {
            let mut p = msg(1_000_001);
            p.sending = SendingState::Pending;
            p.content = Body::Content(Content {
                body: vec![],
                attachment: None,
                sensitive: false,
            });
            p.ephemeral = None;
            s.open(C)
                .up(CoreEvent::NewMessage(p))
                .up(CoreEvent::SendSucceeded {
                    old_id: id(1_000_001),
                    message: r(),
                });
        }),
        ("活消息的内容 / 临时内容更新", |s| {
            s.open(C)
                .up(CoreEvent::NewMessage(r()))
                .content(5)
                .ephemeral(5);
        }),
        ("同一条的另一份不带限制信息", |s| {
            s.open(C)
                .up(CoreEvent::NewMessage(r()))
                .up(CoreEvent::NewMessage(msg(5)))
                .hist(C, vec![msg(5)]);
        }),
        ("补丁先到、受限消息后到", |s| {
            s.open(C).content(5).ephemeral(5).hist(C, vec![r()]);
        }),
        (
            "补丁先到、受限消息一直没被存（被撤回后到的历史）",
            |s| {
                s.open(C).content(5).revoke(5).hist(C, vec![r()]);
            },
        ),
        ("受限消息被删旧后再来补丁", |s| {
            s.hist(C, vec![r(), plain(9)]).content(5).ephemeral(5);
        }),
        ("受限消息被撤回后再来补丁", |s| {
            s.open(C)
                .up(CoreEvent::NewMessage(r()))
                .revoke(5)
                .content(5)
                .ephemeral(5);
        }),
        ("回复先到、受限消息后到", |s| {
            s.open(C)
                .up(CoreEvent::NewMessage(reply()))
                .up(CoreEvent::NewMessage(r()));
        }),
        ("受限消息先到、回复后到", |s| {
            s.open(C)
                .up(CoreEvent::NewMessage(r()))
                .up(CoreEvent::NewMessage(reply()));
        }),
        (
            "回复先到、受限消息没加载就被撤回、受限历史后到",
            |s| {
                s.open(C)
                    .up(CoreEvent::NewMessage(reply()))
                    .revoke(5)
                    .hist(C, vec![r()]);
            },
        ),
        ("受限消息被删旧后回复才到", |s| {
            s.hist(C, vec![r(), plain(9)])
                .open(D)
                .hist(C, vec![reply()])
                .up(CoreEvent::NewMessage(reply()));
        }),
        ("回复走历史、受限消息走换 id", |s| {
            let mut p = msg(1_000_001);
            p.sending = SendingState::Pending;
            p.ephemeral = None;
            p.content = Body::Content(Content {
                body: vec![],
                attachment: None,
                sensitive: false,
            });
            let mut rep = reply();
            rep.reply_to = Some(ReplyRef::Message {
                target: id(1_000_001),
                quote: Some(vec![Span::Text {
                    text: SECRET.into(),
                }]),
                origin: None,
            });
            s.open(C)
                .up(CoreEvent::NewMessage(p))
                .hist(C, vec![rep])
                .up(CoreEvent::SendSucceeded {
                    old_id: id(1_000_001),
                    message: r(),
                });
        }),
    ];
    for (name, run) in scenarios {
        let mut s = S::new();
        run(&mut s);
        s.clean(name);
        // 窗口切来切去、重拉，也不会把原文带出来
        s.open(D).open(C);
        s.0.apply(Input::Refetch);
        s.clean(name);
    }
}

fn chat() -> Chat {
    Chat {
        id: C,
        title: "c".into(),
        kind: ChatKind::Group,
        positions: vec![],
        unread_count: 0,
        unread_mention_count: 0,
        last_read_inbox: 0,
        last_read_outbox: 0,
        notify: ChatNotify::default(),
        peer_user: None,
        last_message: None,
    }
}
