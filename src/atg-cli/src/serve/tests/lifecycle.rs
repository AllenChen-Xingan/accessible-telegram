//! 接口的起停与边界：拒执行、执行方不在 / 中途退出、旧套接字与档案锁、转发 `watch`
use super::*;

#[test]
fn secrets_and_unknown_commands_are_not_executed() {
    let p = profile("deny", ServerKind::Main);
    let s = start_server(&p, Format::Json);
    // 交验证码这类不转：执行方也不执行
    let (code, f) = s.ask(Format::Json, "auth", &json!({"sub": "code"}));
    assert_eq!(code, 2);
    assert!(f.out[0].contains("\"usage\""));
    let (code, f) = s.ask(Format::Json, "delete", &json!({}));
    assert_eq!(code, 1);
    let v: Value = serde_json::from_str(&f.out[0]).unwrap();
    assert_eq!(v["data"]["code"], "not_implemented");
    assert_eq!(v["data"]["command"], "delete");
    // 发送的参数形状不对：不执行
    let (code, f) = s.ask(Format::Json, "send", &json!({}));
    assert_eq!(code, 1);
    assert!(f.out[0].contains("ipc_bad_request"));
    // 参数形状不对
    let (code, f) = s.ask(Format::Json, "chats", &json!({"limit": "x"}));
    assert_eq!(code, 1);
    assert!(f.out[0].contains("ipc_bad_request"));
    // 绕过 clap 的范围：执行方补查
    let mut a = serde_json::to_value(chats_args(1)).unwrap();
    a["limit"] = json!(0);
    let (code, f) = s.ask(Format::Text, "chats", &a);
    assert_eq!(code, 2);
    assert_eq!(f.out, vec!["-n 要在 1 到 100000 之间".to_owned()]);
    // 版本号不对 / 不是 JSON：先看版本号，都回一行错误 + exit 帧，不是直接断开
    for (raw, what) in [
        ("{\"v\":99,\"origin\":\"gui2\"}\n", "ipc_version_mismatch"),
        ("not json\n", "ipc_bad_request"),
        ("{\"v\":0}\n", "ipc_bad_request"),
    ] {
        let mut st = std::os::unix::net::UnixStream::connect(ipc::socket_path(&p)).unwrap();
        std::io::Write::write_all(&mut st, raw.as_bytes()).unwrap();
        let mut got = String::new();
        std::io::Read::read_to_string(&mut st, &mut got).unwrap();
        let lines: Vec<Value> = got
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 2, "{what}");
        let err: Value = serde_json::from_str(lines[0]["line"].as_str().unwrap()).unwrap();
        assert_eq!(err["data"]["what"], what);
        assert_eq!(lines[1], json!({"t": "exit", "code": 1}));
    }
    // 只连上就断（探一下执行方在不在）：不算请求、不记日志
    drop(ipc::connect(&p));
    let (_, own) = s.stop();
    let r: Value = serde_json::from_str(own.out.last().unwrap()).unwrap();
    assert_eq!(r["data"]["served"], 8);
    assert!(own.err.iter().all(|l| !l.contains("超时")));
}

#[test]
fn unreachable_executor_is_profile_busy() {
    let mut cap = Cap::default();
    let code = forward(
        Connect::Unreachable,
        &CoreError::ProfileBusy {
            holder: Some(Holder::Cli),
        },
        Format::Json,
        "chats",
        &json!({}),
        false,
        &mut cap,
    );
    assert_eq!(code, 9);
    let v: Value = serde_json::from_str(&cap.out[0]).unwrap();
    assert_eq!(v["data"]["code"], "profile_busy");
    assert_eq!(v["data"]["holder"], "cli");
    assert_eq!(v["data"]["message_zh"], crate::forward::BUSY_ONESHOT_ZH);
}

#[test]
fn executor_dying_midway_is_disconnected() {
    let p = profile("die", ServerKind::Main);
    let l = ipc::Listener::bind(&p).unwrap();
    let t = std::thread::spawn(move || {
        let Some(Accepted::Conn(mut c)) = l.accept(Duration::from_secs(5)).unwrap() else {
            panic!("没接到");
        };
        let _: Request = c.recv().ok().unwrap();
        c.send(&Frame::Out {
            line: "{\"v\":1}".into(),
        })
        .unwrap();
        // 不发 exit 就走
    });
    let mut cap = Cap::default();
    let code = forward(
        ipc::connect(&p),
        &CoreError::ProfileBusy { holder: None },
        Format::Json,
        "chats",
        &json!({}),
        false,
        &mut cap,
    );
    t.join().unwrap();
    assert_eq!(code, 4);
    assert_eq!(cap.out.len(), 2);
    let v: Value = serde_json::from_str(&cap.out[1]).unwrap();
    assert_eq!(v["kind"], "error");
    assert_eq!(v["data"]["code"], "disconnected");
    assert_eq!(v["data"]["message_zh"], EXECUTOR_GONE_ZH);
}

