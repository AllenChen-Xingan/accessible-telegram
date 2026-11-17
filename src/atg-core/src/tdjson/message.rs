//! 消息的映射：TDLib `message` 对象 → [`Message`]，连同正文（系统消息 / 普通内容）、发送者、转发来源、带格式文字。
//! 从 `tdjson` 拆出（#60 R7：单文件 600 行上限，见 `docs/plan/010_complexity-budget.md`；只挪不改）；取字段的小工具、导入都经 `use super::*` 从根上拿

use super::*;

/// 认识、但 v0 没做专门节点的系统消息。读法读出类型。清单随 `docs/research/002_features/content-types.md` 增补
const OTHER_SERVICE: &[&str] = &[
    "messageScreenshotTaken",
    "messageChatSetMessageAutoDeleteTime",
    "messageChatUpgradeTo",
    "messageChatUpgradeFrom",
    "messageForumTopicCreated",
    "messageForumTopicEdited",
    "messageForumTopicIsClosedToggled",
    "messageForumTopicIsHiddenToggled",
    "messageVideoChatScheduled",
    "messageVideoChatStarted",
    "messageVideoChatEnded",
    "messageInviteVideoChatParticipants",
    "messageGameScore",
    "messageCustomServiceAction",
    "messageChatSetBackground",
    "messageChatSetTheme",
    "messageChatBoost",
    "messageProximityAlertTriggered",
];

/// 认识、但 v0 没做专门节点的普通内容
const OTHER_CONTENT: &[&str] = &[
    "messageLiveLocation",
    "messageDice",
    "messageStory",
    "messagePaidMedia",
    "messageGame",
    "messageInvoice",
    "messageChecklist",
    "messageGroupCall",
    "messageRichMessage",
];

pub fn message(m: &Value) -> Message {
    let id = msg_id(m);
    let reply_to = match ty(&m["reply_to"]) {
        "" => None,
        "messageReplyToMessage" => {
            let r = &m["reply_to"];
            let quote = (!r["quote"].is_null())
                .then(|| formatted(&r["quote"]["text"]))
                .filter(|q| !q.is_empty());
            Some(ReplyRef::Message {
                target: MsgId {
                    chat_id: i(r, "chat_id"),
                    message_id: i(r, "message_id"),
                },
                quote,
                origin: r["origin"].is_object().then(|| origin(&r["origin"])),
            })
        }
        other => Some(ReplyRef::Unknown {
            td_type: other.to_owned(),
        }),
    };
    let forward = (!m["forward_info"].is_null()).then(|| ForwardRef {
        origin: origin(&m["forward_info"]["origin"]),
        date: Time(i(&m["forward_info"], "date")),
    });
    let album_id = Some(str_or_num(&m["media_album_id"])).filter(|a| !a.is_empty() && a != "0");

    let topic = topic(&m["topic_id"]);

    let edit = i(m, "edit_date");
    let reason = s(&m["restriction_info"], "restriction_reason");
    let mut msg = Message {
        id,
        sender: sender(&m["sender_id"]),
        date: Time(i(m, "date")),
        outgoing: b(m, "is_outgoing"),
        reply_to,
        forward,
        album_id,
        topic,
        content: body(&m["content"], id.chat_id),
        ephemeral: ephemeral(&m["ephemeral_content"], id.chat_id),
        restriction: (!reason.is_empty()).then(|| reason.to_owned()),
        sensitive: b(&m["restriction_info"], "has_sensitive_content"),
        edited: (edit > 0).then_some(Time(edit)),
        reactions: reactions(&m["interaction_info"]),
        sending: match ty(&m["sending_state"]) {
            "messageSendingStatePending" => SendingState::Pending,
            "messageSendingStateFailed" => SendingState::Failed,
            _ => SendingState::Sent,
        },
        mentions_me: b(m, "contains_unread_mention"),
        self_destruct: !m["self_destruct_type"].is_null(),
        buttons: super::interactive::keyboard(m),
    };
    msg.enforce();
    msg
}

pub(super) fn ephemeral(e: &Value, chat_id: i64) -> Option<Body> {
    e.is_object().then(|| body(&e["content"], chat_id))
}

