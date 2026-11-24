//! `press`：按一条消息上的第 N 个按钮（#13 13b-2；契约 `specs/contracts/cli.md`）。
//!
//! `press CHAT_ID MSG_ID N [--again] [--dry-run]`：N 是 `history --detail` 详情里读的第几个按钮（从 1 起，跨行连续数）。
//! - 回调按钮：一行 `plan` → 闸开凭据（直发、不确认；测试服务器只许收藏夹）→ 交出 → 一行 `result`（带机器人的回话）。
//!   10 分钟内同一个按钮再按拦下（`duplicate_recent`），`--again` 放行。回话带网址的**自动打开**（人定 B 2026-11-24，
//!   照官方客户端），只开 `http` / `https`，别的只读出；打开经 [`execute`] 的 `open` 出口，测试里换成只记录
//! - 键盘文字按钮：转给 `send`（以你的名义发出按钮上的字），计划里说明按的是哪个按钮、键盘换没换过
//! - 网址 / 登录网址 / 复制文字 / 用户：只读出，一行 `result`
//! - 其余拒（`button_not_supported`）
//!
//! 能经本机接口转给占着档案的执行方（闸在执行方）。流程只在这里，GUI 以后同样调 `atg_core::send::press`。

use crate::args::{Format, Global, MsgRef};
use crate::chats::{self, Feed};
use crate::deadlines::{QUERY_TIMEOUT, bounded};
use crate::history;
use crate::out::{self, Fail, Sink, emit_to, envelope};
use crate::send::{KeyPress, SendParams};
use atg_core::backend::Backend;
use atg_core::error::CoreError;
use atg_core::link::EventStream;
use atg_core::model::MsgId;
use atg_core::send::press::{self, CallbackPlan, Press};
use atg_core::send::{self, CallbackReply, SendOrigin};
use atg_core::tree::{Button, ButtonAction};
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// `press` 的参数
#[derive(Args)]
pub struct PressArgs {
    #[command(flatten)]
    pub msg: MsgRef,
    /// 第几个按钮：`history --detail` 详情里读的顺序（从 1 起，跨行连续数）
    #[arg(value_name = "N")]
    pub button: usize,
    /// 按过同一个按钮会被拦下（已按下的看 10 分钟，结果未知的看 24 小时）；确要再按一次就加它
    #[arg(long)]
    pub again: bool,
    /// 只输出要按什么，不按
    #[arg(long)]
    pub dry_run: bool,
}

/// 一次 `press` 要的东西（经本机接口转发时原样序列化过去）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressParams {
    pub chat: String,
    pub message_id: i64,
    pub button: usize,
    #[serde(default)]
    pub again: bool,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub tz: Option<String>,
}

/// 参数检查（直跑与执行方都走这里：执行方收到的参数绕过了 clap）
pub fn check(p: &PressParams) -> Result<(), Fail> {
    history::chat_arg(&p.chat)?;
    if p.button == 0 {
        return Err(Fail::Usage(
            "按钮从 1 数起：history --detail 详情里每个按钮前念的「第 N 个」",
        ));
    }
    Ok(())
}

