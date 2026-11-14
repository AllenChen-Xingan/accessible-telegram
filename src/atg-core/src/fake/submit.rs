//! 假后端的交出口里两种较长的：重发（`resendMessages`）与删除（`deleteMessages`），从 `fake.rs` 挪出

use super::{Call, FakeBackend, lock};
use crate::error::CoreError;
use crate::model::MsgId;
use crate::send::Answer;
use serde_json::{Value, json};

impl FakeBackend {
    /// `resendMessages`：能重发的先删掉失败那条（不是撤回），再造一条待发的，按脚本推结果
    pub(super) fn resend(&self, request: &Value) -> Answer {
        let chat = request["chat_id"].as_i64().unwrap_or(0);
        let ids: Vec<i64> = request["message_ids"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default();
        lock(&self.calls).push(Call::Submit {
            chat,
            preview: false,
            resend: true,
        });
        let mut out = Vec::new();
        for id in ids {
            let old = self.find_message(MsgId {
                chat_id: chat,
                message_id: id,
            });
            let ok = old
                .as_ref()
                .and_then(crate::tdjson::failed_info)
                .is_some_and(|f| f.can_retry);
            let Some(old) = old.filter(|_| ok) else {
                out.push(None);
                continue;
            };
            lock(&self.submitted)
                .push(json!({"@type": "resendMessages", "chat_id": chat, "message_ids": [id]}));
            let temp = self.new_id();
            let request = json!({"input_message_content": {"text": old["content"]["text"].clone()},
                "topic_id": old["topic_id"].clone(), "reply_to": null});
            let pending = self.own_message(
                chat,
                temp,
                &request,
                json!({"@type": "messageSendingStatePending", "sending_id": 0}),
            );
            // TDLib 重发时先删掉失败那条（不是撤回），再造一条待发的
            lock(&self.histories)
                .entry(chat)
                .or_default()
                .retain(|m| m["id"].as_i64() != Some(id));
            self.push(json!({"@type": "updateDeleteMessages", "chat_id": chat,
                "message_ids": [id], "is_permanent": true, "from_cache": false}));
            self.push(json!({"@type": "updateNewMessage", "message": pending.clone()}));
            self.play_send(chat, temp, &request);
            out.push(Some(crate::tdjson::message(&pending)));
        }
        Answer::Resent(out)
    }

    /// `deleteMessages`：记账、记下请求，从历史里拿掉，推一条永久删除（与 TDLib 一样，待发的也是这么没的）。
    /// 可用 [`FakeBackend::fail_next`]`("delete", …)` 注入一次错误
    pub(super) async fn delete(&self, request: &Value) -> Result<Answer, CoreError> {
        self.injected("delete").await?;
        let chat = request["chat_id"].as_i64().unwrap_or(0);
        let ids: Vec<i64> = request["message_ids"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default();
        lock(&self.calls).push(Call::Delete {
            chat,
            ids: ids.clone(),
            revoke: request["revoke"].as_bool().unwrap_or(false),
        });
        lock(&self.submitted).push(request.clone());
        lock(&self.histories)
            .entry(chat)
            .or_default()
            .retain(|m| m["id"].as_i64().is_none_or(|i| !ids.contains(&i)));
        self.push(json!({"@type": "updateDeleteMessages", "chat_id": chat,
            "message_ids": ids, "is_permanent": true, "from_cache": false}));
        Ok(Answer::Deleted)
    }
}
