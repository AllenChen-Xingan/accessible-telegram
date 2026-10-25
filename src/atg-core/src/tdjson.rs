//! 原型的输入：TDLib JSON（夹具用）→ 内容树。
//!
//! **这是临时入口。** 第一刀第 2、3 块（model + reducer、接 TDLib）做好后，
//! 正式路径是「tdlib-rs 类型 → model → 投影成树」，这个模块会退成测试用的夹具读取器。
//! 与输入无关、会留下来的是 [`crate::tree::text::build`]（实体建树）和节点类型本身。
//!
//! 夹具 = 一个 JSON 数组，元素是 TDLib 的更新对象，按顺序喂进来（`updateUser`、`updateNewChat`、
//! `updateNewMessage`……）。**建树在 [`World::docs`] 被调用时才做**——模拟「渲染时从 store 现算」，
//! 所以用户信息比消息晚到也能查到名字。
//!
//! 一律用 `serde_json::Value` 逐字段取，字段缺了取默认值，不认识的类型落「未知」，绝不 panic。

use crate::tree::text::{self, Entity, EntityKind};
use crate::tree::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

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

#[derive(Default)]
struct ChatRec {
    raw: Value,
    last_read_outbox: i64,
}

/// 一份夹具喂完之后的「世界」：用户、会话、按到达顺序排的事件。
#[derive(Default)]
pub struct World {
    users: HashMap<i64, String>,
    chat_order: Vec<i64>,
    chats: HashMap<i64, ChatRec>,
    /// 会产出 Doc 的事件（消息、连接状态、不认识的）
    events: Vec<Value>,
    deleted: HashSet<MsgId>,
}

impl World {
    pub fn from_json(src: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(src).map_err(|e| format!("夹具不是合法 JSON：{e}"))?;
        let Value::Array(items) = v else {
            return Err("夹具顶层必须是数组".into());
        };
        let mut w = World::default();
        for it in items {
            w.apply(it);
        }
        Ok(w)
    }

    pub fn apply(&mut self, v: Value) {
        match ty(&v) {
            "updateUser" => {
                let u = &v["user"];
                let name = join_name(s(u, "first_name"), s(u, "last_name"));
                self.users.insert(i(u, "id"), name);
            }
            "updateNewChat" => {
                let c = v["chat"].clone();
                let id = i(&c, "id");
                if !self.chats.contains_key(&id) {
                    self.chat_order.push(id);
                }
                let last_read_outbox = i(&c, "last_read_outbox_message_id");
                self.chats.insert(
                    id,
                    ChatRec {
                        raw: c,
                        last_read_outbox,
                    },
                );
            }
            "updateChatReadOutbox" => {
                if let Some(c) = self.chats.get_mut(&i(&v, "chat_id")) {
                    c.last_read_outbox = i(&v, "last_read_outbox_message_id");
                }
            }
            // 只改状态、不出 Doc 的更新（坏输入一律跳过，不 panic）
            "updateChatLastMessage" => self.patch_chat(&v, "last_message"),
            "updateChatReadInbox" => self.patch_chat(&v, "unread_count"),
            "updateChatUnreadMentionCount" => self.patch_chat(&v, "unread_mention_count"),
            "updateChatNotificationSettings" => self.patch_chat(&v, "notification_settings"),
            "updateChatTitle" => self.patch_chat(&v, "title"),
            "updateChatPosition" => {
                if let Some(c) = self.chats.get_mut(&i(&v, "chat_id"))
                    && c.raw.is_object()
                {
                    c.raw["positions"] = Value::Array(vec![v["position"].clone()]);
                }
            }
            "updateDeleteMessages" => {
                if v["is_permanent"].as_bool().unwrap_or(false) {
                    let chat_id = i(&v, "chat_id");
                    for m in arr(&v, "message_ids") {
                        self.deleted.insert(MsgId {
                            chat_id,
                            message_id: as_i64(m),
                        });
                    }
                }
            }
            _ => self.events.push(v),
        }
    }

    /// 把更新里的同名字段抄进会话
    fn patch_chat(&mut self, v: &Value, field: &str) {
        if let Some(c) = self.chats.get_mut(&i(v, "chat_id"))
            && c.raw.is_object()
        {
            c.raw[field] = v[field].clone();
        }
    }

    /// 按到达顺序投影出所有 Doc（撤回过的消息原位变成占位）
    pub fn docs(&self) -> Vec<Doc> {
        self.events.iter().map(|v| self.event_doc(v)).collect()
    }

