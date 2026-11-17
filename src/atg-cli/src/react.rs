//! `react` / `reactions`：给一条消息加 / 去掉表情回应，看谁点了回应（#9；契约 `specs/contracts/cli.md`）。
//!
//! - `react CHAT_ID MSG_ID EMOJI [--remove | --replace] [--dry-run]`：一行 `plan` → 闸开凭据（**不确认**，B18-2；测试服务器
//!   只许收藏夹）→ 交出 → 一行 `result`。L2：不自动重试；没等到应答按 `send_outcome_unknown` 报（回应可能已点上）
//! - `reactions CHAT_ID MSG_ID [--emoji E] [-n N] [--cursor C]`：L0 只读，一行一个 `kind:"reactor"`（树节点
//!   `Doc::Reactor`，读法「张三，👍，19:33」）+ 一行 `result`
//!
//! 两条都能经本机接口转给占着档案的执行方（闸在执行方）。流程只在这里，GUI 以后同样调 `atg_core::send::react`。

use crate::args::Format;
use crate::chats::{self, Feed};
use crate::deadlines::{QUERY_TIMEOUT, SYNC_TIMEOUT, bounded};
use crate::history::{self, ChatRef};
use crate::out::{self, Fail, Sink, emit_to, envelope};
use atg_core::backend::Backend;
use atg_core::error::{CoreError, InvalidSendWhy};
use atg_core::link::EventStream;
use atg_core::model::MsgId;
use atg_core::send::react::{self, ReactPlan};
use atg_core::send::{self, SendOrigin};
use atg_core::speech;
use atg_core::status::NetState;
use atg_core::tree::ReactionKind;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// 「谁点了回应」一页最多几个（TDLib 的上限是 100）
pub const REACTIONS_MAX: u32 = 100;

/// 一次 `react` 要的东西（经本机接口转发时原样序列化过去）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactParams {
    pub chat: String,
    pub message_id: i64,
    pub emoji: String,
    #[serde(default)]
    pub remove: bool,
    /// 这条上我已点了别的回应时，允许它被挤掉
    #[serde(default)]
    pub replace: bool,
    #[serde(default)]
    pub dry_run: bool,
}

/// 一次 `reactions` 要的东西
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionsParams {
    pub chat: String,
    pub message_id: i64,
    #[serde(default)]
    pub emoji: Option<String>,
    pub limit: u32,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub tz: Option<String>,
}

fn chat_ref(s: &str) -> Result<ChatRef, Fail> {
    ChatRef::parse(s).ok_or(Fail::Usage("会话写数字 id，或 saved（收藏夹）"))
}

/// 参数检查（直跑与执行方都走这里：执行方收到的参数绕过了 clap）
pub fn check_react(p: &ReactParams) -> Result<(), Fail> {
    chat_ref(&p.chat)?;
    if p.remove && p.replace {
        return Err(Fail::Usage("--remove 与 --replace 不能同用"));
    }
    react::check_emoji(&p.emoji).map_err(Fail::Core)
}

pub fn check_reactions(p: &ReactionsParams) -> Result<(), Fail> {
    chat_ref(&p.chat)?;
    if !(1..=REACTIONS_MAX).contains(&p.limit) {
        return Err(Fail::Usage("-n 要在 1 到 100 之间"));
    }
    if let Some(e) = &p.emoji {
        react::check_emoji(e).map_err(Fail::Core)?;
    }
    history::speech_ctx(p.tz.as_deref(), false).map(|_| ())
}

fn result_line(sink: &mut dyn Sink, format: Format, data: Value, zh: &str) {
    match format {
        Format::Json => emit_to(sink, &envelope("result", data)),
        Format::Text => sink.out(zh),
    }
}

/// 错误行：L2 的 `retryable` 一律为假
fn fail_line(sink: &mut dyn Sink, format: Format, f: &Fail, l2: bool) -> u8 {
    match f {
        Fail::Core(e) if l2 => {
            let mut data = out::core_error_data(e);
            data["retryable"] = json!(false);
            // 内核的说法不写死前端的放行办法（同 duplicate_recent 的 --again）：CLI 在这里补
            let hint = matches!(
                e,
                CoreError::InvalidSend {
                    why: InvalidSendWhy::ReactionWouldReplace
                        | InvalidSendWhy::ReactionStateUnknown
                }
            )
            .then_some("确要点就加 --replace");
            if let Some(h) = hint {
                data["hint_zh"] = json!(h);
            }
            match format {
                Format::Json => emit_to(sink, &envelope("error", data)),
                Format::Text => sink.out(&match hint {
                    Some(h) => format!("{}；{h}", e.message_zh()),
                    None => e.message_zh().to_owned(),
                }),
            }
            out::core_exit(e)
        }
        other => out::fail_with_to(sink, format, other),
    }
}

// ───────────────────────── react ─────────────────────────

