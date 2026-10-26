//! `login` · `auth *` · `logout` 的流程（T01）。对任何 [`Backend`] 都一样，跟人说话经 [`Prompter`]
//! （正式是 `/dev/tty`，单测换成脚本）。所以假后端单测走到的就是正式的流程。
//!
//! 规约：`specs/contracts/cli.md`（`login` 交互、`auth *` 分步、`logout` 须终端确认）、`actions.md`「登录」、
//! `errors.md`（`auth_*`）、`testing.md` 第一节第 5 小节（测试号）。
//!
//! **秘密只走一条路**：号码、验证码、邮箱、密码由 [`Prompter::ask_secret`] 读进 [`Secret`]，直接交给后端，
//! 不进标准输出、不进日志、不进 `Debug`。标准输出（经 `emit`）只收授权步骤名；给人看的提示语、密码提示、
//! 出错原因只经 [`Prompter::say`] 写终端。
//!
//! 每一步提交之后等授权线**离开这一步**（最多 [`STEP_TIMEOUT`]）：提交成功只表示 TDLib 收下了，下一步是什么
//! 由授权状态更新说了算。测试档案上号码还没注册（测试服务器会定期清空）时自动注册（名字写死）。

use crate::out::Fail;
use crate::tty::ReadEnd;
use atg_core::auth;
use atg_core::backend::Backend;
use atg_core::config::ServerKind;
use atg_core::error::CoreError;
use atg_core::link::block_on;
use atg_core::secret::Secret;
use atg_core::status::AuthStep;
use serde_json::{Value, json};
use std::time::Duration;

/// 等授权线变化的上限：TDLib 起来、提交号码之后都要连服务器
pub const STEP_TIMEOUT: Duration = Duration::from_secs(30);

/// 交互登录里同一步最多输错几次就放弃（之后照常输出 `error` 行退出）
pub const MAX_TRIES: u32 = 3;

/// 跟人说话的那一侧
pub trait Prompter {
    /// 对人说一句。终端打不开时什么都不做
    fn say(&mut self, s: &str);
    /// 问一个秘密值，不回显
    fn ask_secret(&mut self, prompt: &str) -> Result<Secret, Fail>;
    /// 问一句是 / 否（回显）。只有回答 `yes` 才算是
    fn confirm(&mut self, prompt: &str) -> Result<bool, Fail>;
}

/// 读的时候人按了 Ctrl+C / Ctrl+D
#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
pub fn read_end(e: ReadEnd) -> Fail {
    match e {
        ReadEnd::Interrupted => Fail::Interrupted,
        ReadEnd::Cancelled => Fail::Core(CoreError::Cancelled),
    }
}

/// 一步的说法（占位措辞；`--format text` 与终端共用）。不带任何值
pub fn step_zh(step: AuthStep) -> &'static str {
    match step {
        AuthStep::WaitParameters => "正在启动",
        AuthStep::WaitPhone => "还没登录，需要手机号",
        AuthStep::WaitCode => "需要验证码",
        AuthStep::WaitEmail => "需要登录邮箱",
        AuthStep::WaitEmailCode => "需要邮箱里收到的验证码",
        AuthStep::WaitPassword => "需要二步验证密码",
        AuthStep::WaitRegistration => "这个号码还没注册",
        AuthStep::WaitOtherDevice => "需要在另一台设备上确认",
        AuthStep::Ready => "已登录",
        AuthStep::LoggingOut => "正在登出",
        AuthStep::Closing | AuthStep::Closed => "已关闭",
        AuthStep::Unsupported => crate::out::UNSUPPORTED_STEP_ZH,
    }
}

fn step_name(step: AuthStep) -> Value {
    serde_json::to_value(step).unwrap_or(Value::Null)
}

/// 等 TDLib 起来、授权线离开 `wait_parameters`
fn settle<B: Backend>(b: &B) -> Result<AuthStep, Fail> {
    match b.wait_auth(STEP_TIMEOUT, &|s| s != AuthStep::WaitParameters) {
        Some(s) => Ok(s),
        None => Err(stalled(b)),
    }
}

/// 授权线等不到变化：内核故障 → `disconnected`；否则当网络问题
fn stalled<B: Backend>(b: &B) -> Fail {
    if b.status().lines.core == atg_core::status::CoreState::Faulted {
        Fail::Core(CoreError::Disconnected)
    } else {
        Fail::Core(CoreError::Network { td: None })
    }
}

