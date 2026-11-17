//! `delete CHAT_ID MSG_ID...`：给所有人删除自己发的消息（#8，M2.7，T04）。契约：`specs/contracts/cli.md` `delete` 行，
//! 动作：`specs/contracts/actions.md`「发送」表 `delete_prepare` / `delete`；流程在 [`atg_core::delete`]。
//!
//! 输出（直跑与经本机接口转发同一段代码）：一行 `plan`（目标、每条 `message_id` + 正文 + 简短读法 `brief`、`note_zh`、
//! 确认方式、摘要）→ `--dry-run` 到此为止 → 闸开凭据（直发，不确认）→ 交出 `deleteMessages`（`revoke=true`）→
//! 结尾 `result`（`deleted`：删了哪几条）。TDLib 应答 `ok` = 本机已删、给服务器的删除 TDLib 已记下、在后台做完
//! （断网时联网后补做），不再等事件；要核对就用 `history` 看。
//! 交出到点没回或断线 → `send_outcome_unknown`（删没删掉未知，不许直接重做）。
//! 只删自己发的、已发出的；收藏夹里的、还没发出的、TDLib 说不能给所有人删的（过了时限……）一律 `forbidden`（带 `why`），
//! 什么都不删。

use crate::args::Format;
use crate::chats::{self, Feed};
use crate::deadlines::{QUERY_TIMEOUT, bounded};
use crate::history;
use crate::out::{self, Fail, Sink, emit_to, envelope};
use atg_core::backend::Backend;
use atg_core::delete::{self, DeletePlan};
use atg_core::error::{CoreError, UnknownWhy};
use atg_core::gate::Outcome;
use atg_core::link::{EventStream, block_on_timeout};
use atg_core::model::MsgId;
use atg_core::project;
use atg_core::send::SendOrigin;
use atg_core::speech;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// 一次 `delete` 要的东西（经本机接口转发时原样序列化过去）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteParams {
    pub chat: String,
    pub ids: Vec<i64>,
    pub dry_run: bool,
    /// 读法里的时区（同全局 `--tz`）
    pub tz: Option<String>,
}

/// 删除的「结果未知」说明（`send_outcome_unknown` 的通用说明讲的是发送）
pub(crate) const DELETE_UNKNOWN_ZH: &str =
    "删除已交出，但还不知道删没删掉；不要直接重做，先去会话里看最近的消息（history）";

/// `delete` 入口：档案被占就转给执行方（闸在执行方），否则直跑
pub fn run(
    g: &crate::args::Global,
    format: Format,
    a: crate::args::DeleteArgs,
    started: Instant,
) -> std::process::ExitCode {
    // plan 行带正文，打码还没做：同 send，加 --redact 先拒
    if let Some(f) = crate::no_redact(g, "delete") {
        return out::fail_with(format, &f);
    }
    let params = json!(DeleteParams {
        chat: a.chat,
        ids: a.ids,
        dry_run: a.dry_run,
        tz: g.tz.clone(),
    });
    if let Some(code) = crate::forward::try_forward(g, format, "delete", &params) {
        return std::process::ExitCode::from(code);
    }
    std::process::ExitCode::from(crate::session::delete(g, format, &params, started))
}

/// 在已起好的后端上删除（直跑与执行方共用）
pub fn execute_delete<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &DeleteParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
) -> u8 {
    execute_within(b, stream, feed, p, sink, format, started, QUERY_TIMEOUT)
}

/// 同 [`execute_delete`]，交出的期限由调用方给（单测用短的）
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_within<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &DeleteParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    submit_timeout: Duration,
) -> u8 {
    match run(b, stream, feed, p, sink, format, started, submit_timeout) {
        Ok(code) => code,
        // 交出后断线等「可能已交出」：换成删除的说明
        Err(Fail::Core(e @ CoreError::SendOutcomeUnknown { .. })) => {
            crate::send::l2_fail(sink, format, &e, json!({ "message_zh": DELETE_UNKNOWN_ZH }))
        }
        // L2 的错误一律 `retryable:false`（同 `send`）
        Err(Fail::Core(e)) => crate::send::l2_fail(sink, format, &e, json!({})),
        Err(f) => out::fail_with_to(sink, format, &f),
    }
}

