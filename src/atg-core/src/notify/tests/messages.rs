//! 新消息提不提示：出站与收藏夹、别处已读、太旧、静音（会话 / 范围 / 话题）、提及、置顶、连发。夹具在 `../tests.rs`。

use super::*;

#[test]
fn outgoing_and_saved_are_quiet() {
    let mut w = W::synced();
    assert_eq!(
        w.reason(new_msg(PEER, 1, json!({"is_outgoing": true}))),
        (false, Reason::Outgoing)
    );
    // 以频道身份发言：发送人不是我，但 is_outgoing 为真
    assert_eq!(
        w.reason(new_msg(
            GROUP,
            1,
            json!({"is_outgoing": true,
            "sender_id": {"@type": "messageSenderChat", "chat_id": -5}})
        )),
        (false, Reason::Outgoing)
    );
    w.feed(chat(
        ME,
        json!({"@type": "chatTypePrivate", "user_id": ME}),
        "我自己",
        notif(0, true),
    ));
    assert_eq!(w.reason(new_msg(ME, 1, json!({}))), (false, Reason::Saved));
}

#[test]
fn read_elsewhere_is_quiet() {
    let mut w = W::synced();
    w.feed(json!({"@type": "updateChatReadInbox", "chat_id": PEER, "last_read_inbox_message_id": 5, "unread_count": 0}));
    assert_eq!(
        w.reason(new_msg(PEER, 5, json!({}))),
        (false, Reason::ReadElsewhere)
    );
    assert_eq!(w.reason(new_msg(PEER, 6, json!({}))), (true, Reason::New));
}

#[test]
fn older_than_an_hour_is_quiet() {
    let mut w = W::synced();
    let old =
        json!({"@type": "updateNewMessage", "message": msg(PEER, 1, NOW_S - 3601, json!({}))});
    assert_eq!(w.reason(old), (false, Reason::TooOld));
    let edge =
        json!({"@type": "updateNewMessage", "message": msg(PEER, 2, NOW_S - 3600, json!({}))});
    assert_eq!(w.reason(edge), (true, Reason::New));
}

#[test]
fn chat_mute_and_scope_mute() {
    let mut w = W::synced();
    // 群那一档静音、会话跟随默认 → 静音（范围）
    w.feed(scope("notificationSettingsScopeGroupChats", 3600, false));
    assert_eq!(
        w.reason(new_msg(GROUP, 1, json!({}))),
        (false, Reason::MutedScope)
    );
    // 会话自己设了不静音 → 盖过范围
    w.feed(json!({"@type": "updateChatNotificationSettings", "chat_id": GROUP, "notification_settings": notif(0, false)}));
    assert_eq!(w.reason(new_msg(GROUP, 2, json!({}))), (true, Reason::New));
    // 会话自己静音
    w.feed(json!({"@type": "updateChatNotificationSettings", "chat_id": PEER, "notification_settings": notif(600, false)}));
    assert_eq!(
        w.reason(new_msg(PEER, 1, json!({}))),
        (false, Reason::MutedChat)
    );
    // 私聊那一档静音不影响群
    w.feed(scope("notificationSettingsScopePrivateChats", 3600, false));
    w.feed(json!({"@type": "updateChatNotificationSettings", "chat_id": GROUP, "notification_settings": notif(0, true)}));
    assert_eq!(
        w.reason(new_msg(GROUP, 3, json!({}))),
        (false, Reason::MutedScope)
    );
}