pub(super) fn sender(v: &Value) -> SenderId {
    match ty(v) {
        "messageSenderUser" => SenderId::User(i(v, "user_id")),
        "messageSenderChat" => SenderId::Chat(i(v, "chat_id")),
        other => SenderId::Unknown {
            td_type: other.to_owned(),
        },
    }
}

fn origin(v: &Value) -> OriginId {
    match ty(v) {
        "messageOriginUser" => OriginId::User(i(v, "sender_user_id")),
        "messageOriginHiddenUser" => OriginId::HiddenUser {
            name: s(v, "sender_name").to_owned(),
        },
        "messageOriginChat" => OriginId::Chat(i(v, "sender_chat_id")),
        "messageOriginChannel" => OriginId::Channel(i(v, "chat_id")),
        other => OriginId::Unknown {
            td_type: other.to_owned(),
        },
    }
}

/// `MessageContent` → 系统消息或普通内容。系统消息里提到的人只存 id，名字投影时填
pub(super) fn body(c: &Value, chat_id: i64) -> Body {
    let user = |uid: i64| UserRef {
        user_id: uid,
        name: None,
        is_me: false,
        deleted: false,
    };
    Body::Service(match ty(c) {
        "messageChatAddMembers" => Action::MembersAdded {
            users: arr(c, "member_user_ids")
                .iter()
                .map(|x| user(as_i64(x)))
                .collect(),
        },
        "messageChatJoinByLink" => Action::JoinedByLink,
        "messageChatJoinByRequest" => Action::JoinedByRequest,
        "messageChatDeleteMember" => Action::MemberRemoved {
            user: user(i(c, "user_id")),
        },
        "messageChatChangeTitle" => Action::TitleChanged {
            title: s(c, "title").to_owned(),
        },
        "messageChatChangePhoto" => Action::PhotoChanged,
        "messageChatDeletePhoto" => Action::PhotoDeleted,
        // 被置顶那条在同一会话
        "messagePinMessage" => Action::Pinned {
            pinned: MsgId {
                chat_id,
                message_id: i(c, "message_id"),
            },
            content: None,
        },
        "messageBasicGroupChatCreate" | "messageSupergroupChatCreate" => Action::GroupCreated {
            title: s(c, "title").to_owned(),
        },
        "messageContactRegistered" => Action::ContactRegistered,
        t if OTHER_SERVICE.contains(&t) => Action::Other {
            td_type: t.to_owned(),
        },
        _ => return Body::Content(content(c)),
    })
}

fn content(c: &Value) -> Content {
    let caption = || formatted(&c["caption"]);
    let dur = |obj: &str| u(&c[obj], "duration");
    let spoiler = b(c, "has_spoiler");
    let (body, attachment) = match ty(c) {
        "messageText" => (formatted(&c["text"]), None),
        "messagePhoto" => (caption(), Some(Attachment::Photo { spoiler })),
        "messageVideo" => (
            caption(),
            Some(Attachment::Video {
                duration: dur("video"),
                spoiler,
            }),
        ),
        "messageVoiceNote" => (caption(), Some(file::voice(c))),
        "messageVideoNote" => (
            vec![],
            Some(Attachment::VideoNote {
                duration: dur("video_note"),
                viewed: b(c, "is_viewed"),
            }),
        ),
        "messageAudio" => (
            caption(),
            Some(Attachment::Audio {
                duration: dur("audio"),
                title: s(&c["audio"], "title").to_owned(),
                performer: s(&c["audio"], "performer").to_owned(),
            }),
        ),
        "messageAnimation" => (
            caption(),
            Some(Attachment::Animation {
                duration: dur("animation"),
            }),
        ),
        "messageDocument" => (
            caption(),
            Some(Attachment::Document {
                file_name: s(&c["document"], "file_name").to_owned(),
                size: file_size(&c["document"]["document"]),
            }),
        ),
        "messageSticker" => (
            vec![],
            Some(Attachment::Sticker {
                emoji: s(&c["sticker"], "emoji").to_owned(),
            }),
        ),
        "messageAnimatedEmoji" => (
            vec![],
            Some(Attachment::AnimatedEmoji {
                emoji: s(c, "emoji").to_owned(),
            }),
        ),
        "messageLocation" => (vec![], Some(Attachment::Location)),
        "messageVenue" => (
            vec![],
            Some(Attachment::Venue {
                title: s(&c["venue"], "title").to_owned(),
            }),
        ),
        "messageContact" => (
            vec![],
            Some(Attachment::Contact {
                name: join_name(
                    s(&c["contact"], "first_name"),
                    s(&c["contact"], "last_name"),
                ),
            }),
        ),
        "messagePoll" => (vec![], Some(super::interactive::poll(&c["poll"]))),
        "messageCall" => (
            vec![],
            Some(Attachment::Call {
                video: b(c, "is_video"),
                duration: u(c, "duration"),
                missed: ty(&c["discard_reason"]) == "callDiscardReasonMissed",
            }),
        ),
        "messageExpiredPhoto" => expired(Expirable::Photo),
        "messageExpiredVideo" => expired(Expirable::Video),
        "messageExpiredVideoNote" => expired(Expirable::VideoNote),
        "messageExpiredVoiceNote" => expired(Expirable::Voice),
        t if OTHER_CONTENT.contains(&t) => (
            vec![],
            Some(Attachment::Other {
                td_type: t.to_owned(),
            }),
        ),
        t => (
            vec![],
            Some(Attachment::Unknown {
                td_type: if t.is_empty() { "?" } else { t }.to_owned(),
            }),
        ),
    };
    Content {
        body,
        attachment,
        sensitive: false,
    }
}

