//! 假后端的交出口（发文字、重发、删除、表情回应）与「谁点了回应」（#9）。从 `fake` 拆出（#60 行数线）。
//! 交出成功、成败还要等的（发文字、重发）在 [`FakeBackend::play_send`] 里登记进发送限制（#88）；表情回应没有临时 id，
//! 交出期间由 `send_limits::submit` 的守卫盖着。
//!
//! 表情回应：改那条消息的 `interaction_info.reactions` 并推 `updateMessageInteractionInfo`；「谁点了」记在消息
//! 对象的私有键 `_added`（TDLib `addedReaction` 的数组，测试也可直接放进历史里），`tdjson::message` 不认它

use super::{Call, FakeBackend, FakeSend, lock};
use crate::error::CoreError;
use crate::model::{AddedReactions, ChatId, MsgId};
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
            SealedKind::Resend { request } => Ok(self.resend(request)),
            SealedKind::Delete { request } => self.delete(request).await,
            SealedKind::Forward { request } => self.forward(request),
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
        // 测试夹具：消息带 `_added_fail` 时模拟服务器不给名单
        if m["_added_fail"].as_bool() == Some(true) {
            return Err(CoreError::Network { td: None });
        }
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

    /// `forwardMessages`（#10）：来源里在、能转（没被 [`FakeBackend::deny`] 关掉转发）的逐条造一条待发的，按剧本推结果；
    /// 转不了的那条应答里是 `None`（同 TDLib 的 null）；消息带私有键 `_forward_null` 时也给 `None`（模拟准备时能转、
    /// 交出时 TDLib 才说转不了）。剧本下一步是「拒」时整批拒
    pub(super) fn forward(&self, request: &Value) -> Result<Answer, CoreError> {
        let chat = request["chat_id"].as_i64().unwrap_or(0);
        let from = request["from_chat_id"].as_i64().unwrap_or(0);
        let ids: Vec<i64> = request["message_ids"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default();
        lock(&self.calls).push(Call::Submit {
            chat,
            preview: false,
            resend: false,
        });
        if let Some(e) = self.scripted_reject() {
            return Err(e);
        }
        lock(&self.submitted).push(request.clone());
        let mut out = Vec::new();
        for id in ids {
            let old = self.find_message(MsgId {
                chat_id: from,
                message_id: id,
            });
            let fwd = self.caps(MsgId {
                chat_id: from,
                message_id: id,
            });
            let ok = fwd.is_ok_and(|c| c.can_forward);
            let Some(old) = old.filter(|m| ok && m["_forward_null"] != true) else {
                out.push(None);
                continue;
            };
            let temp = self.new_id();
            let req = json!({"input_message_content": {"text": old["content"]["text"].clone()},
                "topic_id": request["topic_id"].clone(), "reply_to": null});
            let pending = self.own_message(
                chat,
                temp,
                &req,
                json!({"@type": "messageSendingStatePending", "sending_id": 0}),
            );
            self.push(json!({"@type": "updateNewMessage", "message": pending.clone()}));
            self.play_send(chat, temp, &req);
            out.push(Some(crate::tdjson::message(&pending)));
        }
        // 测试夹具：来源第一条带 `_forward_answer` 时造认不出的应答（条数少一条 / 形状不对 / 回序列化失败）
        let first = request["message_ids"][0].as_i64().unwrap_or(0);
        let odd = self.find_message(MsgId {
            chat_id: from,
            message_id: first,
        });
        match odd.as_ref().and_then(|m| m["_forward_answer"].as_str()) {
            Some("short") => {
                out.pop();
                Ok(Answer::Resent(out))
            }
            Some("shape") => Ok(Answer::Done),
            Some("reserialize") => Err(CoreError::Internal {
                what: "td_message_reserialize_failed",
            }),
            _ => Ok(Answer::Resent(out)),
        }
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
