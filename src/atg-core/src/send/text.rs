//! 发文字：准备（[`prepare_text`]）→ 交出（[`send_text`]）→ 等结果的全程（[`send_and_wait`]），以及它的计划行。流程与规则见模块根 `send.rs` 的头注释。

use super::*;

// ───────────────────────── 发文字 ─────────────────────────

/// 一份已定稿的文字发送：确认给人看的就是它，交给 TDLib 的也是它（摘要绑的就是 `request`）
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct TextPlan {
    target: ChatTarget,
    text: String,
    reply_to: Option<MsgId>,
    /// 最终话题（被回复那条的话题；只认论坛话题与讨论串）
    topic: Option<Topic>,
    request: Value,
    digest: [u8; 32],
    intent: [u8; 32],
    origin: SendOrigin,
    mode: ConfirmMode,
}

impl TextPlan {
    pub fn target(&self) -> &ChatTarget {
        &self.target
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn reply_to(&self) -> Option<MsgId> {
        self.reply_to
    }
    pub fn topic(&self) -> Option<&Topic> {
        self.topic.as_ref()
    }
    /// 闸会用哪种方式确认（准备时算的；开凭据时还会再判一次）
    pub fn mode(&self) -> ConfirmMode {
        self.mode
    }
    pub fn origin(&self) -> SendOrigin {
        self.origin
    }
    pub fn digest_hex(&self) -> String {
        gate::hex(&self.digest)
    }
}

impl gate::Sealed for TextPlan {}
impl Gated for TextPlan {
    fn target(&self) -> ChatId {
        self.target.id
    }
    fn digest(&self) -> [u8; 32] {
        self.digest
    }
    fn intent(&self) -> [u8; 32] {
        self.intent
    }
    fn dedup(&self) -> bool {
        true
    }
    fn kind(&self) -> &'static str {
        "text"
    }
    fn confirm_text(&self) -> String {
        // VoiceOver 会读这段：发给谁放最前，句子短；正文放最后（太长被截也只截正文，且已明说）。措辞待人实听
        let reply = self
            .reply_to
            .map(|r| format!("，回复 id {} 那条", r.message_id))
            .unwrap_or_default();
        format!(
            "发给 {}{reply}\n{}，{}\n{}",
            target_zh(&self.target),
            origin_zh(self.origin),
            CONFIRM_QUEUE_ZH,
            confirm_body(&self.text),
        )
    }
}

/// 要发什么
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct Draft {
    pub text: String,
    /// 回复同一会话里的哪条
    pub reply_to: Option<i64>,
}

/// 只认论坛话题与讨论串：这两种要随请求带过去（话题取被回复那条的，发1）。
/// - **论坛话题**：开了「论坛」的超级群里，消息分在一个个有名字的话题里，回复落错话题别人就看不到上下文
/// - **讨论串**：频道帖子下的评论、普通超级群里某条消息底下的回复串
/// - 另两种第一阶段不带、交给 TDLib 按回复关系定：**收藏夹分组**（自己收藏夹里按来源自动分的组）、
///   **频道私信话题**（频道管理员收到的私信，按发信人分）——它们只在自己的收藏夹 / 管理的频道里出现
pub(super) fn send_topic(t: Option<&Topic>) -> Option<Topic> {
    match t {
        Some(t @ (Topic::Forum { .. } | Topic::Thread { .. })) => Some(t.clone()),
        _ => None,
    }
}

fn topic_json(t: Option<&Topic>) -> Value {
    match t {
        Some(Topic::Forum { topic_id, .. }) => {
            json!({"@type": "messageTopicForum", "forum_topic_id": topic_id})
        }
        Some(Topic::Thread { thread_id }) => {
            json!({"@type": "messageTopicThread", "message_thread_id": thread_id})
        }
        _ => Value::Null,
    }
}

