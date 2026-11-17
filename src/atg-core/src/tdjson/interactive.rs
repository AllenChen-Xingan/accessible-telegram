//! 能互动的那部分（#13）：投票全貌（`messagePoll` → `Attachment::Poll`）与消息上的按钮（`reply_markup` → [`Keyboard`]）。
//! 只读；按按钮、投票是 13b / 13a 的事

use super::message::formatted;
use super::{arr, b, i, s, ty, u};
use crate::tree::{Attachment, Button, ButtonAction, Keyboard, KeyboardKind, PollOption};
use serde_json::Value;

/// `messagePoll.poll`（`td_api.tl:731`）→ 投票附件
pub(super) fn poll(p: &Value) -> Attachment {
    Attachment::Poll {
        question: formatted(&p["question"]),
        options: arr(p, "options")
            .iter()
            .map(|o| PollOption {
                text: formatted(&o["text"]),
                votes: u(o, "voter_count"),
                percent: u(o, "vote_percentage").min(100),
                chosen: b(o, "is_chosen"),
            })
            .collect(),
        total: u(p, "total_voter_count"),
        anonymous: b(p, "is_anonymous"),
        multiple: b(p, "allows_multiple_answers"),
        revotable: b(p, "allows_revoting"),
        quiz: ty(&p["type"]) == "pollTypeQuiz",
        closed: b(p, "is_closed"),
        // 不许投的原因（`vote_restriction_reason`）为空才能投；已结束的也不能
        can_vote: p["vote_restriction_reason"].is_null() && !b(p, "is_closed"),
    }
}

/// `message.reply_markup`（`td_api.tl:4294-4314`）→ 按钮。没有、收起键盘、强制回复、一个按钮都没有 → `None`
pub(super) fn keyboard(m: &Value) -> Option<Keyboard> {
    let r = &m["reply_markup"];
    let (kind, button): (KeyboardKind, fn(&Value) -> ButtonAction) = match ty(r) {
        "replyMarkupInlineKeyboard" => (KeyboardKind::Inline, inline_action),
        "replyMarkupShowKeyboard" => (KeyboardKind::Reply, reply_action),
        _ => return None,
    };
    let rows: Vec<Vec<Button>> = arr(r, "rows")
        .iter()
        .map(|row| {
            row.as_array()
                .map(|bs| {
                    bs.iter()
                        .map(|x| Button {
                            text: s(x, "text").to_owned(),
                            action: button(&x["type"]),
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .filter(|row: &Vec<Button>| !row.is_empty())
        .collect();
    (!rows.is_empty()).then_some(Keyboard { kind, rows })
}

/// 内联按钮（`inlineKeyboardButtonType*`，`td_api.tl:4230-4266`）
fn inline_action(t: &Value) -> ButtonAction {
    match ty(t) {
        "inlineKeyboardButtonTypeUrl" => ButtonAction::Url {
            url: s(t, "url").to_owned(),
        },
        "inlineKeyboardButtonTypeLoginUrl" => ButtonAction::LoginUrl {
            url: s(t, "url").to_owned(),
        },
        "inlineKeyboardButtonTypeWebApp" => ButtonAction::WebApp,
        "inlineKeyboardButtonTypeCallback" => ButtonAction::Callback,
        "inlineKeyboardButtonTypeCallbackWithPassword" => ButtonAction::CallbackWithPassword,
        "inlineKeyboardButtonTypeCallbackGame" => ButtonAction::Game,
        "inlineKeyboardButtonTypeSwitchInline" => ButtonAction::SwitchInline,
        "inlineKeyboardButtonTypeBuy" => ButtonAction::Buy,
        "inlineKeyboardButtonTypeUser" => ButtonAction::User {
            user_id: i(t, "user_id"),
        },
        "inlineKeyboardButtonTypeCopyText" => ButtonAction::CopyText {
            text: s(t, "text").to_owned(),
        },
        "inlineKeyboardButtonTypeDisabled" => ButtonAction::Disabled,
        other => ButtonAction::Unknown {
            td_type: other.to_owned(),
        },
    }
}

/// 键盘按钮（`keyboardButtonType*`，`td_api.tl:4167-4215`）
fn reply_action(t: &Value) -> ButtonAction {
    match ty(t) {
        "keyboardButtonTypeText" => ButtonAction::Text,
        "keyboardButtonTypeRequestPhoneNumber" => ButtonAction::RequestPhone,
        "keyboardButtonTypeRequestLocation" => ButtonAction::RequestLocation,
        "keyboardButtonTypeRequestPoll" => ButtonAction::RequestPoll,
        "keyboardButtonTypeRequestUsers" => ButtonAction::RequestUsers,
        "keyboardButtonTypeRequestChat" => ButtonAction::RequestChat,
        "keyboardButtonTypeWebApp" => ButtonAction::WebApp,
        other => ButtonAction::Unknown {
            td_type: other.to_owned(),
        },
    }
}
