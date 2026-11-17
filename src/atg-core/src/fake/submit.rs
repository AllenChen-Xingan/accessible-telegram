//! 假后端的交出口（发文字、重发、表情回应）与「谁点了回应」（#9）。从 `fake` 拆出（#60 行数线）。
//!
//! 表情回应：改那条消息的 `interaction_info.reactions` 并推 `updateMessageInteractionInfo`；「谁点了」记在消息
//! 对象的私有键 `_added`（TDLib `addedReaction` 的数组，测试也可直接放进历史里），`tdjson::message` 不认它

use super::{Call, FakeBackend, FakeSend, lock};
use crate::error::CoreError;
use crate::model::{AddedReactions, MsgId};
use crate::send::{Answer, Sealed, SealedKind};
use crate::tree::ReactionKind;
use serde_json::{Value, json};

impl FakeBackend {
    pub(super) async fn do_submit(&self, req: Sealed<'_>) -> Result<Answer, CoreError> {
        self.started()?;
        self.ready_or_401("submit").await?;
        match req.into_kind() {
            SealedKind::Message { request, preview } => {
                let chat = request["chat_id"].as_i64().unwrap_or(0);
                lock(&self.calls).push(Call::Submit {
                    chat,
                    preview,
                    resend: false,
                });
                if !lock(&self.chat_info).contains_key(&chat) {
                    return Err(CoreError::NotFound {
                        what: "chat",
                        td: None,
                    });
                }
                if preview {
                    let hook = lock(&self.preview_hook).take();
                    if let Some(hook) = hook {
                        hook();
                    }
                    let mut r = request.clone();
                    if lock(&self.preview_drops).pop_front().unwrap_or(false) {
                        r["reply_to"] = Value::Null;
                    }
                    let m = self.own_message(chat, 0, &r, Value::Null);
                    return Ok(Answer::Message(crate::tdjson::message(&m)));
                }
                if let Some(e) = self.scripted_reject() {
                    return Err(e);
                }
                lock(&self.submitted).push(request.clone());
                let temp = self.new_id();
                let pending = self.own_message(
                    chat,
                    temp,
                    request,
                    json!({"@type": "messageSendingStatePending", "sending_id": request["options"]["sending_id"].clone()}),
                );
                self.push(json!({"@type": "updateNewMessage", "message": pending.clone()}));
                self.play_send(chat, temp, request);
                Ok(Answer::Message(crate::tdjson::message(&pending)))
            }
            SealedKind::Resend { request } => {
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
                    lock(&self.submitted).push(
                        json!({"@type": "resendMessages", "chat_id": chat, "message_ids": [id]}),
                    );
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
                Ok(Answer::Resent(out))
            }
            SealedKind::React { request } => {
                let chat = request["chat_id"].as_i64().unwrap_or(0);
                lock(&self.calls).push(Call::Submit {
                    chat,
                    preview: false,
                    resend: false,
                });
                if let Some(e) = self.scripted_reject() {
                    return Err(e);
                }
                lock(&self.submitted).push(request.clone());
                self.apply_reaction(request)?;
                Ok(Answer::Done)
            }
        }
    }

    /// 剧本里下一步是「拒」就取出来
    fn scripted_reject(&self) -> Option<CoreError> {
        let mut script = lock(&self.send_script);
        match script.front() {
            Some(FakeSend::Reject(e)) => {
                let e = e.clone();
                script.pop_front();
                Some(e)
            }
            _ => None,
        }
    }

