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
            // V13 人定：多处隐藏内容，各处只读「隐藏内容」，按键提示在整条末尾、钟点前说一次
            let body = match (hidden, cx.expand_key) {
                (true, Some(k)) => then(&body, &format!("按 {} 展开", k.to_ascii_uppercase())),
                (false, _) | (true, None) => body,
            };
            let line = then(&body, &hm(m.head.date, cx));
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
        Doc::Service(sv) => then(&service(sv, cx.reveal), &hm(sv.date, cx)),
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
        Doc::Status(s) => status(s.status).to_owned(),
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
            ChatLast::Service(sv) => service(sv, reveal),
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
/// 自己发的送达状态，有哪项说哪项，用「；」分段。系统消息、撤回占位只有日期；别的节点没有详情（空串）。
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
            parts.join("；")
        }
        Doc::Service(sv) => date(sv.date, cx),
        Doc::Tombstone(t) => t.date.map(|d| date(d, cx)).unwrap_or_default(),
        Doc::ChatItem(_) | Doc::Alert(_) | Doc::Status(_) | Doc::Unknown(_) => String::new(),
    }
}

/// 全文（`--full`）：正文原样——保留换行、末尾标点，表情不合并，不接发送人与钟点；附件照名称的说法放在前面。
/// 隐藏内容照样只读「隐藏内容」，[`SpeechCtx::reveal`] 为真才读原文（R1；大总管授权 2026-10-26）。
/// 系统消息、撤回占位与名称同一句、不带钟点；别的节点同 [`brief`]
pub fn full(doc: &Doc, cx: &SpeechCtx) -> String {
    match doc {
        Doc::Message(m) => full_content(&m.content, cx.reveal),
        Doc::Service(sv) => service(sv, cx.reveal),
        Doc::Tombstone(t) => format!("{}{REVOKED}", tomb_who(t)),
        Doc::ChatItem(_) | Doc::Alert(_) | Doc::Status(_) | Doc::Unknown(_) => brief(doc, cx),
    }
}

fn full_content(c: &Content, reveal: bool) -> String {
    let body = full_spans(&c.body, reveal);
    let label = c.attachment.as_ref().map(|a| {
        let (l, spoiler) = attachment(a, reveal);
        if (spoiler || c.sensitive) && !reveal {
            format!("{l}，{HIDDEN}")
        } else {
            l
        }
    });
    match (label, body.trim().is_empty()) {
        (None, true) => "空消息".to_owned(),
        (None, false) => body,
        (Some(l), true) => l,
        (Some(l), false) => format!("{l}，{body}"),
    }
}

/// 遮住处先放占位符，最后看两边：紧挨着别的字才补逗号（原文自己的空格、标点、换行照留）
const MASK: char = '\u{FFFC}';

fn full_spans(list: &[Span], reveal: bool) -> String {
    let mut raw = String::new();
    for s in list {
        full_span(&mut raw, s, reveal);
    }
    let pause = |c: char| c.is_whitespace() || "，、。；：！？,.;:!?「」（）()".contains(c);
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    for (i, &c) in chars.iter().enumerate() {
        if c != MASK {
            out.push(c);
            continue;
        }
        if out.chars().last().is_some_and(|p| !pause(p) && p != MASK) {
            out.push('，');
        }
        out.push_str(HIDDEN);
        if chars.get(i + 1).is_some_and(|&n| !pause(n)) {
            out.push('，');
        }
    }
    out
}

