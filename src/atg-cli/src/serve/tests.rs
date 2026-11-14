//! 假后端 + 真套接字：数据全是编的（名字「会话N」），不碰真实账号，也不碰本机真实档案目录。
use super::*;
use crate::forward::{EXECUTOR_GONE_ZH, forward};
use atg_core::config::ServerKind;
use atg_core::error::Holder;
use atg_core::fake::{FakeAuth, FakeBackend};
use atg_core::ipc::{self, Connect};
use atg_core::model::ChatList;
use serde_json::{Value, json};
use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::thread::JoinHandle;

mod lifecycle;

/// 收集输出
#[derive(Default)]
struct Cap {
    out: Vec<String>,
    err: Vec<String>,
}

impl Sink for Cap {
    fn out(&mut self, line: &str) {
        self.out.push(line.to_owned());
    }
    fn err(&mut self, line: &str) {
        self.err.push(line.to_owned());
    }
}

/// 短路径（套接字路径有 103 字节上限）
fn profile(tag: &str, kind: ServerKind) -> Profile {
    let root = std::env::temp_dir().join(format!("atgs-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    Profile::create(&root, "main", kind).unwrap()
}

fn chat(id: i64, order: i64, unread: i32) -> Vec<Value> {
    vec![
        json!({"@type": "updateNewChat", "chat": {
            "@type": "chat", "id": id, "title": format!("会话{id}"),
            "type": {"@type": "chatTypePrivate", "user_id": id},
            "positions": [], "unread_count": unread, "unread_mention_count": 0,
            "last_read_inbox_message_id": 0, "last_read_outbox_message_id": 0,
            "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0},
        }}),
        json!({"@type": "updateChatPosition", "chat_id": id, "position": {
            "@type": "chatPosition", "list": {"@type": "chatListMain"}, "order": order.to_string(), "is_pinned": false,
        }}),
    ]
}

/// 已登录、有三个会话、连接已就绪的假后端
fn backend() -> FakeBackend {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth {
        // 测试号段（编的）：测试档案上发收藏夹走测试豁免
        phone: "+9996612345".into(),
        logged_in: true,
        ..FakeAuth::default()
    });
    // `send` 用：自己是 9，收藏夹即与 9 的私聊
    b.set_me(9);
    b.set_chat(9, chat(9, 5, 0));
    b.set_chats(
        ChatList::Main,
        vec![chat(1, 30, 2), chat(2, 20, 0), chat(3, 10, 1)],
    );
    // `history` / `links` 用：会话 1 的三条消息，第 3 条带一个链接
    let mut info = chat(1, 30, 2);
    info.insert(
        0,
        json!({"@type": "updateUser", "user": {"@type": "user", "id": 1, "first_name": "林小雨", "last_name": "",
            "usernames": null, "phone_number": "", "is_contact": true, "type": {"@type": "userTypeRegular"}}}),
    );
    b.set_chat(1, info);
    let m = |id: i64, t: &str, entities: Value| {
        json!({"@type": "message", "id": id, "chat_id": 1,
            "sender_id": {"@type": "messageSenderUser", "user_id": 1}, "is_outgoing": false, "is_pinned": false,
            "date": 1_793_014_200 + id * 60, "edit_date": 0, "media_album_id": "0",
            "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": t, "entities": entities}}})
    };
    b.set_history(
        1,
        vec![
            m(1, "第1条", json!([])),
            m(2, "第2条", json!([])),
            m(3, "看下路线图", json!([{"@type": "textEntity", "offset": 2, "length": 3,
                "type": {"@type": "textEntityTypeTextUrl", "url": "https://example.invalid/route"}}])),
        ],
    );
    b.push_net("connectionStateReady");
    b
}

fn fast() -> Limits {
    Limits {
        load: Duration::from_secs(5),
        sync: Duration::from_millis(300),
    }
}

fn chats_args(limit: u32) -> ChatsArgs {
    ChatsArgs {
        list: crate::args::ListArg::Main,
        limit,
        unread: false,
        cursor: None,
        reveal: false,
    }
}

/// 起一个执行方线程；等到套接字出现
struct Server {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<(u8, Cap)>,
    profile: Profile,
}