/// 提交成功后等下一条授权状态更新（`seen` = 提交之前的编号）。同一步重新下发也算（换号、重发验证码）
fn advance<B: Backend>(b: &B, seen: u64) -> Result<AuthStep, Fail> {
    b.wait_auth_update(STEP_TIMEOUT, seen)
        .ok_or_else(|| stalled(b))
}

/// 测试档案上要注册：自动注册并等下一步；正式档案 → 不支持
fn register<B: Backend>(b: &B, kind: ServerKind, p: &mut dyn Prompter) -> Result<AuthStep, Fail> {
    if kind != ServerKind::Test {
        return Err(Fail::Unsupported(AuthStep::WaitRegistration));
    }
    p.say("测试服务器上这个号码还没注册（测试服务器会定期清空），现在自动注册");
    let seen = b.auth_generation();
    block_on(b.auth_register()).map_err(Fail::Core)?;
    advance(b, seen)
}

/// 输入出错时多给人一句（只写终端）
fn explain(e: &CoreError, kind: ServerKind, p: &mut dyn Prompter) {
    p.say(e.message_zh());
    if matches!(e, CoreError::AuthPhoneNotForProfile) {
        p.say(match kind {
            ServerKind::Test => "测试档案只收 99966 开头的测试号",
            ServerKind::Main => "99966 开头的是测试号，正式档案不收；测试请加 --test-dc",
        });
    }
}

/// 人输错了、可以再输一次的错
fn human_retryable(e: &CoreError) -> bool {
    matches!(
        e,
        CoreError::AuthPhoneInvalid { .. }
            | CoreError::AuthCodeInvalid { .. }
            | CoreError::AuthCodeExpired { .. }
            | CoreError::AuthPasswordInvalid { .. }
            | CoreError::InvalidInput { .. }
            | CoreError::AuthPhoneNotForProfile
    )
}

/// 人对一步的回答
enum Answer {
    /// 交这个值
    Value(Secret),
    /// 输入了 `r`：换一种方式重发验证码
    Resend,
}

/// **一步怎么问**（交互 `login` 与分步 `auth *` 共用这一份）：先说这一步的提示，再读输入，再回一声「收到 N 个字符」
/// （读屏用户听不到回显；密码不报长度——旁边的人听得见）。只处理要人输入的五步
fn ask_step<B: Backend>(b: &B, step: AuthStep, p: &mut dyn Prompter) -> Result<Answer, Fail> {
    let prompt = b.auth_prompt();
    let (ask, ack): (&str, bool) = match step {
        AuthStep::WaitPhone => ("手机号（带国家码，如 +86 开头）：", true),
        AuthStep::WaitCode => {
            if let Some(via) = prompt.code_via {
                p.say(via.describe_zh());
            }
            if let Some(n) = prompt.code_length {
                p.say(&format!("验证码 {n} 位"));
            }
            if prompt.can_resend {
                ("验证码（收不到就输入 r 换一种方式重发）：", true)
            } else {
                ("验证码：", true)
            }
        }
        AuthStep::WaitEmail => ("登录邮箱：", true),
        AuthStep::WaitEmailCode => {
            if let Some(pattern) = prompt.email_pattern() {
                p.say(&format!("验证码已发到邮箱 {pattern}"));
            }
            if let Some(n) = prompt.code_length {
                p.say(&format!("验证码 {n} 位"));
            }
            ("邮箱里的验证码：", true)
        }
        AuthStep::WaitPassword => {
            if let Some(hint) = prompt.password_hint() {
                p.say(&format!("密码提示：{hint}"));
            }
            ("二步验证密码：", false)
        }
        _ => return Err(Fail::Core(CoreError::AuthWrongState { expected: step })),
    };
    let raw = p.ask_secret(ask)?;
    // 密码原样交（空格也算密码的一部分）；其余去掉空白
    let v = if step == AuthStep::WaitPassword {
        raw
    } else {
        auth::strip_spaces(raw.expose())
    };
    if step == AuthStep::WaitCode && prompt.can_resend && v.expose().eq_ignore_ascii_case(b"r") {
        return Ok(Answer::Resend);
    }
    if ack {
        let n = v.expose_str().chars().count();
        p.say(&format!("收到 {n} 个字符"));
    }
    Ok(Answer::Value(v))
}

/// 把一步的值交给后端
fn submit<B: Backend>(b: &B, step: AuthStep, v: &Secret) -> Result<(), CoreError> {
    match step {
        AuthStep::WaitPhone => block_on(b.auth_submit_phone(v)),
        AuthStep::WaitCode => block_on(b.auth_submit_code(v)),
        AuthStep::WaitEmail => block_on(b.auth_submit_email(v)),
        AuthStep::WaitEmailCode => block_on(b.auth_submit_email_code(v)),
        AuthStep::WaitPassword => block_on(b.auth_submit_password(v)),
        _ => Err(CoreError::AuthWrongState { expected: step }),
    }
}

