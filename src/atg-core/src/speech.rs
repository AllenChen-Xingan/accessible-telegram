//! 读法：内容树 → 读屏要读的那句话。纯函数，可快照测试。
//!
//! v0 只做 `brief`（列表里每项的名称，先读最要紧的）。`detail` 等读法规约定了再做。
//! 这里的每个说法都是 **AI 起草、待人定标** 的（`specs/content-tree.md`「待人拍板」）。
//!
//! 渲染器规矩：对树的每个枚举都用**不带通配符**的 `match`，加了新节点种类，这里编不过，
//! 逼着人决定怎么读。下面这条 lint 把 `_ =>` 也拦住。
#![deny(clippy::wildcard_enum_match_arm)]

use crate::tree::*;

/// 读法需要、但不在树里的东西
#[derive(Debug, Clone)]
pub struct SpeechCtx {
    /// 本地时区偏移（秒），由调用方给，读法不碰系统时钟
    pub utc_offset: i32,
    /// 新消息提醒里「按 M 收听」那个键；`None` 就不读（CLI 没有这个键）
    pub listen_key: Option<char>,
}

/// 会话列表里「最后一条」摘要最多读几个字（超出用「…」）。待人定
pub const LAST_MAX_CHARS: usize = 40;

pub fn brief(doc: &Doc, cx: &SpeechCtx) -> String {
    match doc {
        Doc::Message(m) => format!(
            "{}：{}，{}",
            who(&m.head.sender, m.head.outgoing),
            content(&m.content),
            hm(m.head.date, cx)
        ),
        Doc::Service(sv) => format!("{}，{}", service(sv), hm(sv.date, cx)),
        Doc::Tombstone(_) => "已撤回的消息".to_owned(),
        Doc::ChatItem(c) => chat_brief(c),
        Doc::Alert(a) => alert(a, cx),
        Doc::Status(s) => status(s.status).to_owned(),
        Doc::Unknown(_) => UNSUPPORTED.to_owned(),
    }
}

/// 会话列表一项：名字，未读，最后一条摘要（不读时间，约 3 秒内读完，T02）
pub fn chat_brief(c: &ChatItem) -> String {
    let mut parts = vec![if c.title.is_empty() {
        "未命名会话".to_owned()
    } else {
        c.title.clone()
    }];
    if c.unread_count > 0 {
        parts.push(format!("{} 条未读", c.unread_count));
    }
    if c.unread_mention_count > 0 {
        parts.push(format!("{} 条提到你", c.unread_mention_count));
    }
    if let Some(last) = &c.last {
        let private = matches!(c.kind, ChatKind::Private | ChatKind::Secret);
        let summary = match last.as_ref() {
            // 私聊里对方发的、频道以自己名义发的，都不重复读名字
            ChatLast::Message(m) if private && !m.head.outgoing => content(&m.content),
            ChatLast::Message(m) if matches!(m.head.sender, Sender::Chat { chat_id, .. } if chat_id == c.chat_id) => {
                content(&m.content)
            }
            ChatLast::Message(m) => format!(
                "{}：{}",
                who(&m.head.sender, m.head.outgoing),
                content(&m.content)
            ),
            ChatLast::Service(sv) => service(sv),
            ChatLast::Tombstone(_) => "已撤回的消息".to_owned(),
        };
        parts.push(truncate(&summary, LAST_MAX_CHARS));
    }
    parts.join("，")
}

