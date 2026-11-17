//! 重发：把发送失败的消息原样再交一次（[`prepare_resend`] → [`fn@resend`] → [`resend_and_wait`]），以及它的计划行。流程与规则见模块根 `send.rs` 的头注释。

use super::*;

// ───────────────────────── 重发 ─────────────────────────

/// 一条要重发的失败消息
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub struct ResendItem {
    pub id: i64,
    /// 重发时 TDLib 会去掉回复关系（`need_drop_reply`）
    pub drops_reply: bool,
    /// 要重发的正文（确认时给人看全文；凭据也绑它）。不是纯文字的消息（图片、文件……）是 `None`：确认框里
    /// 展示不了，正式服务器默认模式下拒
    pub text: Option<String>,
    /// 重发后仍保留的回复关系（确认时给人看、凭据也绑它；codex 016 第 1 轮 #4）。会被去掉的（`drops_reply`）记 `None`
    pub reply: ResendReply,
}

/// 重发那条回复的是谁
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub enum ResendReply {
    /// 不是回复，或重发时回复关系会被去掉
    None,
    /// 回复某条消息（可能在别的会话）
    Message(MsgId),
    /// 回复的对象 TDLib 给了、本项目认不出（如回复动态）
    Unknown,
}

impl ResendReply {
    fn of(m: &Message, drops: bool) -> Self {
        match (&m.reply_to, drops) {
            (_, true) | (None, _) => Self::None,
            (Some(ReplyRef::Message { target, .. }), _) => Self::Message(*target),
            (Some(ReplyRef::Unknown { .. }), _) => Self::Unknown,
        }
    }

    /// 确认说明里的那段（不是回复 → 空）；终端的文字计划也用它
    pub fn zh(self, chat: ChatId) -> String {
        match self {
            Self::None => String::new(),
            Self::Message(t) if t.chat_id == chat => format!("（回复 id {} 那条）", t.message_id),
            Self::Message(t) => format!(
                "（回复别的会话 id {} 里 id {} 那条）",
                t.chat_id, t.message_id
            ),
            Self::Unknown => "（回复的对象认不出）".into(),
        }
    }

    fn json(self) -> Value {
        match self {
            Self::None => Value::Null,
            Self::Message(t) => json!({"chat_id": t.chat_id, "message_id": t.message_id}),
            Self::Unknown => json!("unknown"),
        }
    }
}

/// 消息里能给人确认的正文：纯文字消息的全文；带媒体、系统消息给不出全文 → `None`
fn confirmable_text(m: &crate::model::Message) -> Option<String> {
    match m.effective() {
        crate::model::Body::Content(c) if c.attachment.is_none() => {
            Some(crate::tree::text::plain_text(&c.body))
        }
        _ => None,
    }
}

/// 重发计划：同一会话里的几条失败消息，一张凭据
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct ResendPlan {
    target: ChatTarget,
    pub(super) items: Vec<ResendItem>,
    /// 交给 TDLib 的 `resendMessages` 请求（摘要绑的就是它）
    pub(super) request: Value,
    pub(super) digest: [u8; 32],
    origin: SendOrigin,
    mode: ConfirmMode,
}

impl ResendPlan {
    pub fn target(&self) -> &ChatTarget {
        &self.target
    }
    pub fn items(&self) -> &[ResendItem] {
        &self.items
    }
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

impl gate::Sealed for ResendPlan {}
impl Gated for ResendPlan {
    fn target(&self) -> ChatId {
        self.target.id
    }
    fn digest(&self) -> [u8; 32] {
        self.digest
    }
    fn intent(&self) -> [u8; 32] {
        self.digest
    }
    fn dedup(&self) -> bool {
        false
    }
    fn kind(&self) -> &'static str {
        "resend"
    }
}

/// 重发凭据绑的摘要：请求 + 每条重发后的回复关系（去不去掉、回复谁）+ 每条的正文（人确认的就是这些）
pub(super) fn resend_digest(
    request: &Value,
    items: &[ResendItem],
    account: crate::model::UserId,
    server: ServerKind,
) -> [u8; 32] {
    let drops: Vec<bool> = items.iter().map(|i| i.drops_reply).collect();
    let texts: Vec<Option<&str>> = items.iter().map(|i| i.text.as_deref()).collect();
    let replies: Vec<Value> = items.iter().map(|i| i.reply.json()).collect();
    gate::digest(
        &json!({"request": request, "drops_reply": drops, "texts": texts, "replies": replies}),
        account,
        server,
    )
}

