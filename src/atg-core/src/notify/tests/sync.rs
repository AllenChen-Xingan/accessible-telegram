//! 补发与重连：补发期间不响、「最后一条变了」只认新的、重连汇总、同一条只判一次。夹具在 `../tests.rs`。

use super::*;

#[test]
fn backlog_before_sync_done_is_quiet() {
    let mut w = W::new();
    assert_eq!(
        w.reason(new_msg(PEER, 1, json!({}))),
        (false, Reason::Backlog)
    );
    w.event(CoreEvent::SyncDone);
    assert_eq!(w.reason(new_msg(PEER, 2, json!({}))), (true, Reason::New));
}

#[test]
fn reconnect_backlog_is_quiet_until_next_sync_done() {
    let mut w = W::synced();
    w.event(CoreEvent::NetStateChanged(NetState::Updating));
    assert_eq!(
        w.reason(new_msg(PEER, 1, json!({}))),
        (false, Reason::Backlog)
    );
    w.event(CoreEvent::NetStateChanged(NetState::Ready));
    w.event(CoreEvent::SyncDone);
    assert_eq!(w.reason(new_msg(PEER, 2, json!({}))), (true, Reason::New));
}

#[test]
fn old_last_message_of_chat_loaded_after_sync_is_not_new() {
    let mut w = W::synced();
    // 补发结束之后才加载进来的会话，「最后一条」是一小时内、没读过的旧消息：不当新来的
    let last = json!({"@type": "updateChatLastMessage", "chat_id": GROUP,
        "last_message": msg(GROUP, 3, NOW_S - 600, json!({})), "positions": []});
    assert!(w.feed(last).is_none());
}

fn summary(j: Option<Judged>) -> Option<(u32, u32, Verdict)> {
    match j {
        Some(Judged::Summary {
            messages,
            chats,
            verdict,
        }) => Some((messages, chats, verdict)),
        _ => None,
    }
}

#[test]
fn reconnect_backlog_summarised_once_on_sync_done() {
    let mut w = W::synced();
    w.event(CoreEvent::NetStateChanged(NetState::Updating));
    // 补发来的：两条该提示（两个会话）、一条自己发的、一条太旧——都逐条不提示
    assert_eq!(
        w.reason(new_msg(PEER, 1, json!({}))),
        (false, Reason::Backlog)
    );
    assert_eq!(
        w.reason(new_msg(GROUP, 1, json!({}))),
        (false, Reason::Backlog)
    );
    assert_eq!(
        w.reason(new_msg(PEER, 2, json!({"is_outgoing": true}))),
        (false, Reason::Backlog)
    );
    let old =
        json!({"@type": "updateNewMessage", "message": msg(PEER, 3, NOW_S - 7200, json!({}))});
    assert_eq!(w.reason(old), (false, Reason::Backlog));
    w.event(CoreEvent::NetStateChanged(NetState::Ready));
    let (n, m, v) = summary(w.event(CoreEvent::SyncDone)).expect("要汇总");
    assert_eq!((n, m), (2, 2));
    assert!(v.alert && v.sound && v.reason == Reason::ReconnectSummary);
    // 之后照常；下一次 sync_done（没有补发）不汇总
    assert_eq!(w.reason(new_msg(PEER, 4, json!({}))), (true, Reason::New));
    assert!(summary(w.event(CoreEvent::SyncDone)).is_none());
}

#[test]
fn summary_rechecks_at_sync_done() {
    let mut w = W::synced();
    w.event(CoreEvent::NetStateChanged(NetState::Updating));
    w.feed(new_msg(PEER, 1, json!({})));
    w.feed(new_msg(PEER, 2, json!({})));
    w.feed(new_msg(GROUP, 1, json!({})));
    // 补发里后到：PEER 在手机上已读到 2
    w.feed(json!({"@type": "updateChatReadInbox", "chat_id": PEER, "last_read_inbox_message_id": 2, "unread_count": 0}));
    // 中途又断一次：两段合成一次汇总
    w.event(CoreEvent::NetStateChanged(NetState::Connecting));
    w.feed(new_msg(GROUP, 2, json!({})));
    let (n, m, _) = summary(w.event(CoreEvent::SyncDone)).expect("要汇总");
    assert_eq!((n, m), (2, 1));
}

#[test]
fn late_subscriber_during_reconnect_gets_summary() {
    let mut w = W::new();
    w.policy.mark_reconnecting();
    w.feed(new_msg(PEER, 1, json!({})));
    let (n, m, _) = summary(w.event(CoreEvent::SyncDone)).expect("要汇总");
    assert_eq!((n, m), (1, 1));
}

#[test]
fn first_sync_and_quiet_reconnect_have_no_summary() {
    let mut w = W::new();
    w.feed(new_msg(PEER, 1, json!({})));
    // 启动时的第一次补发：不汇总
    assert!(summary(w.event(CoreEvent::SyncDone)).is_none());
    // 重连但补发里没有该提示的：不汇总
    w.event(CoreEvent::NetStateChanged(NetState::Connecting));
    w.feed(new_msg(PEER, 2, json!({"is_outgoing": true})));
    assert!(summary(w.event(CoreEvent::SyncDone)).is_none());
}

#[test]
fn same_message_judged_once_and_last_message_fallback() {
    let mut w = W::synced();
    assert!(w.feed(new_msg(PEER, 1, json!({}))).is_some());
    let last = |id| {
        json!({"@type": "updateChatLastMessage", "chat_id": PEER,
        "last_message": msg(PEER, id, NOW_S - 1, json!({})), "positions": []})
    };
    // 同一条又随「最后一条变了」来：不再判
    assert!(w.feed(last(1)).is_none());
    // 只随「最后一条变了」到达的新消息：判
    assert_eq!(w.reason(last(2)), (true, Reason::New));
    // 删掉最新的，「最后一条」退回更旧的：不是新来的
    assert!(
        w.feed(json!({"@type": "updateChatLastMessage", "chat_id": PEER,
        "last_message": msg(PEER, 1, NOW_S - 1, json!({})), "positions": []}))
            .is_none()
    );
}
