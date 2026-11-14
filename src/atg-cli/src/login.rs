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
//! 每一步提交之后等授权线**离开这一步**：提交成功只表示 TDLib 收下了，下一步是什么由授权状态更新说了算。
//! 提交与等授权线共用一个截止时刻（[`STEP_TIMEOUT`]），过了报 `network`；途中按 Ctrl+C 报 `interrupted`，
//! 照常收尾（#63）。测试档案上号码还没注册（测试服务器会定期清空）时自动注册（名字写死）。

use crate::deadlines::{AUTH_POLL, STEP_TIMEOUT, stalled, within};
use crate::out::Fail;
use crate::tty::ReadEnd;
use atg_core::auth;
use atg_core::backend::Backend;
use atg_core::config::ServerKind;
use atg_core::error::CoreError;
use atg_core::secret::Secret;
use atg_core::status::AuthStep;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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

/// 等授权线：按剩余时间切片（每片最多 [`AUTH_POLL`]），片间看 Ctrl+C。`wait` 收一片的时长
fn wait_line<B: Backend>(
    b: &B,
    deadline: Instant,
    stop: &AtomicBool,
    wait: impl Fn(Duration) -> Option<AuthStep>,
) -> Result<AuthStep, Fail> {
    loop {
        if stop.load(Ordering::SeqCst) {
            return Err(Fail::Interrupted);
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(Fail::Core(stalled(b)));
        }
        if let Some(s) = wait(left.min(AUTH_POLL)) {
            // 等的那一片里按了 Ctrl+C、新状态又同时到了：照样算打断，不往下问人（codex 022 第 2 轮 #1）
            if stop.load(Ordering::SeqCst) {
                return Err(Fail::Interrupted);
            }
            return Ok(s);
        }
    }
}

/// 等 TDLib 起来、授权线离开 `wait_parameters`
fn settle<B: Backend>(b: &B, stop: &AtomicBool) -> Result<AuthStep, Fail> {
    let deadline = Instant::now() + STEP_TIMEOUT;
    wait_line(b, deadline, stop, |t| {
        b.wait_auth(t, &|s| s != AuthStep::WaitParameters)
    })
}

/// 提交成功后等下一条授权状态更新（`seen` = 提交之前的编号）。同一步重新下发也算（换号、重发验证码）。
/// `deadline` 与提交共用
fn advance<B: Backend>(
    b: &B,
    seen: u64,
    deadline: Instant,
    stop: &AtomicBool,
) -> Result<AuthStep, Fail> {
    wait_line(b, deadline, stop, |t| b.wait_auth_update(t, seen))
}

