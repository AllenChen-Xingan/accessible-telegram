#[cfg(feature = "tdlib-api")]
mod roundtrip {
    use crate::tdjson;
    use serde_json::{Value, json};
    use tdlib_rs::enums::Update;

    /// 把 `patch` 深合并到 `base` 上
    fn merge(base: &mut Value, patch: Value) {
        match (base, patch) {
            (Value::Object(b), Value::Object(p)) => {
                for (k, v) in p {
                    merge(b.entry(k).or_insert(Value::Null), v);
                }
            }
            (b, p) => *b = p,
        }
    }

    fn text(t: &str) -> Value {
        json!({"@type": "formattedText", "text": t, "entities": [
            {"@type": "textEntity", "offset": 0, "length": 2, "type": {"@type": "textEntityTypeBold"}}
        ]})
    }

    /// TDLib 真会给的整条消息：必带字段齐全（编的数据，id 在保留区间）
    fn message(id: i64, patch: Value) -> Value {
        let mut m = json!({
            "@type": "message", "id": id, "chat_id": 777,
            "sender_id": {"@type": "messageSenderUser", "user_id": 777},
            "is_outgoing": false, "is_pinned": false, "is_from_offline": false, "can_be_saved": true,
            "has_timestamped_media": false, "is_channel_post": false,
            "is_paid_star_suggested_post": false, "is_paid_gram_suggested_post": false,
            "contains_unread_mention": false, "contains_unread_poll_votes": false,
            "date": 1792755420, "edit_date": 0, "unread_reactions": [],
            "self_destruct_in": 0.0, "auto_delete_in": 0.0, "via_bot_user_id": 0,
            "sender_business_bot_user_id": 0, "sender_boost_count": 0, "sender_tag": "",
            "paid_message_star_count": 0, "author_signature": "", "media_album_id": "0",
            "effect_id": "0", "summary_language_code": "",
            "content": {"@type": "messageText", "text": text("晚上好")},
        });
        merge(&mut m, patch);
        m
    }

    fn position(order: &str) -> Value {
        json!({"@type": "chatPosition", "list": {"@type": "chatListMain"}, "order": order, "is_pinned": false})
    }

    fn samples() -> Vec<Value> {
        vec![
            json!({"@type": "updateNewMessage", "message": message(5, json!({}))}),
            json!({"@type": "updateNewMessage", "message": message(6, json!({
                "edit_date": 1792755500,
                "reply_to": {"@type": "messageReplyToMessage", "chat_id": 777, "message_id": 5,
                             "checklist_task_id": 0, "poll_option_id": "", "origin_send_date": 0},
                "forward_info": {"@type": "messageForwardInfo", "date": 1792755000,
                    "origin": {"@type": "messageOriginHiddenUser", "sender_name": "苏木"},
                    "public_service_announcement_type": ""},
                "sending_state": {"@type": "messageSendingStatePending", "sending_id": 0},
            }))}),
            json!({"@type": "updateNewMessage", "message": message(7, json!({
                "content": {"@type": "messageChatChangeTitle", "title": "周末读书会"},
            }))}),
            json!({"@type": "updateMessageContent", "chat_id": 777, "message_id": 5,
               "new_content": {"@type": "messageText", "text": text("改过的")}}),
            json!({"@type": "updateMessageEdited", "chat_id": 777, "message_id": 5, "edit_date": 1792755600}),
            json!({"@type": "updateDeleteMessages", "chat_id": 777, "message_ids": [5, 6],
               "is_permanent": true, "from_cache": false}),
            json!({"@type": "updateMessageSendFailed", "old_message_id": 6,
               "message": message(8, json!({"sending_state": {"@type": "messageSendingStateFailed",
                    "error": {"@type": "error", "code": 400, "message": "MESSAGE_TOO_LONG"},
                    "can_retry": true, "need_another_sender": false, "need_another_reply_quote": false,
                    "need_drop_reply": false, "required_paid_message_star_count": 0, "retry_after": 0.0}})),
               "error": {"@type": "error", "code": 400, "message": "MESSAGE_TOO_LONG"}}),
            json!({"@type": "updateChatPosition", "chat_id": 777, "position": position("200")}),
            json!({"@type": "updateChatLastMessage", "chat_id": 777, "last_message": message(5, json!({})),
               "positions": [position("300")]}),
            json!({"@type": "updateChatDraftMessage", "chat_id": 777, "positions": [position("400")]}),
            json!({"@type": "updateChatReadInbox", "chat_id": 777, "last_read_inbox_message_id": 5, "unread_count": 2}),
            json!({"@type": "updateChatTitle", "chat_id": 777, "title": "林小雨"}),
            json!({"@type": "updateConnectionState", "state": {"@type": "connectionStateReady"}}),
            json!({"@type": "updateAuthorizationState",
               "authorization_state": {"@type": "authorizationStateWaitPhoneNumber"}}),
        ]
    }

    /// 映射只有一份的前提：tdlib-rs 的 `Update` 序列化回去，与 TDLib 原样 JSON 走 `tdjson::classify` / `event`
    /// 得到同一个结果（`td` 的来源就是这么喂收线程的）
    #[test]
    fn reserialized_updates_map_to_the_same_event() {
        // 防空转：样本确实映射出了有内容的消息，回复、转发、待发送都认出来了
        use crate::model::{Body, CoreEvent, SendingState};
        let s = samples();
        let CoreEvent::NewMessage(m) = tdjson::event(&s[1]) else {
            panic!("样本 1 不是新消息");
        };
        assert!(m.reply_to.is_some() && m.forward.is_some() && m.edited.is_some());
        assert_eq!(m.sending, SendingState::Pending);
        assert!(matches!(&m.content, Body::Content(c) if !c.body.is_empty()));
        assert!(matches!(
            tdjson::event(&s[6]),
            CoreEvent::SendFailed {
                can_retry: true,
                ..
            }
        ));
        for v in s {
            let ty = v["@type"].as_str().unwrap_or("?").to_owned();
            let update: Update = match serde_json::from_value(v.clone()) {
                Ok(u) => u,
                Err(e) => panic!("{ty} 解析不了：{e}"),
            };
            let round = serde_json::to_value(&update).unwrap();
            assert_eq!(tdjson::event(&round), tdjson::event(&v), "{ty}");
            match (tdjson::classify(&round), tdjson::classify(&v)) {
                (tdjson::Classified::Auth(a), tdjson::Classified::Auth(b)) => assert_eq!(a, b),
                (tdjson::Classified::Net(a), tdjson::Classified::Net(b)) => assert_eq!(a, b),
                (tdjson::Classified::Event(a), tdjson::Classified::Event(b)) => {
                    assert_eq!(a, b)
                }
                _ => panic!("{ty} 去向不同"),
            }
        }
    }
}