fn start_server(p: &Profile, format: Format) -> Server {
    let stop = Arc::new(AtomicBool::new(false));
    let (st, pc) = (stop.clone(), p.clone());
    let handle = std::thread::spawn(move || {
        let b = backend();
        let mut cap = Cap::default();
        let code = serve(&b, &pc, format, &st, &mut cap, fast(), Instant::now());
        (code, cap)
    });
    let path = ipc::socket_path(p);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !matches!(ipc::connect_at(&path), Connect::Ok(_)) {
        assert!(Instant::now() < deadline, "执行方没起来");
        std::thread::sleep(Duration::from_millis(20));
    }
    Server {
        stop,
        handle,
        profile: p.clone(),
    }
}

impl Server {
    fn ask(&self, format: Format, command: &str, params: &Value) -> (u8, Cap) {
        let mut cap = Cap::default();
        let code = forward(
            ipc::connect(&self.profile),
            &CoreError::ProfileBusy {
                holder: Some(Holder::Cli),
            },
            format,
            command,
            params,
            false,
            &mut cap,
        );
        (code, cap)
    }

    fn stop(self) -> (u8, Cap) {
        self.stop.store(true, Ordering::SeqCst);
        self.handle.join().unwrap()
    }
}

/// 直跑 `chats`：同一段渲染代码写进 `Cap`
fn direct_chats(format: Format, a: &ChatsArgs) -> (u8, Cap) {
    let p = profile("direct", ServerKind::Main);
    let b = backend();
    let mut cap = Cap::default();
    let code = match crate::chats_query(a)
        .and_then(|q| chats::run_with(&b, &p, &q, Instant::now(), fast()))
    {
        Ok(l) => crate::render_chats(&mut cap, format, a, &l, Instant::now()),
        Err(f) => fail_with_to(&mut cap, format, &f),
    };
    (code, cap)
}

/// 抹掉执行方当场生成的时间字段：`at`、`elapsed_ms`、`first_chat_ms`；其余按原顺序重新写出
fn norm_json(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|l| {
            let mut v: Value = serde_json::from_str(l).unwrap();
            let o = v.as_object_mut().unwrap();
            o.insert("at".into(), json!("<at>"));
            if let Some(d) = o.get_mut("data").and_then(Value::as_object_mut) {
                // `digest` 里有每次随机的 sending_id
                for k in ["elapsed_ms", "first_chat_ms", "digest"] {
                    if d.contains_key(k) {
                        d.insert(k.into(), json!("<ms>"));
                    }
                }
            }
            v.to_string()
        })
        .collect()
}

/// 文字模式标准错误里的毫秒数
fn norm_text(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|l| {
            let mut out = String::new();
            let mut in_num = false;
            for ch in l.chars() {
                if ch.is_ascii_digit() {
                    if !in_num {
                        out.push('N');
                    }
                    in_num = true;
                } else {
                    in_num = false;
                    out.push(ch);
                }
            }
            out
        })
        .collect()
}

/// `send` 经本机接口转发与直跑逐行一致（测试档案发收藏夹：测试豁免，不弹框）；快照固定输出形状
#[test]
fn forwarded_send_matches_direct() {
    for format in [Format::Json, Format::Text] {
        let p = profile("send", ServerKind::Test);
        let s = start_server(&p, format);
        let params = json!({"chat": "saved", "text": "测试", "reply_to": null, "dry_run": false,
            "again": false, "tz": "+08:00"});
        let (fc, f) = s.ask(format, "send", &params);
        let _ = s.stop();

        let pd = profile("sendd", ServerKind::Test);
        let b = backend();
        let sp: crate::send::SendParams = serde_json::from_value(params).unwrap();
        let mut cap = Cap::default();
        let dc = crate::history::with_backend(&b, &pd, Instant::now(), |b, st, fe| {
            Ok(crate::send::execute_send(
                b,
                st,
                fe,
                &sp,
                &mut cap,
                format,
                Instant::now(),
                Duration::from_secs(2),
            ))
        })
        .unwrap_or_else(|_| panic!("直跑起不来"));
        assert_eq!((fc, dc), (0, 0), "{f:?}", f = f.out);
        match format {
            Format::Json => {
                assert_eq!(norm_json(&f.out), norm_json(&cap.out));
                let kinds: Vec<String> = cap
                    .out
                    .iter()
                    .map(|l| serde_json::from_str::<Value>(l).unwrap()["kind"].to_string())
                    .collect();
                assert_eq!(kinds, ["\"plan\"", "\"message\"", "\"result\""]);
                insta::assert_snapshot!("send_json", norm_json(&cap.out).join("\n"));
            }
            Format::Text => {
                assert_eq!(f.out, cap.out);
                insta::assert_snapshot!("send_text", cap.out.join("\n"));
            }
        }
    }
}

