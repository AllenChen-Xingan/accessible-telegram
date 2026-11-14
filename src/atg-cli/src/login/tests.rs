//! 登录流程的单测：公共夹具在这里，按主题分到 `tests/` 下（交互 `login`、分步 `auth *` 与登出、期限与 Ctrl+C）
use super::*;
use crate::session::{self, Job};
use atg_core::config::Profile;
use atg_core::error::Holder;
use atg_core::fake::{Call, FakeAuth, FakeBackend};
use atg_core::link::block_on;
use std::collections::VecDeque;
use std::path::PathBuf;

/// 这些测试不按 Ctrl+C（按的那几条在 `tests/bounded.rs`，用自己的标志）
static NEVER: AtomicBool = AtomicBool::new(false);

// 暗号：这些值绝不许出现在标准输出那一侧（步骤行、结尾行）和记账 / Debug 里
const PHONE: &str = "+99966 2 7731";
const PHONE_DIGITS: &str = "9996627731";
const CODE: &str = "40419";
const PASSWORD: &str = "暗号-pw-5521";
const HINT: &str = "暗号-hint-6630";
const EMAIL: &str = "anhao-mail@example.com";
const EMAIL_CODE: &str = "818283";

/// 脚本化的「终端」：按顺序给答案，记下说过的话
#[derive(Default)]
struct Script {
    answers: VecDeque<Result<String, Fail>>,
    said: Vec<String>,
    asked: Vec<String>,
}

impl Script {
    fn new(answers: &[&str]) -> Self {
        Self {
            answers: answers.iter().map(|a| Ok((*a).to_owned())).collect(),
            ..Self::default()
        }
    }
    fn said(&self, s: &str) -> bool {
        self.said.iter().any(|x| x.contains(s))
    }
}

impl Prompter for Script {
    fn say(&mut self, s: &str) {
        self.said.push(s.to_owned());
    }
    fn ask_secret(&mut self, prompt: &str) -> Result<Secret, Fail> {
        self.asked.push(prompt.to_owned());
        match self.answers.pop_front() {
            Some(Ok(a)) => Ok(Secret::new(a.into_bytes())),
            Some(Err(f)) => Err(f),
            None => panic!("脚本答案用完了：{prompt}"),
        }
    }
    fn confirm(&mut self, prompt: &str) -> Result<bool, Fail> {
        self.ask_secret(prompt).map(|a| a.expose() == b"yes")
    }
}

fn temp_root(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "atg-cli-login-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn account() -> FakeAuth {
    FakeAuth {
        phone: PHONE.into(),
        code: CODE.into(),
        ..FakeAuth::default()
    }
}

/// 起一个带假账号的假后端（测试档案或正式档案）
fn started(tag: &str, kind: ServerKind, a: FakeAuth) -> (FakeBackend, Profile) {
    let root = temp_root(tag);
    let name = if kind == ServerKind::Test {
        "test"
    } else {
        "main"
    };
    let p = Profile::create(&root, name, kind).unwrap();
    let b = FakeBackend::new().unwrap();
    b.set_auth(a);
    drop(b.subscribe());
    block_on(b.start(&p)).unwrap();
    // 起完之后授权状态还可能在路上：等它离开「等参数」再交出去，直接调单步的测试才不撞上
    let settled = b.wait_auth(Duration::from_secs(5), &|s| s != S::WaitParameters);
    assert!(settled.is_some_and(|s| s != S::WaitParameters));
    (b, p)
}

/// 跑交互登录，收集标准输出那一侧的步骤
fn run_login(
    b: &FakeBackend,
    kind: ServerKind,
    s: &mut Script,
) -> (Result<Value, Fail>, Vec<AuthStep>) {
    let mut steps = Vec::new();
    let r = login(b, kind, s, &mut |st| steps.push(st), &NEVER);
    (r, steps)
}

fn core(r: Result<Value, Fail>) -> CoreError {
    match r {
        Err(Fail::Core(e)) => e,
        Err(_) => panic!("不是 CoreError"),
        Ok(_) => panic!("没出错"),
    }
}

