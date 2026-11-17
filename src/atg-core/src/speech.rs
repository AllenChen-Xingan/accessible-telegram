//! 读法：内容树 → 读屏要读的那句话。纯函数，可快照测试。
//!
//! 三层：`brief`（名称）与会话项的 `chat_brief`；[`detail`]（按详情键展开的那些：日期、回复谁、转发自、已编辑、
//! 表情回应、话题名、送达状态，R3–R5）；[`full`]（全文：正文原样，保留换行与标点，不接发送人与钟点）。
//! 「名称」= 无障碍名称：VoiceOver 读到列表里一项时念的那句话，GUI 和 CLI 的 `speech.brief` 都取这里。
//! `detail` 与 `full` 由路线图 M1.6 加（`atg-cli history --detail / --full`，#21）。**详情的句子是 AI 草稿，待人实听**
//! （`specs/style/` 只定了哪些进详情，没定句子；快照 `*detail*.snap` 为正本）。
//!
//! 隐藏内容（剧透、18+ 遮挡）三层都只读「隐藏内容」（R1）；[`SpeechCtx::reveal`] 为真 = 人按了展开键，才读原文。
//!
//! 规则正本 `specs/content-tree.md` 第九节（原则与 R1–R7 人定）。具体说法的题 V1–V23 **人定 2026-10-26**，
//! 记录在 `docs/plan/007_decisions-b15.md`；
//! 题面与例子在三份 `specs/style/`（#20 的读法规约，随 #20 合并进来）：`chat-item.md`（会话项，例 C*、
//! 题 V1–V7、V20）· `message.md`（消息，例 M*、题 V8–V15、V21–V22）· `service.md`（系统消息，例 S*、题 V16–V19、V23）。
//! 下面注释里「Vn 人定」就指 007 那一行。「N8」等是更早拍板的题，记在 `docs/plan/00N_decisions-*.md`
//! （见 `lib.rs`「注释里的代号」）。
//!
//! 渲染器规矩：对树的每个枚举都用**不带通配符**的 `match`，加了新节点种类，这里编不过，
//! 逼着人决定怎么读。下面这条 lint 把 `_ =>` 也拦住。
#![deny(clippy::wildcard_enum_match_arm)]

use crate::tree::*;

mod content;
mod interactive;
mod playback;
pub mod reaction;
use reaction::reactions;
mod service;
mod status;
mod transcription;
use content::{content, dur, spans};
// 只给测试用（`tests.rs` 经 `use super::*` 拿）；正式路径在 content.rs 里由 `spans` 调用
#[cfg(test)]
use content::merge_emoji;
use service::service;

/// 读法需要、但不在树里的东西
#[derive(Debug, Clone)]
pub struct SpeechCtx {
    /// 本地时区偏移（秒），由调用方给，读法不碰系统时钟。`offset_at` 为空时所有钟点都用它（如 `--tz +08:00`）
    pub utc_offset: i32,
    /// 按时间点求偏移（Unix 秒 → 秒）。有夏令时的时区，偏移随日期变，所以跟系统时区读时由调用方给这个函数，
    /// 每个钟点各取各的；为空就一律用 `utc_offset`
    pub offset_at: Option<fn(i64) -> i32>,
    /// 新消息提醒里「按 M 收听」那个键；`None` 就不读（CLI 没有这个键）
    pub listen_key: Option<char>,
    /// 隐藏内容「按 D 展开」那个键（R1）；`None` 就只读「隐藏内容」（CLI 没有这个键）
    pub expand_key: Option<char>,
    /// 已展开隐藏内容（= 人按了展开键；CLI 的 `--reveal`）：剧透、18+ 遮挡照读原文，不再说「隐藏内容」与按键。
    /// R1 人定默认遮住，所以调用方只在人明确要求时才置真
    pub reveal: bool,
}

