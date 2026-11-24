//! `send::vote` 的单测：假后端走全流程（数据全是编的，选项文字取自 `fixtures/names.txt` 的风格）

use super::*;
use crate::config::{Profile, ServerKind};
use crate::error::{Holder, SendRefusedReason};
use crate::fake::{FakeAuth, FakeBackend, FakeSend};
use crate::gate::{DIRECT_LOG_FILE, GUARD_FILE, Gate};
use crate::link::{EventStream, block_on};
use crate::status::AuthStep;
use std::path::PathBuf;

const ME: i64 = 4242;
const FRIEND: i64 = 777;

struct Rig {
    b: FakeBackend,
    _stream: EventStream,
    dir: PathBuf,
}

fn new_chat(id: i64, title: &str) -> Value {
    json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title,
        "type": {"@type": "chatTypePrivate", "user_id": id}, "positions": [], "notification_settings": {}}})
}

fn text(t: &str) -> Value {
    json!({"@type": "formattedText", "text": t, "entities": []})
}

fn option(t: &str, chosen: bool) -> Value {
    json!({"@type": "pollOption", "id": "", "text": text(t), "voter_count": 0, "vote_percentage": 0,
        "recent_voter_ids": [], "is_chosen": chosen, "is_being_chosen": false})
}

/// 一个投票：选项「七点」「八点」「九点」，`order` 为显示顺序（`option_order`）
#[derive(Clone)]
struct Poll {
    chosen: Option<usize>,
    multiple: bool,
    revoting: bool,
    quiz: bool,
    closed: bool,
    order: Vec<u32>,
    first: &'static str,
}

impl Default for Poll {
    fn default() -> Self {
        Self {
            chosen: None,
            multiple: false,
            revoting: true,
            quiz: false,
            closed: false,
            order: vec![],
            first: "七点",
        }
    }
}

fn poll_msg(chat: i64, p: &Poll) -> Value {
    let names = [p.first, "八点", "九点"];
    let options: Vec<Value> = names
        .iter()
        .enumerate()
        .map(|(i, n)| option(n, p.chosen == Some(i)))
        .collect();
    let ty = if p.quiz {
        json!({"@type": "pollTypeQuiz", "correct_option_ids": [], "explanation": text("")})
    } else {
        json!({"@type": "pollTypeRegular"})
    };
    json!({"@type": "message", "id": 5, "chat_id": chat,
        "sender_id": {"@type": "messageSenderUser", "user_id": FRIEND}, "is_outgoing": false, "date": 1_792_755_000,
        "content": {"@type": "messagePoll", "poll": {"@type": "poll", "id": "1", "question": text("几点出发？"),
            "options": options, "total_voter_count": 0, "recent_voter_ids": [], "can_see_results": true,
            "is_anonymous": false, "allows_multiple_answers": p.multiple, "allows_revoting": p.revoting,
            "type": ty, "open_period": 0, "close_date": 0, "is_closed": p.closed, "option_order": p.order,
            "vote_restriction_reason": null}, "description": text(""), "can_add_option": false}})
}

fn rig(tag: &str, test_dc: bool, p: &Poll) -> Rig {
    let b = FakeBackend::with_gate(Gate::new(Holder::Cli)).unwrap();
    b.set_auth(FakeAuth {
        phone: if test_dc {
            "+9996612345"
        } else {
            "+15550001234"
        }
        .into(),
        logged_in: true,
        ..FakeAuth::default()
    });
    b.set_me(ME);
    b.set_chat(ME, vec![new_chat(ME, "我自己")]);
    b.set_chat(FRIEND, vec![new_chat(FRIEND, "林小雨")]);
    b.set_history(ME, vec![poll_msg(ME, p)]);
    b.set_history(FRIEND, vec![poll_msg(FRIEND, p)]);
    let root = crate::testkit::temp_dir(&format!("vote-{tag}"));
    let kind = if test_dc {
        ServerKind::Test
    } else {
        ServerKind::Main
    };
    let profile = Profile::create(&root, "p", kind).unwrap();
    let stream = b.subscribe().unwrap();
    block_on(b.start(&profile)).unwrap();
    b.wait_auth(Duration::from_secs(5), &|s| s == AuthStep::Ready)
        .unwrap();
    Rig {
        b,
        _stream: stream,
        dir: profile.dir().to_path_buf(),
    }
}

fn id(chat: i64) -> MsgId {
    MsgId {
        chat_id: chat,
        message_id: 5,
    }
}

fn prepare(r: &Rig, chat: i64, numbers: &[u32]) -> Result<VotePlan, CoreError> {
    prepare_with(r, chat, numbers, false)
}