#[test]
fn mention_pierces_mute_unless_disabled() {
    let mut w = W::synced();
    w.feed(json!({"@type": "updateChatNotificationSettings", "chat_id": GROUP, "notification_settings": notif(3600, false)}));
    let at_me = json!({"contains_unread_mention": true});
    assert_eq!(
        w.reason(new_msg(GROUP, 1, at_me.clone())),
        (true, Reason::Mention)
    );
    assert_eq!(
        w.reason(new_msg(GROUP, 2, json!({}))),
        (false, Reason::MutedChat)
    );
    // 范围里关了提及通知，会话跟随 → 不穿透
    w.feed(scope("notificationSettingsScopeGroupChats", 0, true));
    assert_eq!(
        w.reason(new_msg(GROUP, 3, at_me.clone())),
        (false, Reason::MutedChat)
    );
    // 会话自己再打开提及通知 → 盖过范围
    let mut n = notif(3600, false);
    n["use_default_disable_mention_notifications"] = json!(false);
    n["disable_mention_notifications"] = json!(false);
    w.feed(json!({"@type": "updateChatNotificationSettings", "chat_id": GROUP, "notification_settings": n}));
    assert_eq!(w.reason(new_msg(GROUP, 4, at_me)), (true, Reason::Mention));
}

#[test]
fn mention_uses_sender_mute_like_tdlib() {
    let mut w = W::synced();
    let at_me = json!({"contains_unread_mention": true});
    // 群没静音、发送人没静音：提及
    assert_eq!(
        w.reason(new_msg(GROUP, 1, at_me.clone())),
        (true, Reason::Mention)
    );
    // 静音了发送人（与他的私聊）：群里 @ 我也不响，群静不静音都一样
    w.feed(json!({"@type": "updateChatNotificationSettings", "chat_id": PEER, "notification_settings": notif(3600, false)}));
    assert_eq!(
        w.reason(new_msg(GROUP, 2, at_me.clone())),
        (false, Reason::MutedSender)
    );
    w.feed(json!({"@type": "updateChatNotificationSettings", "chat_id": GROUP, "notification_settings": notif(3600, false)}));
    assert_eq!(
        w.reason(new_msg(GROUP, 3, at_me)),
        (false, Reason::MutedSender)
    );
    // 不含提及的照群的静音判
    assert_eq!(
        w.reason(new_msg(GROUP, 4, json!({}))),
        (false, Reason::MutedChat)
    );
}

fn topic_update(mute_for: i64, use_default: bool, read: i64) -> Value {
    json!({"@type": "updateForumTopic", "chat_id": FORUM, "forum_topic_id": 7, "is_pinned": false,
           "last_read_inbox_message_id": read, "last_read_outbox_message_id": 0, "unread_mention_count": 0,
           "unread_reaction_count": 0, "unread_poll_vote_count": 0, "notification_settings": notif(mute_for, use_default)})
}

fn in_topic(id: i64, patch: Value) -> Value {
    let mut p = json!({"topic_id": {"@type": "messageTopicForum", "forum_topic_id": 7}});
    if let (Some(o), Value::Object(x)) = (p.as_object_mut(), patch) {
        o.extend(x);
    }
    new_msg(FORUM, id, p)
}

#[test]
fn topic_mute_uses_absolute_deadline() {
    let mut w = W::synced();
    // 话题静音 60 秒；会话本身不静音
    w.feed(topic_update(60, false, 0));
    assert_eq!(
        w.reason(in_topic(1, json!({}))),
        (false, Reason::MutedTopic)
    );
    // 别的话题、不在话题里的照常提示
    assert_eq!(w.reason(new_msg(FORUM, 2, json!({}))), (true, Reason::New));
    // 假时钟拨过 60 秒：TDLib 不会再推话题设置，到期自己判
    w.now += 60_000;
    assert_eq!(w.reason(in_topic(3, json!({}))), (true, Reason::New));
}

#[test]
fn late_subscriber_still_sees_topic_mute() {
    // 执行方早已收到话题设置；之后才接入的订阅者是一份新的策略状态，话题静音照样认（状态在 store 里）
    let mut w = W::synced();
    w.feed(topic_update(600, false, 4));
    w.policy = Policy::new(Config::default());
    w.policy.mark_synced(w.now);
    assert_eq!(
        w.reason(in_topic(5, json!({}))),
        (false, Reason::MutedTopic)
    );
    assert_eq!(
        w.reason(in_topic(4, json!({}))),
        (false, Reason::ReadElsewhere)
    );
}

