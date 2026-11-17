//! 假时钟单测：时间全是手拨的数字；会话名、人名只用 `fixtures/names.txt` 里的
//! 本文件只放夹具（假 store、造 TDLib JSON 的助手）；用例按主题在 `tests/{messages,sync,typing}.rs`。
use super::*;
use crate::model::ChatNotify;
use crate::model::ForumTopicState;
use crate::store::Input;
use crate::tdjson;
use serde_json::{Value, json};

const ME: i64 = 9;
const PEER: i64 = 101;
const GROUP: i64 = -1001;
const FORUM: i64 = -1002;
/// 假时钟的「现在」：2026-10-26 某时（秒）
const NOW_S: i64 = 1_793_000_000;
const NOW: i64 = NOW_S * 1000;

struct W {
    store: Store,
    policy: Policy,
    seq: u64,
    now: i64,
}

fn notif(mute_for: i64, use_default: bool) -> Value {
    json!({"@type": "chatNotificationSettings", "use_default_mute_for": use_default, "mute_for": mute_for,
           "use_default_disable_mention_notifications": true, "disable_mention_notifications": false,
           "use_default_disable_pinned_message_notifications": true, "disable_pinned_message_notifications": false})
}

fn chat(id: i64, ty: Value, title: &str, n: Value) -> Value {
    json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title, "type": ty,
           "positions": [], "unread_count": 0, "last_read_inbox_message_id": 0,
           "notification_settings": n}})
}

fn private() -> Value {
    json!({"@type": "chatTypePrivate", "user_id": PEER})
}

fn group() -> Value {
    json!({"@type": "chatTypeSupergroup", "supergroup_id": 1, "is_channel": false})
}

fn msg(chat_id: i64, id: i64, date: i64, patch: Value) -> Value {
    let mut m = json!({"@type": "message", "id": id, "chat_id": chat_id, "date": date,
        "sender_id": {"@type": "messageSenderUser", "user_id": PEER}, "is_outgoing": false,
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": "第一条", "entities": []}}});
    if let (Some(o), Value::Object(p)) = (m.as_object_mut(), patch) {
        o.extend(p);
    }
    m
}

fn new_msg(chat_id: i64, id: i64, patch: Value) -> Value {
    json!({"@type": "updateNewMessage", "message": msg(chat_id, id, NOW_S - 5, patch)})
}

fn scope(ty: &str, mute_for: i64, disable_mention: bool) -> Value {
    json!({"@type": "updateScopeNotificationSettings", "scope": {"@type": ty},
           "notification_settings": {"@type": "scopeNotificationSettings", "mute_for": mute_for,
           "disable_mention_notifications": disable_mention, "disable_pinned_message_notifications": false}})
}

impl W {
    fn new() -> Self {
        let mut w = Self {
            store: Store::new(),
            policy: Policy::new(Config::default()),
            seq: 0,
            now: NOW,
        };
        w.feed(json!({"@type": "updateOption", "name": "my_id", "value": {"@type": "optionValueInteger", "value": ME.to_string()}}));
        w.feed(chat(PEER, private(), "林小雨", notif(0, true)));
        w.feed(chat(GROUP, group(), "周末爬山群", notif(0, true)));
        w.feed(chat(FORUM, group(), "周末爬山群（十月）", notif(0, true)));
        w
    }

    fn synced() -> Self {
        let mut w = Self::new();
        w.event(CoreEvent::SyncDone);
        w
    }

    fn event(&mut self, e: CoreEvent) -> Option<Judged> {
        // 收线程那一步：话题设置记下收到时刻（假时钟）
        let e = match e {
            CoreEvent::ForumTopicChanged {
                chat_id,
                topic_id,
                last_read_inbox,
                notify,
                received_ms: None,
            } => CoreEvent::ForumTopicChanged {
                chat_id,
                topic_id,
                last_read_inbox,
                notify,
                received_ms: Some(self.now),
            },
            other => other,
        };
        self.seq += 1;
        self.store.apply(Input::Update {
            seq: self.seq,
            event: e.clone(),
        });
        self.policy.observe(&self.store, &e, self.now)
    }

    fn feed(&mut self, v: Value) -> Option<Judged> {
        self.event(tdjson::event(&v))
    }

    fn verdict(&mut self, v: Value) -> Verdict {
        match self.feed(v) {
            Some(
                Judged::Message { verdict, .. }
                | Judged::Typing { verdict, .. }
                | Judged::Summary { verdict, .. },
            ) => verdict,
            None => panic!("没有判定"),
        }
    }

    fn reason(&mut self, v: Value) -> (bool, Reason) {
        let v = self.verdict(v);
        (v.alert, v.reason)
    }
}

mod messages;
mod sync;
mod typing;
