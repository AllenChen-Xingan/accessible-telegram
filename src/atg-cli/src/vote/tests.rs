//! 假后端：数据全是编的，不碰真实账号与本机档案目录。
use super::*;
use atg_core::config::{Profile, ServerKind};
use atg_core::error::Holder;
use atg_core::fake::{FakeAuth, FakeBackend};
use atg_core::gate::Gate;

#[derive(Default)]
struct Cap {
    out: Vec<String>,
}

impl Sink for Cap {
    fn out(&mut self, line: &str) {
        self.out.push(line.to_owned());
    }
    fn err(&mut self, _: &str) {}
}

const ME: i64 = 4242;
const FRIEND: i64 = 777;

fn text(t: &str) -> Value {
    json!({"@type": "formattedText", "text": t, "entities": []})
}

fn backend(quiz: bool, revoting: bool) -> FakeBackend {
    let b = FakeBackend::with_gate(Gate::new(Holder::Cli)).unwrap();
    b.set_auth(FakeAuth {
        phone: "+15550001234".into(),
        logged_in: true,
        ..FakeAuth::default()
    });
    b.set_me(ME);
    b.set_chat(
        FRIEND,
        vec![json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": FRIEND, "title": "林小雨",
            "type": {"@type": "chatTypePrivate", "user_id": FRIEND}, "positions": [],
            "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0}}})],
    );
    let options: Vec<Value> = ["七点", "八点"]
        .iter()
        .map(|t| json!({"@type": "pollOption", "id": "", "text": text(t), "voter_count": 0,
            "vote_percentage": 0, "recent_voter_ids": [], "is_chosen": false, "is_being_chosen": false}))
        .collect();
    let ty = if quiz {
        json!({"@type": "pollTypeQuiz", "correct_option_ids": [], "explanation": text("")})
    } else {
        json!({"@type": "pollTypeRegular"})
    };
    b.set_history(
        FRIEND,
        vec![json!({"@type": "message", "id": 5, "chat_id": FRIEND,
            "sender_id": {"@type": "messageSenderUser", "user_id": FRIEND}, "is_outgoing": false, "date": 1_792_755_000,
            "content": {"@type": "messagePoll", "poll": {"@type": "poll", "id": "1", "question": text("几点出发？"),
                "options": options, "total_voter_count": 0, "recent_voter_ids": [], "can_see_results": true,
                "is_anonymous": false, "allows_multiple_answers": false, "allows_revoting": revoting,
                "type": ty, "open_period": 0, "close_date": 0, "is_closed": false,
                "vote_restriction_reason": null}, "description": text(""), "can_add_option": false}})],
    );
    b.push_net("connectionStateReady");
    b
}

fn profile() -> Profile {
    let root = atg_core::testkit::temp_dir("cli-vote");
    Profile::create(&root, "p", ServerKind::Main).unwrap()
}

fn vote_p(choices: &[u32], dry_run: bool) -> VoteParams {
    VoteParams {
        chat: FRIEND.to_string(),
        message_id: 5,
        choices: choices.to_vec(),
        replace: false,
        dry_run,
    }
}

fn lines(cap: &Cap) -> Vec<Value> {
    cap.out
        .iter()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn run(b: &FakeBackend, p: &VoteParams, format: Format) -> (u8, Cap) {
    crate::history::with_backend(b, &profile(), Instant::now(), |b, st, fe| {
        let mut cap = Cap::default();
        let t = Duration::from_millis(500);
        let code = execute(b, st, fe, p, &mut cap, format, Instant::now(), t);
        Ok((code, cap))
    })
    .unwrap_or_else(|_| panic!("起不来"))
}

#[test]
fn vote_on_main_sends_directly_and_reports() {
    let b = backend(false, true);
    let (code, cap) = run(&b, &vote_p(&[2], false), Format::Json);
    assert_eq!(code, 0, "{:?}", cap.out);
    let ls = lines(&cap);
    assert_eq!(ls[0]["kind"], "plan");
    assert_eq!(ls[0]["data"]["action"], "vote");
    assert_eq!(ls[0]["data"]["confirm"], "direct_mode");
    assert_eq!(ls[0]["data"]["choices"][0]["text"], "八点");
    let end = ls.last().unwrap();
    assert_eq!(end["kind"], "result");
    assert_eq!(end["data"]["choices"], json!([2]));
    assert_eq!(b.submitted()[0]["option_ids"], json!([1]));
}

#[test]
fn dry_run_does_not_submit() {
    let b = backend(false, true);
    let (code, cap) = run(&b, &vote_p(&[1], true), Format::Json);
    assert_eq!(code, 0);
    assert_eq!(lines(&cap).last().unwrap()["data"]["dry_run"], true);
    assert!(b.submitted().is_empty());
}

/// 计划句照实际设置说能不能改；测验另说投后公布答案（人改定 6）
#[test]
fn plan_sentence_says_revoting_and_quiz() {
    let b = backend(true, false);
    let (_, cap) = run(&b, &vote_p(&[1], true), Format::Text);
    let plan = &cap.out[0];
    assert!(plan.contains("第 1 项，七点"), "{plan}");
    assert!(plan.contains("公开投票，别人看得到你投了什么"), "{plan}");
    assert!(plan.contains("投了不能改"), "{plan}");
    assert!(plan.contains("投后会公布答案"), "{plan}");
    let b = backend(false, true);
    let (_, cap) = run(&b, &vote_p(&[1], true), Format::Text);
    assert!(cap.out[0].contains("投了能改"), "{}", cap.out[0]);
    assert!(!cap.out[0].contains("测验"), "{}", cap.out[0]);
}

#[test]
fn bad_choice_is_invalid_input_and_not_retryable() {
    // 一个假后端只起得了一次：每种情形各起一个
    for p in [vote_p(&[3], false), vote_p(&[1, 2], false)] {
        let b = backend(false, true);
        let (code, cap) = run(&b, &p, Format::Json);
        let end = lines(&cap).pop().unwrap();
        assert_eq!(end["kind"], "error");
        assert_eq!(end["data"]["code"], "invalid_input");
        assert_eq!(end["data"]["why"], "vote_bad_choice");
        assert_eq!(end["data"]["retryable"], false);
        assert_ne!(code, 0);
        assert!(b.submitted().is_empty());
    }
}

#[test]
fn check_limits_choices() {
    assert!(check(&vote_p(&[], false)).is_err());
    assert!(check(&vote_p(&[1; 13], false)).is_err());
    assert!(check(&vote_p(&[1, 2], false)).is_ok());
}

#[test]
fn would_replace_error_carries_the_replace_hint() {
    let mut cap = Cap::default();
    let e = CoreError::InvalidSend {
        why: InvalidSendWhy::VoteWouldReplace,
    };
    assert_ne!(fail_line(&mut cap, Format::Json, &Fail::Core(e)), 0);
    let end = lines(&cap).pop().unwrap();
    assert_eq!(end["data"]["why"], "vote_would_replace");
    assert!(
        end["data"]["hint_zh"]
            .as_str()
            .unwrap()
            .contains("--replace")
    );
}
