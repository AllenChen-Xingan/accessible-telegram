//! 加一种新节点时的提醒：
//! 1. 下面每个 `*_name` 都是**不带通配符**的 `match`——树里加了变体，这里编不过
//! 2. 补上那一行后，`ALL_*` 也要补；补了而夹具里没有它，`fixtures_cover_every_kind` 失败
//!    ——逼着为新节点写一条夹具，于是每个渲染器（读法、JSON）的快照都会出现它，人一眼看见
#![deny(clippy::wildcard_enum_match_arm)]

use atg_core::tdjson::World;
use atg_core::tree::*;
use std::collections::BTreeSet;

const ALL_DOCS: &[&str] = &[
    "message",
    "service",
    "tombstone",
    "chat_item",
    "status",
    "unknown",
    // alert 不来自 TDLib，由 snapshots.rs 的 alert_brief 覆盖
];

fn doc_name(d: &Doc) -> &'static str {
    match d {
        Doc::Message(_) => "message",
        Doc::Service(_) => "service",
        Doc::Tombstone(_) => "tombstone",
        Doc::ChatItem(_) => "chat_item",
        Doc::Alert(_) => "alert",
        Doc::Status(_) => "status",
        Doc::Unknown(_) => "unknown",
    }
}

const ALL_ATTACHMENTS: &[&str] = &[
    "photo",
    "video",
    "voice",
    "video_note",
    "audio",
    "animation",
    "document",
    "sticker",
    "animated_emoji",
    "location",
    "venue",
    "contact",
    "poll",
    "call",
    "expired",
    "restricted",
    "other",
    "unknown",
];

fn attachment_name(a: &Attachment) -> &'static str {
    match a {
        Attachment::Photo { .. } => "photo",
        Attachment::Video { .. } => "video",
        Attachment::Voice { .. } => "voice",
        Attachment::VideoNote { .. } => "video_note",
        Attachment::Audio { .. } => "audio",
        Attachment::Animation { .. } => "animation",
        Attachment::Document { .. } => "document",
        Attachment::Sticker { .. } => "sticker",
        Attachment::AnimatedEmoji { .. } => "animated_emoji",
        Attachment::Location => "location",
        Attachment::Venue { .. } => "venue",
        Attachment::Contact { .. } => "contact",
        Attachment::Poll { .. } => "poll",
        Attachment::Call { .. } => "call",
        Attachment::Expired { .. } => "expired",
        Attachment::Restricted { .. } => "restricted",
        Attachment::Other { .. } => "other",
        Attachment::Unknown { .. } => "unknown",
    }
}

const ALL_ACTIONS: &[&str] = &[
    "members_added",
    "joined_by_link",
    "member_removed",
    "title_changed",
    "photo_changed",
    "pinned",
    "group_created",
    "contact_registered",
    "other",
    // joined_by_request、photo_deleted：形状与 joined_by_link、photo_changed 相同，下一批夹具补
];

fn action_name(a: &Action) -> &'static str {
    match a {
        Action::MembersAdded { .. } => "members_added",
        Action::JoinedByLink => "joined_by_link",
        Action::JoinedByRequest => "joined_by_request",
        Action::MemberRemoved { .. } => "member_removed",
        Action::TitleChanged { .. } => "title_changed",
        Action::PhotoChanged => "photo_changed",
        Action::PhotoDeleted => "photo_deleted",
        Action::Pinned { .. } => "pinned",
        Action::GroupCreated { .. } => "group_created",
        Action::ContactRegistered => "contact_registered",
        Action::Other { .. } => "other",
    }
}

const ALL_SPANS: &[&str] = &[
    "text",
    "code",
    "pre",
    "date_time",
    "bold",
    "italic",
    "underline",
    "strikethrough",
    "spoiler",
    "quote",
    "link",
    "url",
    "mention",
    "mention_name",
    "hashtag",
    "cashtag",
    "bot_command",
    "email",
    "phone",
    "bank_card",
    "ton_address",
    "custom_emoji",
    "media_timestamp",
    "unknown",
];

