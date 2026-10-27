//! 发起方：档案被别的进程占着时，把命令经本机接口转给它执行（`config-privacy.md` 第一节第 2 小节「本机接口」）。
//!
//! 1. 参数在本进程按同一份定义解析好（用法错误在本地就报，和直跑一样）
//! 2. 探档案锁（共享锁，只探不写）：没人占 → 不转，直跑；被占 → 连 `atg.sock`
//! 3. 连上：发一行请求，把应答的 `out` / `err` 帧原样写到本进程的标准输出 / 标准错误，按 `exit` 帧的码退出
//! 4. 连不上：再探一次锁，已经空了（占着的一次性命令刚退）就改直跑；还占着 → `profile_busy`，不抢锁，
//!    说明里提示稍后重试或常开 `serve`
//! 5. 连上了却没等到 `exit` 帧就断了，或两帧之间 [`FRAME_TIMEOUT`] 没动静 → `disconnected`，退出码 4
//!
//! 只转只读且不带秘密的命令：`chats`、`history`、`links`、`auth state`。`--unsafe-direct-send` 不随请求过去：直发听执行方的（N1），
//! 连上之后在标准错误说一声。档案不存在、种类不对等情况不在这里报，交回直跑的路径照常报。
//!
//! 不转的命令（登录这组、`serve` 自己）撞上占着档案的常驻进程时，用 [`busy_reason`] 换一句对路的说明。

use crate::args::{Format, Global};
use crate::out::{Code, Sink, Stdio, busy_to, emit_to, envelope, fail_to};
use atg_core::config::{self, Profile, ServerKind};
use atg_core::error::CoreError;
use atg_core::ipc::{self, Connect, Frame, Origin, PROTOCOL_V, ReadError, Request};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

/// 请求里 `args` 的形状：输出格式 + 子命令自己的参数
#[derive(Serialize, Deserialize)]
pub struct FwdArgs {
    pub format: Format,
    #[serde(default)]
    pub params: Value,
}

/// 两帧之间最多等多久：一条 `chats` 最长约 30 秒（等授权 + 加载 + 等补发各 10 秒），执行方还可能先在处理
/// 别人的请求，留足两条的量
pub const FRAME_TIMEOUT: Duration = Duration::from_secs(90);

/// 执行方中途退出时给人的话
pub const EXECUTOR_GONE_ZH: &str = "替你执行的进程中途退出了，结果不完整，请重试";
/// 执行方太久没回话
pub const EXECUTOR_SILENT_ZH: &str = "替你执行的进程 90 秒没回话，已放弃；可以重试";
/// 占着档案的进程没开本机接口（一次性命令）
pub const BUSY_ONESHOT_ZH: &str =
    "本地档案正被另一个命令占着，它不接转发；稍后重试，或先开着 atg-cli serve";
/// 占着档案的是常驻进程，但这条命令不经本机接口（登录这组）
pub const BUSY_NOT_FORWARDED_ZH: &str =
    "本地档案正被常驻进程（atg-cli serve 或界面）占着；登录、登出不经本机接口，请先停掉它再跑";
/// 已经有常驻进程在服务这个档案，又起一个 `serve`
pub const BUSY_SERVE_RUNNING_ZH: &str = "这个档案已经有常驻进程在服务，不用再开 serve";

/// 档案被占且接口连得上就转过去，返回退出码；该直跑时回 `None`
pub fn try_forward(g: &Global, format: Format, command: &str, params: &Value) -> Option<u8> {
    let (conn, busy) = locate(g)?;
    Some(forward(
        conn,
        &busy,
        format,
        command,
        params,
        g.unsafe_direct_send,
        &mut Stdio,
    ))
}

/// 同 [`try_forward`]，给流式的 `watch`：没有帧超时（事件可能很久才来一个），`stop`（Ctrl+C）时在本地出结尾行
pub fn try_forward_stream(
    g: &Global,
    format: Format,
    command: &str,
    params: &Value,
    stop: &'static AtomicBool,
    started: Instant,
) -> Option<u8> {
    let (conn, busy) = locate(g)?;
    Some(forward_stream(
        conn, &busy, format, command, params, stop, started, &mut Stdio,
    ))
}