/// 组 `sendMessage` 的请求（TDLib JSON 形状，字段顺序固定，摘要据此算）。`clear_draft=false`（发2）
pub(crate) fn text_request(
    chat: ChatId,
    text: &str,
    reply_to: Option<i64>,
    topic: Option<&Topic>,
    sending_id: i32,
) -> Value {
    json!({
        "@type": "sendMessage",
        "chat_id": chat,
        "topic_id": topic_json(topic),
        "reply_to": reply_to.map_or(Value::Null, |id| json!({
            "@type": "inputMessageReplyToMessage",
            "message_id": id,
            "quote": null,
            "checklist_task_id": 0,
            "poll_option_id": "",
        })),
        "options": {
            "@type": "messageSendOptions",
            "suggested_post_info": null,
            "disable_notification": false,
            "from_background": false,
            "paid_message_star_count": 0,
            "update_order_of_installed_sticker_sets": false,
            "scheduling_state": null,
            "effect_id": "0",
            "sending_id": sending_id,
            "only_preview": false,
        },
        "input_message_content": {
            "@type": "inputMessageText",
            "text": {"@type": "formattedText", "text": text, "entities": []},
            "link_preview_options": null,
            "clear_draft": false,
        },
    })
}

/// `sending_id`：我们自定的一个数，TDLib 原样放回这条待发消息的状态里（`messageSendingStatePending.sending_id`）。
/// 「待发」那条的新消息更新可能比带临时 id 的应答先到，那时还不知道哪个临时 id 是自己这次的（同一进程里可能同时
/// 有别的发送）；带上它就能认出来（发7）。每次随机，所以防重复的「意图摘要」要把它清零，否则同一句话每次指纹都不同
fn random_sending_id() -> Result<i32, CoreError> {
    let mut b = [0u8; 4];
    getrandom::fill(&mut b).map_err(|_| CoreError::Internal { what: "random" })?;
    // 正数、非 0（0 在 TDLib 里表示「没给」）
    Ok(i32::from_le_bytes(b).checked_abs().unwrap_or(1).max(1))
}

/// `send_text_prepare`：只查不发，组好要确认的计划（含一次 TDLib 预览比对）
pub async fn prepare_text<B: Backend>(
    b: &B,
    chat: ChatId,
    draft: &Draft,
    origin: SendOrigin,
) -> Result<TextPlan, CoreError> {
    if draft.text.trim().is_empty() {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::EmptyText,
        });
    }
    if u64::try_from(draft.text.len()).unwrap_or(u64::MAX) > TEXT_MAX_BYTES {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::TooLong,
        });
    }
    let me = b.me().await?;
    let mut target = b.chat_target(chat).await?;
    target.saved = target.id == me.saved_chat_id;
    let mode = b.gate().mode_for(target.id)?;

    let (reply_to, topic) = match draft.reply_to {
        None => (None, None),
        Some(id) => {
            let mid = MsgId {
                chat_id: target.id,
                message_id: id,
            };
            let m = b.message_get(mid).await?;
            if !b.can_reply(mid).await? {
                return Err(CoreError::InvalidSend {
                    why: InvalidSendWhy::ReplyNotAllowed,
                });
            }
            (Some(mid), send_topic(m.topic.as_ref()))
        }
    };
    // 话题群里不回复任何一条的新消息：第一阶段拒发（`actions.md` `send_text_prepare`）。不带回复、不带话题的
    // 新消息 TDLib 会放进「General」那个默认话题，多半不是人想发的地方；要发新话题得先能选话题，那是界面的事
    if target.is_forum && reply_to.is_none() {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::ForumNeedsReply,
        });
    }

    let sending_id = random_sending_id()?;
    let request = text_request(
        target.id,
        &draft.text,
        reply_to.map(|r| r.message_id),
        topic.as_ref(),
        sending_id,
    );
    let server = server_of(b)?;
    let digest = gate::digest(&request, me.user_id, server);
    // 防重复看的是「同一意图」：每次都变的 sending_id 不算
    let mut stable = request.clone();
    stable["options"]["sending_id"] = json!(0);
    let intent = gate::digest(&stable, me.user_id, server);

    let plan = TextPlan {
        target,
        text: draft.text.clone(),
        reply_to,
        topic,
        request,
        digest,
        intent,
        origin,
        mode,
    };
    // 发6①：先让 TDLib 走一遍会话、权限、内容校验，看它会不会改掉回复与话题
    preview_check(b, &plan).await?;
    Ok(plan)
}

