//! `serve`：常驻的执行方（M1 阶段由 CLI 当；M3.1 起 GUI 也当）。契约：`config-privacy.md` 第一节第 2 小节「本机接口」。
//!
//! 拿档案锁、起后端 → 在 `profiles/<档案>/atg.sock` 听 → 一个一个处理后起的 `atg-cli` 转来的命令 → Ctrl+C / SIGTERM
//! 正常关（删套接字、关后端；正在处理的那个请求先做完）。再按一次 Ctrl+C / SIGTERM 立刻以 130 退出，不出结尾行
//! （正在处理的请求的发起方收到 `disconnected`；套接字留下，下次启动清掉）。只有 `serve` 开接口；一次性命令占着档案的那几秒不听。
//!
//! - 标准输出：起好后一行 `status`（带 `listening:true`），结束时一行 `result`（`reason:"interrupted"`，退出码 0，
//!   同 `watch` 被打断算正常结束）；后端故障就以 `disconnected` 结尾，好让人重开
//! - 标准错误：每处理一个请求记一句「命令名 · 退出码 · 耗时」，不记参数（参数以后会带内容）
//! - 能执行的：`chats`、`history`、`links`、`auth state`。登录、登出与其余授权步骤不经接口，回 `usage`；不认的命令回 `not_implemented`；
//!   版本号不对、请求形状不对回 `internal{ipc_version_mismatch | ipc_bad_request}`（这时还不知道要哪种格式，一律 JSON）
//! - 只连上就断的（发起方探一下执行方在不在）不记
//! - 请求一个处理完才接下一个；处理中途后来者在队列里等。空闲时每 [`POLL`] 把事件流喂进常驻的 Store，
//!   不让事件在通道里越攒越多
//! - 时间字段（`at`、`elapsed_ms`、`first_chat_ms`）在这边当场生成：`elapsed_ms` 从收到请求起算，
//!   `first_chat_ms` 是执行方启动到这个列表第一次有会话（主列表、归档从启动起就在记，不是「第一次被问」的时刻）；
//!   其余与直跑逐字节一致（同一段渲染代码）

use crate::args::{ChatsArgs, Format, Global, HistoryArgs, LinksArgs, WatchArgs};
use crate::chats::{self, Feed, Limits};
use crate::forward::FwdArgs;
use crate::login::{self, AuthSub, Prompter};
use crate::out::{Code, Fail, Sink, Stdio, emit_to, envelope, fail_to, fail_with_to};
use crate::session::Job;
use crate::watch::{LineOut, Sub, Watcher};
use atg_core::backend::{Backend, SHUTDOWN_TIMEOUT};
use atg_core::config::Profile;
use atg_core::error::CoreError;
use atg_core::ipc::{Accepted, Conn, Frame, Listener, PROTOCOL_V, ReadError, Request};
use atg_core::link::{EventStream, block_on};
use atg_core::model::CoreEvent;
use atg_core::secret::Secret;
use atg_core::status::{AuthStep, CoreState};
use serde_json::json;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// 空闲时多久看一次退出标志、喂一次事件流
pub const POLL: Duration = Duration::from_millis(200);

