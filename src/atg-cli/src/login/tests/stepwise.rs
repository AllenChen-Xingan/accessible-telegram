//! 分步 `auth *` 与登出

use super::*;

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
    let r = auth_step(
        b,
        kind,
        sub,
        tp.as_ref(),
        s,
        &mut |st| steps.push(st),
        &NEVER,
    );
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
    let r = logout(&b, &mut Script::new(&["no"]), &mut |_| {}, &NEVER);
    assert_eq!(core(r).code(), "cancelled");
    assert!(!auth_calls(&b).contains(&"logout"));

    let (b, _p) = started("logout-yes", ServerKind::Test, a);
    let mut steps = Vec::new();
    let r = logout(
        &b,
        &mut Script::new(&["yes"]),
        &mut |s| steps.push(s),
        &NEVER,
    );
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
    let d = logout(&b, &mut s, &mut |_| {}, &NEVER).ok().unwrap();
    assert_eq!(d["already_signed_out"], true);
    assert!(s.asked.is_empty());
}
