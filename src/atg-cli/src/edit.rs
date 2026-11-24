//! `edit CHAT_ID MSG_ID (--text T | --text-file PATH) [--dry-run]`：改自己发的一条纯文字消息的正文（#8 B，M5 ①）。
//! 契约：`specs/contracts/cli.md` `edit` 行；动作：`actions.md`「发送」表 `edit_prepare` / `edit_text`；流程在
//! [`atg_core::edit`]。
//!
//! 输出（直跑与经本机接口转发同一段代码）：一行 `plan`（目标、`message_id`、原文 `old_text`、新文 `text`、
//! `hidden_chars`、`formatting_dropped`、`note_zh`、确认方式、摘要）→ `--dry-run` 到此为止 → 闸开凭据（直发，不确认）
//! → 交出 `editMessageText`：应答 = 服务器已改完 → 回显改后的那条（读法同 `history`）+ `result`。
//! 交出到点没回或断线 → `send_outcome_unknown`（改没改成未知，用 `history` 看那条现在的文字）。
//! 正文只从 `--text` / `--text-file` 来，不从标准输入读（同 `send`）。

use crate::args::Format;
use crate::chats::{self, Feed};
use crate::deadlines::{QUERY_TIMEOUT, bounded};
use crate::history;
use crate::out::{self, Fail, Sink, emit_to, envelope};
use atg_core::backend::Backend;
use atg_core::edit::{self, EditPlan};
use atg_core::error::{CoreError, UnknownWhy};
use atg_core::gate::Outcome;
use atg_core::link::{EventStream, block_on_timeout};
use atg_core::project;
use atg_core::send::SendOrigin;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// 一次 `edit` 要的东西（经本机接口转发时原样序列化过去；`--text-file` 在发起方读成 `text`）
// 带正文：`Debug` 只在单测里派生（同 `send`）
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(Debug))]
pub struct EditParams {
    pub chat: String,
    pub message_id: i64,
    pub text: String,
    pub dry_run: bool,
    /// 读法里的时区（同全局 `--tz`）
    pub tz: Option<String>,
}

/// 编辑的「结果未知」说明（`send_outcome_unknown` 的通用说明讲的是发送）
pub(crate) const EDIT_UNKNOWN_ZH: &str =
    "编辑已交出，但还不知道改没改成；不要直接重做，先用 history 看那条现在的文字";

/// `edit` 入口：档案被占就转给执行方（闸在执行方），否则直跑
pub fn run(
    g: &crate::args::Global,
    format: Format,
    a: crate::args::EditArgs,
    started: Instant,
) -> std::process::ExitCode {
    // plan 行带原文与新文，打码还没做：同 send，加 --redact 先拒
    if let Some(f) = crate::no_redact(g, "edit") {
        return out::fail_with(format, &f);
    }
    let text = match (a.text, a.text_file) {
        (Some(t), None) => t,
        (None, Some(p)) => match crate::send::read_text_file(&p) {
            Ok(t) => t,
            Err(f) => return out::fail_with(format, &f),
        },
        _ => {
            let f = Fail::Usage("新的文字用 --text 或 --text-file 给，二选一");
            return out::fail_with(format, &f);
        }
    };
    if let Err(f) = crate::send::check_len(&text) {
        return out::fail_with(format, &f);
    }
    let p = EditParams {
        chat: a.chat,
        message_id: a.message_id,
        text,
        dry_run: a.dry_run,
        tz: g.tz.clone(),
    };
    if let Some(code) = crate::forward::try_forward(g, format, "edit", &json!(p)) {
        return std::process::ExitCode::from(code);
    }
    let code = crate::session::l2_run(g, format, "edit", started, &mut |b, st, fe, sink| {
        execute_edit(b, st, fe, &p, sink, format, started)
    });
    std::process::ExitCode::from(code)
}

/// 在已起好的后端上编辑（直跑与执行方共用）
pub fn execute_edit<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &EditParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
) -> u8 {
    execute_within(b, stream, feed, p, sink, format, started, QUERY_TIMEOUT)
}