/// 在已起好的后端上点 / 去掉一个回应，输出写到 `sink`，返回退出码（直跑与执行方共用）
#[allow(clippy::too_many_arguments)]
pub fn execute_react<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &ReactParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
) -> u8 {
    match run_react(b, stream, feed, p, sink, format, started, timeout) {
        Ok(code) => code,
        Err(f) => fail_line(sink, format, &f, true),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_react<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &ReactParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
) -> Result<u8, Fail> {
    check_react(p)?;
    chats::ensure_ready(b)?;
    let chat_id = history::resolve(b, stream, feed, chat_ref(&p.chat)?)?;
    history::load_chat(b, stream, feed, chat_id)?;
    let id = MsgId {
        chat_id,
        message_id: p.message_id,
    };
    // 「我点了哪些」要等补发完才准（人可能刚在手机上点过）；等不到也照样准备，由内核按拿不准处理
    let synced = b
        .wait_net(SYNC_TIMEOUT, &|n| *n == NetState::Ready)
        .is_some();
    feed.drain(stream);
    let ask = react::ReactAsk {
        emoji: &p.emoji,
        remove: p.remove,
        replace: p.replace,
        synced,
    };
    let plan = bounded(
        b,
        react::prepare_react(b, id, ask, SendOrigin::Cli),
        QUERY_TIMEOUT,
    )
    .map_err(Fail::Core)?;
    let what = said(&plan);
    match format {
        Format::Json => emit_to(sink, &envelope("plan", send::react_plan_json(&plan))),
        Format::Text => sink.out(&format!("将{what}；不用确认")),
    }
    let base = json!({"command": "react", "chat_id": chat_id, "message_id": p.message_id,
        "reaction": plan.kind(), "removed": p.remove});
    if p.dry_run {
        let mut d = base;
        d["dry_run"] = json!(true);
        d["elapsed_ms"] = json!(crate::elapsed_ms(started));
        result_line(sink, format, d, "只看了计划，没有点");
        return Ok(0);
    }
    let token = b.gate().issue(&plan, false).map_err(Fail::Core)?;
    let mode = react::react_within(b, &plan, token, timeout).map_err(Fail::Core)?;
    feed.drain(stream);
    let mut d = base;
    d["confirm"] = json!(mode);
    d["elapsed_ms"] = json!(crate::elapsed_ms(started));
    result_line(sink, format, d, &format!("已{what}"));
    Ok(0)
}

/// 「给 私聊 · 张三 · 777 里 id 5 那条加上回应 👍（可能挤掉你原先点的 ❤️）」
fn said(plan: &ReactPlan) -> String {
    let swapped: Vec<String> = plan.replaces().iter().map(react::emoji_of).collect();
    let swapped = if swapped.is_empty() {
        String::new()
    } else {
        format!("（可能挤掉你原先点的 {}）", swapped.join("、"))
    };
    format!(
        "给 {} 里 id {} 那条{}回应 {}{swapped}",
        send::target_zh(plan.target()),
        plan.message().message_id,
        if plan.remove() { "去掉" } else { "加上" },
        react::emoji_of(plan.kind()),
    )
}

// ───────────────────────── reactions ─────────────────────────

/// 看谁点了回应（L0），输出写到 `sink`，返回退出码
pub fn execute_reactions<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &ReactionsParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
) -> u8 {
    match run_reactions(b, stream, feed, p, sink, format, started) {
        Ok(code) => code,
        Err(f) => fail_line(sink, format, &f, false),
    }
}

fn run_reactions<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &ReactionsParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
) -> Result<u8, Fail> {
    check_reactions(p)?;
    let cx = history::speech_ctx(p.tz.as_deref(), false)?;
    chats::ensure_ready(b)?;
    let chat_id = history::resolve(b, stream, feed, chat_ref(&p.chat)?)?;
    history::load_chat(b, stream, feed, chat_id)?;
    let id = MsgId {
        chat_id,
        message_id: p.message_id,
    };
    let kind = p
        .emoji
        .as_ref()
        .map(|e| ReactionKind::Emoji { emoji: e.clone() });
    let page = bounded(
        b,
        b.added_reactions(
            id,
            kind.as_ref(),
            p.cursor.as_deref().unwrap_or(""),
            p.limit,
        ),
        QUERY_TIMEOUT,
    )
    .map_err(Fail::Core)?;
    // 点回应的人经 `updateUser` 先进事件流：喂进 store 再投影，名字才查得到
    feed.drain(stream);
    for r in &page.items {
        let doc = atg_core::project::reactor(feed.store(), r);
        match format {
            Format::Json => {
                let mut line = atg_core::json::value(&doc, &cx);
                line["at"] = json!(out::now_rfc3339_ms());
                emit_to(sink, &line);
            }
            Format::Text => sink.out(&speech::brief(&doc, &cx)),
        }
    }
    let more = !page.next_offset.is_empty();
    let mut data = json!({"command": "reactions", "chat_id": chat_id, "message_id": p.message_id,
        "count": page.items.len(), "total": page.total, "more": more,
        "elapsed_ms": crate::elapsed_ms(started)});
    // 同 chats / history：`next_cursor` 只在 `more` 为真时才有
    if more {
        data["next_cursor"] = json!(page.next_offset);
    }
    let next = if more {
        format!("；还有更多，接着看加 --cursor {}", page.next_offset)
    } else {
        String::new()
    };
    let zh = format!(
        "共 {} 个回应，列出 {} 个{next}",
        page.total,
        page.items.len()
    );
    result_line(sink, format, data, &zh);
    Ok(0)
}