/// `atg-cli serve` 入口
pub fn run(g: &Global, format: Format, started: Instant) -> ExitCode {
    if g.fake.is_some() {
        return ExitCode::from(fail_with_to(
            &mut Stdio,
            format,
            &Fail::NotImplemented {
                command: "serve",
                message_zh: "假后端的场景目录还没做；serve 要用真实账号或测试服务器（--test-dc）",
            },
        ));
    }
    #[cfg(not(feature = "tdlib"))]
    {
        let _ = started;
        ExitCode::from(fail_with_to(
            &mut Stdio,
            format,
            &Fail::NotImplemented {
                command: "serve",
                message_zh: "这个版本没编进 TDLib：cargo build -p atg-cli --features tdlib",
            },
        ))
    }
    #[cfg(feature = "tdlib")]
    {
        let profile = match crate::session::open_profile(g) {
            Ok(p) => p,
            Err(f) => return ExitCode::from(fail_with_to(&mut Stdio, format, &f)),
        };
        // 已经有常驻进程在服务这个档案：直说，别让人以为要重试
        if let Err(e @ CoreError::ProfileBusy { .. }) = profile.probe_lock()
            && let Some(m) = crate::forward::busy_reason(g, true)
        {
            return ExitCode::from(crate::out::busy_to(&mut Stdio, format, &e, m));
        }
        let b = match crate::session::td_backend() {
            Ok(b) => b,
            Err(f) => return ExitCode::from(fail_with_to(&mut Stdio, format, &f)),
        };
        if g.unsafe_direct_send {
            Stdio.err("atg-cli serve: 直发模式还没做（发送在第二阶段），这次运行照常确认");
        }
        crate::signals::install();
        ExitCode::from(serve(
            &b,
            &profile,
            format,
            &crate::signals::STOP,
            &mut Stdio,
            Limits::DEFAULT,
            started,
        ))
    }
}

/// 起后端、开接口、处理请求直到 `stop`；返回退出码。自己的输出写 `own`
pub fn serve<B: Backend>(
    b: &B,
    profile: &Profile,
    format: Format,
    stop: &AtomicBool,
    own: &mut dyn Sink,
    limits: Limits,
    started: Instant,
) -> u8 {
    let mut stream = match b.subscribe() {
        Ok(s) => s,
        Err(e) => return fail_with_to(own, format, &Fail::Core(e)),
    };
    // 拿档案锁在 `start` 里：拿不到就 `profile_busy`，不开接口
    if let Err(e) = block_on(b.start(profile)) {
        let _ = b.shutdown(SHUTDOWN_TIMEOUT);
        return fail_with_to(own, format, &Fail::Core(e));
    }
    let listener = match Listener::bind(profile) {
        Ok(l) => l,
        Err(e) => {
            let _ = b.shutdown(SHUTDOWN_TIMEOUT);
            return fail_with_to(own, format, &Fail::Core(e));
        }
    };
    // 等授权线离开「等参数」再报状态（等不到也照常开始，状态如实报）
    let _ = b.wait_auth(chats::AUTH_TIMEOUT, &|s| s != AuthStep::WaitParameters);
    status_line(b, format, own);

    let kind = profile.kind();
    let mut feed = Feed::new(started);
    let (mut served, mut refused) = (0u64, 0u64);
    let mut faulted = false;
    while !stop.load(Ordering::SeqCst) {
        match listener.accept(POLL) {
            Ok(None) => {
                feed.drain(&mut stream);
            }
            Ok(Some(Accepted::Refused)) => {
                refused += 1;
                own.err("atg-cli serve: 拒绝了一个不是本用户的连接");
            }
            Ok(Some(Accepted::Conn(c))) => {
                let ctx = Ctx {
                    b,
                    kind,
                    limits,
                    stream: &mut stream,
                    feed: &mut feed,
                };
                if handle(ctx, c, own) {
                    served += 1;
                }
            }
            // 接不了（如文件句柄用完）：歇一下再接
            Err(_) => std::thread::sleep(POLL),
        }
        crate::watch::fetch_topics(b, &mut feed);
        for c in feed.hub.reap() {
            close_chat(b, c, own);
        }
        if b.status().lines.core == CoreState::Faulted {
            faulted = true;
            break;
        }
    }
    // 挂着的 watch 一并收尾：正常停是 result（interrupted）+ 0；故障是 disconnected
    let closing = if faulted {
        feed.hub.fail_all(&Fail::Core(CoreError::Disconnected))
    } else {
        feed.hub.finish_all()
    };
    if !faulted {
        for c in closing {
            close_chat(b, c, own);
        }
    }
    // 先删套接字再放锁（关后端时放），中间没有「锁空着、旧套接字还在」的窗口让后来者误连
    drop(listener);
    if let Err(e) = b.shutdown(SHUTDOWN_TIMEOUT) {
        own.err(&format!("atg-cli serve: 关闭时出错（{}）", e.code()));
    }
    if faulted {
        return fail_with_to(own, format, &Fail::Core(CoreError::Disconnected));
    }
    match format {
        Format::Json => emit_to(
            own,
            &envelope(
                "result",
                json!({
                    "command": "serve",
                    "reason": "interrupted",
                    "served": served,
                    "refused": refused,
                    "elapsed_ms": crate::elapsed_ms(started),
                }),
            ),
        ),
        Format::Text => own.out(&format!(
            "已停止；处理了 {served} 个请求，拒绝了 {refused} 个连接"
        )),
    }
    0
}