#[test]
fn serve_replaces_stale_socket_and_refuses_busy_profile() {
    let p = profile("stale", ServerKind::Test);
    // 上次没正常关留下的套接字文件
    drop(UnixListener::bind(ipc::socket_path(&p)).unwrap());
    assert!(ipc::socket_path(&p).exists());
    let s = start_server(&p, Format::Json);
    // 档案正被占着：锁探得到占用方；第二个执行方起不来、也不碰别人的套接字
    assert_eq!(
        p.probe_lock(),
        Err(CoreError::ProfileBusy {
            holder: Some(Holder::Cli)
        })
    );
    let b2 = backend();
    let mut cap = Cap::default();
    let stop = AtomicBool::new(false);
    let code = serve(
        &b2,
        &p,
        Format::Json,
        &stop,
        &mut cap,
        fast(),
        Instant::now(),
    );
    assert_eq!(code, 9);
    assert!(
        matches!(ipc::connect(&p), Connect::Ok(_)),
        "第一个执行方照常在听"
    );
    let (code, _) = s.stop();
    assert_eq!(code, 0);
    assert_eq!(p.probe_lock(), Ok(()), "关掉后锁放了");
}

#[test]
fn serve_does_not_open_socket_when_profile_locked() {
    let p = profile("locked", ServerKind::Main);
    let _held = p.lock(Holder::Gui).unwrap();
    let b = backend();
    let mut cap = Cap::default();
    let stop = AtomicBool::new(false);
    let code = serve(
        &b,
        &p,
        Format::Json,
        &stop,
        &mut cap,
        fast(),
        Instant::now(),
    );
    assert_eq!(code, 9);
    let v: Value = serde_json::from_str(&cap.out[0]).unwrap();
    assert_eq!(v["data"]["holder"], "gui");
    assert!(!ipc::socket_path(&p).exists());
}

#[test]
fn paths_fit_in_tests() {
    let p = profile("len", ServerKind::Main);
    let len = ipc::socket_path(&p).as_os_str().len();
    assert!(len <= ipc::SOCKET_PATH_MAX, "{len}");
}

/// 收集流式转发的输出（另一条线程里写，这边看）
#[derive(Clone, Default)]
struct Shared(Arc<std::sync::Mutex<Vec<String>>>);

impl Sink for Shared {
    fn out(&mut self, line: &str) {
        self.0.lock().unwrap().push(line.to_owned());
    }
    fn err(&mut self, _: &str) {}
}