/// 交互登录：从当前步骤一直走到 `ready`。走到新的一步调一次 `on_step`（标准输出那一行；文字模式下就是
/// 那一步的说法，所以终端这边不再重复说步骤，只说提示与出错原因）。返回 `result.data`
pub fn login<B: Backend>(
    b: &B,
    kind: ServerKind,
    p: &mut dyn Prompter,
    on_step: &mut dyn FnMut(AuthStep),
) -> Result<Value, Fail> {
    if kind == ServerKind::Main && std::env::var_os("TMUX").is_some() {
        p.say("提醒：你在 tmux 里。tmux 窗格的内容能被别的程序读走，正式号最好在 tmux 以外的终端里登录");
    }
    let mut step = settle(b)?;
    on_step(step);
    let mut tries = 0u32;
    loop {
        match step {
            AuthStep::Ready => return Ok(json!({ "command": "login", "step": step_name(step) })),
            AuthStep::WaitRegistration => {
                step = register(b, kind, p)?;
                on_step(step);
                tries = 0;
                continue;
            }
            AuthStep::WaitOtherDevice | AuthStep::Unsupported => {
                return Err(Fail::Unsupported(step));
            }
            AuthStep::WaitParameters
            | AuthStep::LoggingOut
            | AuthStep::Closing
            | AuthStep::Closed => return Err(Fail::Core(CoreError::Disconnected)),
            AuthStep::WaitPhone
            | AuthStep::WaitCode
            | AuthStep::WaitEmail
            | AuthStep::WaitEmailCode
            | AuthStep::WaitPassword => {}
        }
        let answer = ask_step(b, step, p)?;
        let seen = b.auth_generation();
        let r = match &answer {
            Answer::Resend => block_on(b.auth_resend_code()),
            Answer::Value(v) => submit(b, step, v),
        };
        match r {
            Ok(()) => {
                if matches!(answer, Answer::Resend) {
                    p.say("已请求重发");
                }
                let next = advance(b, seen)?;
                // 重发之后还是等验证码：标准输出不再重复这一步
                if next != step {
                    on_step(next);
                    tries = 0;
                }
                step = next;
            }
            Err(e) if human_retryable(&e) => {
                explain(&e, kind, p);
                tries += 1;
                if tries >= MAX_TRIES {
                    return Err(Fail::Core(e));
                }
                // 出错时步骤可能也变了（如验证码过期后 TDLib 退回等号码）
                let now = b.auth_state();
                if now != step {
                    on_step(now);
                    tries = 0;
                }
                step = now;
            }
            Err(e) => return Err(Fail::Core(e)),
        }
    }
}

/// `auth` 的分步子命令
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AuthSub {
    State,
    Phone,
    Code,
    Password,
    Email,
    EmailCode,
    Resend,
}

impl AuthSub {
    pub fn name(self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Phone => "phone",
            Self::Code => "code",
            Self::Password => "password",
            Self::Email => "email",
            Self::EmailCode => "email-code",
            Self::Resend => "resend",
        }
    }

    /// 这一子命令交的是哪一步的值（`state`、`resend` 不交值）
    fn step(self) -> Option<AuthStep> {
        match self {
            Self::Phone => Some(AuthStep::WaitPhone),
            Self::Code => Some(AuthStep::WaitCode),
            Self::Password => Some(AuthStep::WaitPassword),
            Self::Email => Some(AuthStep::WaitEmail),
            Self::EmailCode => Some(AuthStep::WaitEmailCode),
            Self::State | Self::Resend => None,
        }
    }
}