fn full_span(out: &mut String, s: &Span, reveal: bool) {
    match s {
        Span::Text { text }
        | Span::Code { text }
        | Span::Pre { text, .. }
        | Span::DateTime { text, .. } => out.push_str(&text.replace(MASK, "")),
        Span::Spoiler { children } if reveal => {
            for c in children {
                full_span(out, c, reveal);
            }
        }
        // 相邻的几段剧透只说一次
        Span::Spoiler { .. } => {
            if !out.ends_with(MASK) {
                out.push(MASK);
            }
        }
        Span::Bold { children }
        | Span::Italic { children }
        | Span::Underline { children }
        | Span::Strikethrough { children }
        | Span::Quote { children, .. }
        | Span::Link { children, .. }
        | Span::Url { children }
        | Span::Mention { children }
        | Span::MentionName { children, .. }
        | Span::Hashtag { children }
        | Span::Cashtag { children }
        | Span::BotCommand { children }
        | Span::Email { children }
        | Span::Phone { children }
        | Span::BankCard { children }
        | Span::TonAddress { children }
        | Span::CustomEmoji { children, .. }
        | Span::MediaTimestamp { children, .. }
        | Span::Unknown { children, .. } => {
            for c in children {
                full_span(out, c, reveal);
            }
        }
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

/// 「回应：3 个👍、1 个❤️，我点了👍」
fn reactions(list: &[Reaction]) -> String {
    let label = |r: &Reaction| match &r.kind {
        ReactionKind::Emoji { emoji } => emoji.clone(),
        ReactionKind::CustomEmoji { .. } => "自定义表情".to_owned(),
        ReactionKind::Paid => "星星".to_owned(),
        ReactionKind::Unknown { .. } => "暂不支持的回应".to_owned(),
    };
    let all: Vec<String> = list
        .iter()
        .map(|r| format!("{} 个{}", r.count, label(r)))
        .collect();
    let mine: Vec<String> = list.iter().filter(|r| r.chosen).map(label).collect();
    let mut s = format!("回应：{}", all.join("、"));
    if !mine.is_empty() {
        s.push_str(&format!("，我点了{}", mine.join("、")));
    }
    s
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

fn status(s: UserStatus) -> &'static str {
    match s {
        UserStatus::Fault => "连接已中断，需要重启",
        // `specs/contracts/errors.md` 第一节第 3 小节：这一态不主动播报；但焦点落在状态项上时仍要有一句名称。说法待人实听定标
        UserStatus::Stopping => "正在退出",
        UserStatus::SignedOut => "未登录",
        UserStatus::SigningIn => "正在登录",
        UserStatus::UnsupportedStep => "这一步本应用暂不支持，请先在官方客户端完成",
        UserStatus::Offline => "网络已断开，正在等待网络",
        UserStatus::Connecting => "正在连接",
        UserStatus::Syncing => "正在同步",
        UserStatus::Online => "已连接",
    }
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

/// 系统消息：「谁做了什么」，不带钟点。
/// - 操作人：自己读「我」；群里以群的名义做的读「匿名管理员」（V11）；查不到读「有人」（V18）；已注销读「已注销的账号」（V14）
/// - 被操作的人：自己读「我」（V17 人定「周大山添加了我」）；查不到读「未知用户」（V18）；已注销读「已注销的账号」
/// - 频道里「群组 / 群名 / 群头像」换成「频道 / 频道名 / 频道头像」（style 例 S7）
fn service(sv: &Service, reveal: bool) -> String {
    let actor = if sv.outgoing {
        ME.to_owned()
    } else {
        match &sv.actor {
            Sender::Chat { chat_id, .. } if *chat_id == sv.id.chat_id && !sv.in_channel => {
                ANONYMOUS_ADMIN.to_owned()
            }
            Sender::User { .. } | Sender::Chat { .. } | Sender::Unknown { .. } => {
                sender_name(&sv.actor, UNKNOWN_ACTOR)
            }
        }
    };
    let user_name = |u: &UserRef| {
        if u.is_me {
            ME.to_owned()
        } else if u.deleted {
            DELETED_ACCOUNT.to_owned()
        } else {
            name_or(&u.name, UNKNOWN_USER)
        }
    };
    let (place, title_word, photo) = if sv.in_channel {
        ("频道", "频道名", "频道头像")
    } else {
        ("群组", "群名", "群头像")
    };
    match &sv.action {
        Action::MembersAdded { users } => {
            let names: Vec<String> = users.iter().map(user_name).collect();
            let joined_self = matches!(
                (&sv.actor, users.as_slice()),
                (Sender::User { user_id, .. }, [only]) if only.user_id == *user_id
            );
            if joined_self {
                format!("{actor}加入了{place}")
            } else {
                format!("{actor}添加了{}", names.join("、"))
            }
        }
        Action::JoinedByLink => format!("{actor}通过邀请链接加入了{place}"),
        Action::JoinedByRequest => {
            let what = if sv.in_channel {
                "加入申请"
            } else {
                "入群申请"
            };
            format!("{actor}的{what}已通过")
        }
        Action::MemberRemoved { user } => {
            let left_self =
                matches!(&sv.actor, Sender::User { user_id, .. } if *user_id == user.user_id);
            if left_self {
                format!("{actor}退出了{place}")
            } else {
                format!("{actor}移出了{}", user_name(user))
            }
        }
        Action::TitleChanged { title } => format!("{actor}把{title_word}改为「{title}」"),
        Action::PhotoChanged => format!("{actor}更换了{photo}"),
        Action::PhotoDeleted => format!("{actor}删除了{photo}"),
        // V16 人定：带上被置顶那条的内容；查不到（不在本地、已撤回）就只说置顶了一条
        Action::Pinned {
            content: Some(c), ..
        } => {
            format!("{actor}置顶了一条消息：{}", content(c, reveal).0)
        }
        Action::Pinned { content: None, .. } => format!("{actor}置顶了一条消息"),
        Action::GroupCreated { title } => format!("{actor}创建了{place}「{title}」"),
        Action::ContactRegistered => format!("{actor}加入了 Telegram"),
        Action::Other { .. } => UNSUPPORTED_SERVICE.to_owned(),
    }
}

/// 正文 + 附件的简短说法，以及里面有没有隐藏内容（剧透、18+ 遮挡）。按键提示由调用方决定加不加
fn content(c: &Content, reveal: bool) -> (String, bool) {
    let (body, body_hidden) = spans(&c.body, reveal);
    // 句号、逗号后面还要接「，时间」，去掉免得读成「。，」；问号叹号带语气、英文句点可能是省略号，留着
    let body = body.trim().trim_end_matches(['。', '，', ',']).trim_end();
    let mut hidden = body_hidden;
    let label = c.attachment.as_ref().map(|a| {
        let (l, spoiler) = attachment(a, reveal);
        // 剧透、18+ 遮挡同一种说法，两样都有也只读一次；已展开就不说
        if (spoiler || c.sensitive) && !reveal {
            hidden = true;
            format!("{l}，{HIDDEN}")
        } else {
            l
        }
    });
    let text = match (label, body.is_empty()) {
        (None, true) => "空消息".to_owned(),
        (None, false) => body.to_owned(),
        (Some(l), true) => l,
        (Some(l), false) => format!("{l}，{body}"),
    };
    (text, hidden)
}

fn attachment(a: &Attachment, reveal: bool) -> (String, bool) {
    let label = match a {
        Attachment::Photo { spoiler } => return ("图片".to_owned(), *spoiler),
        Attachment::Video { duration, spoiler } => {
            return (format!("视频 {}", dur(*duration)), *spoiler);
        }
        Attachment::Voice { duration, .. } => format!("语音 {}", dur(*duration)),
        Attachment::VideoNote { duration, .. } => format!("圆形视频 {}", dur(*duration)),
        Attachment::Audio {
            duration,
            title,
            performer,
        } => {
            let mut parts = vec!["音频".to_owned()];
            parts.extend(
                [title, performer]
                    .into_iter()
                    .filter(|x| !x.is_empty())
                    .cloned(),
            );
            parts.push(dur(*duration));
            parts.join("，")
        }
        Attachment::Animation { .. } => "动图".to_owned(),
        Attachment::Document { file_name, .. } => {
            if file_name.is_empty() {
                "文件".to_owned()
            } else {
                format!("文件，{file_name}")
            }
        }
        Attachment::Sticker { emoji } => labeled("贴纸", emoji),
        Attachment::AnimatedEmoji { emoji } => emoji.clone(),
        Attachment::Location => "位置".to_owned(),
        Attachment::Venue { title } => labeled("地点", title),
        Attachment::Contact { name } => labeled("联系人", name),
        // 投票题目里的剧透只读「隐藏内容」，不算进整条的按键提示（展开的是整条消息）
        Attachment::Poll { question } => labeled("投票", &spans(question, reveal).0),
        Attachment::Call {
            video,
            duration,
            missed,
        } => {
            let kind = if *video {
                "视频通话"
            } else {
                "语音通话"
            };
            if *missed {
                format!("未接{kind}")
            } else {
                format!("{kind} {}", dur(*duration))
            }
        }
        Attachment::Expired { what } => {
            let w = match what {
                Expirable::Photo => "图片",
                Expirable::Video => "视频",
                Expirable::VideoNote => "圆形视频",
                Expirable::Voice => "语音",
            };
            format!("已过期的{w}")
        }
        Attachment::Restricted { .. } => "受限内容，无法查看".to_owned(),
        Attachment::Other { .. } | Attachment::Unknown { .. } => UNSUPPORTED.to_owned(),
    };
    (label, false)
}

/// 「类型，内容」；内容为空只读类型
fn labeled(kind: &str, what: &str) -> String {
    let what = what.trim();
    if what.is_empty() {
        kind.to_owned()
    } else {
        format!("{kind}，{what}")
    }
}

/// 行内树的简短说法，以及有没有剧透：格式不读；剧透只读「隐藏内容」（R1）；链接只读文字不读地址；
/// 连续 3 个及以上相同的表情合并成「5 个😂」（V12 人定）
fn spans(list: &[Span], reveal: bool) -> (String, bool) {
    let mut out = String::new();
    let mut hidden = false;
    for s in list {
        span(&mut out, s, &mut hidden, reveal);
    }
    // 「隐藏内容」两边加的逗号可能和原文的空格、逗号、开头撞上
    let mut out = out.replace(" ，", "，");
    while out.contains("，，") {
        out = out.replace("，，", "，");
    }
    (merge_emoji(out.trim_start_matches('，')), hidden)
}

fn span(out: &mut String, s: &Span, hidden: &mut bool, reveal: bool) {
    match s {
        Span::Text { text }
        | Span::Code { text }
        | Span::Pre { text, .. }
        | Span::DateTime { text, .. } => out.push_str(text),
        // 已展开：照读里面的字
        Span::Spoiler { children } if reveal => {
            for c in children {
                span(out, c, hidden, reveal);
            }
        }
        // VoiceOver 不念括号，用逗号隔开才听得出这是标注
        Span::Spoiler { .. } => {
            *hidden = true;
            out.push('，');
            out.push_str(HIDDEN);
            out.push('，');
        }
        Span::Bold { children }
        | Span::Italic { children }
        | Span::Underline { children }
        | Span::Strikethrough { children }
        | Span::Quote { children, .. }
        | Span::Link { children, .. }
        | Span::Url { children }
        | Span::Mention { children }
        | Span::MentionName { children, .. }
        | Span::Hashtag { children }
        | Span::Cashtag { children }
        | Span::BotCommand { children }
        | Span::Email { children }
        | Span::Phone { children }
        | Span::BankCard { children }
        | Span::TonAddress { children }
        | Span::CustomEmoji { children, .. }
        | Span::MediaTimestamp { children, .. }
        | Span::Unknown { children, .. } => {
            for c in children {
                span(out, c, hidden, reveal);
            }
        }
    }
}

/// 连续 3 个及以上相同的表情 → 「N 个😂」（V12 人定）。只改名称，复制全文、阅读视图照原样。
/// 合并后紧挨着文字时两边加逗号（「好的，3 个👍」），免得听成「好的三个」——这一点 style 稿没写，
/// AI 先定的默认，**待人定**（另一选项：紧挨文字的表情不合并）；题面已交 `state/coord.md`「待定清单」
fn merge_emoji(s: &str) -> String {
    // 已经有停顿的地方不再加逗号
    let pause = |c: char| c.is_whitespace() || "，、。；：！？,.;:!?「」（）()".contains(c);
    let clusters = emoji_clusters(s);
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < clusters.len() {
        let (text, is_emoji) = clusters[i];
        let mut n = 1;
        while is_emoji && i + n < clusters.len() && clusters[i + n] == (text, true) {
            n += 1;
        }
        if is_emoji && n >= 3 {
            if out.chars().last().is_some_and(|c| !pause(c)) {
                out.push('，');
            }
            out.push_str(&format!("{n} 个{text}"));
            if let Some(&(next, _)) = clusters.get(i + n)
                && next.chars().next().is_some_and(|c| !pause(c))
            {
                out.push('，');
            }
        } else {
            for _ in 0..n {
                out.push_str(text);
            }
        }
        i += n;
    }
    out
}

/// 把文字切成「表情」与「别的字」两种片段。表情 = 一个图形符号，后面跟着的变体选择符、肤色、
/// 键帽符、零宽连接（及它连上的下一个符号）、成对的国旗字母都算同一个。不引字素切分库（与
/// `specs/content-tree.md` 第四节同一取舍），覆盖常见表情就够
fn emoji_clusters(s: &str) -> Vec<(&str, bool)> {
    let pictographic = |c: char| {
        matches!(c as u32,
            0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2B00..=0x2BFF | 0x2190..=0x21FF
            | 0x2300..=0x23FF | 0x3030 | 0x303D | 0x3297 | 0x3299 | 0x00A9 | 0x00AE | 0x203C | 0x2049)
    };
    let flag = |c: char| matches!(c as u32, 0x1F1E6..=0x1F1FF);
    let extend =
        |c: char| matches!(c as u32, 0xFE0F | 0x20E3 | 0x1F3FB..=0x1F3FF | 0xE0020..=0xE007F);
    let mut out = Vec::new();
    let mut it = s.char_indices().peekable();
    while let Some((start, c)) = it.next() {
        if !pictographic(c) {
            let end = start + c.len_utf8();
            out.push((&s[start..end], false));
            continue;
        }
        let mut end = start + c.len_utf8();
        // 国旗是两个国旗字母成对
        if flag(c)
            && let Some(&(j, d)) = it.peek()
            && flag(d)
        {
            end = j + d.len_utf8();
            it.next();
        }
        while let Some(&(j, d)) = it.peek() {
            if extend(d) {
                end = j + d.len_utf8();
                it.next();
            } else if d == '\u{200D}' {
                end = j + d.len_utf8();
                it.next();
                if let Some(&(k, e)) = it.peek() {
                    end = k + e.len_utf8();
                    it.next();
                }
            } else {
                break;
            }
        }
        out.push((&s[start..end], true));
    }
    out
}

fn hm(t: Time, cx: &SpeechCtx) -> String {
    let offset = cx.offset_at.map_or(cx.utc_offset, |f| f(t.0));
    let (h, m) = t.local_hm(offset);
    format!("{h}:{m:02}")
}

/// 时长：「12 秒」「1 分 5 秒」「1 小时 2 分」
fn dur(secs: u32) -> String {
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    match (h, m) {
        (0, 0) => format!("{s} 秒"),
        (0, _) if s == 0 => format!("{m} 分"),
        (0, _) => format!("{m} 分 {s} 秒"),
        _ if m == 0 => format!("{h} 小时"),
        _ => format!("{h} 小时 {m} 分"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(dur(0), "0 秒");
        assert_eq!(dur(12), "12 秒");
        assert_eq!(dur(60), "1 分");
        assert_eq!(dur(65), "1 分 5 秒");
        assert_eq!(dur(3600), "1 小时");
        assert_eq!(dur(3725), "1 小时 2 分");
    }

    /// V12 人定：连续 3 个及以上相同的才合并；不连续、不一样、只有两个的照原样
    #[test]
    fn emoji_runs() {
        assert_eq!(merge_emoji("😂😂😂😂😂"), "5 个😂");
        // 紧挨文字时加逗号；已有停顿不再加
        assert_eq!(merge_emoji("好的👍👍👍"), "好的，3 个👍");
        assert_eq!(merge_emoji("😂😂😂好的"), "3 个😂，好的");
        assert_eq!(merge_emoji("好的 👍👍👍！"), "好的 3 个👍！");
        assert_eq!(merge_emoji("👍👍👍😂😂😂"), "3 个👍，3 个😂");
        assert_eq!(merge_emoji("😀😀好消息"), "😀😀好消息");
        assert_eq!(merge_emoji("好😂😂哈哈🤣😂"), "好😂😂哈哈🤣😂");
        // 肤色、变体选择符、零宽连接、国旗算一个表情
        assert_eq!(merge_emoji("👍🏽👍🏽👍🏽"), "3 个👍🏽");
        assert_eq!(merge_emoji("👍👍🏽👍"), "👍👍🏽👍");
        assert_eq!(merge_emoji("❤️❤️❤️"), "3 个❤️");
        assert_eq!(merge_emoji("👨‍👩‍👧👨‍👩‍👧👨‍👩‍👧"), "3 个👨‍👩‍👧");
        assert_eq!(merge_emoji("🇨🇳🇨🇳🇨🇳"), "3 个🇨🇳");
        // 普通文字三连不动
        assert_eq!(merge_emoji("哈哈哈"), "哈哈哈");
        assert_eq!(merge_emoji(""), "");
    }

    /// 跟系统时区读时，每个钟点按它自己的时间点取偏移（`offset_at`）：夏令时前后两条当地都是 19:36，
    /// 都读「19:36」；固定偏移（`offset_at` 为空）时一律按 `utc_offset` 算
    #[test]
    fn offset_at_is_per_timestamp() {
        // 假的「纽约」：2026-11-01T06:00Z 之前 -4 小时（夏令时），之后 -5 小时
        fn fake_ny(t: i64) -> i32 {
            if t < 1_793_512_800 {
                -4 * 3600
            } else {
                -5 * 3600
            }
        }
        let summer = Time(1_793_403_360); // 2026-10-30T23:36Z = 当地 19:36（-4）
        let winter = Time(1_793_579_760); // 2026-11-02T00:36Z = 当地 19:36（-5）
        let system = SpeechCtx {
            utc_offset: 0,
            offset_at: Some(fake_ny),
            listen_key: None,
            expand_key: None,
            reveal: false,
        };
        assert_eq!(hm(summer, &system), "19:36");
        assert_eq!(hm(winter, &system), "19:36");
        // 固定 -4 小时：冬天那条读成 20:36，`utc_offset` 不被 `offset_at` 以外的东西改
        let fixed = SpeechCtx {
            utc_offset: -4 * 3600,
            offset_at: None,
            ..system
        };
        assert_eq!(hm(summer, &fixed), "19:36");
        assert_eq!(hm(winter, &fixed), "20:36");
    }

    /// V22 人定：问号、叹号后面直接接，不再加逗号
    #[test]
    fn then_after_question_mark() {
        assert_eq!(then("几点出发？", "19:32"), "几点出发？19:32");
        assert_eq!(then("好！", "19:32"), "好！19:32");
        assert_eq!(then("why?", "19:32"), "why?19:32");
        assert_eq!(then("好的", "19:32"), "好的，19:32");
    }
}