pub fn brief(doc: &Doc, cx: &SpeechCtx) -> String {
    match doc {
        Doc::Message(m) => {
            let (body, hidden) = content(&m.content, cx.reveal);
            let line = then(&expand_hint(&body, hidden, cx), &hm(m.head.date, cx));
            match speaker(
                &m.head.sender,
                m.head.outgoing,
                m.id.chat_id,
                m.head.in_channel,
            ) {
                Some(who) => format!("{who}：{line}"),
                None => line,
            }
        }
        // #68：置顶系统消息带的内容里有隐藏内容，同样在末尾给按键提示
        Doc::Service(sv) => {
            let (said, hidden) = service(sv, cx.reveal);
            then(&expand_hint(&said, hidden, cx), &hm(sv.date, cx))
        }
        // R7：「林小雨撤回了一条消息，19:36」；V15 人定：没记到时间就不读时间
        Doc::Tombstone(t) => {
            let who = tomb_who(t);
            match t.date {
                Some(d) => format!("{who}{REVOKED}，{}", hm(d, cx)),
                None => format!("{who}{REVOKED}"),
            }
        }
        Doc::ChatItem(c) => chat_brief_with(c, cx.reveal),
        Doc::Alert(a) => alert(a, cx),
        Doc::Status(s) => status::brief(s.status).to_owned(),
        Doc::Playback(p) => playback::brief(p),
        Doc::Reactor(r) => reaction::reactor(r, cx),
        Doc::Transcription(t) => transcription::brief(t, cx.reveal),
        Doc::Unknown(_) => UNSUPPORTED.to_owned(),
    }
}

/// 会话列表一项：名字，未读，提到你，最后一条摘要。最后一条不读时间、**不截断**（R2 人定：听够了人自己打断，
/// `specs/content-tree.md` 第九节）。V1、V2、V7 人定：静音、置顶、新回应、标为未读都不读；
/// V20 人定：没有消息不说「暂无消息」；V5 人定：隐藏内容不带按键（会话列表里展开不了一条消息）
pub fn chat_brief(c: &ChatItem) -> String {
    chat_brief_with(c, false)
}

/// 同 [`chat_brief`]；`reveal` 为真（人展开了隐藏内容，CLI 的 `--reveal`）时最后一条里的剧透照读原文
pub fn chat_brief_with(c: &ChatItem, reveal: bool) -> String {
    let private = matches!(c.kind, ChatKind::Private | ChatKind::Secret);
    let name = if c.saved_messages {
        // V6 人定：和自己的私聊读「收藏夹」（TDLib 给的标题是自己的名字）
        SAVED.to_owned()
    } else if !c.title.trim().is_empty() {
        c.title.clone()
    } else if private && c.peer_deleted {
        // 人定（#20 评论）+ V4 人定：私聊标题为空、且对方确为注销账号才读「已注销的账号」
        DELETED_ACCOUNT.to_owned()
    } else {
        // V4 人定：标题为空但不是注销（如用户信息还没到）
        UNTITLED.to_owned()
    };
    let mut parts = vec![name];
    if c.unread_count > 0 {
        parts.push(format!("{} 条未读", c.unread_count));
    }
    if c.unread_mention_count > 0 {
        parts.push(format!("{} 条提到你", c.unread_mention_count));
    }
    if let Some(last) = &c.last {
        // 私聊里对方发的、频道以自己名义发的，都不重复读名字；V3 人定：撤回占位同理（私聊省名）
        let channel = c.kind == ChatKind::Channel;
        let omit_name = |sender: &Sender, outgoing: bool| {
            !outgoing
                && (private
                    || (channel
                        && matches!(sender, Sender::Chat { chat_id, .. } if *chat_id == c.chat_id)))
        };
        let summary = match last.as_ref() {
            ChatLast::Message(m) if omit_name(&m.head.sender, m.head.outgoing) => {
                content(&m.content, reveal).0
            }
            ChatLast::Message(m) => {
                let body = content(&m.content, reveal).0;
                match speaker(&m.head.sender, m.head.outgoing, c.chat_id, channel) {
                    Some(who) => format!("{who}：{body}"),
                    None => body,
                }
            }
            ChatLast::Service(sv) => service(sv, reveal).0,
            // R7 同形，会话项里不带时间（N8 人定，`docs/plan/005_decisions-b3.md`）
            // 私聊里不是自己撤回的就一定是对方，记没记到发送人都省名
            ChatLast::Tombstone(t) => match &t.sender {
                _ if private && !t.outgoing => REVOKED.to_owned(),
                Some(s) if omit_name(s, t.outgoing) => REVOKED.to_owned(),
                Some(_) | None => format!("{}{REVOKED}", tomb_who(t)),
            },
        };
        parts.push(summary);
    }
    parts.join("，")
}