/// 档案被占着：连执行方（连上没连上都交回，另带连不上时要报的 `profile_busy`）；该直跑时回 `None`
fn locate(g: &Global) -> Option<(Connect, CoreError)> {
    if g.fake.is_some() {
        return None;
    }
    let profile = existing_profile(g)?;
    let busy = match profile.probe_lock() {
        Ok(()) => return None,
        Err(e @ CoreError::ProfileBusy { .. }) => e,
        // 锁文件读写出错：交给直跑的路径照常报
        Err(_) => return None,
    };
    let conn = ipc::connect(&profile);
    // 连不上且锁已经空了：占着的一次性命令刚好退出，改为直跑
    if matches!(conn, Connect::Unreachable) && profile.probe_lock().is_ok() {
        return None;
    }
    Some((conn, busy))
}

fn request(format: Format, command: &str, params: &Value) -> Request {
    Request {
        v: PROTOCOL_V,
        origin: Origin::Cli,
        command: command.to_owned(),
        args: serde_json::to_value(FwdArgs {
            format,
            params: params.clone(),
        })
        .unwrap_or_default(),
    }
}

/// 流式转发（协议 v0「流式订阅」，大总管定 2026-10-26）：执行方一直回 `out` 帧，直到它收尾发 `exit`。
/// 收帧放在另一条线程（读是阻塞的，不设超时），这边每 [`STREAM_POLL`] 看一次 `stop`：Ctrl+C 时不等执行方，
/// 在本地写结尾行 `result`（`reason:"interrupted"`）、以 0 退出；连接随进程关掉，执行方看到断开就摘掉这个订阅
#[allow(clippy::too_many_arguments)]
pub fn forward_stream(
    conn: Connect,
    busy: &CoreError,
    format: Format,
    command: &str,
    params: &Value,
    stop: &AtomicBool,
    started: Instant,
    sink: &mut dyn Sink,
) -> u8 {
    let Connect::Ok(mut conn) = conn else {
        return busy_to(sink, format, busy, BUSY_ONESHOT_ZH);
    };
    if conn.send(&request(format, command, params)).is_err() {
        return busy_to(sink, format, busy, BUSY_ONESHOT_ZH);
    }
    // 留一个把手：Ctrl+C 时主动关掉连接，收帧线程随之醒来退出，执行方也立刻看到断开
    let handle = conn.stream().try_clone().ok();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::Builder::new()
        .name("atg-fwd-recv".into())
        .spawn(move || {
            loop {
                let f = conn.recv::<Frame>();
                let end = !matches!(f, Ok(Frame::Out { .. } | Frame::Err { .. }));
                if tx.send(f).is_err() || end {
                    return;
                }
            }
        });
    if reader.is_err() {
        return fail_to(
            sink,
            format,
            Code::ExecutorGone,
            EXECUTOR_GONE_ZH,
            json!({}),
        );
    }
    let (mut events, mut alerts, mut first) = (0u64, 0u64, true);
    loop {
        match rx.recv_timeout(STREAM_POLL) {
            Ok(Ok(Frame::Out { line })) => {
                // 第一行是 status，不算事件
                if !std::mem::take(&mut first) {
                    count_line(format, &line, &mut events, &mut alerts);
                }
                // 标准输出的读方走了（`watch | head`）：同直跑，不再写、关掉连接（执行方随之摘掉订阅、关会话），以 0 退出
                if !sink.out_ok(&line) {
                    if let Some(h) = &handle {
                        let _ = h.shutdown(std::net::Shutdown::Both);
                    }
                    return 0;
                }
            }
            Ok(Ok(Frame::Err { line })) => sink.err(&line),
            Ok(Ok(Frame::Exit { code })) => return code,
            Ok(Err(_)) | Err(RecvTimeoutError::Disconnected) => {
                return fail_to(
                    sink,
                    format,
                    Code::ExecutorGone,
                    EXECUTOR_GONE_ZH,
                    json!({}),
                );
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        if stop.load(Ordering::SeqCst) {
            if let Some(h) = &handle {
                let _ = h.shutdown(std::net::Shutdown::Both);
            }
            match format {
                Format::Json => emit_to(
                    sink,
                    &envelope(
                        "result",
                        json!({
                            "command": command,
                            "reason": "interrupted",
                            "events": events,
                            "alerts": alerts,
                            "elapsed_ms": crate::elapsed_ms(started),
                        }),
                    ),
                ),
                Format::Text => sink.out(&format!(
                    "已停止；写出了 {events} 条事件，其中 {alerts} 条要提示"
                )),
            }
            return 0;
        }
    }
}

/// 流式转发时多久看一次 Ctrl+C
pub const STREAM_POLL: Duration = Duration::from_millis(200);

/// 本地结尾行要的计数，口径同直跑（只数写出来的事件行）：JSON 模式数 `event` 行和其中 `notify.alert` 为真的；
/// 文字模式见下
pub(crate) fn count_line(format: Format, line: &str, events: &mut u64, alerts: &mut u64) {
    match format {
        Format::Json => {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                return;
            };
            if v["kind"] == "event" {
                *events += 1;
                if v["data"]["notify"]["alert"] == true {
                    *alerts += 1;
                }
            }
        }
        // 文字模式：每行一个事件；要提示的是「新消息，……」「对方正在输入」和重连汇总「断网期间收到……」三种（同 `watch::Watcher::render`）
        Format::Text => {
            *events += 1;
            if line.starts_with(crate::watch::NEW_PREFIX_ZH)
                || line == crate::watch::TYPING_ZH
                || line.starts_with(crate::watch::MISSED_PREFIX_ZH)
            {
                *alerts += 1;
            }
        }
    }
}

/// 不转的命令撞上档案被占时，该换成哪句话：占着的进程接口连得上 → 它是常驻的（`serve` 或界面）。
/// 连一下就断，执行方读到对端已关，不当错误记
pub fn busy_reason(g: &Global, serve: bool) -> Option<&'static str> {
    let profile = existing_profile(g)?;
    matches!(ipc::connect(&profile), Connect::Ok(_)).then_some(if serve {
        BUSY_SERVE_RUNNING_ZH
    } else {
        BUSY_NOT_FORWARDED_ZH
    })
}