/// 关掉替 `watch` 开的会话（与 `chat_open` 配对）；出错只记一句
fn close_chat<B: Backend>(b: &B, chat: atg_core::model::ChatId, own: &mut dyn Sink) {
    if let Err(e) = block_on(b.chat_close(chat)) {
        own.err(&format!("atg-cli serve: 关闭会话时出错（{}）", e.code()));
    }
}

/// 起好后的那一行 `status`
fn status_line<B: Backend>(b: &B, format: Format, own: &mut dyn Sink) {
    let r = b.status();
    match format {
        Format::Json => emit_to(
            own,
            &envelope(
                "status",
                json!({
                    "core": r.lines.core,
                    "auth": r.lines.auth,
                    "net": r.lines.net,
                    "user_status": r.status,
                    "pending_sends": [],
                    "direct_send": false,
                    "direct_sent": 0,
                    "listening": true,
                }),
            ),
        ),
        Format::Text => own.out("已开本机接口，等别的 atg-cli 转命令过来；Ctrl+C 结束"),
    }
}

/// 处理一个请求要用的东西
struct Ctx<'a, B: Backend> {
    b: &'a B,
    kind: atg_core::config::ServerKind,
    limits: Limits,
    stream: &'a mut EventStream,
    feed: &'a mut Feed,
}

/// 读请求 → 执行 → 写回 `exit` 帧；在自己的标准错误记一句（不记参数）。没读到请求（探一下就断、超时）回 `false`
fn handle<B: Backend>(ctx: Ctx<'_, B>, mut conn: Conn, own: &mut dyn Sink) -> bool {
    let started = Instant::now();
    // 先只当 JSON 读，看版本号；对上了再按请求的形状解析（以后的版本加了字段、改了 origin 也能说清是版本不对）
    let first = conn.recv::<serde_json::Value>();
    // `watch` 是流式的：连接交给订阅者表，不占着这个「一个处理完才接下一个」的循环（协议 v0「流式订阅」）
    if let Ok(v) = &first
        && v.get("v").and_then(serde_json::Value::as_u64) == Some(u64::from(PROTOCOL_V))
        && let Ok(req) = serde_json::from_value::<Request>(v.clone())
        && req.command == "watch"
    {
        join_watch(ctx, conn, &req, own);
        return true;
    }
    let mut sink = ConnSink {
        conn: &mut conn,
        broken: false,
    };
    let bad = |what| Fail::Core(CoreError::Internal { what });
    let (name, code) = match first {
        // 连上就断：发起方探一下执行方在不在
        Err(ReadError::Closed) => return false,
        Err(ReadError::TimedOut | ReadError::Io) => {
            own.err("atg-cli serve: 等请求超时，已断开");
            return false;
        }
        Err(ReadError::Malformed) => (
            "?",
            fail_with_to(&mut sink, Format::Json, &bad("ipc_bad_request")),
        ),
        Ok(v) if v.get("v").and_then(serde_json::Value::as_u64) != Some(u64::from(PROTOCOL_V)) => (
            "?",
            fail_with_to(&mut sink, Format::Json, &bad("ipc_version_mismatch")),
        ),
        Ok(v) => match serde_json::from_value::<Request>(v) {
            Ok(req) => execute(ctx, &req, &mut sink, started),
            Err(_) => (
                "?",
                fail_with_to(&mut sink, Format::Json, &bad("ipc_bad_request")),
            ),
        },
    };
    let broken = sink.broken;
    if !broken {
        let _ = conn.send(&Frame::Exit { code });
    }
    let gone = if broken {
        "；对方没读完就走了"
    } else {
        ""
    };
    own.err(&format!(
        "atg-cli serve: {name} → 退出码 {code}（{} 毫秒）{gone}",
        crate::elapsed_ms(started)
    ));
    true
}