impl Shared {
    fn wait(&self, what: &str, pred: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(v) = self
                .0
                .lock()
                .unwrap()
                .iter()
                .map(|l| serde_json::from_str::<Value>(l).unwrap())
                .find(|v| pred(v))
            {
                return v;
            }
            assert!(Instant::now() < deadline, "没等到{what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn wait_call(b: &FakeBackend, c: &atg_core::fake::Call) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !b.calls().contains(c) {
        assert!(Instant::now() < deadline, "没等到调用");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// 起一个 `watch` 客户端线程（流式转发），回（输出，停它的开关，线程）
fn watch_client(p: &Profile, params: Value) -> (Shared, &'static AtomicBool, JoinHandle<u8>) {
    let out = Shared::default();
    let stop: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    let (mut sink, pc) = (out.clone(), p.clone());
    let h = std::thread::spawn(move || {
        crate::forward::forward_stream(
            ipc::connect(&pc),
            &CoreError::ProfileBusy {
                holder: Some(Holder::Cli),
            },
            Format::Json,
            "watch",
            &params,
            stop,
            Instant::now(),
            &mut sink,
        )
    });
    (out, stop, h)
}

#[test]
fn watch_streams_through_executor_and_detaches() {
    use atg_core::fake::Call;
    let p = profile("watch", ServerKind::Main);
    let b = Arc::new(backend());
    let stop = Arc::new(AtomicBool::new(false));
    let (st, pc, bb) = (stop.clone(), p.clone(), b.clone());
    let server = std::thread::spawn(move || {
        let mut cap = Cap::default();
        let code = serve(
            &*bb,
            &pc,
            Format::Json,
            &st,
            &mut cap,
            fast(),
            Instant::now(),
        );
        (code, cap)
    });
    let path = ipc::socket_path(&p);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !matches!(ipc::connect_at(&path), Connect::Ok(_)) {
        assert!(Instant::now() < deadline, "执行方没起来");
        std::thread::sleep(Duration::from_millis(20));
    }

    // 第一个 watch 打开会话 1：执行方早已补发完，先补一行 sync_done
    let (out1, stop1, c1) = watch_client(&p, json!({"chat": "1", "reveal": false, "tz": "+08:00"}));
    out1.wait("status", |v| v["kind"] == "status");
    out1.wait("sync_done", |v| v["data"]["type"] == "sync_done");
    wait_call(&b, &Call::ChatOpen(1));
    // 第二个 watch 不开会话
    let (out2, _stop2, c2) = watch_client(&p, json!({"reveal": false}));
    out2.wait("sync_done", |v| v["data"]["type"] == "sync_done");

    // 来一条新消息：两个都收到，判为要提示（执行方用真时钟，消息时间取现在）
    let now = crate::watch::now_ms() / 1000;
    b.push(json!({"@type": "updateNewMessage", "message": {"@type": "message", "id": 10, "chat_id": 1,
        "sender_id": {"@type": "messageSenderUser", "user_id": 1}, "is_outgoing": false, "date": now,
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": "第10条", "entities": []}}}}));
    for out in [&out1, &out2] {
        let m = out.wait("新消息", |v| {
            v["data"]["type"] == "new_message" && v["data"]["message_id"] == 10
        });
        assert_eq!(m["data"]["notify"]["alert"], true, "{m}");
        assert!(m["speech"]["brief"].as_str().unwrap().contains("第10条"));
    }

    // 第一个按 Ctrl+C：本地出结尾行、退出码 0；执行方发现它走了，配对关掉会话
    stop1.store(true, Ordering::SeqCst);
    assert_eq!(c1.join().unwrap(), 0);
    let r = out1.wait("结尾行", |v| v["kind"] == "result");
    assert_eq!(r["data"]["reason"], "interrupted");
    assert_eq!(r["data"]["alerts"], 1);
    wait_call(&b, &Call::ChatClose(1));

    // 执行方停：还挂着的那个收到执行方的结尾行，退出码 0
    stop.store(true, Ordering::SeqCst);
    assert_eq!(c2.join().unwrap(), 0);
    let r = out2.wait("结尾行", |v| v["kind"] == "result");
    assert_eq!(r["data"]["command"], "watch");
    let (code, _) = server.join().unwrap();
    assert_eq!(code, 0);
    // 开、关各一次
    let calls = b.calls();
    assert_eq!(calls.iter().filter(|c| **c == Call::ChatOpen(1)).count(), 1);
    assert_eq!(
        calls.iter().filter(|c| **c == Call::ChatClose(1)).count(),
        1
    );
}

/// 读方写两行就走（`watch | head -n 2`）：发起方发现写不出去就退出（0），执行方摘掉它、配对关会话
#[test]
fn forwarded_watch_ends_when_stdout_reader_leaves() {
    use atg_core::fake::Call;
    struct Head(usize);
    impl Sink for Head {
        fn out(&mut self, _: &str) {}
        fn err(&mut self, _: &str) {}
        fn out_ok(&mut self, _: &str) -> bool {
            self.0 = self.0.saturating_sub(1);
            self.0 > 0
        }
    }
    let p = profile("whead", ServerKind::Main);
    let b = Arc::new(backend());
    let stop = Arc::new(AtomicBool::new(false));
    let (st, pc, bb) = (stop.clone(), p.clone(), b.clone());
    let server = std::thread::spawn(move || {
        let mut cap = Cap::default();
        serve(
            &*bb,
            &pc,
            Format::Json,
            &st,
            &mut cap,
            fast(),
            Instant::now(),
        )
    });
    let path = ipc::socket_path(&p);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !matches!(ipc::connect_at(&path), Connect::Ok(_)) {
        assert!(Instant::now() < deadline, "执行方没起来");
        std::thread::sleep(Duration::from_millis(20));
    }
    let never: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    // 第 1 行 status 写成，第 2 行（补的 sync_done）写不出去
    let code = crate::forward::forward_stream(
        ipc::connect(&p),
        &CoreError::ProfileBusy {
            holder: Some(Holder::Cli),
        },
        Format::Json,
        "watch",
        &json!({"chat": "1", "reveal": false}),
        never,
        Instant::now(),
        &mut Head(2),
    );
    assert_eq!(code, 0);
    wait_call(&b, &Call::ChatOpen(1));
    wait_call(&b, &Call::ChatClose(1));
    stop.store(true, Ordering::SeqCst);
    assert_eq!(server.join().unwrap(), 0);
}