fn alert(a: &Alert, cx: &SpeechCtx) -> String {
    let mut s = format!("{} 条新消息", a.total);
    if let [only] = a.chats.as_slice() {
        s.push_str(&format!(
            "，来自{}",
            only.title.as_deref().unwrap_or(UNKNOWN_NAME)
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
        // 说法待 #19 定（规约：这一态不播报；名称仍要有一句）
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

fn who(s: &Sender, outgoing: bool) -> String {
    if outgoing {
        return "我".to_owned();
    }
    sender_name(s)
}

fn sender_name(s: &Sender) -> String {
    match s {
        Sender::User { name, .. } => name_or(name, UNKNOWN_NAME),
        Sender::Chat { title, .. } => name_or(title, UNKNOWN_NAME),
        Sender::Unknown { .. } => UNKNOWN_NAME.to_owned(),
    }
}

fn name_or(n: &Option<String>, fallback: &str) -> String {
    match n.as_deref() {
        Some(x) if !x.trim().is_empty() => x.to_owned(),
        _ => fallback.to_owned(),
    }
}

fn user_name(u: &UserRef) -> String {
    name_or(&u.name, UNKNOWN_NAME)
}

/// 名字查不到时的说法：用户、会话、不认识的发送人一律这一种
const UNKNOWN_NAME: &str = "未知发送人";
/// 不认识、或认识但还没做读法的内容：一律这一种，类型名只进 JSON
const UNSUPPORTED: &str = "暂不支持的消息";
/// 剧透、18+ 遮挡统一叫「隐藏」
const HIDDEN: &str = "隐藏内容";

fn service(sv: &Service) -> String {
    let actor = sender_name(&sv.actor);
    match &sv.action {
        Action::MembersAdded { users } => {
            let names: Vec<String> = users.iter().map(user_name).collect();
            let joined_self = matches!(
                (&sv.actor, users.as_slice()),
                (Sender::User { user_id, .. }, [only]) if only.user_id == *user_id
            );
            if joined_self {
                format!("{actor}加入了群组")
            } else {
                format!("{actor}添加了{}", names.join("、"))
            }
        }
        Action::JoinedByLink => format!("{actor}通过邀请链接加入了群组"),
        Action::JoinedByRequest => format!("{actor}的入群申请已通过"),
        Action::MemberRemoved { user } => {
            let left_self =
                matches!(&sv.actor, Sender::User { user_id, .. } if *user_id == user.user_id);
            if left_self {
                format!("{actor}退出了群组")
            } else {
                format!("{actor}移出了{}", user_name(user))
            }
        }
        Action::TitleChanged { title } => format!("{actor}把群名改为「{title}」"),
        Action::PhotoChanged => format!("{actor}更换了群头像"),
        Action::PhotoDeleted => format!("{actor}删除了群头像"),
        Action::Pinned { .. } => format!("{actor}置顶了一条消息"),
        Action::GroupCreated { title } => format!("{actor}创建了群组「{title}」"),
        Action::ContactRegistered => format!("{actor}加入了 Telegram"),
        Action::Other { .. } => "一条系统消息".to_owned(),
    }
}

/// 正文 + 附件的简短说法
fn content(c: &Content) -> String {
    let body = spans(&c.body);
    // 句号、逗号后面还要接「，时间」，去掉免得读成「。，」；问号叹号带语气、英文句点可能是省略号，留着
    let body = body.trim().trim_end_matches(['。', '，', ',']).trim_end();
    let label = c.attachment.as_ref().map(|a| {
        let l = attachment(a);
        if c.sensitive {
            format!("{l}，{HIDDEN}")
        } else {
            l
        }
    });
    match (label, body.is_empty()) {
        (None, true) => "空消息".to_owned(),
        (None, false) => body.to_owned(),
        (Some(l), true) => l,
        (Some(l), false) => format!("{l}，{body}"),
    }
}

fn attachment(a: &Attachment) -> String {
    match a {
        Attachment::Photo { spoiler } => hidden("图片".to_owned(), *spoiler),
        Attachment::Video { duration, spoiler } => {
            hidden(format!("视频 {}", dur(*duration)), *spoiler)
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
        Attachment::Poll { question } => labeled("投票", &spans(question)),
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
    }
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

fn hidden(label: String, yes: bool) -> String {
    if yes {
        format!("{label}，{HIDDEN}")
    } else {
        label
    }
}

/// 行内树的简短说法：格式不读；剧透只读「剧透」；链接只读文字不读地址
fn spans(list: &[Span]) -> String {
    let mut out = String::new();
    for s in list {
        span(&mut out, s);
    }
    // 「隐藏内容」两边加的逗号可能和原文的空格、逗号、开头撞上
    let mut out = out.replace(" ，", "，");
    while out.contains("，，") {
        out = out.replace("，，", "，");
    }
    out.trim_start_matches('，').to_owned()
}

fn span(out: &mut String, s: &Span) {
    match s {
        Span::Text { text }
        | Span::Code { text }
        | Span::Pre { text, .. }
        | Span::DateTime { text, .. } => out.push_str(text),
        // VoiceOver 不念括号，用逗号隔开才听得出这是标注
        Span::Spoiler { .. } => {
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
                span(out, c);
            }
        }
    }
}

fn hm(t: Time, cx: &SpeechCtx) -> String {
    let (h, m) = t.local_hm(cx.utc_offset);
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

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
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

    #[test]
    fn truncate_counts_chars_not_bytes() {
        assert_eq!(truncate("一二三", 3), "一二三");
        assert_eq!(truncate("一二三四", 3), "一二三…");
    }
}