#[test]
fn unknown_topic_is_quiet_until_fetched_once() {
    let mut w = W::synced();
    // 话题设置没推过（TDLib 没加载这个话题）：不提示，请调用方取一次
    assert_eq!(
        w.reason(in_topic(1, json!({}))),
        (false, Reason::TopicUnknown)
    );
    assert_eq!(
        w.reason(in_topic(2, json!({}))),
        (false, Reason::TopicUnknown)
    );
    assert_eq!(w.policy.take_missing_topics(), vec![(FORUM, 7)]);
    assert!(w.policy.take_missing_topics().is_empty());
    // 取到了：话题静音 60 秒
    let n = ChatNotify {
        mute_for: 60,
        ..ChatNotify::default()
    };
    w.store.apply(Input::TopicLoaded {
        chat_id: FORUM,
        topic_id: 7,
        state: ForumTopicState {
            last_read_inbox: 0,
            notify: n,
        },
        received_ms: w.now,
    });
    assert_eq!(
        w.reason(in_topic(3, json!({}))),
        (false, Reason::MutedTopic)
    );
    // 应答不盖掉更新：之后推来的不静音照用；再来一份旧应答也不改
    w.feed(topic_update(0, false, 0));
    w.store.apply(Input::TopicLoaded {
        chat_id: FORUM,
        topic_id: 7,
        state: ForumTopicState {
            last_read_inbox: 0,
            notify: n,
        },
        received_ms: w.now,
    });
    assert_eq!(w.reason(in_topic(4, json!({}))), (true, Reason::New));
}

#[test]
fn topic_unmuted_inside_muted_chat_alerts() {
    let mut w = W::synced();
    w.feed(json!({"@type": "updateChatNotificationSettings", "chat_id": FORUM, "notification_settings": notif(3600, false)}));
    // 话题自己设了不静音
    w.feed(topic_update(0, false, 0));
    assert_eq!(w.reason(in_topic(1, json!({}))), (true, Reason::New));
    // 话题跟随默认 → 回落到会话
    w.feed(topic_update(0, true, 0));
    assert_eq!(w.reason(in_topic(2, json!({}))), (false, Reason::MutedChat));
}

#[test]
fn topic_read_position_counts() {
    let mut w = W::synced();
    w.feed(topic_update(0, true, 10));
    assert_eq!(
        w.reason(in_topic(10, json!({}))),
        (false, Reason::ReadElsewhere)
    );
    assert_eq!(w.reason(in_topic(11, json!({}))), (true, Reason::New));
}

#[test]
fn pinned_notifications_off() {
    let mut w = W::synced();
    let mut n = notif(0, false);
    n["use_default_disable_pinned_message_notifications"] = json!(false);
    n["disable_pinned_message_notifications"] = json!(true);
    w.feed(json!({"@type": "updateChatNotificationSettings", "chat_id": GROUP, "notification_settings": n}));
    let pin = json!({"content": {"@type": "messagePinMessage", "message_id": 1}});
    assert_eq!(w.reason(new_msg(GROUP, 2, pin)), (false, Reason::PinnedOff));
}

#[test]
fn burst_is_per_chat() {
    let mut w = W::synced();
    let a = w.verdict(new_msg(PEER, 1, json!({})));
    assert!(a.sound && a.burst == 1);
    w.now += 3000;
    let b = w.verdict(new_msg(PEER, 2, json!({})));
    assert!(b.alert && !b.sound && b.burst == 2);
    // 别的会话：各响各的
    w.now += 3000;
    let c = w.verdict(new_msg(GROUP, 1, json!({})));
    assert!(c.alert && c.sound && c.burst == 1);
    // 同一会话离上次出声满 7 秒：再出声，重新计数
    w.now += 1000;
    let d = w.verdict(new_msg(PEER, 3, json!({})));
    assert!(d.sound && d.burst == 1);
    let e = w.verdict(new_msg(GROUP, 2, json!({})));
    assert!(!e.sound && e.burst == 2);
}