/// 分步登录的一步：交一项输入 → 等下一条授权状态 → 输出一行 `auth`（只有步骤名）。出错不重试，直接是结尾行。
/// `test_phone`：测试档案上 `.env.local` 的 `ATG_TEST_PHONE`（有就不问人，验证码按测试号规则推出）
pub fn auth_step<B: Backend>(
    b: &B,
    kind: ServerKind,
    sub: AuthSub,
    test_phone: Option<&Secret>,
    p: &mut dyn Prompter,
    on_step: &mut dyn FnMut(AuthStep),
) -> Result<Value, Fail> {
    let test_phone = test_phone.filter(|_| kind == ServerKind::Test);
    let step = settle(b)?;
    let done = |s: AuthStep| json!({ "command": "auth", "sub": sub.name(), "step": step_name(s) });
    let seen = b.auth_generation();
    match sub.step() {
        None if sub == AuthSub::State => {
            on_step(step);
            return Ok(done(step));
        }
        None => block_on(b.auth_resend_code()).map_err(Fail::Core)?,
        Some(target) => {
            // 当前步骤不收这项输入：在问人之前就报，不白问
            auth::AuthAction::for_step(target)
                .check(step)
                .map_err(Fail::Core)?;
            let preset = match target {
                AuthStep::WaitPhone => test_phone.map(|t| Secret::new(t.expose().to_vec())),
                AuthStep::WaitCode => test_phone.and_then(|t| auth::test_code_for(t.expose())),
                _ => None,
            };
            let v = match preset {
                Some(v) => v,
                None => match ask_step(b, target, p)? {
                    Answer::Value(v) => v,
                    // 分步模式重发走 `auth resend`；这里输入的 r 就当验证码交（会报验证码不对）
                    Answer::Resend => Secret::new(b"r".to_vec()),
                },
            };
            submit(b, target, &v).map_err(Fail::Core)?;
        }
    }
    let mut next = advance(b, seen)?;
    if next == AuthStep::WaitRegistration && kind == ServerKind::Test {
        next = register(b, kind, p)?;
    }
    on_step(next);
    Ok(done(next))
}