#[test]
fn forwarded_chats_matches_direct_json_and_text() {
    let p = profile("fwd", ServerKind::Main);
    let s = start_server(&p, Format::Json);
    let a = chats_args(2);
    let params = serde_json::to_value(&a).unwrap();

    let (dc, d) = direct_chats(Format::Json, &a);
    let (fc, f) = s.ask(Format::Json, "chats", &params);
    assert_eq!((dc, fc), (0, 0));
    assert_eq!(f.out.len(), 3, "两行会话 + 一行 result");
    assert_eq!(norm_json(&f.out), norm_json(&d.out));
    assert_eq!(f.err, d.err);
    insta::assert_snapshot!("forwarded_chats_json", norm_json(&f.out).join("\n"));

    let (dc, d) = direct_chats(Format::Text, &a);
    let (fc, f) = s.ask(Format::Text, "chats", &params);
    assert_eq!((dc, fc), (0, 0));
    assert_eq!(f.out, d.out, "文字模式标准输出没有时间，应逐字节一致");
    assert_eq!(norm_text(&f.err), norm_text(&d.err));

    // 第二次：执行方常驻的 Store 里已有会话，照样一致（这回要全部 + 游标为空）
    let a = chats_args(10);
    let params = serde_json::to_value(&a).unwrap();
    let (_, d) = direct_chats(Format::Json, &a);
    let (fc, f) = s.ask(Format::Json, "chats", &params);
    assert_eq!(fc, 0);
    assert_eq!(norm_json(&f.out), norm_json(&d.out));

    let (code, own) = s.stop();
    assert_eq!(code, 0);
    assert_eq!(own.out.len(), 2, "一行 status + 一行 result");
    let st: Value = serde_json::from_str(&own.out[0]).unwrap();
    assert_eq!(st["kind"], "status");
    assert_eq!(st["data"]["listening"], true);
    assert_eq!(st["data"]["user_status"], "online");
    let r: Value = serde_json::from_str(&own.out[1]).unwrap();
    assert_eq!(r["data"]["served"], 3);
    assert_eq!(r["data"]["reason"], "interrupted");
    // 自己的日志只有命令名，不带会话名
    assert!(own.err.iter().all(|l| !l.contains("会话")));
    assert!(!ipc::socket_path(&p).exists(), "正常关后套接字删掉");
}

fn history_args() -> crate::args::HistoryArgs {
    crate::args::HistoryArgs {
        chat: "1".into(),
        limit: 2,
        before: None,
        after: None,
        detail: true,
        full: true,
        reveal: false,
        local: false,
        tz: Some("+08:00".to_owned()),
    }
}

/// 直跑 `history`：同一段渲染代码写进 `Cap`
fn direct_history(format: Format, a: &crate::args::HistoryArgs) -> (u8, Cap) {
    let p = profile("dhist", ServerKind::Main);
    let b = backend();
    let mut cap = Cap::default();
    let cx =
        crate::history::speech_ctx(Some("+08:00"), a.reveal).unwrap_or_else(|_| panic!("时区"));
    let code = match crate::history::query(a)
        .and_then(|q| crate::history::run(&b, &p, &q, Instant::now()))
    {
        Ok(pg) => crate::history::render(&mut cap, format, a, &cx, &pg, Instant::now()),
        Err(f) => fail_with_to(&mut cap, format, &f),
    };
    (code, cap)
}