/// 同 [`execute_edit`]，交出的期限由调用方给（单测用短的）
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_within<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &EditParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    submit_timeout: Duration,
) -> u8 {
    match run_on(b, stream, feed, p, sink, format, started, submit_timeout) {
        Ok(code) => code,
        Err(Fail::Core(e @ CoreError::SendOutcomeUnknown { .. })) => {
            crate::send::l2_fail(sink, format, &e, json!({ "message_zh": EDIT_UNKNOWN_ZH }))
        }
        // L2 的错误一律 `retryable:false`（同 `send`）
        Err(Fail::Core(e)) => crate::send::l2_fail(sink, format, &e, json!({})),
        Err(f) => out::fail_with_to(sink, format, &f),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_on<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &EditParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    submit_timeout: Duration,
) -> Result<u8, Fail> {
    let chat = history::chat_arg(&p.chat)?;
    let cx = history::speech_ctx(p.tz.as_deref(), false)?;
    // 经本机接口转来的参数绕过了发起方的检查，这里再查一次
    crate::send::check_len(&p.text)?;
    chats::ensure_ready(b)?;
    let chat_id = history::resolve_chat(b, chat)?;
    let plan = bounded(
        b,
        edit::prepare_edit(b, chat_id, p.message_id, &p.text, SendOrigin::Cli),
        QUERY_TIMEOUT,
    )
    .map_err(Fail::Core)?;
    feed.drain(stream);
    emit_plan(sink, format, &plan);
    if p.dry_run {
        let data = json!({"command": "edit", "dry_run": true, "chat_id": chat_id,
            "elapsed_ms": crate::elapsed_ms(started)});
        finish(sink, format, data, "只看了计划，没有改");
        return Ok(0);
    }
    let token = b.gate().issue(&plan, false).map_err(Fail::Core)?;
    // 交出（验凭据 → 复查 → 交给 TDLib、等服务器改完）到点没回：可能已经改了，未知（同 `delete`）
    let (mode, m) = match block_on_timeout(edit::edit_text(b, &plan, token), submit_timeout) {
        Some(r) => r.map_err(Fail::Core)?,
        None => return Ok(unknown(b, &plan, sink, format)),
    };
    // 改后的更新（`message_content_changed` / `message_edited`）先进 Store，再回显（读法同 `history`）
    feed.drain(stream);
    let id = m.id;
    let edit_date = m.edited.map(|t| t.0);
    crate::send::remember(feed, m);
    if let Some(doc) = project::message(feed.store(), id) {
        crate::send::echo(sink, format, &cx, &doc);
    }
    let data = json!({"command": "edit", "chat_id": chat_id, "message_id": id.message_id,
        "confirm": mode.as_str(), "edit_date": edit_date, "elapsed_ms": crate::elapsed_ms(started)});
    finish(sink, format, data, "已改好");
    Ok(0)
}

/// 交出到点没回：内核已故障报 `disconnected`，否则 `timeout`；退出码 10、不可重试，留痕记未知
fn unknown<B: Backend>(b: &B, plan: &EditPlan, sink: &mut dyn Sink, format: Format) -> u8 {
    let why = match crate::deadlines::stalled(b) {
        CoreError::Disconnected => UnknownWhy::Disconnected,
        _ => UnknownWhy::Timeout,
    };
    b.gate().after_outcome(
        plan.mode(),
        plan,
        0,
        Some(plan.message_id()),
        Outcome::Unknown,
        None,
    );
    let e = CoreError::SendOutcomeUnknown {
        chat_id: Some(plan.target().id),
        temp_message_id: 0,
        why,
    };
    crate::send::l2_fail(sink, format, &e, json!({ "message_zh": EDIT_UNKNOWN_ZH }))
}

fn emit_plan(sink: &mut dyn Sink, format: Format, plan: &EditPlan) {
    match format {
        Format::Json => emit_to(sink, &envelope("plan", plan.json())),
        // 句子在 CLI 拼，同 `send` / `delete`（改到树上见 #132）
        Format::Text => {
            let dropped = if plan.formatting_dropped() {
                "；原文的格式（加粗、文字链接等）改后会丢"
            } else {
                ""
            };
            sink.out(&format!(
                "将把 {} 里的 id {}「{}」改成「{}」{}；{}",
                atg_core::send::target_zh(plan.target()),
                plan.message_id(),
                atg_core::gate::visible(plan.old_text()),
                atg_core::gate::visible(plan.text()),
                dropped,
                edit::EDIT_NOTE_ZH,
            ));
        }
    }
}

fn finish(sink: &mut dyn Sink, format: Format, data: Value, zh: &str) {
    match format {
        Format::Json => emit_to(sink, &envelope("result", data)),
        Format::Text => sink.out(zh),
    }
}

// 真后端实测（`#[ignore]`，要显式开）：见 `edit/exp_real.rs` 文件头
#[cfg(all(test, feature = "tdlib"))]
mod exp_real;
#[cfg(test)]
mod tests;