#[allow(clippy::too_many_arguments)]
fn run<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &DeleteParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    submit_timeout: Duration,
) -> Result<u8, Fail> {
    // 收藏夹里的不能给所有人删：用法提示里不提 `saved`（写了也照样解析，准备时报 `forbidden why:saved`）
    let chat = history::ChatRef::parse(&p.chat).ok_or(Fail::Usage("会话写数字 id"))?;
    let cx = history::speech_ctx(p.tz.as_deref(), false)?;
    // 经本机接口转来的参数绕过了 clap，这里再查一次
    if p.ids.is_empty() || p.ids.len() > delete::MAX_IDS {
        return Err(Fail::Usage("消息 id 要给 1 到 100 个"));
    }
    chats::ensure_ready(b)?;
    let chat_id = crate::send::resolve(b, chat)?;
    let plan = bounded(
        b,
        delete::prepare_delete(b, chat_id, &p.ids, SendOrigin::Cli),
        QUERY_TIMEOUT,
    )
    .map_err(Fail::Core)?;
    feed.drain(stream);
    // 每条的简短读法（同 `history`）：取不到就不带，不挡删除
    let briefs: Vec<Option<String>> = plan
        .items()
        .iter()
        .map(|i| {
            let id = MsgId {
                chat_id,
                message_id: i.id,
            };
            let m = bounded(b, b.message_get(id), QUERY_TIMEOUT).ok()?;
            crate::send::remember(feed, m);
            project::message(feed.store(), id).map(|d| speech::brief(&d, &cx).replace('\n', " "))
        })
        .collect();
    emit_plan(sink, format, &plan, &briefs);
    if p.dry_run {
        let data = json!({"command": "delete", "dry_run": true, "chat_id": chat_id,
            "elapsed_ms": crate::elapsed_ms(started)});
        finish(sink, format, data, "只看了计划，没有删");
        return Ok(0);
    }
    let token = b.gate().issue(&plan, false).map_err(Fail::Core)?;
    // 交出（验凭据 → 复查 → 交给 TDLib）到点没回：可能已经交出，删没删掉未知——同 `pending cancel`（#109）
    let mode = match block_on_timeout(delete::delete(b, &plan, token), submit_timeout) {
        Some(r) => r.map_err(Fail::Core)?,
        None => return Ok(unknown(b, &plan, sink, format)),
    };
    let ids: Vec<i64> = plan.items().iter().map(|i| i.id).collect();
    let n = ids.len();
    let data = json!({"command": "delete", "chat_id": chat_id, "confirm": mode.as_str(),
        "deleted": ids, "elapsed_ms": crate::elapsed_ms(started)});
    finish(
        sink,
        format,
        data,
        &format!("已在本机删除 {n} 条，服务器那边由 Telegram 在后台给所有人删（断网时联网后补做）"),
    );
    Ok(0)
}

/// 交出到点没回：内核已故障报 `disconnected`，否则 `timeout`；退出码 10、不可重试，留痕记未知
fn unknown<B: Backend>(b: &B, plan: &DeletePlan, sink: &mut dyn Sink, format: Format) -> u8 {
    let why = match crate::deadlines::stalled(b) {
        CoreError::Disconnected => UnknownWhy::Disconnected,
        _ => UnknownWhy::Timeout,
    };
    b.gate()
        .after_outcome(plan.mode(), plan, 0, None, Outcome::Unknown, None);
    let e = CoreError::SendOutcomeUnknown {
        chat_id: Some(plan.target().id),
        temp_message_id: 0,
        why,
    };
    crate::send::l2_fail(sink, format, &e, json!({ "message_zh": DELETE_UNKNOWN_ZH }))
}

fn emit_plan(sink: &mut dyn Sink, format: Format, plan: &DeletePlan, briefs: &[Option<String>]) {
    match format {
        Format::Json => {
            let mut d = plan.json();
            if let Some(items) = d["items"].as_array_mut() {
                for (item, brief) in items.iter_mut().zip(briefs) {
                    item["brief"] = json!(brief);
                }
            }
            emit_to(sink, &envelope("plan", d));
        }
        // 句子在 CLI 拼，同 `send` / `pending`（改到树上见 #132）
        Format::Text => {
            let items: Vec<String> = plan
                .items()
                .iter()
                .zip(briefs)
                .map(|(i, brief)| match brief {
                    Some(b) => format!("id {}：{}", i.id, b),
                    None => format!("id {}", i.id),
                })
                .collect();
            sink.out(&format!(
                "将给所有人删除 {} 里的 {}；{}",
                atg_core::send::target_zh(plan.target()),
                items.join("；"),
                delete::DELETE_NOTE_ZH,
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

#[cfg(test)]
mod tests;
