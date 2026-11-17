//! 假后端的 `editMessageText`（#8 B）：照 TDLib 的样子，服务器「改完」才应答——改历史里那条的正文与 `edit_date`，
//! 推 `updateMessageContent` + `updateMessageEdited`，应答改后的那条。可用 [`FakeBackend::fail_next`]`("edit", …)`
//! 注入一次错误（含 [`FakeBackend::HANG`] 一直不应答）

use super::{Call, FakeBackend, lock};
use crate::error::CoreError;
use crate::send::Answer;
use serde_json::{Value, json};

/// 改后的 `edit_date`（编的，比夹具里的发出时间晚）
const EDIT_DATE: i64 = 1_792_760_000;

impl FakeBackend {
    pub(super) async fn edit(&self, request: &Value) -> Result<Answer, CoreError> {
        self.injected("edit").await?;
        let chat = request["chat_id"].as_i64().unwrap_or(0);
        let id = request["message_id"].as_i64().unwrap_or(0);
        let text = request["input_message_content"]["text"].clone();
        lock(&self.calls).push(Call::Edit {
            chat,
            id,
            text: text["text"].as_str().unwrap_or_default().to_owned(),
        });
        lock(&self.submitted).push(request.clone());
        let content = json!({"@type": "messageText", "text": text});
        let edited = {
            let mut h = lock(&self.histories);
            let m = h
                .entry(chat)
                .or_default()
                .iter_mut()
                .find(|m| m["id"].as_i64() == Some(id))
                .ok_or(CoreError::NotFound {
                    what: "message",
                    td: None,
                })?;
            m["content"] = content.clone();
            m["edit_date"] = json!(EDIT_DATE);
            m.clone()
        };
        self.push(
            json!({"@type": "updateMessageContent", "chat_id": chat, "message_id": id,
            "new_content": content}),
        );
        self.push(
            json!({"@type": "updateMessageEdited", "chat_id": chat, "message_id": id,
            "edit_date": EDIT_DATE, "reply_markup": null}),
        );
        Ok(Answer::Message(crate::tdjson::message(&edited)))
    }
}