/// 一个 `watch` 接进来：检查参数 → 写 `status` → 挂进订阅者表（之后喂的每条事件都写给它）→ 有 `--chat` 就替它开会话。
/// 中途出错就写一行 `error` + `exit`，不挂。执行方已经补发完的，先补一行 `sync_done`，之后的都算新消息
fn join_watch<B: Backend>(ctx: Ctx<'_, B>, mut conn: Conn, req: &Request, own: &mut dyn Sink) {
    let bad = |what| Fail::Core(CoreError::Internal { what });
    let mut refuse = |conn: &mut Conn, format: Format, f: &Fail| {
        let mut sink = ConnSink {
            conn,
            broken: false,
        };
        let code = fail_with_to(&mut sink, format, f);
        if !sink.broken {
            let _ = sink.conn.send(&Frame::Exit { code });
        }
        own.err(&format!("atg-cli serve: watch → 退出码 {code}"));
    };
    let Ok(fa) = serde_json::from_value::<FwdArgs>(req.args.clone()) else {
        return refuse(&mut conn, Format::Json, &bad("ipc_bad_request"));
    };
    let format = fa.format;
    let Ok(a) = serde_json::from_value::<WatchArgs>(fa.params) else {
        return refuse(&mut conn, format, &bad("ipc_bad_request"));
    };
    let parsed = crate::watch::query(&a)
        .and_then(|q| crate::history::speech_ctx(a.tz.as_deref(), a.reveal).map(|cx| (q, cx)));
    let (q, cx) = match parsed {
        Ok(x) => x,
        Err(f) => return refuse(&mut conn, format, &f),
    };
    if ctx.feed.hub.len() >= crate::watch::MAX_SUBSCRIBERS {
        return refuse(
            &mut conn,
            format,
            // 容量满了不是参数错，但第一阶段没有更贴切的码；说明里讲清楚
            &Fail::Usage(WATCH_FULL_ZH),
        );
    }
    if let Err(f) = chats::ensure_ready(ctx.b) {
        return refuse(&mut conn, format, &f);
    }
    let mut out = ConnOut { conn };
    if !out.line(&crate::watch::status_line(ctx.b, format)) {
        return;
    }
    // 先把流里已到的喂完（给已挂着的订阅者），再按 Feed 自己喂到的状态判「已补发完」：
    // 后端的连接线可能已到 ready，补发来的消息和 sync_done 却还排在流里
    ctx.feed.drain(ctx.stream);
    let mut watcher = Watcher::new(cx, format, None, crate::watch::now_ms);
    if ctx.feed.synced() {
        watcher.mark_synced();
        // 执行方早已补发完：它那条 sync_done 这个订阅者没看到，补一行（序号沿用最后喂过的那个）
        if let Some(line) =
            watcher.render(ctx.feed.store(), ctx.feed.last_seq(), &CoreEvent::SyncDone)
            && !out.line(&line)
        {
            return;
        }
    } else if ctx.feed.ever_synced() {
        watcher.mark_reconnecting();
    }
    ctx.feed.hub.add(Sub {
        out: Box::new(out),
        watcher,
        chat: None,
        dead: false,
    });
    if let Some(c) = q.chat {
        let opened = crate::watch::resolve(ctx.b, ctx.stream, ctx.feed, c).and_then(|id| {
            // 同一会话已有别的订阅者替它开着：不再开（关时按引用计数配对）
            if ctx.feed.hub.opened(id) == 0 {
                block_on(ctx.b.chat_open(id)).map_err(Fail::Core)?;
            }
            Ok(id)
        });
        match opened {
            Ok(id) => ctx.feed.hub.open_chat(id),
            Err(f) => {
                let code = ctx.feed.hub.fail_last(&f);
                own.err(&format!("atg-cli serve: watch → 退出码 {code}"));
                return;
            }
        }
    }
    own.err(&format!(
        "atg-cli serve: watch 接入（现有 {} 个）",
        ctx.feed.hub.len()
    ));
}