/// 测试档案上要注册：自动注册并等下一步；正式档案 → 不支持
fn register<B: Backend>(
    b: &B,
    kind: ServerKind,
    p: &mut dyn Prompter,
    stop: &AtomicBool,
) -> Result<AuthStep, Fail> {
    if kind != ServerKind::Test {
        return Err(Fail::Unsupported(AuthStep::WaitRegistration));
    }
    p.say("测试服务器上这个号码还没注册（测试服务器会定期清空），现在自动注册");
    let seen = b.auth_generation();
    let deadline = Instant::now() + STEP_TIMEOUT;
    within(b, b.auth_register(), deadline, stop)?.map_err(Fail::Core)?;
    advance(b, seen, deadline, stop)
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

/// 把一步的值交给后端，最多等到 `deadline`。外层错是没等到（超时 / Ctrl+C），内层是后端的应答
fn submit<B: Backend>(
    b: &B,
    step: AuthStep,
    v: &Secret,
    deadline: Instant,
    stop: &AtomicBool,
) -> Result<Result<(), CoreError>, Fail> {
    match step {
        AuthStep::WaitPhone => within(b, b.auth_submit_phone(v), deadline, stop),
        AuthStep::WaitCode => within(b, b.auth_submit_code(v), deadline, stop),
        AuthStep::WaitEmail => within(b, b.auth_submit_email(v), deadline, stop),
        AuthStep::WaitEmailCode => within(b, b.auth_submit_email_code(v), deadline, stop),
        AuthStep::WaitPassword => within(b, b.auth_submit_password(v), deadline, stop),
        _ => Ok(Err(CoreError::AuthWrongState { expected: step })),
    }
}

/// 交互登录：从当前步骤一直走到 `ready`。走到新的一步调一次 `on_step`（标准输出那一行；文字模式下就是
/// 那一步的说法，所以终端这边不再重复说步骤，只说提示与出错原因）。返回 `result.data`。
/// `stop` 立起（Ctrl+C）→ `interrupted`；读秘密时的 Ctrl+C 由终端那边报，同一个结果
pub fn login<B: Backend>(
    b: &B,
    kind: ServerKind,
    p: &mut dyn Prompter,
    on_step: &mut dyn FnMut(AuthStep),
    stop: &AtomicBool,
) -> Result<Value, Fail> {
    if kind == ServerKind::Main && std::env::var_os("TMUX").is_some() {
        p.say("提醒：你在 tmux 里。tmux 窗格的内容能被别的程序读走，正式号最好在 tmux 以外的终端里登录");
    }
    let mut step = settle(b, stop)?;
    on_step(step);
    let mut tries = 0u32;
    loop {
        match step {
            AuthStep::Ready => return Ok(json!({ "command": "login", "step": step_name(step) })),
            AuthStep::WaitRegistration => {
                step = register(b, kind, p, stop)?;
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
        if stop.load(Ordering::SeqCst) {
            return Err(Fail::Interrupted);
        }
        let answer = ask_step(b, step, p)?;
        let seen = b.auth_generation();
        // 期限从人答完开始算：问人的时间不算
        let deadline = Instant::now() + STEP_TIMEOUT;
        let r = match &answer {
            Answer::Resend => within(b, b.auth_resend_code(), deadline, stop)?,
            Answer::Value(v) => submit(b, step, v, deadline, stop)?,
        };
        match r {
            Ok(()) => {
                if matches!(answer, Answer::Resend) {
                    p.say("已请求重发");
                }
                let next = advance(b, seen, deadline, stop)?;
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
    stop: &AtomicBool,
) -> Result<Value, Fail> {
    let test_phone = test_phone.filter(|_| kind == ServerKind::Test);
    let step = settle(b, stop)?;
    let done = |s: AuthStep| json!({ "command": "auth", "sub": sub.name(), "step": step_name(s) });
    let seen = b.auth_generation();
    let deadline = match sub.step() {
        None if sub == AuthSub::State => {
            on_step(step);
            return Ok(done(step));
        }
        None => {
            let deadline = Instant::now() + STEP_TIMEOUT;
            within(b, b.auth_resend_code(), deadline, stop)?.map_err(Fail::Core)?;
            deadline
        }
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
            let deadline = Instant::now() + STEP_TIMEOUT;
            submit(b, target, &v, deadline, stop)?.map_err(Fail::Core)?;
            deadline
        }
    };
    let mut next = advance(b, seen, deadline, stop)?;
    if next == AuthStep::WaitRegistration && kind == ServerKind::Test {
        next = register(b, kind, p, stop)?;
    }
    on_step(next);
    Ok(done(next))
}

/// 登出：没登录就直接说；否则终端当场确认（回答 `yes`）→ `logOut` → 等到客户端关掉
pub fn logout<B: Backend>(
    b: &B,
    p: &mut dyn Prompter,
    on_step: &mut dyn FnMut(AuthStep),
    stop: &AtomicBool,
) -> Result<Value, Fail> {
    let step = settle(b, stop)?;
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
    let deadline = Instant::now() + STEP_TIMEOUT;
    within(b, b.logout(), deadline, stop)?.map_err(Fail::Core)?;
    let end = wait_line(b, deadline, stop, |t| {
        b.wait_auth(t, &|s| s == AuthStep::Closed)
    })?;
    on_step(end);
    Ok(json!({ "command": "logout", "step": step_name(end), "already_signed_out": false }))
}

#[cfg(test)]
mod tests;