// ───────────────────────── 入口 ─────────────────────────

/// `react` 直跑入口：档案被占就转给执行方（闸在执行方）
pub fn run(
    g: &crate::args::Global,
    format: Format,
    a: &crate::args::ReactArgs,
    started: Instant,
) -> ExitCode {
    // plan 行带会话名，打码还没做：同 send，加 --redact 先拒
    if let Some(f) = crate::no_redact(g, "react") {
        return out::fail_with(format, &f);
    }
    let p = ReactParams {
        chat: a.msg.chat.clone(),
        message_id: a.msg.message,
        emoji: a.emoji.clone(),
        remove: a.remove,
        replace: a.replace,
        dry_run: a.dry_run,
    };
    if let Err(f) = check_react(&p) {
        return out::fail_with(format, &f);
    }
    let params = serde_json::to_value(&p).unwrap_or_default();
    if let Some(code) = crate::forward::try_forward(g, format, "react", &params) {
        return ExitCode::from(code);
    }
    ExitCode::from(crate::session::l2_run(
        g,
        format,
        "react",
        started,
        &mut |b, stream, feed, sink| {
            execute_react(
                b,
                stream,
                feed,
                &p,
                sink,
                format,
                started,
                send::RESULT_TIMEOUT,
            )
        },
    ))
}

/// `reactions` 直跑入口（输出带名字：`--redact` 还没做，先拒）
pub fn run_list(
    g: &crate::args::Global,
    format: Format,
    a: &crate::args::ReactionsArgs,
    started: Instant,
) -> ExitCode {
    if let Some(f) = crate::no_redact(g, "reactions") {
        return out::fail_with(format, &f);
    }
    let p = ReactionsParams {
        chat: a.msg.chat.clone(),
        message_id: a.msg.message,
        emoji: a.emoji.clone(),
        limit: a.limit,
        cursor: a.cursor.clone(),
        tz: g.tz.clone(),
    };
    if let Err(f) = check_reactions(&p) {
        return out::fail_with(format, &f);
    }
    let params = serde_json::to_value(&p).unwrap_or_default();
    if let Some(code) = crate::forward::try_forward(g, format, "reactions", &params) {
        return ExitCode::from(code);
    }
    // 只读：不过闸、不留痕、不出直发的开场提示
    let code = crate::session::l0_run(
        g,
        format,
        "reactions",
        started,
        &mut |b, stream, feed, sink| execute_reactions(b, stream, feed, &p, sink, format, started),
    );
    ExitCode::from(code)
}

/// 执行方：转来的 `react` / `reactions`（参数绕过了 clap，检查在 `execute_*` 里照做）
pub(crate) fn serve<B: Backend>(
    ctx: crate::serve::Ctx<'_, B>,
    command: &str,
    fa: crate::forward::FwdArgs,
    sink: &mut dyn Sink,
    started: Instant,
) -> (&'static str, u8) {
    let format = fa.format;
    let bad = Fail::Core(CoreError::Internal {
        what: "ipc_bad_request",
    });
    if command != "react" && command != "reactions" {
        return ("?", out::fail_with_to(sink, format, &bad));
    }
    if command == "react" {
        let code = match serde_json::from_value::<ReactParams>(fa.params) {
            Err(_) => out::fail_with_to(sink, format, &bad),
            Ok(p) => {
                // 执行方以直发启动：发起方要和直跑一样先看到醒目提示与 status 行
                if ctx.b.gate().direct() {
                    crate::send::direct_banner(ctx.b, sink, format);
                }
                let t = send::RESULT_TIMEOUT;
                execute_react(ctx.b, ctx.stream, ctx.feed, &p, sink, format, started, t)
            }
        };
        return ("react", code);
    }
    let code = match serde_json::from_value::<ReactionsParams>(fa.params) {
        Err(_) => out::fail_with_to(sink, format, &bad),
        Ok(p) => execute_reactions(ctx.b, ctx.stream, ctx.feed, &p, sink, format, started),
    };
    ("reactions", code)
}

#[cfg(test)]
mod tests;