fn prepare_with(r: &Rig, chat: i64, n: &[u32], replace: bool) -> Result<VotePlan, CoreError> {
    block_on(prepare_vote(&r.b, id(chat), n, replace, SendOrigin::Cli))
}

fn once(r: &Rig, chat: i64, numbers: &[u32]) -> Result<ConfirmMode, CoreError> {
    once_with(r, chat, numbers, false)
}

fn once_with(r: &Rig, chat: i64, n: &[u32], replace: bool) -> Result<ConfirmMode, CoreError> {
    let plan = prepare_with(r, chat, n, replace)?;
    let token = r.b.gate().issue(&plan, false)?;
    vote_within(&r.b, &plan, token, Duration::from_secs(2))
}

fn why(e: CoreError) -> InvalidSendWhy {
    match e {
        CoreError::InvalidSend { why } => why,
        other => panic!("{other:?}"),
    }
}

fn log_lines(r: &Rig) -> Vec<Value> {
    std::fs::read_to_string(r.dir.join(DIRECT_LOG_FILE))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn request_shape_is_fixed() {
    let r = rig("shape", true, &Poll::default());
    let plan = prepare(&r, ME, &[2]).unwrap();
    assert_eq!(
        plan.request,
        json!({"@type": "setPollAnswer", "chat_id": ME, "message_id": 5, "option_ids": [1]})
    );
    let j = vote_plan_json(&plan);
    assert_eq!(j["action"], "vote");
    assert_eq!(
        j["choices"],
        json!([{"number": 2, "index": 1, "text": "八点"}])
    );
    assert_eq!(j["revotable"], true);
    assert_eq!(j["anonymous"], false);
    assert_eq!(j["quiz"], false);
}

/// 「第 N 项」按显示顺序（`option_order`）数，交给 TDLib 的是它的 0 起编号
#[test]
fn numbers_follow_display_order() {
    let p = Poll {
        order: vec![2, 0, 1],
        multiple: true,
        ..Poll::default()
    };
    let r = rig("order", true, &p);
    let plan = prepare(&r, ME, &[1, 3]).unwrap();
    let texts: Vec<_> = plan.choices().iter().map(|c| c.text.clone()).collect();
    assert_eq!(
        texts,
        vec![Some("九点".to_owned()), Some("八点".to_owned())]
    );
    assert_eq!(plan.request["option_ids"], json!([1, 2]));
}

#[test]
fn test_dc_saved_votes_without_log() {
    let r = rig("tdc", true, &Poll::default());
    assert_eq!(once(&r, ME, &[1]).unwrap(), ConfirmMode::TestExempt);
    let sub = r.b.submitted();
    assert_eq!(sub.len(), 1);
    assert_eq!(sub[0]["@type"], "setPollAnswer");
    assert!(log_lines(&r).is_empty());
}

#[test]
fn test_dc_other_chat_is_refused() {
    let r = rig("tdc-other", true, &Poll::default());
    let e = prepare(&r, FRIEND, &[1]).unwrap_err();
    assert!(matches!(
        e,
        CoreError::SendRefused {
            reason: SendRefusedReason::TestDcSavedOnly,
            ..
        }
    ));
}

/// 正式服务器直发：留痕记 `vote` 与那条消息的 id，不记选了哪几项；不进防重复，同样的票能再投
#[test]
fn main_server_votes_directly_and_logs() {
    let r = rig("main", false, &Poll::default());
    assert_eq!(once(&r, FRIEND, &[2]).unwrap(), ConfirmMode::DirectMode);
    let log = log_lines(&r);
    assert_eq!(log.len(), 2, "{log:?}");
    for (l, want) in log.iter().zip(["submitted", "succeeded"]) {
        assert_eq!(l["kind"], "vote");
        assert_eq!(l["result"], want);
        assert_eq!(l["chat_id"], FRIEND);
        assert!(!l.to_string().contains("八点"));
        assert!(l.get("option_ids").is_none());
    }
    assert_eq!(log[1]["final_id"], 5);
    assert!(!r.dir.join(GUARD_FILE).exists());
    once(&r, FRIEND, &[2]).unwrap();
    assert_eq!(r.b.submitted().len(), 2);
}

#[test]
fn bad_choices_are_refused_locally() {
    let r = rig("bad", true, &Poll::default());
    for n in [&[][..], &[0], &[4], &[1, 1], &[1, 2]] {
        assert_eq!(
            why(prepare(&r, ME, n).unwrap_err()),
            InvalidSendWhy::VoteBadChoice,
            "{n:?}"
        );
    }
    let multi = rig(
        "bad-multi",
        true,
        &Poll {
            multiple: true,
            ..Poll::default()
        },
    );
    assert!(prepare(&multi, ME, &[1, 2]).is_ok());
    assert!(r.b.submitted().is_empty());
}

#[test]
fn closed_and_not_revotable_are_refused() {
    let closed = Poll {
        closed: true,
        ..Poll::default()
    };
    let r = rig("closed", true, &closed);
    assert_eq!(
        why(prepare(&r, ME, &[1]).unwrap_err()),
        InvalidSendWhy::VoteBlocked
    );
    let voted = Poll {
        chosen: Some(0),
        revoting: false,
        ..Poll::default()
    };
    let r = rig("voted", true, &voted);
    assert_eq!(
        why(prepare(&r, ME, &[2]).unwrap_err()),
        InvalidSendWhy::VoteNotRevotable
    );
}

/// 自审 13b-1 第 1 轮 P1：许改票、我已投过，`setPollAnswer` 整组替换——默认拒，`replace` 放行并列出会换掉的那几项
#[test]
fn revote_replaces_and_needs_replace() {
    let voted = Poll {
        chosen: Some(0),
        multiple: true,
        ..Poll::default()
    };
    let r = rig("revote", true, &voted);
    assert_eq!(
        why(prepare(&r, ME, &[3]).unwrap_err()),
        InvalidSendWhy::VoteWouldReplace
    );
    let plan = prepare_with(&r, ME, &[3], true).unwrap();
    assert_eq!(
        vote_plan_json(&plan)["replaces"],
        json!([{"number": 1, "index": 0, "text": "七点"}])
    );
    // 自审第 2 轮 P2：把原来投的也写上 = 追加，一项都不丢——不拒、不算换；和现在投的一样的也不算
    let add = prepare(&r, ME, &[1, 3]).unwrap();
    assert!(add.replaces().is_empty());
    assert!(prepare(&r, ME, &[1]).unwrap().replaces().is_empty());
    // 部分重叠：只列真会丢掉的那几项
    let two = Poll {
        chosen: Some(0),
        multiple: true,
        ..Poll::default()
    };
    let r2 = rig("revote-overlap", true, &two);
    let mut m = poll_msg(ME, &two);
    m["content"]["poll"]["options"][1]["is_chosen"] = json!(true);
    r2.b.set_history(ME, vec![m]);
    let p = prepare_with(&r2, ME, &[1, 3], true).unwrap();
    let lost: Vec<u32> = p.replaces().iter().map(|c| c.number).collect();
    assert_eq!(lost, vec![2]);
    assert!(once_with(&r, ME, &[3], true).is_ok());
    assert!(r.b.submitted().len() == 1);
}

/// 认不出的投票类型不投（读法说「暂不支持」，投了会怎样不知道）
#[test]
fn unknown_poll_type_is_refused() {
    let r = rig("unknown", true, &Poll::default());
    let mut m = poll_msg(ME, &Poll::default());
    m["content"]["poll"]["type"] = json!({"@type": "pollTypeFuture"});
    r.b.set_history(ME, vec![m]);
    assert_eq!(
        why(prepare(&r, ME, &[1]).unwrap_err()),
        InvalidSendWhy::VoteBlocked
    );
}

/// 准备之后我在别处投了票（许改）：没给 replace 的拒；给了 replace 但会换掉的已和计划说的不同 → changed
#[test]
fn recheck_catches_my_vote_elsewhere() {
    let later = Poll {
        chosen: Some(2),
        ..Poll::default()
    };
    for (replace, want) in [
        (false, InvalidSendWhy::VoteWouldReplace),
        (true, InvalidSendWhy::Changed),
    ] {
        let r = rig(&format!("elsewhere-{replace}"), false, &Poll::default());
        let plan = prepare_with(&r, FRIEND, &[1], replace).unwrap();
        let token = r.b.gate().issue(&plan, false).unwrap();
        r.b.set_history(FRIEND, vec![poll_msg(FRIEND, &later)]);
        let e = vote_within(&r.b, &plan, token, Duration::from_secs(2)).unwrap_err();
        assert_eq!(why(e), want);
        assert!(r.b.submitted().is_empty());
    }
}

/// 交出口报连接断了：请求可能已交出 → 按结果未知报、记 unknown；失败与未知的留痕都带那条消息的 id
#[test]
fn disconnected_submit_is_outcome_unknown() {
    let r = rig("disc", false, &Poll::default());
    r.b.script_sends(vec![FakeSend::Reject(CoreError::Disconnected)]);
    let e = once(&r, FRIEND, &[1]).unwrap_err();
    assert_eq!(e.code(), "send_outcome_unknown");
    let log = log_lines(&r);
    assert_eq!(log[1]["result"], "unknown");
    assert_eq!(log[1]["final_id"], 5);
}

#[test]
fn not_a_poll_is_refused() {
    let r = rig("text", true, &Poll::default());
    r.b.set_history(
        ME,
        vec![json!({"@type": "message", "id": 5, "chat_id": ME,
            "sender_id": {"@type": "messageSenderUser", "user_id": ME}, "is_outgoing": true, "date": 1_792_755_000,
            "content": {"@type": "messageText", "text": text("第5条")}})],
    );
    assert_eq!(
        why(prepare(&r, ME, &[1]).unwrap_err()),
        InvalidSendWhy::NotAPoll
    );
}

/// 人改定 6：测验许投，计划照实际设置说能不能改；测验另说投后公布答案
#[test]
fn quiz_is_allowed_and_says_so() {
    let p = Poll {
        quiz: true,
        revoting: true,
        ..Poll::default()
    };
    let r = rig("quiz", true, &p);
    let plan = prepare(&r, ME, &[1]).unwrap();
    assert!(plan.quiz());
    assert!(plan.revotable());
    assert!(once(&r, ME, &[1]).is_ok());
}

/// 交出前复核：准备之后投票结束了、我在别处投过（不许改）、选项被改了 → 不交出
#[test]
fn recheck_before_submit() {
    let base = Poll {
        revoting: false,
        ..Poll::default()
    };
    let cases: [(Poll, InvalidSendWhy); 3] = [
        (
            Poll {
                closed: true,
                ..base.clone()
            },
            InvalidSendWhy::VoteBlocked,
        ),
        (
            Poll {
                chosen: Some(2),
                ..base.clone()
            },
            InvalidSendWhy::VoteNotRevotable,
        ),
        (
            Poll {
                first: "六点",
                ..base.clone()
            },
            InvalidSendWhy::Changed,
        ),
    ];
    for (i, (later, want)) in cases.into_iter().enumerate() {
        let r = rig(&format!("recheck-{i}"), false, &base);
        let plan = prepare(&r, FRIEND, &[1]).unwrap();
        let token = r.b.gate().issue(&plan, false).unwrap();
        r.b.set_history(FRIEND, vec![poll_msg(FRIEND, &later)]);
        let e = vote_within(&r.b, &plan, token, Duration::from_secs(2)).unwrap_err();
        assert_eq!(why(e), want, "case {i}");
        assert!(r.b.submitted().is_empty(), "case {i}");
        assert!(log_lines(&r).is_empty(), "case {i}");
    }
}

#[test]
fn token_is_bound_to_the_plan() {
    let r = rig("bind", false, &Poll::default());
    let one = prepare(&r, FRIEND, &[1]).unwrap();
    let two = prepare(&r, FRIEND, &[2]).unwrap();
    let token = r.b.gate().issue(&one, false).unwrap();
    let e = vote_within(&r.b, &two, token, Duration::from_secs(2)).unwrap_err();
    assert!(matches!(
        e,
        CoreError::SendRefused {
            reason: SendRefusedReason::ContentMismatch,
            ..
        }
    ));
    assert!(r.b.submitted().is_empty());
}

#[test]
fn rejected_submit_is_logged_as_failed() {
    let r = rig("reject", false, &Poll::default());
    r.b.script_sends(vec![FakeSend::Reject(CoreError::InvalidInput { td: None })]);
    let e = once(&r, FRIEND, &[1]).unwrap_err();
    assert_eq!(e.code(), "invalid_input");
    let log = log_lines(&r);
    assert_eq!(log[1]["result"], "failed");
    assert_eq!(log[1]["final_id"], 5);
}

/// 含剧透的选项：计划行不替人揭开
#[test]
fn spoiler_option_text_is_withheld() {
    let r = rig("spoiler", true, &Poll::default());
    let mut m = poll_msg(ME, &Poll::default());
    m["content"]["poll"]["options"][0]["text"] = json!({"@type": "formattedText", "text": "七点",
        "entities": [{"@type": "textEntity", "offset": 0, "length": 2, "type": {"@type": "textEntityTypeSpoiler"}}]});
    r.b.set_history(ME, vec![m]);
    let plan = prepare(&r, ME, &[1]).unwrap();
    assert_eq!(plan.choices()[0].text, None);
    assert!(!vote_plan_json(&plan).to_string().contains("七点"));
}
