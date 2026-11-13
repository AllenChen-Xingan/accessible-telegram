//! `send`：发文字、回复、重发（M2.1 / M2.2，T04）。规约：`specs/contracts/actions.md` 第一节第 3、4、7 小节；
//! 先例与坑：`docs/research/003_send-receive-prior-art/README.md` 发1–发9。
//!
//! 几个词：**临时 id** = TDLib 交出时先给这条消息编的号（`messageSendingStatePending`），服务器收下后换成**正式 id**；
//! **应答** = `sendMessage` 这次请求的直接回复（带临时 id）；**结果** = 之后 TDLib 主动推的 `send_succeeded` /
//! `send_failed` 事件。应答与结果走两条路，谁先到没有保证（发4）。源码行号指 TDLib 1.8.69（`resources/td/`）。
//!
//! 与契约的名字对照（`specs/contracts/actions.md` 第一节第 3 小节）：`send_text_prepare` = [`prepare_text`]，
//! `send_text` = [`send_text`]（全程用 [`send_and_wait`]），`resend_prepare` = [`prepare_resend`]，`resend` = [`resend`]；
//! 契约的 `SendPlan` 在这里分成 [`TextPlan`] 与 [`ResendPlan`]，`PendingMessage` = [`Submitted`]。
//!
//! 一次发送的流程（CLI 与以后的 GUI 共用）：
//! 1. [`prepare_text`]（L0，只查不发）：自己是谁 → 目标会话 → 被回复那条（能不能回复、在哪个话题）→ 话题规则 →
//!    组好交给 TDLib 的那份请求（`clear_draft=false` 不动人在手机上的草稿；带 `sending_id`）→ 算摘要 →
//!    **调一次 `only_preview`**，回复对象与话题和请求逐项比对，不一致就拒（发6①：TDLib 会把不合规的回复悄悄去掉）
//! 2. 给人看 [`TextPlan`]，由闸开凭据（[`crate::gate::Gate::issue`]）
//! 3. [`send_text`]（L2）：闸验凭据 → **紧挨着交出前再预览比对一次**（发6②）→ 直发的话先记「已提交」→ 交给 TDLib，
//!    拿到临时 id
//! 4. [`wait`]（store = 本进程里的消息与会话缓存，界面和读法都从它取）：等 `send_succeeded` / `send_failed` / 临时 id 出现在 `messages_deleted` 里（发3，按结果未知报），
//!    30 秒上限（发9）；事件照常交给调用方喂 store（晚到的也不丢）。结果可能比应答先到（发4）：事件一直在流里排着，
//!    应答回来之后才开始看，所以不会漏
//! 5. [`finish`]：闸记结果（直发改防重复记录、写留痕）
//!
//! 重发（[`prepare_resend`] → [`resend_and_wait`]，内部是 [`resend`] → [`wait`] → 逐条 [`finish`]）：逐条查失败态（能否重发、要不要等、会不会去掉回复）→ 一张凭据 →
//! `resendMessages` → 每条的新临时 id 同样用 [`wait`] 等。重发不进直发防重复（见 `gate::Gated::dedup`）。
//!
//! 只有闸能发：[`Sealed`] 只能在本 crate 里造，后端的交出口 `Backend::submit` 收的就是它。
//!
//! 代号：「发N」「B17-2」见 `docs/research/003_send-receive-prior-art/`；「M2.1」等是 `docs/plan/002_roadmap.md` 的块，
//! 「T04」是 `specs/tasks.md` 的任务（发消息）。

use crate::backend::Backend;
use crate::config::ServerKind;
use crate::error::{CoreError, InvalidSendWhy, SendRefusedReason, UnknownWhy};
use crate::gate::{self, ConfirmMode, ConfirmToken, Gated, Outcome};
use crate::link::{Envelope, EventStream, block_on_timeout};
use crate::model::{ChatId, ChatKind, CoreEvent, Message, MsgId, ReplyRef, Topic};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// 正文最多多少字节（UTF-8）。这是本地的「明显给错了」防线，不是 Telegram 的上限：Telegram 单条文字上限是
/// 4096 字（`getOption message_text_length_max`），在两者之间的正文由 TDLib 的预览当场拒（`invalid_input`，
/// 原文 `MESSAGE_TOO_LONG`），不会自动拆成几条、也不会交出去。
/// 发送动作的规则，准备步骤（[`prepare_text`]）里查，CLI 与 GUI 都受它管
pub const TEXT_MAX_BYTES: u64 = 64 * 1024;

/// `send` 等结果的上限（`cli.md` `send` 行；发9）
pub const RESULT_TIMEOUT: Duration = Duration::from_secs(30);

/// 确认时说给人的「网络中断也会发」（`actions.md` 第一节第 2 小节）
pub const QUEUE_NOTE_ZH: &str = "若网络中断，恢复后仍会发出";
/// 同一个意思的短说法，给 Touch ID 弹框（VoiceOver 会读，要短）
pub const CONFIRM_QUEUE_ZH: &str = "网络断了也会在恢复后发出";

pub use crate::model::ChatTarget;

/// 谁发起的（确认说明里写；命令行注明「可能是 AI」）
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SendOrigin {
    Cli,
    Gui,
}