/// 登出：没登录就直接说；否则终端当场确认（回答 `yes`）→ `logOut` → 等到客户端关掉
pub fn logout<B: Backend>(
    b: &B,
    p: &mut dyn Prompter,
    on_step: &mut dyn FnMut(AuthStep),
) -> Result<Value, Fail> {
    let step = settle(b)?;
    if step == AuthStep::WaitPhone {
        on_step(step);
        return Ok(
            json!({ "command": "logout", "step": step_name(step), "already_signed_out": true }),
        );
    }
    let yes = p.confirm(
        "确认登出这台电脑上的 Telegram？本地缓存会被清空，下次要重新登录。输入 yes 确认：",
    )?;
    if !yes {
        return Err(Fail::Core(CoreError::Cancelled));
    }
    block_on(b.logout()).map_err(Fail::Core)?;
    let end = b
        .wait_auth(STEP_TIMEOUT, &|s| s == AuthStep::Closed)
        .ok_or_else(|| stalled(b))?;
    on_step(end);
    Ok(json!({ "command": "logout", "step": step_name(end), "already_signed_out": false }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{self, Job};
    use atg_core::config::Profile;
    use atg_core::error::Holder;
    use atg_core::fake::{Call, FakeAuth, FakeBackend};
    use std::collections::VecDeque;
    use std::path::PathBuf;

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
        (b, p)
    }

    /// 跑交互登录，收集标准输出那一侧的步骤
    fn run_login(
        b: &FakeBackend,
        kind: ServerKind,
        s: &mut Script,
    ) -> (Result<Value, Fail>, Vec<AuthStep>) {
        let mut steps = Vec::new();
        let r = login(b, kind, s, &mut |st| steps.push(st));
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

    #[test]
    fn phone_then_code_reaches_ready() {
        let (b, _p) = started("basic", ServerKind::Test, account());
        let mut s = Script::new(&[PHONE, CODE]);
        let (r, steps) = run_login(&b, ServerKind::Test, &mut s);
        let data = r.unwrap_or_else(|_| panic!("登录失败"));
        assert_eq!(data["command"], "login");
        assert_eq!(data["step"], "ready");
        assert_eq!(steps, [S::WaitPhone, S::WaitCode, S::Ready]);
        assert_eq!(auth_calls(&b), ["phone", "code"]);
        assert!(
            s.said("验证码已发到你已登录的 Telegram 应用里"),
            "{:?}",
            s.said
        );
        assert!(s.said("验证码 5 位"));
        assert!(s.said("收到 5 个字符"));
    }

    #[test]
    fn password_branch_shows_hint_only_on_terminal() {
        let a = FakeAuth {
            password: Some((PASSWORD.into(), HINT.into())),
            ..account()
        };
        let (b, _p) = started("pw", ServerKind::Test, a);
        let mut s = Script::new(&[PHONE, CODE, PASSWORD]);
        let (r, steps) = run_login(&b, ServerKind::Test, &mut s);
        assert!(r.is_ok());
        assert_eq!(
            steps,
            [S::WaitPhone, S::WaitCode, S::WaitPassword, S::Ready]
        );
        assert!(s.said(&format!("密码提示：{HINT}")), "提示语要写到终端");
        // 密码长度不读出来（旁边的人听得见）
        assert!(!s.said(&format!("收到 {} 个字符", PASSWORD.chars().count())));
    }

    #[test]
    fn email_branch() {
        let a = FakeAuth {
            email: Some((EMAIL.into(), EMAIL_CODE.into())),
            ..account()
        };
        let (b, _p) = started("email", ServerKind::Test, a);
        let mut s = Script::new(&[PHONE, EMAIL, EMAIL_CODE]);
        let (r, steps) = run_login(&b, ServerKind::Test, &mut s);
        assert!(r.is_ok());
        assert_eq!(
            steps,
            [S::WaitPhone, S::WaitEmail, S::WaitEmailCode, S::Ready]
        );
        assert_eq!(auth_calls(&b), ["phone", "email", "email_code"]);
        assert!(s.said("验证码已发到邮箱 e***@example.com"));
        assert!(s.said("验证码 6 位"));
    }

    #[test]
    fn wrong_email_code_is_readable() {
        let a = FakeAuth {
            email: Some((EMAIL.into(), EMAIL_CODE.into())),
            ..account()
        };
        let (b, _p) = started("email-bad", ServerKind::Test, a);
        let mut s = Script::new(&[PHONE, EMAIL, "000000", EMAIL_CODE]);
        let (r, _) = run_login(&b, ServerKind::Test, &mut s);
        assert!(r.is_ok());
        assert!(s.said("验证码不对，请重新输入"));
    }

    #[test]
    fn registration_on_test_profile_is_automatic() {
        let a = FakeAuth {
            needs_registration: true,
            ..account()
        };
        let (b, _p) = started("reg", ServerKind::Test, a);
        let mut s = Script::new(&[PHONE, CODE]);
        let (r, steps) = run_login(&b, ServerKind::Test, &mut s);
        assert!(r.is_ok());
        assert_eq!(
            steps,
            [S::WaitPhone, S::WaitCode, S::WaitRegistration, S::Ready]
        );
        assert_eq!(auth_calls(&b), ["phone", "code", "register"]);
    }

    #[test]
    fn registration_on_main_profile_is_unsupported() {
        let a = FakeAuth {
            phone: "+86 138 0000 0000".into(),
            needs_registration: true,
            ..account()
        };
        let (b, _p) = started("reg-main", ServerKind::Main, a);
        let mut s = Script::new(&["+86 138 0000 0000", CODE]);
        let (r, _) = run_login(&b, ServerKind::Main, &mut s);
        assert!(matches!(r, Err(Fail::Unsupported(S::WaitRegistration))));
        assert!(!auth_calls(&b).contains(&"register"));
    }

    #[test]
    fn wrong_code_then_right_one() {
        let (b, _p) = started("code-retry", ServerKind::Test, account());
        let mut s = Script::new(&[PHONE, "11111", CODE]);
        let (r, steps) = run_login(&b, ServerKind::Test, &mut s);
        assert!(r.is_ok());
        assert_eq!(steps, [S::WaitPhone, S::WaitCode, S::Ready]);
        assert!(s.said("验证码不对，请重新输入"));
        assert_eq!(auth_calls(&b), ["phone", "code", "code"]);
    }

    #[test]
    fn three_wrong_codes_end_with_readable_error() {
        let (b, _p) = started("code-3", ServerKind::Test, account());
        let mut s = Script::new(&[PHONE, "1", "2", "3"]);
        let (r, _) = run_login(&b, ServerKind::Test, &mut s);
        let e = core(r);
        assert_eq!(e.code(), "auth_code_invalid");
        assert_eq!(e.message_zh(), "验证码不对，请重新输入");
        assert_eq!(b.auth_state(), S::WaitCode, "步骤不变");
    }

    #[test]
    fn expired_code_is_explained() {
        let (b, _p) = started("expired", ServerKind::Test, account());
        b.fail_next(
            "code",
            atg_core::error::map_td_error(400, "PHONE_CODE_EXPIRED"),
        );
        let mut s = Script::new(&[PHONE, CODE, CODE]);
        let (r, _) = run_login(&b, ServerKind::Test, &mut s);
        assert!(r.is_ok());
        assert!(s.said("验证码已过期，请重发验证码"));
    }

    #[test]
    fn three_wrong_passwords() {
        let a = FakeAuth {
            password: Some((PASSWORD.into(), HINT.into())),
            ..account()
        };
        let (b, _p) = started("pw-3", ServerKind::Test, a);
        let mut s = Script::new(&[PHONE, CODE, "x", "y", "z"]);
        let (r, _) = run_login(&b, ServerKind::Test, &mut s);
        assert_eq!(core(r).code(), "auth_password_invalid");
        assert!(s.said("二步验证密码不对，请重新输入"));
    }

    #[test]
    fn bad_phone_is_caught_locally_and_retried() {
        let (b, _p) = started("phone-bad", ServerKind::Test, account());
        // 形状不对 → 本地拦下；正式号 → 测试档案不收；最后给对的
        let mut s = Script::new(&["abc", "+86 138 0000 0000", PHONE, CODE]);
        let (r, _) = run_login(&b, ServerKind::Test, &mut s);
        assert!(r.is_ok());
        assert!(s.said("手机号格式不对"));
        assert!(s.said("测试档案只收 99966 开头的测试号"));
        // 本地拦下的两次没交给「TDLib」
        assert_eq!(auth_calls(&b), ["phone", "code"]);
    }

    #[test]
    fn phone_for_wrong_profile_ends_with_its_own_code() {
        let (b, _p) = started("phone-profile", ServerKind::Test, account());
        let n = "+86 138 0000 0000";
        let mut s = Script::new(&[n, n, n]);
        let (r, _) = run_login(&b, ServerKind::Test, &mut s);
        let e = core(r);
        assert_eq!(e.code(), "auth_phone_not_for_profile");
        assert_eq!(crate::out::core_exit(&e), 3);
        let d = crate::out::core_error_data(&e);
        assert!(d.get("td").is_none(), "本地拦下，没有 td 附件");
        assert!(auth_calls(&b).is_empty(), "没交给「TDLib」");
        // 正式档案收到测试号同样
        let a = FakeAuth {
            phone: "+86 138 0000 0000".into(),
            ..account()
        };
        let (b, _p) = started("phone-profile-main", ServerKind::Main, a);
        let mut s = Script::new(&[PHONE, PHONE, PHONE]);
        let (r, _) = run_login(&b, ServerKind::Main, &mut s);
        assert_eq!(core(r).code(), "auth_phone_not_for_profile");
        assert!(s.said("99966 开头的是测试号，正式档案不收；测试请加 --test-dc"));
    }

    #[test]
    fn unknown_phone_is_readable() {
        let (b, _p) = started("phone-unknown", ServerKind::Test, account());
        let mut s = Script::new(&["+99966 2 0000", "+99966 2 0001", "+99966 2 0002"]);
        let (r, _) = run_login(&b, ServerKind::Test, &mut s);
        assert_eq!(core(r).code(), "auth_phone_invalid");
    }

    #[test]
    fn resend_then_code() {
        let (b, _p) = started("resend", ServerKind::Test, account());
        let mut s = Script::new(&[PHONE, "r", CODE]);
        let (r, _) = run_login(&b, ServerKind::Test, &mut s);
        assert!(r.is_ok());
        assert_eq!(auth_calls(&b), ["phone", "resend", "code"]);
        assert!(s.said("已请求重发"));
        assert!(s.said("验证码已用短信发出"), "{:?}", s.said);
        // 重发过一次后没有下一种方式了，不再提示可以输入 r
        assert!(s.asked.last().is_some_and(|a| a == "验证码："));
    }

    #[test]
    fn already_logged_in_asks_nothing() {
        let a = FakeAuth {
            logged_in: true,
            ..account()
        };
        let (b, _p) = started("ready", ServerKind::Test, a);
        let mut s = Script::new(&[]);
        let (r, steps) = run_login(&b, ServerKind::Test, &mut s);
        assert!(r.is_ok());
        assert_eq!(steps, [S::Ready]);
        assert!(s.asked.is_empty());
    }

    #[test]
    fn ctrl_c_and_ctrl_d() {
        let (b, _p) = started("ctrl-c", ServerKind::Test, account());
        let mut s = Script::default();
        s.answers.push_back(Err(read_end(ReadEnd::Interrupted)));
        let (r, _) = run_login(&b, ServerKind::Test, &mut s);
        assert!(matches!(r, Err(Fail::Interrupted)));
        let mut s = Script::default();
        s.answers.push_back(Err(read_end(ReadEnd::Cancelled)));
        let (r, _) = run_login(&b, ServerKind::Test, &mut s);
        assert_eq!(core(r).code(), "cancelled");
    }

    #[test]
    fn core_fault_mid_login_is_disconnected() {
        let (b, _p) = started("fault", ServerKind::Test, account());
        assert_eq!(
            b.wait_auth(STEP_TIMEOUT, &|s| s == S::WaitPhone),
            Some(S::WaitPhone)
        );
        b.crash();
        b.join_receiver();
        let mut s = Script::new(&[PHONE]);
        let (r, _) = run_login(&b, ServerKind::Test, &mut s);
        assert_eq!(core(r).code(), "disconnected");
    }

    #[test]
    fn unsupported_step_is_reported() {
        let (b, _p) = started("other-device", ServerKind::Test, account());
        assert!(b.wait_auth(STEP_TIMEOUT, &|s| s == S::WaitPhone).is_some());
        b.push_auth("authorizationStateWaitOtherDeviceConfirmation");
        assert!(
            b.wait_auth(STEP_TIMEOUT, &|s| s == S::WaitOtherDevice)
                .is_some()
        );
        let mut s = Script::new(&[]);
        let (r, _) = run_login(&b, ServerKind::Test, &mut s);
        assert!(matches!(r, Err(Fail::Unsupported(S::WaitOtherDevice))));
    }

    // ───────────── 分步 ─────────────

    fn step(
        b: &FakeBackend,
        kind: ServerKind,
        sub: AuthSub,
        test_phone: Option<&str>,
        s: &mut Script,
    ) -> (Result<Value, Fail>, Vec<AuthStep>) {
        let tp = test_phone.map(|t| Secret::new(t.as_bytes().to_vec()));
        let mut steps = Vec::new();
        let r = auth_step(b, kind, sub, tp.as_ref(), s, &mut |st| steps.push(st));
        (r, steps)
    }

    #[test]
    fn stepwise_with_test_phone_asks_nothing() {
        // 测试号 99966 2 YYYY：验证码 22222
        let a = FakeAuth {
            phone: "9996621234".into(),
            code: "22222".into(),
            ..FakeAuth::default()
        };
        let (b, _p) = started("step", ServerKind::Test, a);
        let mut s = Script::new(&[]);
        let (r, steps) = step(&b, ServerKind::Test, AuthSub::State, None, &mut s);
        assert_eq!(r.ok().unwrap()["step"], "wait_phone");
        assert_eq!(steps, [S::WaitPhone]);
        let (r, steps) = step(
            &b,
            ServerKind::Test,
            AuthSub::Phone,
            Some("9996621234"),
            &mut s,
        );
        let d = r.ok().unwrap();
        assert_eq!(
            (d["command"].as_str(), d["sub"].as_str()),
            (Some("auth"), Some("phone"))
        );
        assert_eq!(steps, [S::WaitCode]);
        let (r, steps) = step(
            &b,
            ServerKind::Test,
            AuthSub::Code,
            Some("9996621234"),
            &mut s,
        );
        assert!(r.is_ok());
        assert_eq!(steps, [S::Ready]);
        assert!(s.asked.is_empty(), "测试号不问人");
    }

    #[test]
    fn stepwise_errors_are_final() {
        let (b, _p) = started("step-err", ServerKind::Test, account());
        // 等号码时交验证码 → auth_wrong_state，带当前步骤；没交给「TDLib」
        let (r, _) = step(
            &b,
            ServerKind::Test,
            AuthSub::Code,
            None,
            &mut Script::new(&[CODE]),
        );
        let e = core(r);
        assert_eq!(e.code(), "auth_wrong_state");
        assert_eq!(
            e,
            CoreError::AuthWrongState {
                expected: S::WaitPhone
            }
        );
        assert!(auth_calls(&b).is_empty());
        let (r, _) = step(
            &b,
            ServerKind::Test,
            AuthSub::Phone,
            None,
            &mut Script::new(&[PHONE]),
        );
        assert!(r.is_ok());
        let (r, _) = step(
            &b,
            ServerKind::Test,
            AuthSub::Code,
            None,
            &mut Script::new(&["99999"]),
        );
        assert_eq!(core(r).code(), "auth_code_invalid");
        assert_eq!(b.auth_state(), S::WaitCode);
    }

    #[test]
    fn stepwise_main_profile_ignores_test_phone() {
        let a = FakeAuth {
            phone: "+86 138 0000 0000".into(),
            ..account()
        };
        let (b, _p) = started("step-main", ServerKind::Main, a);
        let mut s = Script::new(&["+86 138 0000 0000"]);
        let (r, _) = step(
            &b,
            ServerKind::Main,
            AuthSub::Phone,
            Some("9996621234"),
            &mut s,
        );
        assert!(r.is_ok());
        assert_eq!(s.asked.len(), 1, "正式档案上号码要问人");
    }

    #[test]
    fn stepwise_registers_on_test_profile() {
        let a = FakeAuth {
            phone: "9996621234".into(),
            code: "22222".into(),
            needs_registration: true,
            ..FakeAuth::default()
        };
        let (b, _p) = started("step-reg", ServerKind::Test, a);
        let mut s = Script::new(&[]);
        let tp = Some("9996621234");
        assert!(
            step(&b, ServerKind::Test, AuthSub::Phone, tp, &mut s)
                .0
                .is_ok()
        );
        let (r, steps) = step(&b, ServerKind::Test, AuthSub::Code, tp, &mut s);
        assert!(r.is_ok());
        assert_eq!(steps, [S::Ready]);
        assert_eq!(auth_calls(&b), ["phone", "code", "register"]);
    }

    /// 等验证码时换号重交（TDLib 允许）：下一条还是 `wait_code`，步骤名不变也要算成功，不能等 30 秒报 `network`
    #[test]
    fn stepwise_phone_again_at_wait_code() {
        let (b, _p) = started("step-rephone", ServerKind::Test, account());
        assert!(
            step(
                &b,
                ServerKind::Test,
                AuthSub::Phone,
                None,
                &mut Script::new(&[PHONE])
            )
            .0
            .is_ok()
        );
        let t0 = std::time::Instant::now();
        let (r, steps) = step(
            &b,
            ServerKind::Test,
            AuthSub::Phone,
            None,
            &mut Script::new(&[PHONE]),
        );
        assert!(r.is_ok());
        assert_eq!(steps, [S::WaitCode]);
        assert!(t0.elapsed() < Duration::from_secs(5), "不该等到超时");
        assert_eq!(auth_calls(&b), ["phone", "phone"]);
    }

    #[test]
    fn stepwise_wrong_state_is_reported_before_asking() {
        let (b, _p) = started("step-early", ServerKind::Test, account());
        let mut s = Script::new(&[]);
        let (r, _) = step(&b, ServerKind::Test, AuthSub::Password, None, &mut s);
        assert_eq!(core(r).code(), "auth_wrong_state");
        assert!(s.asked.is_empty(), "不白问");
    }

    #[test]
    fn stepwise_resend() {
        let (b, _p) = started("step-resend", ServerKind::Test, account());
        let mut s = Script::new(&[PHONE]);
        assert!(
            step(&b, ServerKind::Test, AuthSub::Phone, None, &mut s)
                .0
                .is_ok()
        );
        let (r, steps) = step(&b, ServerKind::Test, AuthSub::Resend, None, &mut s);
        assert!(r.is_ok());
        assert_eq!(steps, [S::WaitCode]);
        assert_eq!(auth_calls(&b), ["phone", "resend"]);
    }

    // ───────────── 登出 ─────────────

    #[test]
    fn logout_needs_yes() {
        let a = FakeAuth {
            logged_in: true,
            ..account()
        };
        let (b, _p) = started("logout-no", ServerKind::Test, a.clone());
        assert!(b.wait_auth(STEP_TIMEOUT, &|s| s == S::Ready).is_some());
        let r = logout(&b, &mut Script::new(&["no"]), &mut |_| {});
        assert_eq!(core(r).code(), "cancelled");
        assert!(!auth_calls(&b).contains(&"logout"));

        let (b, _p) = started("logout-yes", ServerKind::Test, a);
        let mut steps = Vec::new();
        let r = logout(&b, &mut Script::new(&["yes"]), &mut |s| steps.push(s));
        let d = r.ok().unwrap();
        assert_eq!(d["step"], "closed");
        assert_eq!(d["already_signed_out"], false);
        assert_eq!(steps, [S::Closed]);
        assert_eq!(auth_calls(&b), ["logout"]);
        // 关掉之后 shutdown 立即返回
        assert!(b.shutdown(Duration::from_secs(2)).is_ok());
    }

    #[test]
    fn logout_when_signed_out_asks_nothing() {
        let (b, _p) = started("logout-already", ServerKind::Test, account());
        let mut s = Script::new(&[]);
        let d = logout(&b, &mut s, &mut |_| {}).ok().unwrap();
        assert_eq!(d["already_signed_out"], true);
        assert!(s.asked.is_empty());
    }

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
        let r = session::drive(&b, &p, Job::Login, &mut Script::new(&[]), &mut |_| {});
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
        let r = login(&b, ServerKind::Test, &mut s, &mut |st| {
            out.push(serde_json::to_string(&json!({ "step": st })).unwrap())
        });
        let data = r.ok().unwrap();
        out.push(data.to_string());
        // 出错路径的结尾行
        let (b2, _p2) = started("anhao-err", ServerKind::Test, account());
        let mut s2 = Script::new(&[PHONE, PASSWORD, HINT, EMAIL]);
        let e = core(login(&b2, ServerKind::Test, &mut s2, &mut |_| {}));
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
}