    /// 会话列表（按 `updateNewChat` 的到达顺序；真正的排序归 store）
    pub fn chat_items(&self) -> Vec<Doc> {
        self.chat_order
            .iter()
            .filter_map(|id| self.chats.get(id))
            .map(|c| Doc::ChatItem(self.chat_item(&c.raw)))
            .collect()
    }

    fn event_doc(&self, v: &Value) -> Doc {
        match ty(v) {
            "updateNewMessage" => {
                let m = &v["message"];
                let id = msg_id(m);
                if self.deleted.contains(&id) {
                    Doc::Tombstone(Tombstone { id })
                } else {
                    self.message_doc(m)
                }
            }
            "updateConnectionState" => Doc::Status(Status {
                status: net_status(&v["state"]),
            }),
            other => Doc::Unknown(Unknown {
                td_type: other.to_owned(),
            }),
        }
    }

    fn user_name(&self, id: i64) -> Option<String> {
        self.users.get(&id).cloned()
    }

    fn chat_title(&self, id: i64) -> Option<String> {
        self.chats.get(&id).map(|c| s(&c.raw, "title").to_owned())
    }

    fn sender(&self, v: &Value) -> Sender {
        match ty(v) {
            "messageSenderUser" => {
                let user_id = i(v, "user_id");
                Sender::User {
                    user_id,
                    name: self.user_name(user_id),
                }
            }
            "messageSenderChat" => {
                let chat_id = i(v, "chat_id");
                Sender::Chat {
                    chat_id,
                    title: self.chat_title(chat_id),
                }
            }
            other => Sender::Unknown {
                td_type: other.to_owned(),
            },
        }
    }

    fn origin(&self, v: &Value) -> Origin {
        match ty(v) {
            "messageOriginUser" => {
                let user_id = i(v, "sender_user_id");
                Origin::User {
                    user_id,
                    name: self.user_name(user_id),
                }
            }
            "messageOriginHiddenUser" => Origin::HiddenUser {
                name: s(v, "sender_name").to_owned(),
            },
            "messageOriginChat" => {
                let chat_id = i(v, "sender_chat_id");
                Origin::Chat {
                    chat_id,
                    title: self.chat_title(chat_id),
                }
            }
            "messageOriginChannel" => {
                let chat_id = i(v, "chat_id");
                Origin::Channel {
                    chat_id,
                    title: self.chat_title(chat_id),
                }
            }
            other => Origin::Unknown {
                td_type: other.to_owned(),
            },
        }
    }

    /// 被回复那条的发送人：先找世界里有没有那条，没有就用 TDLib 附带的 origin 兜底
    fn reply_sender(&self, target: MsgId, r: &Value) -> Option<Sender> {
        let found = self
            .events
            .iter()
            .find(|e| ty(e) == "updateNewMessage" && msg_id(&e["message"]) == target);
        if let Some(e) = found {
            return Some(self.sender(&e["message"]["sender_id"]));
        }
        match self.origin(&r["origin"]) {
            Origin::User { user_id, name } => Some(Sender::User { user_id, name }),
            Origin::Chat { chat_id, title } | Origin::Channel { chat_id, title } => {
                Some(Sender::Chat { chat_id, title })
            }
            Origin::HiddenUser { .. } | Origin::Unknown { .. } => None,
        }
    }