/// 只能在本 crate 里造的「交给 TDLib 的请求」：后端的交出口只收它，所以 crate 外的代码绕不过闸
pub struct Sealed<'a> {
    // 私有：只有本模块（验过凭据之后）造得出；后端用 [`Sealed::into_kind`] 取（codex 016 第 1 轮 #7）
    kind: SealedKind<'a>,
}

impl<'a> Sealed<'a> {
    /// 后端的交出口拆开用
    pub(crate) fn into_kind(self) -> SealedKind<'a> {
        self.kind
    }
}

pub(crate) enum SealedKind<'a> {
    /// `sendMessage`：请求 JSON（TDLib 形状）；`preview` 为真时改成 `only_preview`，不发
    Message { request: &'a Value, preview: bool },
    /// `resendMessages`：请求 JSON（TDLib 形状；摘要算过的就是它）
    Resend { request: &'a Value },
}

/// 交出口的应答
#[allow(clippy::large_enum_variant)]
pub enum Answer {
    /// `sendMessage`：待发的那条（临时 id）；预览时是 TDLib 造的假消息
    Message(Message),
    /// `resendMessages`：按请求顺序，不能重发的那条是 `None`
    Resent(Vec<Option<Message>>),
}

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

/// 弹框里的「内容」：先说全文几字几行，再给正文——换行显示成 ⏎、看不见的字符转义，压成一段。防的是用一串空行
/// 把真正要发的话挤出可见区（自审第 4 轮 P1）。放不放得下由闸按整段说明把关（`gate::fits_confirm_box`，放不下就拒 `too_long_to_confirm`，不做「只显示前段、
/// 全文见终端」——发起 `send` 的终端多半是 AI 的输出，人看不到），这里不截
pub fn confirm_body(text: &str) -> String {
    let chars = text.chars().count();
    let lines = text.lines().count().max(1);
    // 这里的「行」是原文的换行行数，不是确认框里折出来的视觉行（那个由 `gate::fits_confirm_box` 数）
    format!("共 {chars} 字 {lines} 行：\n{}", flatten(text))
}

/// 压成一段给人看：看不见的字符转义、换行显示成 ⏎
fn flatten(text: &str) -> String {
    gate::confirm_visible(text).replace('\n', "⏎")
}

fn origin_zh(o: SendOrigin) -> &'static str {
    match o {
        SendOrigin::Cli => "来自命令行，可能是 AI",
        SendOrigin::Gui => "来自界面",
    }
}

