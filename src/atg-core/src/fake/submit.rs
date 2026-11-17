//! 假后端的交出口里两种较长的：重发（`resendMessages`）与删除（`deleteMessages`），从 `fake.rs` 挪出

use super::{Call, FakeBackend, FakeSend, lock};
use crate::error::CoreError;
use crate::model::{ChatId, MsgId};
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

    /// 真发一条之后按剧本推结果（推在应答之前：结果比应答先到，发4），再把它登记成「交出成功、等成败」
    /// （同真实后端的交出线程，#88；已先推的成败登记时跳过）。从 `fake.rs` 挪来
    pub(super) fn play_send(&self, chat: ChatId, temp: i64, request: &Value) {
        let step = lock(&self.send_script)
            .pop_front()
            .unwrap_or(FakeSend::Succeed);
        match step {
            FakeSend::Succeed => {
                let id = self.new_id();
                let m = self.own_message(chat, id, request, Value::Null);
                // 发出的那条留在会话里（之后 `message_get` / `failed_info` 找得到）
                lock(&self.histories)
                    .entry(chat)
                    .or_default()
                    .push(m.clone());
                self.push(json!({"@type": "updateMessageSendSucceeded", "old_message_id": temp, "message": m}));
            }
            FakeSend::Fail {
                code,
                message,
                can_retry,
            } => {
                let id = self.new_id();
                let err = json!({"@type": "error", "code": code, "message": message});
                let st = json!({"@type": "messageSendingStateFailed", "error": err.clone(),
                    "can_retry": can_retry, "need_another_sender": false, "need_another_reply_quote": false,
                    "need_drop_reply": false, "required_paid_message_star_count": 0, "retry_after": 0.0});
                let m = self.own_message(chat, id, request, st);
                // 失败的那条留在会话里（重发时要找得到）
                lock(&self.histories)
                    .entry(chat)
                    .or_default()
                    .push(m.clone());
                self.push(json!({"@type": "updateMessageSendFailed", "old_message_id": temp, "message": m, "error": err}));
            }
            FakeSend::Delete => self.push(json!({"@type": "updateDeleteMessages", "chat_id": chat,
                "message_ids": [temp], "is_permanent": true, "from_cache": false})),
            FakeSend::Silent | FakeSend::Reject(_) => {}
        }
        self.settle();
        self.accepted(chat, temp);
    }

    /// 交出成功、成败还要等的这条：登记进发送限制
    fn accepted(&self, chat: ChatId, temp: i64) {
        use crate::backend::Backend;
        self.gate().limits().await_outcome(&[MsgId {
            chat_id: chat,
            message_id: temp,
        }]);
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
