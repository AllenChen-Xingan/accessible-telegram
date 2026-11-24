//! 能互动的那部分（#13）：投票全貌（`messagePoll` → `Attachment::Poll`）与消息上的按钮（`reply_markup` → [`Keyboard`]）。
//! 只读；按按钮、投票是 13b / 13a 的事

use super::message::formatted;
use super::{arr, b, i, s, ty, u};
use crate::tree::{
    Attachment, Button, ButtonAction, Keyboard, KeyboardKind, PollKind, PollOption, VoteBlock,
};
use serde_json::Value;

/// `messagePoll.poll`（`td_api.tl:731`）→ 投票附件。选项按 `option_order`（TDLib：「必须按这个顺序显示」）排，
/// 每项留着 TDLib 的 0 起编号；`option_order` 为空或对不上（缺号、重号、越界）就按原顺序
///
/// **隐私守卫**：「谁投了」（`recent_voter_ids`，投票与选项上都有）**有意不读**——别人的投票去向。所有消息 JSON
/// （新消息、取历史、`updateMessageContent` 换内容）都经 `content()` 走到这里，以后改这个函数也不要读它
/// （测试 `snapshots::voters_never_reach_the_tree`）
pub(super) fn poll(p: &Value) -> Attachment {
    let kind = match ty(&p["type"]) {
        "pollTypeRegular" | "" => PollKind::Regular,
        "pollTypeQuiz" => PollKind::Quiz,
        other => PollKind::Unknown {
            td_type: other.to_owned(),
        },
    };
    let quiz = kind == PollKind::Quiz;
    let correct: Vec<i64> = arr(&p["type"], "correct_option_ids")
        .iter()
        .filter_map(Value::as_i64)
        .collect();
    let raw: Vec<PollOption> = arr(p, "options")
        .iter()
        .enumerate()
        .map(|(n, o)| PollOption {
            index: u32::try_from(n).unwrap_or(u32::MAX),
            text: formatted(&o["text"]),
            votes: u(o, "voter_count"),
            percent: u(o, "vote_percentage").min(100),
            chosen: b(o, "is_chosen"),
            pending: b(o, "is_being_chosen"),
            correct: quiz && correct.contains(&i64::try_from(n).unwrap_or(-1)),
        })
        .collect();
    let closed = b(p, "is_closed");
    Attachment::Poll {
        question: formatted(&p["question"]),
        options: ordered(raw, arr(p, "option_order")),
        total: u(p, "total_voter_count"),
        results_visible: b(p, "can_see_results"),
        anonymous: b(p, "is_anonymous"),
        multiple: b(p, "allows_multiple_answers"),
        revotable: b(p, "allows_revoting"),
        kind,
        closed,
        blocked: blocked(&p["vote_restriction_reason"], closed),
        explanation: if quiz {
            formatted(&p["type"]["explanation"])
        } else {
            vec![]
        },
    }
}

/// 按 `option_order` 排；它不是 0..n 的一个排列就不动
fn ordered(raw: Vec<PollOption>, order: &[Value]) -> Vec<PollOption> {
    let idx: Vec<usize> = order
        .iter()
        .filter_map(|v| v.as_u64().and_then(|x| usize::try_from(x).ok()))
        .collect();
    let mut seen = vec![false; raw.len()];
    let valid = idx.len() == raw.len()
        && idx
            .iter()
            .all(|&k| k < raw.len() && !std::mem::replace(&mut seen[k], true));
    if !valid {
        return raw;
    }
    idx.iter().map(|&k| raw[k].clone()).collect()
}

/// 为什么不能投；没给原因但已结束的也算「已结束」
fn blocked(r: &Value, closed: bool) -> Option<VoteBlock> {
    Some(match ty(r) {
        "" if closed => VoteBlock::Closed,
        "" => return None,
        "pollVoteRestrictionReasonClosed" => VoteBlock::Closed,
        "pollVoteRestrictionReasonYetUnsent" => VoteBlock::YetUnsent,
        "pollVoteRestrictionReasonScheduled" => VoteBlock::Scheduled,
        "pollVoteRestrictionReasonCountryRestricted" => VoteBlock::CountryRestricted {
            country_code: s(r, "country_code").to_owned(),
        },
        "pollVoteRestrictionReasonMembershipRequired" => VoteBlock::MembershipRequired {
            chat_id: i(r, "chat_id"),
        },
        "pollVoteRestrictionReasonOther" => VoteBlock::Other,
        other => VoteBlock::Unknown {
            td_type: other.to_owned(),
        },
    })
}

/// `message.reply_markup`（`td_api.tl:4294-4314`）→ 按钮。没有、收起键盘、强制回复、一个按钮都没有 → `None`
pub(super) fn keyboard(m: &Value) -> Option<Keyboard> {
    let r = &m["reply_markup"];
    let (kind, button): (KeyboardKind, fn(&Value) -> ButtonAction) = match ty(r) {
        "replyMarkupInlineKeyboard" => (KeyboardKind::Inline, inline_action),
        "replyMarkupShowKeyboard" => (KeyboardKind::Reply, reply_action),
        // 认识、但没有按钮的两种：不建
        "" | "replyMarkupRemoveKeyboard" | "replyMarkupForceReply" => return None,
        // 不认识的：留下类型名，按钮读不出（开放集合的兜底，第一节硬规矩 2）
        other => {
            return Some(Keyboard {
                kind: KeyboardKind::Unknown {
                    td_type: other.to_owned(),
                },
                rows: vec![],
            });
        }
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

/// 交出口用：消息 JSON 里第 `n` 个按钮（1 起，跨行连续数，同 [`Keyboard::nth`]）是回调按钮、文字还是 `text`，就回它的
/// 回调数据（`inlineKeyboardButtonTypeCallback.data`）；否则 `None`（按钮被机器人换了）。**只在交出口里用**：数据不进树、不进
/// model、不进计划与留痕（#13 13b-2）
pub(crate) fn callback_data(m: &Value, n: usize, text: &str) -> Option<String> {
    let r = &m["reply_markup"];
    if ty(r) != "replyMarkupInlineKeyboard" {
        return None;
    }
    let x = arr(r, "rows")
        .iter()
        .filter_map(Value::as_array)
        .flatten()
        .nth(n.checked_sub(1)?)?;
    (s(x, "text") == text && ty(&x["type"]) == "inlineKeyboardButtonTypeCallback")
        .then(|| s(&x["type"], "data").to_owned())
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
        "keyboardButtonTypeRequestManagedBot" => ButtonAction::RequestManagedBot,
        "keyboardButtonTypeWebApp" => ButtonAction::WebApp,
        other => ButtonAction::Unknown {
            td_type: other.to_owned(),
        },
    }
}