    /// 在那条消息上加 / 去掉我的回应，推一条 `updateMessageInteractionInfo`
    fn apply_reaction(&self, request: &Value) -> Result<(), CoreError> {
        let (chat, msg) = (request["chat_id"].as_i64(), request["message_id"].as_i64());
        let remove = request["@type"] == "removeMessageReaction";
        let ty = request["reaction_type"].clone();
        let me = lock(&self.me_id).unwrap_or(0);
        let mut hs = lock(&self.histories);
        let m = hs
            .get_mut(&chat.unwrap_or(0))
            .and_then(|ms| ms.iter_mut().find(|m| m["id"].as_i64() == msg))
            .ok_or(CoreError::NotFound {
                what: "message",
                td: None,
            })?;
        let mut list: Vec<Value> = m["interaction_info"]["reactions"]["reactions"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        // 非会员每条只能点 1 个：加上新的就挤掉我原先点的别的（TDLib 的上限行为）
        if !remove {
            for r in list
                .iter_mut()
                .filter(|r| r["is_chosen"] == true && r["type"] != ty)
            {
                r["total_count"] = json!(r["total_count"].as_i64().unwrap_or(1) - 1);
                r["is_chosen"] = json!(false);
            }
            list.retain(|r| r["total_count"].as_i64().unwrap_or(0) > 0);
        }
        let pos = list.iter().position(|r| r["type"] == ty);
        match (pos, remove) {
            (Some(i), true) if list[i]["is_chosen"] == true => {
                let n = list[i]["total_count"].as_i64().unwrap_or(1) - 1;
                if n <= 0 {
                    list.remove(i);
                } else {
                    list[i]["total_count"] = json!(n);
                    list[i]["is_chosen"] = json!(false);
                }
            }
            (Some(i), false) if list[i]["is_chosen"] != true => {
                let n = list[i]["total_count"].as_i64().unwrap_or(0) + 1;
                list[i]["total_count"] = json!(n);
                list[i]["is_chosen"] = json!(true);
            }
            (None, false) => list.push(json!({"@type": "messageReaction", "type": ty.clone(),
                "total_count": 1, "is_chosen": true, "recent_sender_ids": []})),
            _ => {} // 已经是要的样子：TDLib 也不报错
        }
        let mut added: Vec<Value> = m["_added"].as_array().cloned().unwrap_or_default();
        added.retain(|a| a["is_outgoing"] != true || (remove && a["type"] != ty));
        if !remove {
            added.insert(
                0,
                json!({"@type": "addedReaction", "type": ty, "is_outgoing": true,
                "sender_id": {"@type": "messageSenderUser", "user_id": me}, "date": 1_792_755_480}),
            );
        }
        m["_added"] = json!(added);
        m["interaction_info"]["reactions"] = json!({"@type": "messageReactions", "reactions": list,
            "are_tags": false, "paid_reactors": [], "can_get_added_reactions": true});
        let info = m["interaction_info"].clone();
        drop(hs);
        self.push(
            json!({"@type": "updateMessageInteractionInfo", "chat_id": chat,
            "message_id": msg, "interaction_info": info}),
        );
        Ok(())
    }

    /// 「谁点了回应」：从消息的 `_added` 里按种类筛、按 `offset`（序号）分页
    pub(super) async fn do_added_reactions(
        &self,
        id: MsgId,
        kind: Option<&ReactionKind>,
        offset: &str,
        limit: u32,
    ) -> Result<AddedReactions, CoreError> {
        self.started()?;
        self.ready_or_401("added_reactions").await?;
        let m = self.find_message(id).ok_or(CoreError::NotFound {
            what: "message",
            td: None,
        })?;
        let want = kind.and_then(crate::tdjson::reaction_type);
        let all: Vec<Value> = m["_added"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|a| want.as_ref().is_none_or(|w| &a["type"] == w))
            .collect();
        let from: usize = offset.parse().unwrap_or(0);
        let to = all.len().min(from + limit as usize);
        let next = if to < all.len() {
            to.to_string()
        } else {
            String::new()
        };
        let page = all.get(from..to).unwrap_or(&[]);
        Ok(crate::tdjson::added_reactions(
            &json!({"@type": "addedReactions",
            "total_count": all.len(), "reactions": page, "next_offset": next}),
        ))
    }
}