/// 逐条查失败态、取正文与回复关系（准备时用；交出前再取一次与计划比对，codex 016 第 2 轮 #2）
async fn load_items<B: Backend>(
    b: &B,
    chat: ChatId,
    ids: &[i64],
) -> Result<Vec<ResendItem>, CoreError> {
    let mut items = Vec::new();
    for &id in ids {
        let f = b
            .failed_info(MsgId {
                chat_id: chat,
                message_id: id,
            })
            .await?
            .ok_or(CoreError::InvalidSend {
                why: InvalidSendWhy::NotFailed,
            })?;
        if !f.can_retry {
            return Err(CoreError::InvalidSend {
                why: InvalidSendWhy::CannotRetry,
            });
        }
        // 下面几种第一阶段一律拒：要付 Telegram Stars 才能发（付费私信 / 付费群）；要换个身份发（原来以频道或
        // 匿名管理员身份发的，那个身份现在用不了）；原来引用的那段已经不在、要换引用——都要人另做选择，CLI 不替人定
        if f.required_paid_message_star_count > 0 {
            return Err(CoreError::InvalidSend {
                why: InvalidSendWhy::Paid,
            });
        }
        if f.need_another_sender || f.need_another_reply_quote {
            return Err(CoreError::InvalidSend {
                why: InvalidSendWhy::NeedsOther,
            });
        }
        if f.retry_after > 0.0 {
            let e = CoreError::RateLimited {
                // 向上取整到秒
                retry_after_s: f.retry_after.ceil().clamp(1.0, f64::from(u32::MAX)) as u32,
                td: None,
            };
            // TDLib 告诉的等待期也记进发送限制（落盘，之后的进程照样遵守）
            b.gate().limits().note(&e);
            return Err(e);
        }
        let m = b
            .message_get(MsgId {
                chat_id: chat,
                message_id: id,
            })
            .await?;
        items.push(ResendItem {
            id,
            drops_reply: f.need_drop_reply,
            text: confirmable_text(&m),
            reply: ResendReply::of(&m, f.need_drop_reply),
        });
    }
    Ok(items)
}

/// `resend_prepare`：逐条查失败态（`actions.md` `resend_prepare`）
pub async fn prepare_resend<B: Backend>(
    b: &B,
    chat: ChatId,
    ids: &[i64],
    origin: SendOrigin,
) -> Result<ResendPlan, CoreError> {
    let me = b.me().await?;
    let mut target = b.chat_target(chat).await?;
    target.saved = target.id == me.saved_chat_id;
    let mode = b.gate().mode_for(target.id)?;
    let items = load_items(b, target.id, ids).await?;
    let request = json!({
        "@type": "resendMessages",
        "chat_id": target.id,
        "message_ids": ids,
        "quote": null,
        "paid_message_star_count": 0,
    });
    let digest = resend_digest(&request, &items, me.user_id, server_of(b)?);
    Ok(ResendPlan {
        target,
        items,
        request,
        digest,
        origin,
        mode,
    })
}