/// 详情（`--detail`，R3–R5 定了这些不进名称、进详情）：日期 · 回复谁 · 转发自 · 已编辑 · 表情回应 · 话题名 ·
/// 自己发的送达状态 · 转写 · 投票的性质与按钮（#13），有哪项说哪项，用「；」分段。系统消息、撤回占位只有日期；别的节点没有详情（空串）。
/// **句子是 AI 草稿，待人实听**（`specs/style/` 没定详情的说法）
pub fn detail(doc: &Doc, cx: &SpeechCtx) -> String {
    match doc {
        Doc::Message(m) => {
            let mut parts = vec![date(m.head.date, cx)];
            if let Some(r) = &m.head.reply_to {
                parts.push(reply(r, m.id.chat_id, cx.reveal));
            }
            if let Some(f) = &m.head.forward {
                parts.push(format!(
                    "转发自{}，原发于 {}",
                    origin_name(&f.origin),
                    date(f.date, cx)
                ));
            }
            if let Some(t) = m.tail.edited {
                parts.push(format!("已编辑，{}", date_or_hm(t, m.head.date, cx)));
            }
            if !m.tail.reactions.is_empty() {
                parts.push(reactions(&m.tail.reactions));
            }
            if let Some(t) = topic(m.head.topic.as_ref()) {
                parts.push(t);
            }
            if let Some(d) = m.tail.delivery {
                parts.push(delivery(d).to_owned());
            }
            parts.extend(content::transcript_detail(m, cx.reveal));
            // #13：投票的性质、按钮逐个
            if let Some(Attachment::Poll {
                options,
                results_visible,
                anonymous,
                multiple,
                revotable,
                quiz,
                closed,
                blocked,
                ..
            }) = &m.content.attachment
            {
                parts.push(interactive::poll_detail(&interactive::PollFacts {
                    options: options.len(),
                    voted: options.iter().any(|o| o.chosen),
                    results_visible: *results_visible,
                    anonymous: *anonymous,
                    multiple: *multiple,
                    revotable: *revotable,
                    quiz: *quiz,
                    closed: *closed,
                    blocked: blocked.as_ref(),
                }));
            }
            if let Some(k) = &m.buttons {
                parts.push(interactive::buttons_detail(k));
            }
            parts.join("；")
        }
        Doc::Service(sv) => date(sv.date, cx),
        Doc::Tombstone(t) => t.date.map(|d| date(d, cx)).unwrap_or_default(),
        Doc::Reactor(r) => date(r.date, cx),
        Doc::ChatItem(_) | Doc::Alert(_) | Doc::Status(_) | Doc::Playback(_) | Doc::Unknown(_) => {
            String::new()
        }
        Doc::Transcription(_) => String::new(),
    }
}

/// 全文（`--full`）：正文原样——保留换行、末尾标点，表情不合并，不接发送人与钟点；附件照名称的说法放在前面。
/// 隐藏内容照样只读「隐藏内容」，[`SpeechCtx::reveal`] 为真才读原文（R1；大总管授权 2026-10-26）。
/// 系统消息、撤回占位与名称同一句、不带钟点；别的节点同 [`brief`]
pub fn full(doc: &Doc, cx: &SpeechCtx) -> String {
    match doc {
        Doc::Message(m) => content::full(m, cx.reveal),
        Doc::Service(sv) => service(sv, cx.reveal).0,
        Doc::Tombstone(t) => format!("{}{REVOKED}", tomb_who(t)),
        Doc::ChatItem(_) | Doc::Alert(_) | Doc::Status(_) | Doc::Playback(_) | Doc::Reactor(_) => {
            brief(doc, cx)
        }
        Doc::Unknown(_) => brief(doc, cx),
        Doc::Transcription(_) => brief(doc, cx),
    }
}

