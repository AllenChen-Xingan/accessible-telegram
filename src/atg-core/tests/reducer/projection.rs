//! 投影：名字回填、回复发送人、送达、静音继承、reducer 是纯函数、用户状态。

use super::*;

#[test]
fn projection_fills_late_names_and_tombstone_sender() {
    let mut s = feed([new(msg(CHAT, 5, text("a")))]);
    push(&mut s, deleted(CHAT, &[5], true, false));
    // 名字最后才到
    push(
        &mut s,
        CoreEvent::UserChanged(User {
            id: LIN,
            first_name: "小雨".into(),
            last_name: String::new(),
            deleted: false,
        }),
    );
    match project::message(&s, id(CHAT, 5)) {
        Some(Doc::Tombstone(t)) => {
            assert_eq!(
                t.sender,
                Some(Sender::User {
                    user_id: LIN,
                    name: Some("小雨".into()),
                    deleted: false,
                })
            );
            assert_eq!(t.date, Some(Time(1_792_755_420)));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn reply_sender_from_store_else_origin() {
    let mut reply = msg(CHAT, 6, text("回你"));
    reply.reply_to = Some(ReplyRef::Message {
        target: id(CHAT, 5),
        quote: None,
        origin: Some(OriginId::User(99)),
    });
    let mut orphan = msg(CHAT, 7, text("回一条不在的"));
    orphan.reply_to = Some(ReplyRef::Message {
        target: id(GROUP, 1),
        quote: None,
        origin: Some(OriginId::User(99)),
    });
    let s = feed([new(msg(CHAT, 5, text("原"))), new(reply), new(orphan)]);
    let sender_of = |mid| match project::message(&s, id(CHAT, mid)) {
        Some(Doc::Message(m)) => match m.head.reply_to {
            Some(tree::ReplyTo::Message { sender, .. }) => sender,
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    assert_eq!(
        sender_of(6),
        Some(Sender::User {
            user_id: LIN,
            name: None,
            deleted: false,
        })
    );
    assert_eq!(
        sender_of(7),
        Some(Sender::User {
            user_id: 99,
            name: None,
            deleted: false,
        })
    );
}

#[test]
fn delivery_read_follows_outbox() {
    let mut m = msg(CHAT, 5, text("a"));
    m.outgoing = true;
    let mut s = feed([
        CoreEvent::ChatAdded {
            chat: chat(CHAT, 1),
            last_message: None,
        },
        new(m),
    ]);
    let delivery = |s: &Store| match project::message(s, id(CHAT, 5)) {
        Some(Doc::Message(m)) => m.tail.delivery,
        other => panic!("{other:?}"),
    };
    assert_eq!(delivery(&s), Some(tree::Delivery::Sent));
    push(
        &mut s,
        CoreEvent::ChatReadOutbox {
            chat_id: CHAT,
            last_read_outbox_message_id: 5,
        },
    );
    assert_eq!(delivery(&s), Some(tree::Delivery::Read));
}

#[test]
fn inherited_mute_uses_scope_setting() {
    let mut c = chat(GROUP, 1);
    c.kind = ChatKind::Supergroup;
    c.notify = ChatNotify {
        use_default_mute_for: true,
        mute_for: 0,
        ..ChatNotify::default()
    };
    let mut s = feed([CoreEvent::ChatAdded {
        chat: c,
        last_message: None,
    }]);
    let item = |s: &Store| project::chat_item(s, GROUP, &ChatList::Main).unwrap();
    assert!(!item(&s).muted && item(&s).mute_inherited);
    push(
        &mut s,
        CoreEvent::ScopeNotificationChanged {
            scope: NotifyScope::Group,
            notify: ScopeNotify {
                mute_for: 3600,
                ..ScopeNotify::default()
            },
        },
    );
    assert!(item(&s).muted);
}

#[test]
fn chat_last_can_be_tombstone() {
    let mut s = feed([CoreEvent::ChatAdded {
        chat: chat(CHAT, 1),
        last_message: Some(msg(CHAT, 5, text("a"))),
    }]);
    push(&mut s, deleted(CHAT, &[5], true, false));
    assert!(matches!(
        project::chat_item(&s, CHAT, &ChatList::Main)
            .unwrap()
            .last
            .as_deref(),
        Some(tree::ChatLast::Tombstone(_))
    ));
}

#[test]
fn reduce_is_a_pure_function_of_inputs() {
    let inputs = [
        Input::OpenChat { chat_id: CHAT },
        Input::Update {
            seq: 1,
            event: new(msg(CHAT, 5, text("a"))),
        },
        Input::Update {
            seq: 2,
            event: deleted(CHAT, &[5], true, false),
        },
    ];
    let run = || inputs.iter().cloned().fold(Store::new(), reduce);
    assert_eq!(project::window(&run()), project::window(&run()));
}

#[test]
fn user_status_is_kept_including_stopping() {
    let mut s = feed([CoreEvent::UserStatusChanged(UserStatus::Online)]);
    assert_eq!(s.status(), Some(UserStatus::Online));
    push(&mut s, CoreEvent::UserStatusChanged(UserStatus::Stopping));
    assert_eq!(s.status(), Some(UserStatus::Stopping));
    let j = serde_json::to_value(tree::Status {
        status: UserStatus::Stopping,
    })
    .unwrap();
    assert_eq!(j["status"], "stopping");
}