/// 预览一次，回复对象与话题和计划比对
async fn preview_check<B: Backend>(b: &B, plan: &TextPlan) -> Result<(), CoreError> {
    let sealed = Sealed {
        kind: SealedKind::Message {
            request: &plan.request,
            preview: true,
        },
    };
    match crate::send_limits::submit(b, sealed).await? {
        Answer::Message(m) => {
            check_kept(plan, &m).map_err(|reason| CoreError::SendRefused { reason, last: None })
        }
        Answer::Resent(_) => Err(CoreError::Internal {
            what: "preview_answer_shape",
        }),
    }
}

/// TDLib 给出的那条（预览或发出后的）有没有保住计划里的回复与话题（发6）
pub fn check_kept(plan: &TextPlan, m: &Message) -> Result<(), SendRefusedReason> {
    if let Some(want) = plan.reply_to {
        let kept = matches!(&m.reply_to, Some(ReplyRef::Message { target, .. }) if *target == want);
        if !kept {
            return Err(SendRefusedReason::ReplyDropped);
        }
    }
    if let Some(want) = &plan.topic
        && !same_topic(m.topic.as_ref(), want)
    {
        return Err(SendRefusedReason::TopicChanged);
    }
    Ok(())
}

fn same_topic(got: Option<&Topic>, want: &Topic) -> bool {
    match (got, want) {
        (Some(Topic::Forum { topic_id: a, .. }), Topic::Forum { topic_id: b, .. }) => a == b,
        (Some(Topic::Thread { thread_id: a }), Topic::Thread { thread_id: b }) => a == b,
        _ => false,
    }
}

/// `send_text`（L2）：验凭据 → 交出前再预览比对 → 直发先记账 → 交给 TDLib。返回临时 id；结果用 [`wait`] 等
pub async fn send_text<B: Backend>(
    b: &B,
    plan: &TextPlan,
    token: ConfirmToken,
) -> Result<Submitted, CoreError> {
    let mode = b.gate().verify(token, plan)?;
    // 发6②：准备到交出之间最长 60 秒、还等了人确认，被回复那条可能已被删、被挪话题
    preview_check(b, plan).await?;
    b.gate().before_submit(mode, plan)?;
    let sealed = Sealed {
        kind: SealedKind::Message {
            request: &plan.request,
            preview: false,
        },
    };
    match crate::send_limits::submit(b, sealed).await {
        Ok(Answer::Message(m)) => Ok(Submitted { mode, temp: m.id }),
        Ok(Answer::Resent(_)) => Err(CoreError::Internal {
            what: "send_answer_shape",
        }),
        Err(e) => Err(submit_failed(b, mode, plan, e)),
    }
}

/// 文字发送的全程：交出 → 等结果（最多 `timeout`，事件照常交给 `on_event`）→ 记账。返回确认方式、临时 id 与下落
pub fn send_and_wait<B: Backend>(
    b: &B,
    plan: &TextPlan,
    token: ConfirmToken,
    stream: &mut EventStream,
    timeout: Duration,
    on_event: &mut dyn FnMut(Envelope),
) -> Result<(ConfirmMode, MsgId, Waited), CoreError> {
    let started = std::time::Instant::now();
    let sub = submit_within(b, plan, plan.mode, timeout, send_text(b, plan, token))?;
    let (mode, temp) = (sub.mode, sub.temp);
    let w = wait(
        stream,
        &[temp],
        timeout.saturating_sub(started.elapsed()),
        on_event,
    )
    .pop()
    .unwrap_or(Waited::Unknown(UnknownWhy::Timeout));
    finish(b, plan, sub, &w);
    Ok((mode, temp, w))
}

/// 计划行（`kind:"plan"` 的 `data`，`actions.md` 第一节第 6 小节 `SendPlan`）。内容全文照给（这是给人确认用的）
pub fn plan_json(plan: &TextPlan) -> Value {
    json!({
        "action": "send_text",
        "target": target_json(&plan.target),
        "text": plan.text,
        // 正文里看不见的字符（零宽、标签字符……）有几个：人确认时看到的是转义后的，AI 读 JSON 时据此留意
        "hidden_chars": gate::hidden_count(&plan.text),
        "reply_to": plan.reply_to,
        "topic": plan.topic,
        "origin": plan.origin,
        "confirm": plan.mode,
        "digest": plan.digest_hex(),
        "note_zh": QUEUE_NOTE_ZH,
    })
}