/// 「2026年10月26日 19:35」（详情里的日期，R3）
fn date(t: Time, cx: &SpeechCtx) -> String {
    let offset = cx.offset_at.map_or(cx.utc_offset, |f| f(t.0));
    let (y, m, d) = t.local_ymd(offset);
    format!("{y}年{m}月{d}日 {}", hm(t, cx))
}

/// 与 `base` 同一天只读钟点，否则读日期
fn date_or_hm(t: Time, base: Time, cx: &SpeechCtx) -> String {
    let day = |x: Time| x.local_ymd(cx.offset_at.map_or(cx.utc_offset, |f| f(x.0)));
    if day(t) == day(base) {
        hm(t, cx)
    } else {
        date(t, cx)
    }
}

/// 回复谁：「回复周大山：〈引用的那段〉」；被回复那条在别的会话里加「另一个会话里」
fn reply(r: &ReplyTo, chat_id: i64, reveal: bool) -> String {
    match r {
        ReplyTo::Message {
            target,
            sender,
            quote,
        } => {
            let elsewhere = target.chat_id != chat_id;
            let who = match (sender, elsewhere) {
                (Some(s), false) => sender_name(s, UNKNOWN_SENDER),
                (Some(s), true) => format!("另一个会话里{}的消息", sender_name(s, UNKNOWN_SENDER)),
                (None, false) => "一条消息".to_owned(),
                (None, true) => "另一个会话里的一条消息".to_owned(),
            };
            match quote.as_deref().map(|q| spans(q, reveal).0) {
                Some(q) if !q.trim().is_empty() => format!("回复{who}：{}", q.trim()),
                Some(_) | None => format!("回复{who}"),
            }
        }
        ReplyTo::Unknown { .. } => "回复了暂不支持的内容".to_owned(),
    }
}

/// 转发来源的名字
fn origin_name(o: &Origin) -> String {
    match o {
        Origin::User { name, .. } => name_or(name, UNKNOWN_SENDER),
        Origin::HiddenUser { name } => name_or(&Some(name.clone()), UNKNOWN_SENDER),
        Origin::Chat { title, .. } | Origin::Channel { title, .. } => name_or(title, UNTITLED),
        Origin::Unknown { .. } => "未知来源".to_owned(),
    }
}

/// 话题名：只有论坛话题读（回复串、私信话题、收藏夹分组不读）；名字没到读「未知话题」
fn topic(t: Option<&Topic>) -> Option<String> {
    match t? {
        Topic::Forum { name, .. } => Some(format!("话题：{}", name_or(name, "未知话题"))),
        Topic::Thread { .. }
        | Topic::DirectMessages { .. }
        | Topic::SavedMessages { .. }
        | Topic::Unknown { .. } => None,
    }
}

/// 自己发的：送达到哪一步
fn delivery(d: Delivery) -> &'static str {
    match d {
        Delivery::Pending => "发送中",
        Delivery::Failed => "发送失败",
        Delivery::Sent => "已发出",
        Delivery::Read => "已读",
    }
}

fn alert(a: &Alert, cx: &SpeechCtx) -> String {
    let mut s = format!("{} 条新消息", a.total);
    if let [only] = a.chats.as_slice() {
        s.push_str(&format!(
            "，来自{}",
            only.title.as_deref().unwrap_or(UNTITLED)
        ));
    }
    if let Some(k) = cx.listen_key {
        s.push_str(&format!("，按 {} 收听", k.to_ascii_uppercase()));
    }
    s
}

/// 消息的说话人；`None` = 不读名字。
/// - 自己发的读「我」
/// - 发送人就是所在会话：频道里是频道自己发的，不读名字（V10 人定）；群里是匿名管理员（V11 人定）
/// - 已注销读「已注销的账号」（V14 人定），查不到读「未知发送人」（V18 人定：消息发送人这一场合）
fn speaker(s: &Sender, outgoing: bool, chat_id: i64, in_channel: bool) -> Option<String> {
    if outgoing {
        return Some(ME.to_owned());
    }
    match s {
        Sender::Chat { chat_id: c, .. } if *c == chat_id => {
            (!in_channel).then(|| ANONYMOUS_ADMIN.to_owned())
        }
        Sender::User { .. } | Sender::Chat { .. } | Sender::Unknown { .. } => {
            Some(sender_name(s, UNKNOWN_SENDER))
        }
    }
}