    fn message_doc(&self, m: &Value) -> Doc {
        let id = msg_id(m);
        let date = Time(i(m, "date"));
        let sender = self.sender(&m["sender_id"]);
        // 有临时内容就显示它，不显示原内容（`td_api.tl:3594`）
        let c = match &m["ephemeral_content"] {
            e if e.is_object() => &e["content"],
            _ => &m["content"],
        };
        if let Some(action) = self.service_action(id, c) {
            return Doc::Service(Service {
                id,
                date,
                actor: sender,
                action,
            });
        }

        let outgoing = m["is_outgoing"].as_bool().unwrap_or(false);
        let reply_to = match ty(&m["reply_to"]) {
            "" => None,
            "messageReplyToMessage" => {
                let r = &m["reply_to"];
                let target = MsgId {
                    chat_id: i(r, "chat_id"),
                    message_id: i(r, "message_id"),
                };
                let quote = (!r["quote"].is_null())
                    .then(|| formatted(&r["quote"]["text"]))
                    .filter(|q| !q.is_empty());
                Some(ReplyTo::Message {
                    target,
                    sender: self.reply_sender(target, r),
                    quote,
                })
            }
            other => Some(ReplyTo::Unknown {
                td_type: other.to_owned(),
            }),
        };
        let forward = (!m["forward_info"].is_null()).then(|| Forward {
            origin: self.origin(&m["forward_info"]["origin"]),
            date: Time(i(&m["forward_info"], "date")),
        });
        let album_id = Some(str_or_num(&m["media_album_id"])).filter(|a| !a.is_empty() && a != "0");

        let t = &m["topic_id"];
        let topic = match ty(t) {
            "" => None,
            "messageTopicThread" => Some(Topic::Thread {
                thread_id: i(t, "message_thread_id"),
            }),
            // 原型没有话题缓存，名字留空
            "messageTopicForum" => Some(Topic::Forum {
                topic_id: i(t, "forum_topic_id"),
                name: None,
            }),
            "messageTopicDirectMessages" => Some(Topic::DirectMessages {
                topic_id: i(t, "direct_messages_chat_topic_id"),
            }),
            "messageTopicSavedMessages" => Some(Topic::SavedMessages {
                topic_id: i(t, "saved_messages_topic_id"),
            }),
            other => Some(Topic::Unknown {
                td_type: other.to_owned(),
            }),
        };

        let edit = i(m, "edit_date");
        let reactions = arr(&m["interaction_info"]["reactions"], "reactions")
            .iter()
            .map(|r| Reaction {
                kind: reaction_kind(&r["type"]),
                count: u(r, "total_count"),
                chosen: r["is_chosen"].as_bool().unwrap_or(false),
            })
            .collect();
        let delivery = outgoing.then(|| match ty(&m["sending_state"]) {
            "messageSendingStatePending" => Delivery::Pending,
            "messageSendingStateFailed" => Delivery::Failed,
            _ => {
                let read_upto = self
                    .chats
                    .get(&id.chat_id)
                    .map_or(0, |c| c.last_read_outbox);
                if read_upto >= id.message_id {
                    Delivery::Read
                } else {
                    Delivery::Sent
                }
            }
        });

        Doc::Message(Message {
            id,
            head: Head {
                sender,
                date,
                outgoing,
                reply_to,
                forward,
                album_id,
                topic,
            },
            content: restricted(&m["restriction_info"]).unwrap_or_else(|| Content {
                sensitive: m["restriction_info"]["has_sensitive_content"]
                    .as_bool()
                    .unwrap_or(false),
                ..content(c)
            }),
            tail: Tail {
                edited: (edit > 0).then_some(Time(edit)),
                reactions,
                delivery,
            },
        })
    }

    fn service_action(&self, id: MsgId, c: &Value) -> Option<Action> {
        let user = |uid: i64| UserRef {
            user_id: uid,
            name: self.user_name(uid),
        };
        Some(match ty(c) {
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
            "messagePinMessage" => Action::Pinned {
                pinned: MsgId {
                    chat_id: id.chat_id,
                    message_id: i(c, "message_id"),
                },
            },
            "messageBasicGroupChatCreate" | "messageSupergroupChatCreate" => Action::GroupCreated {
                title: s(c, "title").to_owned(),
            },
            "messageContactRegistered" => Action::ContactRegistered,
            t if OTHER_SERVICE.contains(&t) => Action::Other {
                td_type: t.to_owned(),
            },
            _ => return None,
        })
    }

    fn chat_item(&self, c: &Value) -> ChatItem {
        let kind = match ty(&c["type"]) {
            "chatTypePrivate" => ChatKind::Private,
            "chatTypeBasicGroup" => ChatKind::Group,
            "chatTypeSupergroup" => {
                if c["type"]["is_channel"].as_bool().unwrap_or(false) {
                    ChatKind::Channel
                } else {
                    ChatKind::Supergroup
                }
            }
            "chatTypeSecret" => ChatKind::Secret,
            other => ChatKind::Unknown {
                td_type: other.to_owned(),
            },
        };
        let inherit_mute = c["notification_settings"]["use_default_mute_for"]
            .as_bool()
            .unwrap_or(false);
        let pinned = arr(c, "positions")
            .iter()
            .any(|p| ty(&p["list"]) == "chatListMain" && p["is_pinned"].as_bool().unwrap_or(false));
        ChatItem {
            chat_id: i(c, "id"),
            title: s(c, "title").to_owned(),
            kind,
            unread_count: u(c, "unread_count"),
            unread_mention_count: u(c, "unread_mention_count"),
            // 原型没有范围设置（scopeNotificationSettings），继承时按不静音算
            muted: !inherit_mute && i(&c["notification_settings"], "mute_for") > 0,
            mute_inherited: inherit_mute,
            pinned,
            last: c["last_message"].is_object().then(|| {
                let m = &c["last_message"];
                let id = msg_id(m);
                Box::new(if self.deleted.contains(&id) {
                    ChatLast::Tombstone(Tombstone { id })
                } else {
                    match self.message_doc(m) {
                        Doc::Service(sv) => ChatLast::Service(sv),
                        Doc::Message(msg) => ChatLast::Message(msg),
                        // message_doc 只产出上面两种；其余不可能，按占位兜底
                        Doc::Tombstone(_)
                        | Doc::ChatItem(_)
                        | Doc::Alert(_)
                        | Doc::Status(_)
                        | Doc::Unknown(_) => ChatLast::Tombstone(Tombstone { id }),
                    }
                })
            }),
        }
    }
}