/// 机器人回话里的网址能不能打开：只开 `http` / `https`（人定 B 的约束），且不带空白与控制字符
pub fn openable(url: &str) -> bool {
    let low = url.to_ascii_lowercase();
    (low.starts_with("https://") || low.starts_with("http://"))
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// 真的打开：交给系统（macOS `open`），网址作为单独一个参数，不经 shell
fn system_open(url: &str) {
    let _ = std::process::Command::new("/usr/bin/open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn result_line(sink: &mut dyn Sink, format: Format, data: Value, zh: &str) {
    match format {
        Format::Json => emit_to(sink, &envelope("result", data)),
        Format::Text => sink.out(zh),
    }
}

/// 错误行：L2 的 `retryable` 一律为假
fn fail_line(sink: &mut dyn Sink, format: Format, f: &Fail) -> u8 {
    match f {
        Fail::Core(e) => {
            let mut data = out::core_error_data(e);
            data["retryable"] = json!(false);
            match format {
                Format::Json => emit_to(sink, &envelope("error", data)),
                Format::Text => sink.out(e.message_zh()),
            }
            out::core_exit(e)
        }
        other => out::fail_with_to(sink, format, other),
    }
}

/// 在已起好的后端上按按钮，输出写到 `sink`，返回退出码（直跑与执行方共用）。`open`：打开机器人回话里的网址
/// （只会拿到已过 [`openable`] 的；测试里换成只记录）
#[allow(clippy::too_many_arguments)]
pub fn execute<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &PressParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
    open: &mut dyn FnMut(&str),
) -> u8 {
    match run_press(b, stream, feed, p, sink, format, started, timeout, open) {
        Ok(code) => code,
        Err(f) => fail_line(sink, format, &f),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_press<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &PressParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
    open: &mut dyn FnMut(&str),
) -> Result<u8, Fail> {
    check(p)?;
    chats::ensure_ready(b)?;
    let chat_id = history::resolve_chat(b, history::chat_arg(&p.chat)?)?;
    history::load_chat(b, stream, feed, chat_id)?;
    let id = MsgId {
        chat_id,
        message_id: p.message_id,
    };
    let pressed = bounded(
        b,
        press::prepare_press(b, id, p.button, SendOrigin::Cli),
        QUERY_TIMEOUT,
    )
    .map_err(Fail::Core)?;
    match pressed {
        Press::ReadOnly { button } => {
            let (data, zh) = read_only(chat_id, p, &button, started);
            result_line(sink, format, data, &zh);
            Ok(0)
        }
        Press::Keyboard {
            text,
            message,
            reply,
            keyboard,
            ..
        } => {
            let sp = SendParams {
                chat: chat_id.to_string(),
                text,
                // 不是私聊：回复键盘所在那条，机器人才收得到（同官方客户端；话题群也因此有了回复对象）
                reply_to: reply.then_some(message.message_id),
                dry_run: p.dry_run,
                again: p.again,
                tz: p.tz.clone(),
                press: Some(KeyPress {
                    button: p.button,
                    message_id: p.message_id,
                    keyboard,
                }),
            };
            Ok(crate::send::execute_send(
                b, stream, feed, &sp, sink, format, started, timeout,
            ))
        }
        Press::Callback(plan) => callback(
            b, stream, feed, p, &plan, sink, format, started, timeout, open,
        ),
    }
}

/// 只读出的按钮：一行 `result`
fn read_only(chat_id: i64, p: &PressParams, button: &Button, started: Instant) -> (Value, String) {
    let (kind, value, zh) = match &button.action {
        ButtonAction::Url { url } => ("url", json!(url), format!("网址 {url}")),
        ButtonAction::LoginUrl { url } => {
            ("login_url", json!(url), format!("以你的身份登录网站 {url}"))
        }
        ButtonAction::CopyText { text } => ("copy_text", json!(text), format!("复制文字：{text}")),
        ButtonAction::User { user_id } => (
            "user",
            json!(user_id),
            format!("按下会打开这个用户的资料，用户 id {user_id}"),
        ),
        _ => ("other", Value::Null, String::new()),
    };
    let data = json!({"command": "press", "chat_id": chat_id, "message_id": p.message_id,
        "button": p.button, "pressed": false, "read_only": {"type": kind, "text": button.text, "value": value},
        "elapsed_ms": crate::elapsed_ms(started)});
    let zh = format!(
        "只读出，没按（本应用不替你按这种按钮，要用就自己去打开或复制）：第 {} 个按钮「{}」，{zh}",
        p.button,
        button.text.trim()
    );
    (data, zh)
}

#[allow(clippy::too_many_arguments)]
fn callback<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &PressParams,
    plan: &CallbackPlan,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
    open: &mut dyn FnMut(&str),
) -> Result<u8, Fail> {
    let who = match plan.sender() {
        atg_core::model::SenderId::User(u) => format!("（发这条的是 id {u}）"),
        atg_core::model::SenderId::Chat(c) => format!("（发这条的是会话 id {c}）"),
        atg_core::model::SenderId::Unknown { .. } => String::new(),
    };
    let what = format!(
        "按 {} 里 id {} 那条的第 {} 个按钮「{}」{who}",
        send::target_zh(plan.target()),
        plan.message().message_id,
        plan.button(),
        plan.text().trim(),
    );
    match format {
        Format::Json => emit_to(sink, &envelope("plan", press::press_plan_json(plan))),
        // 按下会怎样也要说：直发不再确认，这一句是人动手前唯一一次知情（自审第 2 轮 P2）
        Format::Text => sink.out(&format!(
            "将{what} = 把按钮上看不见的数据交给机器人、聊天里不出现，机器人可能回话或弹窗，回话带网页地址会自动打开；不用确认"
        )),
    }
    let base = json!({"command": "press", "chat_id": plan.target().id, "message_id": p.message_id,
        "button": p.button});
    if p.dry_run {
        let mut d = base;
        d["dry_run"] = json!(true);
        d["elapsed_ms"] = json!(crate::elapsed_ms(started));
        result_line(sink, format, d, "只看了计划，没有按");
        return Ok(0);
    }
    let token = b.gate().issue(plan, p.again).map_err(Fail::Core)?;
    let (mode, reply) = press::press_within(b, plan, token, timeout).map_err(Fail::Core)?;
    feed.drain(stream);
    let (reply_json, said, opened) = answer(reply.as_ref(), open);
    let mut d = base;
    d["pressed"] = json!(true);
    d["confirm"] = json!(mode);
    d["reply"] = reply_json;
    d["opened"] = json!(opened);
    d["elapsed_ms"] = json!(crate::elapsed_ms(started));
    result_line(sink, format, d, &format!("已{what}；{said}"));
    Ok(0)
}

/// 机器人的回话：JSON、读给人听的一句、开没开网址
fn answer(reply: Option<&CallbackReply>, open: &mut dyn FnMut(&str)) -> (Value, String, bool) {
    let Some(r) = reply else {
        return (Value::Null, "机器人没回话（按下已送到）".to_owned(), false);
    };
    let mut said = if r.text.trim().is_empty() {
        "机器人没带文字（可能没回话，按下已送到）".to_owned()
    } else if r.alert {
        format!("机器人回话（弹窗）：{}", r.text.trim())
    } else {
        format!("机器人回话：{}", r.text.trim())
    };
    let mut opened = false;
    if !r.url.is_empty() {
        if openable(&r.url) {
            open(&r.url);
            opened = true;
            said.push_str(&format!("；已把网址交给系统打开：{}", r.url));
        } else {
            said.push_str(&format!("；回话带的地址不是网页，没打开：{}", r.url));
        }
    }
    (
        json!({"text": r.text, "alert": r.alert, "url": r.url}),
        said,
        opened,
    )
}

/// `press` 直跑入口：档案被占就转给执行方（闸在执行方）
pub fn run(g: &Global, format: Format, a: &PressArgs, started: Instant) -> ExitCode {
    // plan 行带会话名与按钮文字，打码还没做：同 send，加 --redact 先拒
    if let Some(f) = crate::no_redact(g, "press") {
        return out::fail_with(format, &f);
    }
    let p = PressParams {
        chat: a.msg.chat.clone(),
        message_id: a.msg.message,
        button: a.button,
        again: a.again,
        dry_run: a.dry_run,
        tz: g.tz.clone(),
    };
    if let Err(f) = check(&p) {
        return out::fail_with(format, &f);
    }
    let params = serde_json::to_value(&p).unwrap_or_default();
    if let Some(code) = crate::forward::try_forward(g, format, "press", &params) {
        return ExitCode::from(code);
    }
    ExitCode::from(crate::session::l2_run(
        g,
        format,
        "press",
        started,
        &mut |b, stream, feed, sink| {
            let t = send::RESULT_TIMEOUT;
            execute(
                b,
                stream,
                feed,
                &p,
                sink,
                format,
                started,
                t,
                &mut system_open,
            )
        },
    ))
}

/// 执行方：转来的 `press`（参数绕过了 clap，检查在 [`execute`] 里照做）
pub(crate) fn serve<B: Backend>(
    ctx: crate::serve::Ctx<'_, B>,
    fa: crate::forward::FwdArgs,
    sink: &mut dyn Sink,
    started: Instant,
) -> (&'static str, u8) {
    let format = fa.format;
    let code = match serde_json::from_value::<PressParams>(fa.params) {
        Err(_) => out::fail_with_to(
            sink,
            format,
            &Fail::Core(CoreError::Internal {
                what: "ipc_bad_request",
            }),
        ),
        Ok(p) => {
            let t = send::RESULT_TIMEOUT;
            execute(
                ctx.b,
                ctx.stream,
                ctx.feed,
                &p,
                sink,
                format,
                started,
                t,
                &mut system_open,
            )
        }
    };
    ("press", code)
}

#[cfg(test)]
mod tests;