fn auth_calls(b: &FakeBackend) -> Vec<&'static str> {
    b.calls()
        .into_iter()
        .filter_map(|c| match c {
            Call::Auth(a) => Some(a),
            _ => None,
        })
        .collect()
}

use AuthStep as S;

mod bounded;
mod interactive;
mod stepwise;

// ───────────── 起停（session::drive） ─────────────

#[test]
fn drive_starts_runs_and_shuts_down() {
    let root = temp_root("drive");
    let p = Profile::create(&root, "test", ServerKind::Test).unwrap();
    let b = FakeBackend::new().unwrap();
    b.set_auth(account());
    let mut steps = Vec::new();
    let r = session::drive(
        &b,
        &p,
        Job::Login,
        &mut Script::new(&[PHONE, CODE]),
        &mut |s| steps.push(s),
        &NEVER,
    );
    assert!(r.is_ok());
    assert_eq!(steps.last(), Some(&S::Ready));
    let calls = b.calls();
    assert!(matches!(calls.first(), Some(Call::Subscribe)));
    assert!(matches!(calls.last(), Some(Call::Shutdown)));
    // 正常关闭后档案锁放掉了
    assert!(p.lock(Holder::Cli).is_ok());
}

#[test]
fn drive_reports_start_failure_and_still_shuts_down() {
    let root = temp_root("drive-fail");
    let p = Profile::create(&root, "test", ServerKind::Test).unwrap();
    let b = FakeBackend::new().unwrap();
    b.set_start(atg_core::fake::FakeStart::Fail(CoreError::DbKeyInvalid {
        source: atg_core::error::KeySource::WrongKey,
    }));
    let r = session::drive(
        &b,
        &p,
        Job::Login,
        &mut Script::new(&[]),
        &mut |_| {},
        &NEVER,
    );
    assert_eq!(core(r).code(), "db_key_invalid");
    assert!(matches!(b.calls().last(), Some(Call::Shutdown)));
}

// ───────────── 暗号 ─────────────

/// 每条分支都跑一遍，把标准输出那一侧会出现的东西（步骤行、结尾行、错误行）和记账、`Debug` 都收起来，
/// 暗号一次都不许出现；终端那一侧应当看得到提示语（正向对照，防止测试空转）
#[test]
fn secrets_never_reach_stdout_side_or_debug() {
    let a = FakeAuth {
        email: Some((EMAIL.into(), EMAIL_CODE.into())),
        password: Some((PASSWORD.into(), HINT.into())),
        ..account()
    };
    let (b, _p) = started("anhao", ServerKind::Test, a);
    let mut s = Script::new(&[PHONE, EMAIL, "000000", EMAIL_CODE, "wrong-暗号", PASSWORD]);
    let mut out: Vec<String> = Vec::new();
    let r = login(
        &b,
        ServerKind::Test,
        &mut s,
        &mut |st| out.push(serde_json::to_string(&json!({ "step": st })).unwrap()),
        &NEVER,
    );
    let data = r.ok().unwrap();
    out.push(data.to_string());
    // 出错路径的结尾行
    let (b2, _p2) = started("anhao-err", ServerKind::Test, account());
    let mut s2 = Script::new(&[PHONE, PASSWORD, HINT, EMAIL]);
    let e = core(login(&b2, ServerKind::Test, &mut s2, &mut |_| {}, &NEVER));
    out.push(crate::out::core_error_data(&e).to_string());
    out.push(format!("{:?} {:?} {e:?}", b.calls(), b2.calls()));
    out.push(format!("{:?} {:?}", b.auth_prompt(), b.status()));
    let all = out.join("\n");
    for marker in [
        PHONE,
        PHONE_DIGITS,
        "7731",
        CODE,
        PASSWORD,
        HINT,
        EMAIL,
        EMAIL_CODE,
        "暗号",
    ] {
        assert!(!all.contains(marker), "{marker} 漏到了：{all}");
    }
    assert!(s.said(HINT), "提示语应写到终端（正向对照）");
}
