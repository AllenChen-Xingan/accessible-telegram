//! 期限与 Ctrl+C（#63）：请求一直不应答时不再干等；叫停后照常收尾

use super::*;
use std::sync::Arc;

/// 假后端：这个登录动作一直不应答
fn hang(b: &FakeBackend, action: &'static str) {
    b.fail_next(
        action,
        CoreError::Internal {
            what: FakeBackend::HANG,
        },
    );
}

/// 等假后端记下这次请求（已交出、卡在等应答）再立停止标志：不靠固定延时，机器再慢也先走到挂起点。
/// 5 秒还没记下就照样立起，后面「确实交出」的断言会如实变红
fn stop_once_sent<R>(
    b: &FakeBackend,
    action: &'static str,
    run: impl FnOnce(&AtomicBool) -> R,
) -> R {
    let stop = AtomicBool::new(false);
    std::thread::scope(|s| {
        s.spawn(|| {
            let t0 = Instant::now();
            while !auth_calls(b).contains(&action) && t0.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(5));
            }
            stop.store(true, Ordering::SeqCst);
        });
        run(&stop)
    })
}

/// 过 `after` 之后在别的线程立起停止标志（像信号处理函数那样，不唤醒等的线程）
fn stop_later(after: Duration) -> (Arc<AtomicBool>, std::thread::JoinHandle<()>) {
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    let h = std::thread::spawn(move || {
        std::thread::sleep(after);
        s.store(true, Ordering::SeqCst);
    });
    (stop, h)
}

/// 提交号码时后端一直不应答：Ctrl+C 之后很快以 `interrupted` 结束，后端照常关掉、档案锁放掉
#[test]
fn ctrl_c_while_submit_hangs_is_interrupted_and_shut_down() {
    let root = temp_root("hang-submit");
    let p = Profile::create(&root, "test", ServerKind::Test).unwrap();
    let b = FakeBackend::new().unwrap();
    b.set_auth(account());
    hang(&b, "phone");
    let t0 = Instant::now();
    let r = stop_once_sent(&b, "phone", |stop| {
        session::drive(
            &b,
            &p,
            Job::Login,
            &mut Script::new(&[PHONE]),
            &mut |_| {},
            stop,
        )
    });
    assert!(matches!(r, Err(Fail::Interrupted)));
    assert!(t0.elapsed() < Duration::from_secs(5), "{:?}", t0.elapsed());
    // 真卡在提交上（号码已交出），不是卡在前面等授权线
    assert!(auth_calls(&b).contains(&"phone"), "{:?}", auth_calls(&b));
    assert!(matches!(b.calls().last(), Some(Call::Shutdown)));
    assert!(p.lock(Holder::Cli).is_ok());
}

/// 分步重发时后端一直不应答：同上
#[test]
fn ctrl_c_while_resend_hangs_is_interrupted() {
    let (b, _p) = started("hang-resend", ServerKind::Test, account());
    let mut s = Script::new(&[PHONE]);
    let r = auth_step(
        &b,
        ServerKind::Test,
        AuthSub::Phone,
        None,
        &mut s,
        &mut |_| {},
        &NEVER,
    );
    assert!(r.is_ok(), "先走到等验证码");
    hang(&b, "resend");
    let t0 = Instant::now();
    let r = stop_once_sent(&b, "resend", |stop| {
        auth_step(
            &b,
            ServerKind::Test,
            AuthSub::Resend,
            None,
            &mut s,
            &mut |_| {},
            stop,
        )
    });
    assert!(matches!(r, Err(Fail::Interrupted)));
    assert!(t0.elapsed() < Duration::from_secs(5), "{:?}", t0.elapsed());
    assert!(
        auth_calls(&b).contains(&"resend"),
        "确实交出去了、卡在等应答"
    );
}

/// 登出时后端一直不应答：同上
#[test]
fn ctrl_c_while_logout_hangs_is_interrupted() {
    let a = FakeAuth {
        logged_in: true,
        ..account()
    };
    let (b, _p) = started("hang-logout", ServerKind::Test, a);
    hang(&b, "logout");
    let t0 = Instant::now();
    let r = stop_once_sent(&b, "logout", |stop| {
        logout(&b, &mut Script::new(&["yes"]), &mut |_| {}, stop)
    });
    assert!(matches!(r, Err(Fail::Interrupted)));
    assert!(t0.elapsed() < Duration::from_secs(5), "{:?}", t0.elapsed());
}

/// 确认那一问时立起停止标志（真终端上 Ctrl+C 由 `tty` 自己认、报 `interrupted`；这里模拟标志在确认途中被立起，
/// 又答了 yes）：确认之后再查一次，不把登出交出去
struct YesButStopped<'a>(&'a AtomicBool);

impl Prompter for YesButStopped<'_> {
    fn say(&mut self, _: &str) {}
    fn ask_secret(&mut self, prompt: &str) -> Result<Secret, Fail> {
        panic!("登出不该问秘密：{prompt}")
    }
    fn confirm(&mut self, _: &str) -> Result<bool, Fail> {
        self.0.store(true, Ordering::SeqCst);
        Ok(true)
    }
}

#[test]
fn stopped_during_logout_confirm_never_sends_it() {
    let a = FakeAuth {
        logged_in: true,
        ..account()
    };
    let (b, _p) = started("stopped-logout", ServerKind::Test, a);
    let stop = AtomicBool::new(false);
    let r = logout(&b, &mut YesButStopped(&stop), &mut |_| {}, &stop);
    assert!(matches!(r, Err(Fail::Interrupted)));
    assert!(stop.load(Ordering::SeqCst), "确认那一问应当问到");
    assert!(!auth_calls(&b).contains(&"logout"), "{:?}", auth_calls(&b));
}

/// 期限到了还没应答 → `network`（内核没故障）。用短期限直接测共用的那一处，不真等 30 秒
#[test]
fn hung_request_past_deadline_is_network() {
    let (b, _p) = started("hang-deadline", ServerKind::Test, account());
    hang(&b, "phone");
    let v = Secret::new(PHONE.as_bytes().to_vec());
    let t0 = Instant::now();
    let deadline = t0 + Duration::from_millis(300);
    let r = submit(&b, AuthStep::WaitPhone, &v, deadline, &NEVER);
    assert!(matches!(
        r,
        Err(Fail::Core(CoreError::Network { td: None }))
    ));
    assert!(t0.elapsed() >= Duration::from_millis(300));
}

/// 授权线一直不动：等到期限报 `network`；叫停则 `interrupted`
#[test]
fn waiting_for_auth_line_honours_deadline_and_stop() {
    let (b, _p) = started("line", ServerKind::Test, account());
    let seen = b.auth_generation();
    let t0 = Instant::now();
    let r = advance(&b, seen, t0 + Duration::from_millis(300), &NEVER);
    assert!(matches!(r, Err(Fail::Core(CoreError::Network { .. }))));
    let (stop, h) = stop_later(Duration::from_millis(300));
    let t1 = Instant::now();
    let r = advance(&b, seen, t1 + Duration::from_secs(30), &stop);
    h.join().unwrap();
    assert!(matches!(r, Err(Fail::Interrupted)));
    assert!(t1.elapsed() < Duration::from_secs(5), "{:?}", t1.elapsed());
}