/// 撤回占位的说话人：自己读「我」，记不到读「未知发送人」（N8 人定）
fn tomb_who(t: &Tombstone) -> String {
    if t.outgoing {
        return ME.to_owned();
    }
    t.sender.as_ref().map_or_else(
        || UNKNOWN_SENDER.to_owned(),
        |s| sender_name(s, UNKNOWN_SENDER),
    )
}

/// 发送人的名字；已注销读「已注销的账号」，查不到读 `fallback`
fn sender_name(s: &Sender, fallback: &str) -> String {
    match s {
        Sender::User { deleted: true, .. } => DELETED_ACCOUNT.to_owned(),
        Sender::User { name, .. } => name_or(name, fallback),
        Sender::Chat { title, .. } => name_or(title, fallback),
        Sender::Unknown { .. } => fallback.to_owned(),
    }
}

fn name_or(n: &Option<String>, fallback: &str) -> String {
    match n.as_deref() {
        Some(x) if !x.trim().is_empty() => x.to_owned(),
        _ => fallback.to_owned(),
    }
}

/// V13 人定：多处隐藏内容，各处只读「隐藏内容」，按键提示在整条末尾、钟点前说一次（消息、置顶系统消息都这样）
fn expand_hint(said: &str, hidden: bool, cx: &SpeechCtx) -> String {
    match (hidden, cx.expand_key) {
        (true, Some(k)) => then(said, &format!("按 {} 展开", k.to_ascii_uppercase())),
        (false, _) | (true, None) => said.to_owned(),
    }
}

/// 两段接起来：前一段以问号、叹号结尾就直接接，否则用逗号隔开（V22 人定：问号叹号后不再加逗号）
fn then(a: &str, b: &str) -> String {
    if a.ends_with(['？', '！', '?', '!']) {
        format!("{a}{b}")
    } else {
        format!("{a}，{b}")
    }
}

/// 自己（原则：自己发的读「我」；V17 人定：被操作的是自己也读「我」）
const ME: &str = "我";
/// 名字查不到时（V18 人定：分场合）：消息发送人、撤回占位
const UNKNOWN_SENDER: &str = "未知发送人";
/// 名字查不到时：系统消息的操作人
const UNKNOWN_ACTOR: &str = "有人";
/// 名字查不到时：系统消息里被添加、被移出的人
const UNKNOWN_USER: &str = "未知用户";
/// 群里以群的名义发言（V11 人定）
const ANONYMOUS_ADMIN: &str = "匿名管理员";
/// 不认识、或认识但还没做读法的内容：一律这一种，类型名只进 JSON
const UNSUPPORTED: &str = "暂不支持的消息";
/// 没做读法的系统消息（V19 人定）
const UNSUPPORTED_SERVICE: &str = "暂不支持的系统消息";
/// 剧透、18+ 遮挡统一叫「隐藏」（R1）；按键提示另在整条末尾说（V13）
const HIDDEN: &str = "隐藏内容";
/// 撤回占位（R7）：前面接发送人
const REVOKED: &str = "撤回了一条消息";
/// 私聊对方已注销（人定，#20 评论；V4、V14 人定）
const DELETED_ACCOUNT: &str = "已注销的账号";
/// 群、频道标题为空，或私聊标题为空但对方不是注销账号（V4 人定）
const UNTITLED: &str = "未命名会话";
/// 和自己的私聊（V6 人定）
const SAVED: &str = "收藏夹";

fn hm(t: Time, cx: &SpeechCtx) -> String {
    let offset = cx.offset_at.map_or(cx.utc_offset, |f| f(t.0));
    let (h, m) = t.local_hm(offset);
    format!("{h}:{m:02}")
}

#[cfg(test)]
mod tests;