/// 订阅者表满了（上限 [`crate::watch::MAX_SUBSCRIBERS`]）
const WATCH_FULL_ZH: &str = "执行方上同时挂着的 watch 已到上限，先停掉一个再订阅";

/// 订阅者的出口：本机接口的连接
struct ConnOut {
    conn: Conn,
}

impl LineOut for ConnOut {
    fn line(&mut self, line: &str) -> bool {
        self.conn
            .send(&Frame::Out {
                line: line.to_owned(),
            })
            .is_ok()
    }

    fn gone(&self) -> bool {
        atg_core::ipc::peer_closed(self.conn.stream())
    }

    fn finish(&mut self, code: u8) {
        let _ = self.conn.send(&Frame::Exit { code });
    }
}

/// 按命令名派发；返回（记日志用的命令名，退出码）
fn execute<B: Backend>(
    ctx: Ctx<'_, B>,
    req: &Request,
    sink: &mut dyn Sink,
    started: Instant,
) -> (&'static str, u8) {
    let bad = |what| Fail::Core(CoreError::Internal { what });
    let Ok(fa) = serde_json::from_value::<FwdArgs>(req.args.clone()) else {
        return (
            "?",
            fail_with_to(sink, Format::Json, &bad("ipc_bad_request")),
        );
    };
    let format = fa.format;
    match req.command.as_str() {
        "chats" => {
            let code = match serde_json::from_value::<ChatsArgs>(fa.params) {
                Err(_) => fail_with_to(sink, format, &bad("ipc_bad_request")),
                // serde 绕过了 clap 的范围检查：这里补上
                Ok(a) if !(1..=100_000).contains(&a.limit) => {
                    fail_with_to(sink, format, &Fail::Usage("-n 要在 1 到 100000 之间"))
                }
                Ok(a) => match crate::chats_query(&a)
                    .and_then(|q| chats::collect(ctx.b, ctx.stream, ctx.feed, &q, ctx.limits))
                {
                    Ok(l) => crate::render_chats(sink, format, &a, &l, started),
                    Err(f) => fail_with_to(sink, format, &f),
                },
            };
            ("chats", code)
        }
        "history" => {
            // 参数检查（含 clap 管的范围）都在 `history::query`：serde 绕过了 clap 也照样拦
            let code = match serde_json::from_value::<HistoryArgs>(fa.params) {
                Err(_) => fail_with_to(sink, format, &bad("ipc_bad_request")),
                Ok(a) => match crate::history::query(&a).and_then(|q| {
                    crate::history::speech_ctx(a.tz.as_deref(), a.reveal).map(|cx| (q, cx))
                }) {
                    Err(f) => fail_with_to(sink, format, &f),
                    Ok((q, cx)) => match crate::history::collect(
                        ctx.b,
                        ctx.stream,
                        ctx.feed,
                        &q,
                        ctx.limits.sync,
                    ) {
                        Ok(p) => crate::history::render(sink, format, &a, &cx, &p, started),
                        Err(f) => fail_with_to(sink, format, &f),
                    },
                },
            };
            ("history", code)
        }
        "links" => {
            let code = match serde_json::from_value::<LinksArgs>(fa.params) {
                Err(_) => fail_with_to(sink, format, &bad("ipc_bad_request")),
                Ok(a) => match crate::history::links_query(&a)
                    .and_then(|q| crate::history::collect_links(ctx.b, ctx.stream, ctx.feed, &q))
                {
                    Ok(l) => crate::history::render_links(sink, format, a.reveal, &l, started),
                    Err(f) => fail_with_to(sink, format, &f),
                },
            };
            ("links", code)
        }
        "auth" if fa.params["sub"] == "state" => {
            let r = login::auth_step(
                ctx.b,
                ctx.kind,
                AuthSub::State,
                None,
                &mut NoSecrets,
                &mut |s| crate::emit_step(sink, format, s),
            );
            let job = Job::Auth(AuthSub::State);
            ("auth", crate::job_output(sink, format, job, r, started))
        }
        // 登录交秘密（秘密不过接口）；登出会改执行方自己的状态，也只在执行方那边做
        name @ ("auth" | "login" | "logout") => (
            match name {
                "login" => "login",
                "logout" => "logout",
                _ => "auth",
            },
            fail_to(
                sink,
                format,
                Code::Usage,
                "登录、登出不经本机接口：请先停掉占着档案的进程再跑",
                json!({}),
            ),
        ),
        _ => (
            "?",
            fail_to(
                sink,
                format,
                Code::NotImplemented,
                "这条命令还不能经本机接口执行",
                json!({ "command": req.command }),
            ),
        ),
    }
}