/// 只打开已有的档案，不新建（没有的档案不会被谁占着）；`--test-dc` 打开的不是测试档案也交回直跑去报
fn existing_profile(g: &Global) -> Option<Profile> {
    let root = config::default_root().ok()?;
    let p = Profile::open(&root, crate::session::profile_name(g)).ok()?;
    (!g.test_dc || p.kind() == ServerKind::Test).then_some(p)
}

/// 发请求、转写应答，返回退出码。`busy` 是连不上时报的那个 `profile_busy`；`note_direct`：本次带了
/// `--unsafe-direct-send`，连上后在标准错误说明它在执行方那边不起作用
pub fn forward(
    conn: Connect,
    busy: &CoreError,
    format: Format,
    command: &str,
    params: &Value,
    note_direct: bool,
    sink: &mut dyn Sink,
) -> u8 {
    let Connect::Ok(mut conn) = conn else {
        return busy_to(sink, format, busy, BUSY_ONESHOT_ZH);
    };
    let req = request(format, command, params);
    // 请求没发出去：对面没在收（刚好在关），按连不上算
    if conn.send(&req).is_err() || conn.set_read_timeout(FRAME_TIMEOUT).is_err() {
        return busy_to(sink, format, busy, BUSY_ONESHOT_ZH);
    }
    if note_direct {
        sink.err(
            "atg-cli: 档案正被常驻进程占着，这条命令交给它执行；--unsafe-direct-send 在那边不起作用（直发听执行方的）",
        );
    }
    loop {
        match conn.recv::<Frame>() {
            Ok(Frame::Out { line }) => sink.out(&line),
            Ok(Frame::Err { line }) => sink.err(&line),
            Ok(Frame::Exit { code }) => return code,
            Err(e) => {
                let msg = match e {
                    ReadError::TimedOut => EXECUTOR_SILENT_ZH,
                    ReadError::Closed | ReadError::Io | ReadError::Malformed => EXECUTOR_GONE_ZH,
                };
                return fail_to(sink, format, Code::ExecutorGone, msg, json!({}));
            }
        }
    }
}
