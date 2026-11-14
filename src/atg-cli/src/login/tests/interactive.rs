//! 交互 `login`：各分支走到 `ready`、输错、重发、Ctrl+C

use super::*;

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
