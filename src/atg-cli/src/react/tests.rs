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

fn backend() -> FakeBackend {
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
    b.set_history(
        FRIEND,
        vec![json!({"@type": "message", "id": 5, "chat_id": FRIEND,
            "sender_id": {"@type": "messageSenderUser", "user_id": FRIEND}, "is_outgoing": false, "date": 1_792_755_000,
            "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": "第5条", "entities": []}}})],
    );
    b.push_net("connectionStateReady");
    b
}

fn profile() -> Profile {
    let root = atg_core::testkit::temp_dir("cli-react");
    Profile::create(&root, "p", ServerKind::Main).unwrap()
}

fn react_p(emoji: &str, remove: bool, dry_run: bool) -> ReactParams {
    ReactParams {
        chat: FRIEND.to_string(),
        message_id: 5,
        emoji: emoji.into(),
        remove,
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

/// 在同一个后端上依次跑几次 react，再跑一次 reactions
fn run(b: &FakeBackend, ps: &[ReactParams], list: Option<&ReactionsParams>) -> Vec<(u8, Cap)> {
    crate::history::with_backend(b, &profile(), Instant::now(), |b, st, fe| {
        let mut out: Vec<(u8, Cap)> = ps
            .iter()
            .map(|p| {
                let mut cap = Cap::default();
                let t = Duration::from_millis(500);
                let code = execute_react(b, st, fe, p, &mut cap, Format::Json, Instant::now(), t);
                (code, cap)
            })
            .collect();
        if let Some(q) = list {
            let mut cap = Cap::default();
            let code = execute_reactions(b, st, fe, q, &mut cap, Format::Json, Instant::now());
            out.push((code, cap));
        }
        Ok(out)
    })
    .unwrap_or_else(|_| panic!("起不来"))
}

#[test]
fn react_on_main_sends_directly_and_reports() {
    let b = backend();
    let r = run(&b, &[react_p("👍", false, false)], None);
    let (code, cap) = &r[0];
    assert_eq!(*code, 0, "{:?}", cap.out);
    let ls = lines(cap);
    assert_eq!(ls[0]["kind"], "plan");
    assert_eq!(ls[0]["data"]["action"], "react");
    assert_eq!(ls[0]["data"]["confirm"], "direct_mode");
    assert_eq!(ls[0]["data"]["reaction"]["emoji"], "👍");
    let end = ls.last().unwrap();
    assert_eq!(end["kind"], "result");
    assert_eq!(end["data"]["confirm"], "direct_mode");
    assert_eq!(end["data"]["removed"], false);
    assert_eq!(b.submitted().len(), 1);
}

#[test]
fn dry_run_does_not_submit() {
    let b = backend();
    let r = run(&b, &[react_p("👍", false, true)], None);
    assert_eq!(r[0].0, 0);
    assert_eq!(lines(&r[0].1).last().unwrap()["data"]["dry_run"], true);
    assert!(b.submitted().is_empty());
}

#[test]
fn bad_emoji_and_not_chosen_are_invalid_input() {
    let b = backend();
    let r = run(
        &b,
        &[react_p("ok", false, false), react_p("👍", true, false)],
        None,
    );
    for (code, cap) in &r {
        let end = lines(cap).pop().unwrap();
        assert_eq!(end["kind"], "error");
        assert_eq!(end["data"]["code"], "invalid_input");
        assert_eq!(end["data"]["retryable"], false);
        assert_ne!(*code, 0);
    }
    assert!(b.submitted().is_empty());
}

#[test]
fn reactions_lists_one_line_each() {
    let b = backend();
    let q = ReactionsParams {
        chat: FRIEND.to_string(),
        message_id: 5,
        emoji: None,
        limit: 10,
        cursor: None,
        tz: Some("+08:00".into()),
    };
    let r = run(&b, &[react_p("👍", false, false)], Some(&q));
    let (code, cap) = &r[1];
    assert_eq!(*code, 0, "{:?}", cap.out);
    let ls = lines(cap);
    assert_eq!(ls[0]["kind"], "reactor");
    assert_eq!(ls[0]["data"]["outgoing"], true);
    assert!(
        ls[0]["speech"]["brief"]
            .as_str()
            .unwrap()
            .starts_with("我，👍，")
    );
    let end = ls.last().unwrap();
    assert_eq!(
        (end["data"]["count"].clone(), end["data"]["more"].clone()),
        (json!(1), json!(false))
    );
}

#[test]
fn reactions_params_are_checked() {
    let mut q = ReactionsParams {
        chat: "saved".into(),
        message_id: 1,
        emoji: None,
        limit: 0,
        cursor: None,
        tz: None,
    };
    assert!(matches!(check_reactions(&q), Err(Fail::Usage(_))));
    q.limit = REACTIONS_MAX + 1;
    assert!(check_reactions(&q).is_err());
    q.limit = 1;
    assert!(check_reactions(&q).is_ok());
    q.chat = "abc".into();
    assert!(check_reactions(&q).is_err());
}

#[test]
fn react_over_my_other_reaction_needs_replace() {
    let b = backend();
    let mut swap = react_p("👍", false, false);
    swap.replace = true;
    let r = run(
        &b,
        &[
            react_p("❤️", false, false),
            react_p("👍", false, false),
            swap,
        ],
        None,
    );
    let refused = lines(&r[1].1).pop().unwrap();
    assert_eq!(
        refused["data"]["why"], "reaction_would_replace",
        "{refused}"
    );
    let ls = lines(&r[2].1);
    assert_eq!(r[2].0, 0, "{ls:?}");
    assert_eq!(ls[0]["data"]["replaces"][0]["emoji"], "❤️");
    assert_eq!(b.submitted().len(), 2);
}

#[test]
fn remove_and_replace_conflict_is_usage() {
    // 经本机接口转来的参数绕过了 clap 的 conflicts_with：这里照样拦
    let mut p = react_p("👍", true, false);
    p.replace = true;
    assert!(matches!(check_react(&p), Err(Fail::Usage(_))));
}
