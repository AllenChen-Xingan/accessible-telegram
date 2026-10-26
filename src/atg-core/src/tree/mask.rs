//! 遮住隐藏内容（读法 R1）：剧透的原文不进输出，除非人展开了（CLI 的 `--reveal`）。
//!
//! 大总管授权 2026-10-26（R1：隐藏内容默认遮住，展开要人明确要求）：不展开时，JSON 行 `data` 里的剧透子树换成空的
//! `spoiler` 节点（只留「这里有一段隐藏内容」，不留字），落在剧透里、或文字里套着剧透的链接整个换成空剧透（地址也去掉）。
//! 管到所有带正文的地方：正文、投票题目、回复引用的那段、置顶系统消息带的内容、会话项的最后一条。
//! 18+ 遮挡的媒体本身不进树（只有说明文字，读法照读），不用另遮。
//!
//! 规矩同渲染器：每个枚举都用不带通配符的 `match`，加了新节点种类这里编不过。
#![deny(clippy::wildcard_enum_match_arm)]

use super::*;

/// 遮住后的一份（原树不动）
pub fn masked(doc: &Doc) -> Doc {
    let mut d = doc.clone();
    mask_doc(&mut d);
    d
}

fn mask_doc(d: &mut Doc) {
    match d {
        Doc::Message(m) => mask_message(m),
        Doc::Service(s) => mask_action(&mut s.action),
        Doc::ChatItem(c) => {
            if let Some(last) = c.last.as_deref_mut() {
                match last {
                    ChatLast::Message(m) => mask_message(m),
                    ChatLast::Service(s) => mask_action(&mut s.action),
                    ChatLast::Tombstone(_) => {}
                }
            }
        }
        Doc::Tombstone(_) | Doc::Alert(_) | Doc::Status(_) | Doc::Unknown(_) => {}
    }
}

fn mask_message(m: &mut Message) {
    mask_content(&mut m.content);
    match &mut m.head.reply_to {
        Some(ReplyTo::Message { quote, .. }) => {
            if let Some(q) = quote {
                mask_spans(q);
            }
        }
        Some(ReplyTo::Unknown { .. }) | None => {}
    }
}

fn mask_action(a: &mut Action) {
    match a {
        Action::Pinned {
            content: Some(c), ..
        } => mask_content(c),
        Action::Pinned { content: None, .. }
        | Action::MembersAdded { .. }
        | Action::MemberRemoved { .. }
        | Action::JoinedByLink
        | Action::JoinedByRequest
        | Action::TitleChanged { .. }
        | Action::PhotoChanged
        | Action::PhotoDeleted
        | Action::GroupCreated { .. }
        | Action::ContactRegistered
        | Action::Other { .. } => {}
    }
}

fn mask_content(c: &mut Content) {
    mask_spans(&mut c.body);
    let Some(a) = &mut c.attachment else {
        return;
    };
    // 带正文（行内树）的附件逐个列出：以后加了带正文的种类，这里编不过，逼着决定怎么遮
    match a {
        Attachment::Poll { question } => mask_spans(question),
        Attachment::Photo { .. }
        | Attachment::Video { .. }
        | Attachment::Voice { .. }
        | Attachment::VideoNote { .. }
        | Attachment::Audio { .. }
        | Attachment::Animation { .. }
        | Attachment::Document { .. }
        | Attachment::Sticker { .. }
        | Attachment::AnimatedEmoji { .. }
        | Attachment::Location
        | Attachment::Venue { .. }
        | Attachment::Contact { .. }
        | Attachment::Call { .. }
        | Attachment::Expired { .. }
        | Attachment::Restricted { .. }
        | Attachment::Other { .. }
        | Attachment::Unknown { .. } => {}
    }
}

/// 空的剧透节点：只说明这里有隐藏内容
fn hidden() -> Span {
    Span::Spoiler { children: vec![] }
}

pub(crate) fn mask_spans(list: &mut [Span]) {
    for s in list.iter_mut() {
        mask_span(s);
    }
}

fn mask_span(s: &mut Span) {
    // 链接文字里套着剧透：地址也是隐藏的一部分，整个换掉
    if matches!(s, Span::Link { .. } | Span::Url { .. }) && text::contains_spoiler(s) {
        *s = hidden();
        return;
    }
    match s {
        Span::Spoiler { .. } => *s = hidden(),
        Span::Text { .. } | Span::Code { .. } | Span::Pre { .. } | Span::DateTime { .. } => {}
        Span::Bold { children }
        | Span::Italic { children }
        | Span::Underline { children }
        | Span::Strikethrough { children }
        | Span::Quote { children, .. }
        | Span::Url { children }
        | Span::Link { children, .. }
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
        | Span::Unknown { children, .. } => mask_spans(children),
    }
}