/// `resend`（L2）：一张凭据重发这几条；返回每条的凭条（不能重发的那条 `None`），之后用 [`wait`] 等、逐条 [`finish`]
pub async fn resend<B: Backend>(
    b: &B,
    plan: &ResendPlan,
    token: ConfirmToken,
) -> Result<Vec<Option<Submitted>>, CoreError> {
    let mode = b.gate().verify(token, plan)?;
    // 交出前复核（codex 016 第 2 轮 #2）：从准备到交出最长 60 秒、还等了人确认，这期间失败态、正文、回复关系
    // 可能变了（别的设备重发了、被删了、被编辑了）。重取一遍与人确认过的那份比，不一样就拒，什么都不交出
    let now = load_items(
        b,
        plan.target.id,
        &plan.items.iter().map(|i| i.id).collect::<Vec<_>>(),
    )
    .await
    .map_err(|_| CoreError::SendRefused {
        reason: SendRefusedReason::ContentMismatch,
        last: None,
    })?;
    if now != plan.items {
        return Err(CoreError::SendRefused {
            reason: SendRefusedReason::ContentMismatch,
            last: None,
        });
    }
    b.gate().before_submit(mode, plan)?;
    let sealed = Sealed {
        kind: SealedKind::Resend {
            request: &plan.request,
        },
    };
    match crate::send_limits::submit(b, sealed).await {
        Ok(Answer::Resent(ms)) => Ok(ms
            .into_iter()
            .map(|m| m.map(|m| Submitted { mode, temp: m.id }))
            .collect()),
        Ok(Answer::Message(_) | Answer::Done) => Err(CoreError::Internal {
            what: "resend_answer_shape",
        }),
        Err(e) => Err(submit_failed(b, mode, plan, e)),
    }
}

/// 等结果途中要交给调用方 store 的东西：收线程来的事件，或重发应答带出的「旧 id → 新 id」。后者要在换正式 id
/// （`send_succeeded`）之前喂：store 靠它把新消息接到旧消息的稳定键上；晚了，换 id 时找不到旧键，界面焦点会丢
pub enum Fed {
    Event(Envelope),
    Input(crate::store::Input),
}

/// 一条重发的下落
pub struct ResendOutcome {
    /// 要重发的那条（失败消息的 id）
    pub old: i64,
    /// `None` = TDLib 说这条不能重发（应答里是 null）；否则是新的临时 id 与它的下落
    pub result: Option<(MsgId, Waited)>,
}

/// 重发的全程：交出 → 喂「旧 id → 新 id」→ 逐条等结果 → 逐条记账（CLI 与以后的 GUI 共用，漏不了记账）
pub fn resend_and_wait<B: Backend>(
    b: &B,
    plan: &ResendPlan,
    token: ConfirmToken,
    stream: &mut EventStream,
    timeout: Duration,
    on_fed: &mut dyn FnMut(Fed),
) -> Result<(ConfirmMode, Vec<ResendOutcome>), CoreError> {
    let started = std::time::Instant::now();
    let news = submit_within(b, plan, plan.mode, timeout, resend(b, plan, token))?;
    let timeout = timeout.saturating_sub(started.elapsed());
    let mode = news
        .iter()
        .flatten()
        .next()
        .map_or(plan.mode, Submitted::mode);
    let olds: Vec<i64> = plan.items.iter().map(|i| i.id).collect();
    let answers: Vec<Option<i64>> = news
        .iter()
        .map(|n| n.as_ref().map(|s| s.temp.message_id))
        .collect();
    for input in crate::pump::resent_inputs(plan.target.id, &olds, &answers) {
        on_fed(Fed::Input(input));
    }
    let temps: Vec<MsgId> = news.iter().flatten().map(Submitted::temp).collect();
    let mut waited = wait(stream, &temps, timeout, &mut |e| on_fed(Fed::Event(e))).into_iter();
    let mut out = Vec::with_capacity(olds.len());
    for (old, new) in olds.into_iter().zip(news) {
        let result = new.map(|sub| {
            let temp = sub.temp;
            let w = waited
                .next()
                .unwrap_or(Waited::Unknown(UnknownWhy::Timeout));
            finish(b, plan, sub, &w);
            (temp, w)
        });
        out.push(ResendOutcome { old, result });
    }
    Ok((mode, out))
}

pub fn resend_plan_json(plan: &ResendPlan) -> Value {
    json!({
        "action": "resend",
        "target": target_json(&plan.target),
        "messages": plan.items.iter().map(|i| json!({"message_id": i.id, "drops_reply": i.drops_reply,
            "text": i.text, "hidden_chars": i.text.as_deref().map(gate::hidden_count),
            "reply_to": i.reply.json()})).collect::<Vec<_>>(),
        "origin": plan.origin,
        "confirm": plan.mode,
        "digest": plan.digest_hex(),
        "note_zh": QUEUE_NOTE_ZH,
    })
}
