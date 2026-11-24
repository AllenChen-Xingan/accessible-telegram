//! 造 TDLib JSON 的测试助手（#60 R3b，`docs/plan/010_complexity-budget.md` S11）。
//!
//! 只收在好几个测试里逐字重复的那几种；场景特有的字段由调用方用 [`with`] 补上，或者在本地包一层。
//! 名字用 `src/atg-core/fixtures/names.txt` 里的，id 用测试 id，日期写死假日期（由调用方传）。
//! 同 `testkit` 的规矩：开特性时不算 `cfg(test)`，这里不用 `unwrap` / `expect` / `{:?}`。
//! 只返回 `serde_json::Value`，所以 core 单元测试写 `crate::testkit::td`、别处写 `atg_core::testkit::td` 都行。

use serde_json::{Value, json};

/// `updateUser`：普通用户、是联系人、没有用户名和号码
pub fn user(id: i64, name: &str) -> Value {
    json!({"@type": "updateUser", "user": {"@type": "user", "id": id, "first_name": name, "last_name": "",
        "usernames": null, "phone_number": "", "is_contact": true, "type": {"@type": "userTypeRegular"}}})
}

/// `updateNewChat`：会话种类由 `ty` 给，没有列表位置，通知设置是空对象
pub fn new_chat(id: i64, title: &str, ty: Value) -> Value {
    json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title, "type": ty,
        "positions": [], "notification_settings": {}}})
}

/// 私聊（`chatTypePrivate`，`user_id` 就是会话 id）；同 [`new_chat`]，通知设置是空对象
pub fn new_private_chat(id: i64, title: &str) -> Value {
    new_chat(id, title, private_type(id))
}

/// 私聊，通知设置写全：按默认、不静音（`chatNotificationSettings`）
pub fn new_private_chat_notify_default(id: i64, title: &str) -> Value {
    json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title,
        "type": private_type(id), "positions": [],
        "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0}}})
}

fn private_type(id: i64) -> Value {
    json!({"@type": "chatTypePrivate", "user_id": id})
}

/// `messageText`（没有格式实体）
pub fn text(t: &str) -> Value {
    json!({"@type": "messageText", "text": {"@type": "formattedText", "text": t, "entities": []}})
}

/// `message`：`sender` 发的、不是自己发的（`is_outgoing: false`）、日期 `date`（Unix 秒）、正文 `content`
pub fn message(chat: i64, id: i64, sender: i64, date: i64, content: Value) -> Value {
    json!({"@type": "message", "id": id, "chat_id": chat,
        "sender_id": {"@type": "messageSenderUser", "user_id": sender}, "is_outgoing": false,
        "date": date, "content": content})
}

/// 把 `extra`（JSON 对象）的字段并进 `v`，同名的覆盖；`v` 或 `extra` 不是对象就原样交回 `v`
pub fn with(mut v: Value, extra: Value) -> Value {
    if let (Some(o), Value::Object(x)) = (v.as_object_mut(), extra) {
        o.extend(x);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_chat_is_new_chat_of_private_type() {
        let c = new_private_chat(7, "林");
        assert_eq!(c["chat"]["type"]["user_id"], 7);
        assert_eq!(
            c,
            new_chat(7, "林", json!({"@type": "chatTypePrivate", "user_id": 7}))
        );
        assert_eq!(
            new_private_chat_notify_default(7, "林")["chat"]["notification_settings"]["mute_for"],
            0
        );
    }

    #[test]
    fn with_merges_and_overrides() {
        let m = with(
            message(1, 2, 3, 4, text("x")),
            json!({"topic_id": null, "date": 9}),
        );
        assert_eq!(m["date"], 9);
        assert!(m.as_object().is_some_and(|o| o.contains_key("topic_id")));
        assert_eq!(with(json!(1), json!({"a": 1})), json!(1));
        assert_eq!(with(json!({"a": 1}), json!(2)), json!({"a": 1}));
    }
}