fn expired(what: Expirable) -> (Vec<Span>, Option<Attachment>) {
    (vec![], Some(Attachment::Expired { what }))
}

/// `formattedText` → 行内树（修补记录在原型里丢弃；正式路径交给日志，只记种类）
pub fn formatted(v: &Value) -> Vec<Span> {
    let entities: Vec<Entity> = arr(v, "entities")
        .iter()
        .map(|e| Entity {
            offset: i(e, "offset"),
            length: i(e, "length"),
            kind: entity_kind(&e["type"]),
        })
        .collect();
    text::build(s(v, "text"), &entities).spans
}

fn entity_kind(t: &Value) -> EntityKind {
    use EntityKind as K;
    match ty(t) {
        "textEntityTypeBold" => K::Bold,
        "textEntityTypeItalic" => K::Italic,
        "textEntityTypeUnderline" => K::Underline,
        "textEntityTypeStrikethrough" => K::Strikethrough,
        "textEntityTypeSpoiler" => K::Spoiler,
        "textEntityTypeCode" => K::Code,
        "textEntityTypePre" => K::Pre { language: None },
        "textEntityTypePreCode" => K::Pre {
            language: Some(s(t, "language").to_owned()),
        },
        "textEntityTypeDateTime" => K::DateTime {
            unix_time: i(t, "unix_time"),
        },
        "textEntityTypeBlockQuote" => K::Quote { expandable: false },
        "textEntityTypeExpandableBlockQuote" => K::Quote { expandable: true },
        "textEntityTypeTextUrl" => K::Link {
            url: s(t, "url").to_owned(),
        },
        "textEntityTypeUrl" => K::Url,
        "textEntityTypeMention" => K::Mention,
        "textEntityTypeMentionName" => K::MentionName {
            user_id: i(t, "user_id"),
        },
        "textEntityTypeHashtag" => K::Hashtag,
        "textEntityTypeCashtag" => K::Cashtag,
        "textEntityTypeBotCommand" => K::BotCommand,
        "textEntityTypeEmailAddress" => K::Email,
        "textEntityTypePhoneNumber" => K::Phone,
        "textEntityTypeBankCardNumber" => K::BankCard,
        "textEntityTypeTonAddress" => K::TonAddress,
        "textEntityTypeCustomEmoji" => K::CustomEmoji {
            custom_emoji_id: str_or_num(&t["custom_emoji_id"]),
        },
        "textEntityTypeMediaTimestamp" => K::MediaTimestamp {
            seconds: u(t, "media_timestamp"),
        },
        other => K::Unknown {
            td_type: other.to_owned(),
        },
    }
}
