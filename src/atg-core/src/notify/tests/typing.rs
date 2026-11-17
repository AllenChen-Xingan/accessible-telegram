//! 正在输入：只在打开的私聊里、一轮一次、按会话 + 话题 + 发送人区分。夹具在 `../tests.rs`。

use super::*;

fn typing(chat_id: i64, user: i64, action: &str) -> Value {
    json!({"@type": "updateChatAction", "chat_id": chat_id, "topic_id": null,
           "sender_id": {"@type": "messageSenderUser", "user_id": user}, "action": {"@type": action}})
}

#[test]
fn typing_only_in_open_private_chat_once_per_round() {
    let mut w = W::synced();
    // 没打开任何会话
    assert_eq!(
        w.reason(typing(PEER, PEER, "chatActionTyping")),
        (false, Reason::NotOpenChat)
    );
    w.feed(typing(PEER, PEER, "chatActionCancel"));
    w.policy.set_open_chat(Some(PEER));
    assert_eq!(
        w.reason(typing(PEER, PEER, "chatActionTyping")),
        (true, Reason::Typing)
    );
    // 同一轮再来：不再播
    assert_eq!(
        w.reason(typing(PEER, PEER, "chatActionTyping")),
        (false, Reason::AlreadyAnnounced)
    );
    // TDLib 推取消（5.5 秒没续或对方发出了消息）后，新一轮再播一次
    assert!(w.feed(typing(PEER, PEER, "chatActionCancel")).is_none());
    assert_eq!(
        w.reason(typing(PEER, PEER, "chatActionTyping")),
        (true, Reason::Typing)
    );
    // 群里不播
    w.policy.set_open_chat(Some(GROUP));
    assert_eq!(
        w.reason(typing(GROUP, PEER, "chatActionTyping")),
        (false, Reason::NotPrivate)
    );
    // 别的动作（录音等）不判
    assert!(
        w.feed(typing(PEER, PEER, "chatActionRecordingVoiceNote"))
            .is_none()
    );
}

#[test]
fn typing_waits_until_user_stops() {
    let mut w = W::synced();
    w.policy.set_open_chat(Some(PEER));
    assert!(w.policy.set_user_idle(&w.store, false).is_empty());
    assert_eq!(
        w.reason(typing(PEER, PEER, "chatActionTyping")),
        (false, Reason::UserBusy)
    );
    // 人停下：补播一次
    let due = w.policy.set_user_idle(&w.store, true);
    assert_eq!(due.len(), 1);
    assert!(matches!(&due[0], Judged::Typing { verdict, .. } if verdict.alert));
    assert_eq!(
        w.reason(typing(PEER, PEER, "chatActionTyping")),
        (false, Reason::AlreadyAnnounced)
    );
    // 人忙时对方已经停了：停下后不补播
    w.feed(typing(PEER, PEER, "chatActionCancel"));
    w.policy.set_user_idle(&w.store, false);
    w.feed(typing(PEER, PEER, "chatActionTyping"));
    w.feed(typing(PEER, PEER, "chatActionCancel"));
    assert!(w.policy.set_user_idle(&w.store, true).is_empty());
}

#[test]
fn typing_key_includes_topic_and_sender() {
    let mut w = W::synced();
    w.policy.set_open_chat(Some(PEER));
    assert!(w.verdict(typing(PEER, PEER, "chatActionTyping")).alert);
    // 同一会话、不同发送人：另一把键（私聊里不常见，键照样分开）
    assert!(w.verdict(typing(PEER, 102, "chatActionTyping")).alert);
    let mut t = typing(PEER, PEER, "chatActionTyping");
    t["topic_id"] = json!({"@type": "messageTopicSavedMessages", "saved_messages_topic_id": 3});
    assert!(w.verdict(t).alert);
}