/// 受限内容：原因非空就整块换掉；只有敏感标记就照常建、标上 sensitive
fn restricted(r: &Value) -> Option<Content> {
    let reason = s(r, "restriction_reason");
    (!reason.is_empty()).then(|| Content {
        body: vec![],
        attachment: Some(Attachment::Restricted {
            reason: reason.to_owned(),
        }),
        sensitive: r["has_sensitive_content"].as_bool().unwrap_or(false),
    })
}

fn content(c: &Value) -> Content {
    let caption = || formatted(&c["caption"]);
    let dur = |obj: &str| u(&c[obj], "duration");
    let spoiler = c["has_spoiler"].as_bool().unwrap_or(false);
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
        "messageVoiceNote" => (
            caption(),
            Some(Attachment::Voice {
                duration: dur("voice_note"),
                listened: c["is_listened"].as_bool().unwrap_or(false),
            }),
        ),
        "messageVideoNote" => (
            vec![],
            Some(Attachment::VideoNote {
                duration: dur("video_note"),
                viewed: c["is_viewed"].as_bool().unwrap_or(false),
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
        "messagePoll" => (
            vec![],
            Some(Attachment::Poll {
                question: formatted(&c["poll"]["question"]),
            }),
        ),
        "messageCall" => (
            vec![],
            Some(Attachment::Call {
                video: c["is_video"].as_bool().unwrap_or(false),
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

fn reaction_kind(v: &Value) -> ReactionKind {
    match ty(v) {
        "reactionTypeEmoji" => ReactionKind::Emoji {
            emoji: s(v, "emoji").to_owned(),
        },
        "reactionTypeCustomEmoji" => ReactionKind::CustomEmoji {
            custom_emoji_id: str_or_num(&v["custom_emoji_id"]),
        },
        "reactionTypePaid" => ReactionKind::Paid,
        other => ReactionKind::Unknown {
            td_type: other.to_owned(),
        },
    }
}

/// 原型简化：假定内核在跑、已登录，只按连接状态合成。正式的三线合成归 infra 的
/// `user_status(core, auth, net)`。不认识的连接状态按「正在连接」处理（不说已连接，宁可保守）
fn net_status(v: &Value) -> UserStatus {
    match ty(v) {
        "connectionStateWaitingForNetwork" => UserStatus::Offline,
        "connectionStateUpdating" => UserStatus::Syncing,
        "connectionStateReady" => UserStatus::Online,
        _ => UserStatus::Connecting,
    }
}

// ───── 取字段的小工具：缺了就给默认值 ─────

fn ty(v: &Value) -> &str {
    v["@type"].as_str().unwrap_or("")
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
/// TDLib JSON 里 int53 是数字，int64 是字符串；两种都认
fn as_i64(v: &Value) -> i64 {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|x| x.parse().ok()))
        .unwrap_or(0)
}
fn i(v: &Value, k: &str) -> i64 {
    as_i64(&v[k])
}
fn u(v: &Value, k: &str) -> u32 {
    u32::try_from(i(v, k).max(0)).unwrap_or(u32::MAX)
}
fn str_or_num(v: &Value) -> String {
    match v {
        Value::String(x) => x.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}
fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v[k].as_array().map_or(&[], Vec::as_slice)
}
fn msg_id(m: &Value) -> MsgId {
    MsgId {
        chat_id: i(m, "chat_id"),
        message_id: i(m, "id"),
    }
}
/// TDLib `file`：`size` 不知道时是 0，再看 `expected_size`
fn file_size(f: &Value) -> u64 {
    let n = i(f, "size");
    let n = if n > 0 { n } else { i(f, "expected_size") };
    u64::try_from(n).unwrap_or(0)
}

fn join_name(first: &str, last: &str) -> String {
    match (first.is_empty(), last.is_empty()) {
        (false, false) => format!("{first} {last}"),
        (false, true) => first.to_owned(),
        (true, false) => last.to_owned(),
        (true, true) => String::new(),
    }
}