fn direct_links(format: Format, a: &crate::args::LinksArgs) -> (u8, Cap) {
    let p = profile("dlink", ServerKind::Main);
    let b = backend();
    let mut cap = Cap::default();
    let code = match crate::history::links_query(a)
        .and_then(|q| crate::history::run_links(&b, &p, &q, Instant::now()))
    {
        Ok(l) => crate::history::render_links(&mut cap, format, a.reveal, &l, Instant::now()),
        Err(f) => fail_with_to(&mut cap, format, &f),
    };
    (code, cap)
}

#[test]
fn forwarded_history_and_links_match_direct() {
    let p = profile("fhist", ServerKind::Main);
    let s = start_server(&p, Format::Json);
    let a = history_args();
    let params = serde_json::to_value(&a).unwrap();
    for format in [Format::Json, Format::Text] {
        let (dc, d) = direct_history(format, &a);
        let (fc, f) = s.ask(format, "history", &params);
        assert_eq!((dc, fc), (0, 0));
        match format {
            Format::Json => {
                assert_eq!(f.out.len(), 3, "两条消息 + 一行 result");
                assert_eq!(norm_json(&f.out), norm_json(&d.out));
                insta::assert_snapshot!("forwarded_history_json", norm_json(&f.out).join("\n"));
            }
            Format::Text => {
                assert_eq!(f.out, d.out);
                assert_eq!(norm_text(&f.err), norm_text(&d.err));
            }
        }
    }
    // 往前翻一页：执行方常驻的 Store 里已有消息，照样一致
    let mut back = history_args();
    back.before = Some(2);
    let params = serde_json::to_value(&back).unwrap();
    let (_, d) = direct_history(Format::Json, &back);
    let (fc, f) = s.ask(Format::Json, "history", &params);
    assert_eq!(fc, 0);
    assert_eq!(norm_json(&f.out), norm_json(&d.out));

    let la = crate::args::LinksArgs {
        chat: "1".into(),
        message: 3,
        reveal: false,
    };
    let params = serde_json::to_value(&la).unwrap();
    for format in [Format::Json, Format::Text] {
        let (dc, d) = direct_links(format, &la);
        let (fc, f) = s.ask(format, "links", &params);
        assert_eq!((dc, fc), (0, 0));
        assert_eq!(
            norm_json_or_text(format, &f.out),
            norm_json_or_text(format, &d.out)
        );
    }
    // 范围检查在执行方补上（serde 绕过了 clap）
    let mut bad = history_args();
    bad.limit = 0;
    let params = serde_json::to_value(&bad).unwrap();
    let (code, _) = s.ask(Format::Json, "history", &params);
    assert_eq!(code, 2);
    let (code, own) = s.stop();
    assert_eq!(code, 0);
    // 自己的日志只有命令名，不带内容
    assert!(
        own.err
            .iter()
            .all(|l| !l.contains("路线图") && !l.contains("林小雨"))
    );
}

fn norm_json_or_text(format: Format, lines: &[String]) -> Vec<String> {
    match format {
        Format::Json => norm_json(lines),
        Format::Text => lines.to_vec(),
    }
}

#[test]
fn forwarded_auth_state_matches_direct() {
    let p = profile("auth", ServerKind::Main);
    let s = start_server(&p, Format::Json);
    let (fc, f) = s.ask(Format::Json, "auth", &json!({"sub": "state"}));

    let pd = profile("authd", ServerKind::Main);
    let b = backend();
    let mut d = Cap::default();
    let r = crate::session::drive(
        &b,
        &pd,
        Job::Auth(AuthSub::State),
        &mut NoSecrets,
        &mut |st| crate::emit_step(&mut d, Format::Json, st),
        &AtomicBool::new(false),
    );
    let dc = crate::job_output(
        &mut d,
        Format::Json,
        Job::Auth(AuthSub::State),
        r,
        Instant::now(),
    );
    assert_eq!((dc, fc), (0, 0));
    assert_eq!(norm_json(&f.out), norm_json(&d.out));
    insta::assert_snapshot!("forwarded_auth_state_json", norm_json(&f.out).join("\n"));
    s.stop();
}
