//! `serve`：常驻的执行方（M1 阶段由 CLI 当；M3.1 起 GUI 也当）。契约：`config-privacy.md` 第一节第 2 小节「本机接口」。
//!
//! 拿档案锁、起后端 → 在 `profiles/<档案>/atg.sock` 听 → 一个一个处理后起的 `atg-cli` 转来的命令 → Ctrl+C / SIGTERM
//! 正常关（删套接字、关后端；正在处理的那个请求先做完）。再按一次 Ctrl+C / SIGTERM 立刻以 130 退出，不出结尾行
//! （正在处理的请求的发起方收到 `disconnected`；套接字留下，下次启动清掉）。只有 `serve` 开接口；一次性命令占着档案的那几秒不听。
//!
//! - 标准输出：起好后一行 `status`（带 `listening:true`），结束时一行 `result`（`reason:"interrupted"`，退出码 0，
//!   同 `watch` 被打断算正常结束）；后端故障就以 `disconnected` 结尾，好让人重开
//! - 标准错误：每处理一个请求记一句「命令名 · 退出码 · 耗时」，不记参数（参数以后会带内容）；`watch` 接入、离开各记一句现有几个（离开那句在摘掉、关完会话之后才记，测试按它认「摘掉做完了」）
//! - 能执行的见 `specs/contracts/config-privacy.md` 第一节第 2 小节「现在能转的」（正本）。发送类的凭据由本进程的闸开；流式的 `watch` 挂进订阅者表（`join_watch`）。登录、登出与其余授权步骤不经接口，回 `usage`；不认的命令回 `not_implemented`；
//!   版本号不对、请求形状不对回 `internal{ipc_version_mismatch | ipc_bad_request}`（这时还不知道要哪种格式，一律 JSON）
//! - 只连上就断的（发起方探一下执行方在不在）不记
//! - 请求一个处理完才接下一个；处理中途后来者在队列里等。空闲时每 [`POLL`] 把事件流喂进常驻的 Store，不让事件在通道里越攒越多
//! - 时间字段（`at`、`elapsed_ms`、`first_chat_ms`）在这边当场生成：`elapsed_ms` 从收到请求起算，
//!   `first_chat_ms` 是执行方启动到这个列表第一次有会话（主列表、归档从启动起就在记，不是「第一次被问」的时刻）；
//!   其余与直跑逐字节一致（同一段渲染代码）

use crate::args::{ChatsArgs, Format, Global, HistoryArgs, LinksArgs, WatchArgs};
use crate::chats::{self, Feed, Limits};
use crate::deadlines::{AUTH_TIMEOUT, START_TIMEOUT, bounded};
use crate::forward::FwdArgs;
use crate::login::{self, AuthSub, Prompter};
use crate::out::{Code, Fail, Sink, Stdio, emit_to, envelope, fail_to, fail_with_to};
use crate::session::Job;
use crate::watch::{LineOut, Sub, Watcher};
use atg_core::backend::{Backend, SHUTDOWN_TIMEOUT};
use atg_core::config::Profile;
use atg_core::error::CoreError;
use atg_core::ipc::{Accepted, Conn, Frame, Listener, PROTOCOL_V, ReadError, Request};
use atg_core::link::EventStream;
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
                message_zh: "假后端场景现在只接了 chats、history；serve 要用真实账号或测试服务器（--test-dc）",
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
    if let Err(e) = bounded(b, b.start(profile), START_TIMEOUT) {
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
    let _ = b.wait_auth(AUTH_TIMEOUT, &|s| s != AuthStep::WaitParameters);
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
        crate::watch::fetch_topics(b, &mut feed, stop);
        let before = feed.hub.len();
        crate::lease::close(b, feed.hub.reap(), "atg-cli serve", own);
        if let Some(n) = Some(feed.hub.len()).filter(|n| *n < before) {
            own.err(&format!("atg-cli serve: watch 离开（现有 {n} 个）"));
        }
        if b.status().lines.core == CoreState::Faulted {
            faulted = true;
            break;
        }
    }
    // 挂着的 watch 一并收尾：正常停是 result（interrupted）+ 0；故障是 disconnected、不关会话（请求发不出去）
    if faulted {
        feed.hub.fail_all(&Fail::Core(CoreError::Disconnected));
    } else {
        crate::lease::close(b, feed.hub.finish_all(), "atg-cli serve", own);
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
                    "direct_send": true,
                    "direct_sent": b.gate().direct_sent(),
                    "listening": true,
                }),
            ),
        ),
        Format::Text => own.out("已开本机接口，等别的 atg-cli 转命令过来；Ctrl+C 结束"),
    }
}

/// 处理一个请求要用的东西
pub(crate) struct Ctx<'a, B: Backend> {
    pub(crate) b: &'a B,
    kind: atg_core::config::ServerKind,
    limits: Limits,
    pub(crate) stream: &'a mut EventStream,
    pub(crate) feed: &'a mut Feed,
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
            // 同一会话已有别人替它开着：不再开（关时按租约配对）
            crate::lease::open(ctx.b, ctx.feed.hub.leases(), id).map_err(Fail::Core)?;
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
    let n = ctx.feed.hub.len();
    own.err(&format!("atg-cli serve: watch 接入（现有 {n} 个）"));
}

/// 订阅者表满了（上限 [`crate::watch::MAX_SUBSCRIBERS`]）
const WATCH_FULL_ZH: &str = "执行方上同时挂着的 watch 已到上限，先停掉一个再订阅";

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
        // 走发送外壳的（L2 五条 + 只读的 `pending` 列表）：同一个出口（`serve/l2.rs`）
        c @ ("send" | "resend" | "forward" | "pending" | "pending_cancel" | "delete" | "edit") => {
            l2::execute(
                ctx.b, ctx.stream, ctx.feed, c, fa.params, sink, format, started,
            )
        }
        "react" | "reactions" => crate::react::serve(ctx, &req.command, fa, sink, started),
        "vote" => crate::vote::serve(ctx, fa, sink, started),
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
                // 执行方收尾时也先做完手上的请求（`cli.md` `serve`）：不看它的停止标志，只按期限等
                &AtomicBool::new(false),
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

// 写进本机接口连接的两种出口：见 [`conn`]（从本文件拆出，#60 行数线）
mod conn;
use conn::{ConnOut, ConnSink};

mod l2;
#[cfg(test)]
mod tests;