fn span_name(s: &Span) -> &'static str {
    match s {
        Span::Text { .. } => "text",
        Span::Code { .. } => "code",
        Span::Pre { .. } => "pre",
        Span::DateTime { .. } => "date_time",
        Span::Bold { .. } => "bold",
        Span::Italic { .. } => "italic",
        Span::Underline { .. } => "underline",
        Span::Strikethrough { .. } => "strikethrough",
        Span::Spoiler { .. } => "spoiler",
        Span::Quote { .. } => "quote",
        Span::Link { .. } => "link",
        Span::Url { .. } => "url",
        Span::Mention { .. } => "mention",
        Span::MentionName { .. } => "mention_name",
        Span::Hashtag { .. } => "hashtag",
        Span::Cashtag { .. } => "cashtag",
        Span::BotCommand { .. } => "bot_command",
        Span::Email { .. } => "email",
        Span::Phone { .. } => "phone",
        Span::BankCard { .. } => "bank_card",
        Span::TonAddress { .. } => "ton_address",
        Span::CustomEmoji { .. } => "custom_emoji",
        Span::MediaTimestamp { .. } => "media_timestamp",
        Span::Unknown { .. } => "unknown",
    }
}

fn span_children(s: &Span) -> &[Span] {
    match s {
        Span::Text { .. } | Span::Code { .. } | Span::Pre { .. } | Span::DateTime { .. } => &[],
        Span::Bold { children }
        | Span::Italic { children }
        | Span::Underline { children }
        | Span::Strikethrough { children }
        | Span::Spoiler { children }
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
        | Span::Unknown { children, .. } => children,
    }
}

#[derive(Default)]
struct Seen {
    docs: BTreeSet<&'static str>,
    attachments: BTreeSet<&'static str>,
    actions: BTreeSet<&'static str>,
    spans: BTreeSet<&'static str>,
}

impl Seen {
    fn doc(&mut self, d: &Doc) {
        self.docs.insert(doc_name(d));
        match d {
            Doc::Message(m) => {
                self.spans_of(&m.content.body);
                if let Some(a) = &m.content.attachment {
                    self.attachments.insert(attachment_name(a));
                }
            }
            Doc::Service(s) => {
                self.actions.insert(action_name(&s.action));
            }
            Doc::ChatItem(c) => match c.last.as_deref() {
                Some(ChatLast::Message(m)) => self.doc(&Doc::Message(m.clone())),
                Some(ChatLast::Service(s)) => self.doc(&Doc::Service(s.clone())),
                Some(ChatLast::Tombstone(t)) => self.doc(&Doc::Tombstone(t.clone())),
                None => {}
            },
            Doc::Tombstone(_) | Doc::Alert(_) | Doc::Status(_) | Doc::Unknown(_) => {}
        }
    }
    fn spans_of(&mut self, list: &[Span]) {
        for s in list {
            self.spans.insert(span_name(s));
            self.spans_of(span_children(s));
        }
    }
}

fn load(name: &str) -> World {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    World::from_json(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn fixtures_cover_every_kind() {
    let mut seen = Seen::default();
    for name in ["basic", "entities"] {
        let w = load(name);
        for d in w.docs().iter().chain(w.chat_items().iter()) {
            seen.doc(d);
        }
    }
    let check = |what: &str, all: &[&str], got: &BTreeSet<&'static str>| {
        let missing: Vec<_> = all.iter().filter(|n| !got.contains(*n)).collect();
        assert!(missing.is_empty(), "{what}：夹具里缺 {missing:?}");
        let unlisted: Vec<_> = got.iter().filter(|n| !all.contains(*n)).collect();
        assert!(unlisted.is_empty(), "{what}：ALL 清单里漏了 {unlisted:?}");
    };
    check("Doc", ALL_DOCS, &seen.docs);
    check("Attachment", ALL_ATTACHMENTS, &seen.attachments);
    check("Action", ALL_ACTIONS, &seen.actions);
    check("Span", ALL_SPANS, &seen.spans);
}