/// 「私聊 · 名字 · id 777」（名字转义控制字符）
pub fn target_zh(t: &ChatTarget) -> String {
    let kind = if t.saved {
        "收藏夹"
    } else {
        match t.kind {
            ChatKind::Private => "私聊",
            ChatKind::Secret => "加密聊天",
            ChatKind::Group => "群",
            ChatKind::Supergroup if t.is_forum => "话题群",
            ChatKind::Supergroup => "超级群",
            ChatKind::Channel => "频道",
            ChatKind::Unknown { .. } => "会话",
        }
    };
    format!("{kind} · {} · id {}", gate::confirm_visible(&t.title), t.id)
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
fn send_topic(t: Option<&Topic>) -> Option<Topic> {
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

fn server_of(b: &impl Backend) -> Result<ServerKind, CoreError> {
    match b.gate().use_test_dc() {
        Some(true) => Ok(ServerKind::Test),
        Some(false) => Ok(ServerKind::Main),
        None => Err(CoreError::Internal {
            what: "gate_not_started",
        }),
    }
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
    match b.submit(sealed).await? {
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

/// 交出之后拿到的凭条：确认方式 + 临时 id。只能由本模块造出；等到结果后**必须**交给 [`finish`]（它吃掉凭条）
/// 记账——漏了，直发的防重复记录就一直停在「已提交」、留痕少一行。文字发送用 [`send_and_wait`] 一步走完
#[derive(Debug, PartialEq, Eq)]
#[must_use = "等到结果后要交给 send::finish 记账（直发的防重复与留痕）"]
pub struct Submitted {
    mode: ConfirmMode,
    temp: MsgId,
}

impl Submitted {
    pub fn mode(&self) -> ConfirmMode {
        self.mode
    }
    /// 临时 id：TDLib 交出时先给消息编的号，服务器收下后换成正式 id（`send_succeeded` 里带）
    pub fn temp(&self) -> MsgId {
        self.temp
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
    match b.submit(sealed).await {
        Ok(Answer::Message(m)) => Ok(Submitted { mode, temp: m.id }),
        Ok(Answer::Resent(_)) => Err(CoreError::Internal {
            what: "send_answer_shape",
        }),
        Err(e) => Err(submit_failed(b, mode, plan, e)),
    }
}

/// 交出口回了错：分清「TDLib 明确拒了、没发出」和「请求可能已经交出、只是没等到应答」。
/// 后一种（连接断了、内核关了、请求中止、应答转不回来）按结果未知记账并报 `send_outcome_unknown`——
/// 记成失败会让直发防重复放过重跑，对方就收到两条（自审第 1 轮 P1）
fn submit_failed<B: Backend>(
    b: &B,
    mode: ConfirmMode,
    plan: &dyn Gated,
    e: CoreError,
) -> CoreError {
    let maybe_left = matches!(
        e,
        CoreError::Disconnected
            | CoreError::Network { .. }
            | CoreError::Internal {
                what: "td_message_reserialize_failed",
            }
    );
    if maybe_left {
        b.gate()
            .after_outcome(mode, plan, 0, None, Outcome::Unknown, None);
        CoreError::SendOutcomeUnknown {
            chat_id: Some(plan.target()),
            temp_message_id: 0,
            why: UnknownWhy::Disconnected,
        }
    } else {
        b.gate()
            .after_outcome(mode, plan, 0, None, Outcome::Failed, Some(&e));
        e
    }
}

// ───────────────────────── 等结果 ─────────────────────────

/// 一条待发消息的下落
#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub enum Waited {
    /// 服务器收下了：发出后的整条（含正式 id）
    Succeeded(Message),
    /// 失败：失败后的那条（TDLib 也给它新 id）、原因、能否重发
    Failed {
        message: Message,
        error: CoreError,
        can_retry: bool,
    },
    /// 结果未知：超时、待发那条被删、连接断了
    Unknown(UnknownWhy),
}

/// 这条事件说的是不是这几条待发消息之一的下落
fn verdict(ev: &CoreEvent, temps: &[MsgId]) -> Option<(MsgId, Waited)> {
    match ev {
        CoreEvent::SendSucceeded { old_id, message } if temps.contains(old_id) => {
            Some((*old_id, Waited::Succeeded(message.clone())))
        }
        CoreEvent::SendFailed {
            old_id,
            message,
            error,
            can_retry,
        } if temps.contains(old_id) => Some((
            *old_id,
            Waited::Failed {
                message: message.clone(),
                error: error.clone(),
                can_retry: *can_retry,
            },
        )),
        // 发3：待发那条被删，TDLib 不来失败。谁会删：人在别的设备上删了它、会话变得不可访问、TDLib 重发前先删旧的。
        // 为什么「被删」反而可能已送达：请求可能早已到了服务器，删除只停本机的等待，服务器的成功应答照样回来
        // （TDLib 1.8.69 `MessagesManager.cpp:27848-27851`）。所以按结果未知报、不许重发——当成失败会让人重发出第二条
        CoreEvent::MessagesDeleted {
            chat_id,
            message_ids,
            ..
        } => temps
            .iter()
            .find(|t| t.chat_id == *chat_id && message_ids.contains(&t.message_id))
            .map(|t| (*t, Waited::Unknown(UnknownWhy::PendingDeleted))),
        _ => None,
    }
}

/// 等这几条待发消息的下落，最多 `timeout`。每个事件都先交给 `on_event`（调用方喂 store），再看是不是要等的。
/// 返回与 `temps` 同序
pub fn wait(
    stream: &mut EventStream,
    temps: &[MsgId],
    timeout: Duration,
    on_event: &mut dyn FnMut(Envelope),
) -> Vec<Waited> {
    let deadline = Instant::now() + timeout;
    let mut got: Vec<Option<Waited>> = vec![None; temps.len()];
    while got.iter().any(Option::is_none) {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match block_on_timeout(stream.next_batch(256), left) {
            None => break,
            // 流结束：收线程故障或已关闭
            Some(None) => {
                for g in got.iter_mut().filter(|g| g.is_none()) {
                    *g = Some(Waited::Unknown(UnknownWhy::Disconnected));
                }
            }
            Some(Some(batch)) => {
                for env in batch {
                    if let Some((id, w)) = verdict(&env.event, temps)
                        && let Some(i) = temps.iter().position(|t| *t == id)
                        && got[i].is_none()
                    {
                        got[i] = Some(w);
                    }
                    on_event(env);
                }
            }
        }
    }
    got.into_iter()
        .map(|g| g.unwrap_or(Waited::Unknown(UnknownWhy::Timeout)))
        .collect()
}

/// 等到结果后交给闸记账（直发的防重复记录与留痕）
pub fn finish<B: Backend>(b: &B, plan: &dyn Gated, sub: Submitted, w: &Waited) {
    let Submitted { mode, temp } = sub;
    let (outcome, final_id, err) = match w {
        Waited::Succeeded(m) => (Outcome::Succeeded, Some(m.id.message_id), None),
        Waited::Failed { message, error, .. } => {
            (Outcome::Failed, Some(message.id.message_id), Some(error))
        }
        Waited::Unknown(_) => (Outcome::Unknown, None, None),
    };
    b.gate()
        .after_outcome(mode, plan, temp.message_id, final_id, outcome, err);
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

/// 交出（验凭据 → 复核 → 交给 TDLib）也算进同一个上限：TDLib 迟迟不回交出的应答时，命令与 serve 不许一直卡着
/// （codex 016 第 1 轮 #3）。到点还没回 → 可能已经交出，按结果未知记账并报 `send_outcome_unknown`（`timeout`）
fn submit_within<B: Backend, T>(
    b: &B,
    plan: &dyn Gated,
    mode: ConfirmMode,
    timeout: Duration,
    fut: impl std::future::Future<Output = Result<T, CoreError>>,
) -> Result<T, CoreError> {
    match crate::link::block_on_timeout(fut, timeout) {
        Some(r) => r,
        None => {
            b.gate()
                .after_outcome(mode, plan, 0, None, Outcome::Unknown, None);
            Err(CoreError::SendOutcomeUnknown {
                chat_id: Some(plan.target()),
                temp_message_id: 0,
                why: UnknownWhy::Timeout,
            })
        }
    }
}

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

    /// 确认说明里的那段（不是回复 → 空）
    fn zh(self, chat: ChatId) -> String {
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
    items: Vec<ResendItem>,
    /// 交给 TDLib 的 `resendMessages` 请求（摘要绑的就是它）
    request: Value,
    digest: [u8; 32],
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
    fn confirm_fits(&self) -> bool {
        // 每条都给得出正文（自审第 6 轮 P1：重发同样要让人看到全文），整段也放得下（第 7 轮：条数多了行数会超）
        self.items.iter().all(|i| i.text.is_some()) && gate::fits_confirm_box(&self.confirm_text())
    }
    fn confirm_text(&self) -> String {
        let drop = if self.items.iter().any(|i| i.drops_reply) {
            "，会去掉回复关系"
        } else {
            ""
        };
        let bodies: Vec<String> = self
            .items
            .iter()
            .map(|i| match &i.text {
                Some(t) => format!(
                    "id {} 那条{}，{}",
                    i.id,
                    i.reply.zh(self.target.id),
                    confirm_body(t)
                ),
                None => format!(
                    "id {} 那条{}：不是纯文字，这里看不到内容",
                    i.id,
                    i.reply.zh(self.target.id)
                ),
            })
            .collect();
        format!(
            "重发到 {}{drop}\n{}，{}\n{}",
            target_zh(&self.target),
            origin_zh(self.origin),
            CONFIRM_QUEUE_ZH,
            bodies.join("\n"),
        )
    }
}

/// 重发凭据绑的摘要：请求 + 每条重发后的回复关系（去不去掉、回复谁）+ 每条的正文（人确认的就是这些）
fn resend_digest(
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
            return Err(CoreError::RateLimited {
                // 向上取整到秒
                retry_after_s: f.retry_after.ceil().clamp(1.0, f64::from(u32::MAX)) as u32,
                td: None,
            });
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
    match b.submit(sealed).await {
        Ok(Answer::Resent(ms)) => Ok(ms
            .into_iter()
            .map(|m| m.map(|m| Submitted { mode, temp: m.id }))
            .collect()),
        Ok(Answer::Message(_)) => Err(CoreError::Internal {
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

// ───────────────────────── 计划行 ─────────────────────────

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

fn target_json(t: &ChatTarget) -> Value {
    json!({
        "chat_id": t.id,
        "kind": t.kind,
        "title": t.title,
        "is_forum": t.is_forum,
        "saved": t.saved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_keeps_draft_and_carries_sending_id() {
        let r = text_request(
            777,
            "好的",
            Some(5),
            Some(&Topic::Forum {
                topic_id: 3,
                name: None,
            }),
            42,
        );
        assert_eq!(r["input_message_content"]["clear_draft"], json!(false));
        assert_eq!(r["options"]["sending_id"], json!(42));
        assert_eq!(r["options"]["only_preview"], json!(false));
        assert_eq!(r["reply_to"]["message_id"], json!(5));
        assert_eq!(r["topic_id"]["forum_topic_id"], json!(3));
        // 文本不做 Unicode 规范化：组合字符原样
        let r = text_request(1, "e\u{301}", None, None, 1);
        assert_eq!(
            r["input_message_content"]["text"]["text"],
            json!("e\u{301}")
        );
    }

    #[test]
    fn only_forum_and_thread_topics_are_sent() {
        assert!(send_topic(Some(&Topic::SavedMessages { topic_id: 1 })).is_none());
        assert!(send_topic(Some(&Topic::Thread { thread_id: 1 })).is_some());
    }

    // ───── 假后端走全流程（数据全是编的；名字取自 fixtures/names.txt）─────

    use crate::backend::Backend;
    use crate::config::Profile;
    use crate::error::Holder;
    use crate::fake::{FakeAuth, FakeBackend, FakeSend};
    use crate::gate::{Authenticator, Gate};
    use crate::link::block_on;
    use crate::status::AuthStep;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    const ME: i64 = 4242;
    const FRIEND: i64 = 777;
    const FORUM: i64 = -100_555;

    struct Auth(Arc<AtomicU32>, Result<(), SendRefusedReason>);
    impl Authenticator for Auth {
        fn authenticate(&self, _: &str) -> Result<(), SendRefusedReason> {
            self.0.fetch_add(1, Ordering::SeqCst);
            self.1
        }
    }

    fn new_chat(id: i64, title: &str, ty: Value) -> Value {
        json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title, "type": ty,
            "positions": [], "notification_settings": {}}})
    }

    fn msg(chat: i64, id: i64, text: &str, topic: Value) -> Value {
        json!({"@type": "message", "id": id, "chat_id": chat,
            "sender_id": {"@type": "messageSenderUser", "user_id": FRIEND}, "is_outgoing": false,
            "date": 1_792_755_000, "topic_id": topic,
            "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": text, "entities": []}}})
    }

    struct Rig {
        b: FakeBackend,
        stream: EventStream,
        asks: Arc<AtomicU32>,
    }

    fn rig(tag: &str, direct: bool, test_dc: bool, auth: Result<(), SendRefusedReason>) -> Rig {
        static N: AtomicU32 = AtomicU32::new(0);
        let asks = Arc::new(AtomicU32::new(0));
        let gate = Gate::new(direct, Holder::Cli, Box::new(Auth(asks.clone(), auth)));
        let b = FakeBackend::with_gate(gate).unwrap();
        b.set_auth(FakeAuth {
            phone: if test_dc {
                "+9996612345"
            } else {
                "+15550001234"
            }
            .into(),
            logged_in: true,
            ..FakeAuth::default()
        });
        b.set_me(ME);
        b.set_chat(
            ME,
            vec![new_chat(
                ME,
                "我自己",
                json!({"@type": "chatTypePrivate", "user_id": ME}),
            )],
        );
        b.set_chat(
            FRIEND,
            vec![new_chat(
                FRIEND,
                "林小雨",
                json!({"@type": "chatTypePrivate", "user_id": FRIEND}),
            )],
        );
        b.set_chat(
            FORUM,
            vec![new_chat(
                FORUM,
                "周末爬山群",
                json!({"@type": "chatTypeSupergroup", "supergroup_id": 555, "is_channel": false}),
            )],
        );
        b.set_forum(FORUM);
        b.set_history(ME, vec![msg(ME, 5, "第5条", Value::Null)]);
        b.set_history(FRIEND, vec![msg(FRIEND, 5, "第5条", Value::Null)]);
        b.set_history(
            FORUM,
            vec![msg(
                FORUM,
                9,
                "第9条",
                json!({"@type": "messageTopicForum", "forum_topic_id": 3}),
            )],
        );
        let root = std::env::temp_dir().join(format!(
            "atg-send-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&root);
        let kind = if test_dc {
            ServerKind::Test
        } else {
            ServerKind::Main
        };
        let profile = Profile::create(&root, "p", kind).unwrap();
        let stream = b.subscribe().unwrap();
        block_on(b.start(&profile)).unwrap();
        b.wait_auth(Duration::from_secs(5), &|s| s == AuthStep::Ready)
            .unwrap();
        Rig { b, stream, asks }
    }

    fn draft(text: &str, reply_to: Option<i64>) -> Draft {
        Draft {
            text: text.into(),
            reply_to,
        }
    }

    /// 准备 → 开凭据 → 交出 → 等结果，返回下落
    fn send_once(r: &mut Rig, chat: i64, d: &Draft, again: bool) -> Result<Waited, CoreError> {
        let plan = block_on(prepare_text(&r.b, chat, d, SendOrigin::Cli))?;
        let token = r.b.gate().issue(&plan, again)?;
        let (_, _, w) = send_and_wait(
            &r.b,
            &plan,
            token,
            &mut r.stream,
            Duration::from_secs(2),
            &mut |_| {},
        )?;
        Ok(w)
    }

    fn refused_reason(e: CoreError) -> SendRefusedReason {
        match e {
            CoreError::SendRefused { reason, .. } => reason,
            other => panic!("不是 send_refused：{other:?}"),
        }
    }

    #[test]
    fn test_dc_saved_sends_without_asking_and_keeps_draft() {
        let mut r = rig("saved", false, true, Err(SendRefusedReason::AuthFailed));
        let w = send_once(&mut r, ME, &draft("测试", None), false).unwrap();
        assert!(matches!(w, Waited::Succeeded(ref m) if m.id.chat_id == ME));
        assert_eq!(r.asks.load(Ordering::SeqCst), 0);
        // 交给「TDLib」的就是摘要算过的那份：不清草稿、带 sending_id、不是预览
        let sent = r.b.submitted();
        assert_eq!(sent.len(), 1);
        assert_eq!(
            sent[0]["input_message_content"]["clear_draft"],
            json!(false)
        );
        assert_ne!(sent[0]["options"]["sending_id"], json!(0));
        assert_eq!(sent[0]["options"]["only_preview"], json!(false));
    }

    #[test]
    fn test_dc_other_chat_is_refused_before_anything_is_sent() {
        let mut r = rig("other", true, true, Ok(()));
        let e = send_once(&mut r, FRIEND, &draft("测试", None), false).unwrap_err();
        assert_eq!(refused_reason(e), SendRefusedReason::TestDcSavedOnly);
        assert!(r.b.submitted().is_empty());
    }

    #[test]
    fn main_server_asks_every_time_and_refusal_sends_nothing() {
        let mut r = rig("main", false, false, Ok(()));
        send_once(&mut r, ME, &draft("测试", None), false).unwrap();
        send_once(&mut r, ME, &draft("测试", None), false).unwrap();
        assert_eq!(r.asks.load(Ordering::SeqCst), 2);
        let mut r = rig("main-no", false, false, Err(SendRefusedReason::AuthFailed));
        let e = send_once(&mut r, ME, &draft("测试", None), false).unwrap_err();
        assert_eq!(refused_reason(e), SendRefusedReason::AuthFailed);
        assert!(r.b.submitted().is_empty());
    }

    #[test]
    fn direct_mode_blocks_second_identical_submit() {
        let mut r = rig("dup", true, false, Err(SendRefusedReason::AuthFailed));
        send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap();
        let e = send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap_err();
        assert_eq!(refused_reason(e), SendRefusedReason::DuplicateRecent);
        assert_eq!(r.b.submitted().len(), 1);
        // --again 放行；改一个字也不算重复
        send_once(&mut r, FRIEND, &draft("好的", None), true).unwrap();
        send_once(&mut r, FRIEND, &draft("好的！", None), false).unwrap();
        assert_eq!(r.b.submitted().len(), 3);
        assert_eq!(r.asks.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn direct_mode_blocks_rerun_after_unknown_outcome() {
        let mut r = rig("unknown", true, false, Ok(()));
        r.b.script_sends(vec![FakeSend::Silent]);
        let w = send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap();
        assert_eq!(w, Waited::Unknown(UnknownWhy::Timeout));
        let e = send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap_err();
        assert_eq!(refused_reason(e), SendRefusedReason::DuplicateRecent);
    }

    #[test]
    fn pending_deleted_is_unknown_not_failure() {
        let mut r = rig("deleted", false, true, Ok(()));
        r.b.script_sends(vec![FakeSend::Delete]);
        let w = send_once(&mut r, ME, &draft("测试", None), false).unwrap();
        assert_eq!(w, Waited::Unknown(UnknownWhy::PendingDeleted));
    }

    #[test]
    fn failure_reports_reason_and_can_be_resent() {
        let mut r = rig("fail", false, true, Ok(()));
        r.b.script_sends(vec![FakeSend::Fail {
            code: 400,
            message: "MESSAGE_TOO_LONG".into(),
            can_retry: true,
        }]);
        let Waited::Failed {
            message,
            error,
            can_retry,
        } = send_once(&mut r, ME, &draft("测试", None), false).unwrap()
        else {
            panic!("应当失败");
        };
        assert!(can_retry);
        assert_eq!(error.code(), "invalid_input");
        // 重发：查失败态 → 一张凭据 → 新临时 id → 成功
        let plan = block_on(prepare_resend(
            &r.b,
            ME,
            &[message.id.message_id],
            SendOrigin::Cli,
        ))
        .unwrap();
        let token = r.b.gate().issue(&plan, true).unwrap();
        let news = block_on(resend(&r.b, &plan, token)).unwrap();
        let subs: Vec<Submitted> = news.into_iter().flatten().collect();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].mode(), ConfirmMode::TestExempt);
        let temps: Vec<MsgId> = subs.iter().map(Submitted::temp).collect();
        let w = wait(&mut r.stream, &temps, Duration::from_secs(2), &mut |_| {});
        assert!(matches!(w[0], Waited::Succeeded(_)));
        for (sub, w) in subs.into_iter().zip(&w) {
            finish(&r.b, &plan, sub, w);
        }
        // 不是失败的那条不许重发
        let e = block_on(prepare_resend(&r.b, ME, &[5], SendOrigin::Cli)).unwrap_err();
        assert_eq!(
            e,
            CoreError::InvalidSend {
                why: InvalidSendWhy::NotFailed
            }
        );
    }

    #[test]
    fn reply_dropped_by_preview_is_refused_at_prepare_and_at_send() {
        // 准备时预览就去掉了回复
        let mut r = rig("drop1", false, true, Ok(()));
        r.b.script_previews(&[true]);
        let e = block_on(prepare_text(
            &r.b,
            ME,
            &draft("回你", Some(5)),
            SendOrigin::Cli,
        ))
        .unwrap_err();
        assert_eq!(refused_reason(e), SendRefusedReason::ReplyDropped);
        // 准备时好好的，交出前那次预览去掉了（被回复那条在确认期间被删）：拒，什么都没交出去
        r.b.script_previews(&[false, true]);
        let plan = block_on(prepare_text(
            &r.b,
            ME,
            &draft("回你", Some(5)),
            SendOrigin::Cli,
        ))
        .unwrap();
        let token = r.b.gate().issue(&plan, false).unwrap();
        let e = block_on(send_text(&r.b, &plan, token)).unwrap_err();
        assert_eq!(refused_reason(e), SendRefusedReason::ReplyDropped);
        assert!(r.b.submitted().is_empty());
        // 照常时回复保住了
        let w = send_once(&mut r, ME, &draft("回你", Some(5)), false).unwrap();
        let Waited::Succeeded(m) = w else { panic!() };
        assert!(check_kept(&plan, &m).is_ok());
    }

    #[test]
    fn forum_needs_reply_and_takes_topic_from_replied_message() {
        let mut r = rig("forum", true, false, Ok(()));
        let e = block_on(prepare_text(
            &r.b,
            FORUM,
            &draft("大家好", None),
            SendOrigin::Cli,
        ))
        .unwrap_err();
        assert_eq!(
            e,
            CoreError::InvalidSend {
                why: InvalidSendWhy::ForumNeedsReply
            }
        );
        let plan = block_on(prepare_text(
            &r.b,
            FORUM,
            &draft("收到", Some(9)),
            SendOrigin::Cli,
        ))
        .unwrap();
        assert!(matches!(
            plan.topic(),
            Some(Topic::Forum { topic_id: 3, .. })
        ));
        assert!(plan.target().is_forum);
        let _ = send_once(&mut r, FORUM, &draft("收到", Some(9)), false).unwrap();
        assert_eq!(r.b.submitted()[0]["topic_id"]["forum_topic_id"], json!(3));
    }

    #[test]
    fn reply_not_allowed_and_empty_text_are_invalid() {
        let r = rig("noreply", false, true, Ok(()));
        r.b.set_no_reply(MsgId {
            chat_id: ME,
            message_id: 5,
        });
        let e = block_on(prepare_text(
            &r.b,
            ME,
            &draft("回你", Some(5)),
            SendOrigin::Cli,
        ))
        .unwrap_err();
        assert_eq!(
            e,
            CoreError::InvalidSend {
                why: InvalidSendWhy::ReplyNotAllowed
            }
        );
        let e = block_on(prepare_text(
            &r.b,
            ME,
            &draft("  \n", None),
            SendOrigin::Cli,
        ))
        .unwrap_err();
        assert_eq!(
            e,
            CoreError::InvalidSend {
                why: InvalidSendWhy::EmptyText
            }
        );
    }

    #[test]
    fn rejected_submit_is_recorded_as_failed_so_retry_is_not_blocked() {
        let mut r = rig("reject", true, false, Ok(()));
        r.b.script_sends(vec![FakeSend::Reject(CoreError::Forbidden { td: None })]);
        let e = send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap_err();
        assert_eq!(e.code(), "forbidden");
        assert!(send_once(&mut r, FRIEND, &draft("好的", None), false).is_ok());
    }

    #[test]
    fn error_after_handing_over_is_unknown_and_still_blocks_rerun() {
        // 交出途中断了（应答没等到）：不知道发没发出——按结果未知报，防重复照样拦重跑
        let mut r = rig("lost", true, false, Ok(()));
        r.b.script_sends(vec![FakeSend::Reject(CoreError::Disconnected)]);
        let e = send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap_err();
        assert!(matches!(
            e,
            CoreError::SendOutcomeUnknown {
                why: UnknownWhy::Disconnected,
                ..
            }
        ));
        let e = send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap_err();
        assert_eq!(refused_reason(e), SendRefusedReason::DuplicateRecent);
    }

    #[test]
    fn resend_in_direct_mode_leaves_no_dedup_record() {
        let mut r = rig("rs-direct", true, false, Ok(()));
        r.b.script_sends(vec![FakeSend::Fail {
            code: 400,
            message: "MESSAGE_TOO_LONG".into(),
            can_retry: true,
        }]);
        let Waited::Failed { message, .. } =
            send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap()
        else {
            panic!("应当失败");
        };
        let plan = block_on(prepare_resend(
            &r.b,
            FRIEND,
            &[message.id.message_id],
            SendOrigin::Cli,
        ))
        .unwrap();
        let token = r.b.gate().issue(&plan, false).unwrap();
        let news = block_on(resend(&r.b, &plan, token)).unwrap();
        let subs: Vec<Submitted> = news.into_iter().flatten().collect();
        assert_eq!(subs.len(), 1);
        let temps: Vec<MsgId> = subs.iter().map(Submitted::temp).collect();
        let w = wait(&mut r.stream, &temps, Duration::from_secs(2), &mut |_| {});
        for (sub, w) in subs.into_iter().zip(&w) {
            finish(&r.b, &plan, sub, w);
        }
        // 同一句话（第一次失败了，不拦）现在照样能直发：重发没往防重复记录里写东西
        assert!(send_once(&mut r, FRIEND, &draft("好的", None), false).is_ok());
    }

    #[test]
    fn confirm_body_cannot_hide_text_behind_blank_lines() {
        let padded = format!("好的{}真正要发的话", "\n".repeat(40));
        let b = confirm_body(&padded);
        // 一段，不靠换行排版；行数与字数在最前面
        assert_eq!(b.lines().count(), 2);
        assert!(b.starts_with("共 "));
        assert!(b.contains("41 行"));
        assert!(b.contains("真正要发的话"));
        // ⏎ 是换行的意思：正文里本来的换行一个不剩
        assert!(
            !confirm_body("一\n二")
                .lines()
                .nth(1)
                .unwrap()
                .contains('\n')
        );
    }

    #[test]
    fn main_server_long_text_is_refused_without_asking() {
        let mut r = rig("long", false, false, Ok(()));
        let long = "字".repeat(gate::CONFIRM_MAX_CHARS);
        let e = send_once(&mut r, ME, &draft(&long, None), false).unwrap_err();
        assert_eq!(refused_reason(e), SendRefusedReason::TooLongToConfirm);
        assert_eq!(r.asks.load(Ordering::SeqCst), 0);
        assert!(r.b.submitted().is_empty());
        // 放得下的照常问人；直发与测试豁免不受这条管
        let ok = "字".repeat(30);
        send_once(&mut r, ME, &draft(&ok, None), false).unwrap();
        assert_eq!(r.asks.load(Ordering::SeqCst), 1);
        let mut d = rig("long-direct", true, false, Ok(()));
        send_once(&mut d, FRIEND, &draft(&long, None), false).unwrap();
    }

    #[test]
    fn resend_confirm_shows_and_binds_the_reply_target() {
        // codex 016 第 1 轮 #4：带回复的失败消息重发时，人要知道它回复谁；回复对象也进摘要
        let mut r = rig("rs-reply", false, true, Ok(()));
        r.b.script_sends(vec![FakeSend::Fail {
            code: 400,
            message: "X".into(),
            can_retry: true,
        }]);
        let Waited::Failed { message, .. } =
            send_once(&mut r, ME, &draft("回你", Some(5)), false).unwrap()
        else {
            panic!("应当失败");
        };
        let plan = block_on(prepare_resend(
            &r.b,
            ME,
            &[message.id.message_id],
            SendOrigin::Cli,
        ))
        .unwrap();
        assert_eq!(
            plan.items()[0].reply,
            ResendReply::Message(MsgId {
                chat_id: ME,
                message_id: 5
            })
        );
        assert!(
            plan.confirm_text().contains("（回复 id 5 那条）"),
            "{}",
            plan.confirm_text()
        );
        let mut other = plan.items.clone();
        other[0].reply = ResendReply::None;
        assert_ne!(
            resend_digest(&plan.request, &other, ME, ServerKind::Test),
            plan.digest
        );
    }

    #[test]
    fn resend_confirm_shows_the_text_and_binds_it() {
        let mut r = rig("rs-show", false, false, Ok(()));
        r.b.script_sends(vec![FakeSend::Fail {
            code: 400,
            message: "MESSAGE_TOO_LONG".into(),
            can_retry: true,
        }]);
        // 正式服务器 + 默认模式：发第一次时人按了（假认证器放行），失败了
        let Waited::Failed { message, .. } =
            send_once(&mut r, ME, &draft("周末北门停车场见", None), false).unwrap()
        else {
            panic!("应当失败");
        };
        let plan = block_on(prepare_resend(
            &r.b,
            ME,
            &[message.id.message_id],
            SendOrigin::Cli,
        ))
        .unwrap();
        assert_eq!(plan.items()[0].text.as_deref(), Some("周末北门停车场见"));
        let t = plan.confirm_text();
        assert!(
            t.starts_with("重发到 ") && t.contains("周末北门停车场见"),
            "{t}"
        );
        assert!(plan.confirm_fits());
        // 正文进了摘要：同 id、不同正文 → 摘要不同；准备时算的就是这个函数
        assert_eq!(
            resend_digest(&plan.request, &plan.items, ME, ServerKind::Main),
            plan.digest
        );
        let mut other = plan.items.clone();
        other[0].text = Some("别的".into());
        assert_ne!(
            resend_digest(&plan.request, &other, ME, ServerKind::Main),
            plan.digest
        );
    }

    #[test]
    fn long_chat_title_counts_toward_what_must_fit() {
        // 正文很短，但会话名长到把整段说明撑出确认框：照样拒，不弹框
        let mut r = rig("long-title", false, false, Ok(()));
        let title = "周末爬山群".repeat(50);
        r.b.set_chat(
            888,
            vec![new_chat(
                888,
                &title,
                json!({"@type": "chatTypePrivate", "user_id": 888}),
            )],
        );
        let e = send_once(&mut r, 888, &draft("好的", None), false).unwrap_err();
        assert_eq!(refused_reason(e), SendRefusedReason::TooLongToConfirm);
        assert_eq!(r.asks.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn resend_rechecks_before_submitting() {
        // codex 016 第 2 轮 #2：准备到交出之间那条被改了 → 拒、什么都不交出；没变 → 照常交出
        let failed = |text: &str| {
            let mut m = msg(ME, 100, text, Value::Null);
            m["sending_state"] = json!({"@type": "messageSendingStateFailed",
                "error": {"@type": "error", "code": 400, "message": "X"}, "can_retry": true,
                "need_another_sender": false, "need_another_reply_quote": false, "need_drop_reply": false,
                "required_paid_message_star_count": 0, "retry_after": 0.0});
            m
        };
        let r = rig("recheck", false, true, Ok(()));
        r.b.set_history(ME, vec![failed("周末见")]);
        let plan = block_on(prepare_resend(&r.b, ME, &[100], SendOrigin::Cli)).unwrap();
        r.b.set_history(ME, vec![failed("改过的话")]);
        let t = r.b.gate().issue(&plan, false).unwrap();
        let e = block_on(resend(&r.b, &plan, t)).unwrap_err();
        assert_eq!(refused_reason(e), SendRefusedReason::ContentMismatch);
        assert!(r.b.submitted().is_empty());
        // 没变：真的交出了重发
        r.b.set_history(ME, vec![failed("周末见")]);
        let plan = block_on(prepare_resend(&r.b, ME, &[100], SendOrigin::Cli)).unwrap();
        let t = r.b.gate().issue(&plan, false).unwrap();
        block_on(resend(&r.b, &plan, t)).unwrap();
        assert_eq!(r.b.submitted().len(), 1);
    }

    #[test]
    fn many_resend_items_do_not_fit() {
        let r = rig("many", false, false, Ok(()));
        let failed = |id: i64| {
            let mut m = msg(ME, id, "一", Value::Null);
            m["sending_state"] = json!({"@type": "messageSendingStateFailed",
                "error": {"@type": "error", "code": 400, "message": "X"}, "can_retry": true,
                "need_another_sender": false, "need_another_reply_quote": false, "need_drop_reply": false,
                "required_paid_message_star_count": 0, "retry_after": 0.0});
            m
        };
        let ids: Vec<i64> = (100..110).collect();
        r.b.set_history(ME, ids.iter().map(|&i| failed(i)).collect());
        let plan = block_on(prepare_resend(&r.b, ME, &ids, SendOrigin::Cli)).unwrap();
        // 每条正文只有一个字，合起来很短，但十条的行数放不下
        assert!(!plan.confirm_fits());
        let e = r.b.gate().issue(&plan, false).unwrap_err();
        assert_eq!(refused_reason(e), SendRefusedReason::TooLongToConfirm);
    }
}
