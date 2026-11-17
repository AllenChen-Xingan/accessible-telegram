//! 系统消息的读法：「谁做了什么」（`specs/style/service.md`，题 V16–V19、V23）。代号、说法的正本（快照）、常量表都见模块根
//! `speech.rs` 的头注释。

use super::content::content;
use super::{
    ANONYMOUS_ADMIN, DELETED_ACCOUNT, ME, UNKNOWN_ACTOR, UNKNOWN_USER, UNSUPPORTED_SERVICE,
    name_or, sender_name,
};
use crate::tree::*;

// ───────────────────────── 系统消息 ─────────────────────────

/// 系统消息：「谁做了什么」，不带钟点。
/// - 操作人：自己读「我」；群里以群的名义做的读「匿名管理员」（V11）；查不到读「有人」（V18）；已注销读「已注销的账号」（V14）
/// - 被操作的人：自己读「我」（V17 人定「周大山添加了我」）；查不到读「未知用户」（V18）；已注销读「已注销的账号」
/// - 频道里「群组 / 群名 / 群头像」换成「频道 / 频道名 / 频道头像」（style 例 S7）
///
/// 返回 (句子, 里面有没有隐藏内容)：只有置顶带的内容可能有；按键提示由调用方决定加不加
pub(super) fn service(sv: &Service, reveal: bool) -> (String, bool) {
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
    let said = match &sv.action {
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
            let (said, hidden) = content(c, reveal);
            return (format!("{actor}置顶了一条消息：{said}"), hidden);
        }
        Action::Pinned { content: None, .. } => format!("{actor}置顶了一条消息"),
        Action::GroupCreated { title } => format!("{actor}创建了{place}「{title}」"),
        Action::ContactRegistered => format!("{actor}加入了 Telegram"),
        Action::Other { .. } => UNSUPPORTED_SERVICE.to_owned(),
    };
    (said, false)
}