/// 执行方这边绝不问秘密：`auth state` 本来就不问；万一走到问的路径就当用法错误
struct NoSecrets;

impl Prompter for NoSecrets {
    fn say(&mut self, _: &str) {}

    fn ask_secret(&mut self, _: &str) -> Result<Secret, Fail> {
        Err(Fail::Usage("秘密不过本机接口"))
    }

    fn confirm(&mut self, _: &str) -> Result<bool, Fail> {
        Err(Fail::Usage("确认不过本机接口"))
    }
}

/// 写进连接的出口。对方走了（写失败）就不再写，命令照常做完
struct ConnSink<'a> {
    conn: &'a mut Conn,
    broken: bool,
}

impl ConnSink<'_> {
    fn send(&mut self, f: &Frame) {
        if !self.broken && self.conn.send(f).is_err() {
            self.broken = true;
        }
    }
}

impl Sink for ConnSink<'_> {
    fn out(&mut self, line: &str) {
        self.send(&Frame::Out {
            line: line.to_owned(),
        });
    }

    fn err(&mut self, line: &str) {
        self.send(&Frame::Err {
            line: line.to_owned(),
        });
    }
}

#[cfg(test)]
mod tests {
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
            logged_in: true,
            ..FakeAuth::default()
        });
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
                    for k in ["elapsed_ms", "first_chat_ms"] {
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

    #[test]
    fn secrets_and_unknown_commands_are_not_executed() {
        let p = profile("deny", ServerKind::Main);
        let s = start_server(&p, Format::Json);
        // 交验证码这类不转：执行方也不执行
        let (code, f) = s.ask(Format::Json, "auth", &json!({"sub": "code"}));
        assert_eq!(code, 2);
        assert!(f.out[0].contains("\"usage\""));
        let (code, f) = s.ask(Format::Json, "send", &json!({}));
        assert_eq!(code, 1);
        let v: Value = serde_json::from_str(&f.out[0]).unwrap();
        assert_eq!(v["data"]["code"], "not_implemented");
        assert_eq!(v["data"]["command"], "send");
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
        assert_eq!(r["data"]["served"], 7);
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
        let (out1, stop1, c1) =
            watch_client(&p, json!({"chat": "1", "reveal": false, "tz": "+08:00"}));
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
}
