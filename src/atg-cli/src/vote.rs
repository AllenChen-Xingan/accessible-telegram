//! `vote`：在一条投票里选一项或几项（#13 13b-1；契约 `specs/contracts/cli.md`）。
//!
//! `vote CHAT_ID MSG_ID N [N…] [--replace] [--dry-run]`：N 是 `history --full` 全文里读的「第 N 项」（显示顺序，1 起）。一行 `plan` → 闸开凭据（和 `send`
//! 一样直发、不确认；测试服务器只许收藏夹）→ 交出 → 一行 `result`。L2：不自动重试；没等到应答按 `send_outcome_unknown` 报
//! （票可能已投上）。能经本机接口转给占着档案的执行方（闸在执行方）。流程只在这里，GUI 以后同样调 `atg_core::send::vote`。

use crate::args::{Format, Global, MsgRef};
use crate::chats::{self, Feed};
use crate::deadlines::{QUERY_TIMEOUT, bounded};
use crate::history::{self, chat_arg};
use crate::out::{self, Fail, Sink, emit_to, envelope};
use atg_core::backend::Backend;
use atg_core::error::{CoreError, InvalidSendWhy};
use atg_core::link::EventStream;
use atg_core::model::MsgId;
use atg_core::send::vote::{self, VotePlan};
use atg_core::send::{self, SendOrigin};
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// 一次最多选几项（Telegram 的投票最多 12 个选项）
pub const CHOICES_MAX: usize = 12;

/// `vote` 的参数
#[derive(Args)]
pub struct VoteArgs {
    #[command(flatten)]
    pub msg: MsgRef,
    /// 选第几项：`history --full` 全文里读的「第 N 项」（从 1 起，按显示顺序）；许多选的投票可给几个
    #[arg(value_name = "N", required = true, num_args = 1..)]
    pub choices: Vec<u32>,
    /// 你已投过、这次没再选上原来投的某项时照样投：那一项会被撤掉（多选要追加就把原来的也写上，不用这个）
    #[arg(long)]
    pub replace: bool,
    /// 只输出要投什么，不投
    #[arg(long)]
    pub dry_run: bool,
}

/// 一次 `vote` 要的东西（经本机接口转发时原样序列化过去）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteParams {
    pub chat: String,
    pub message_id: i64,
    pub choices: Vec<u32>,
    #[serde(default)]
    pub replace: bool,
    #[serde(default)]
    pub dry_run: bool,
}

/// 参数检查（直跑与执行方都走这里：执行方收到的参数绕过了 clap）
pub fn check(p: &VoteParams) -> Result<(), Fail> {
    chat_arg(&p.chat)?;
    if p.choices.is_empty() || p.choices.len() > CHOICES_MAX {
        // 上限同 CHOICES_MAX（`Fail::Usage` 只收字面量）
        return Err(Fail::Usage("给 1 到 12 个选项：全文里读的第几项"));
    }
    Ok(())
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
            // 内核的说法不写死前端的放行办法（同 react 的 --replace）：CLI 在这里补
            let hint = matches!(
                e,
                CoreError::InvalidSend {
                    why: InvalidSendWhy::VoteWouldReplace
                }
            )
            .then_some(
                "确要换掉就加 --replace；许多选的投票想追加，就把原来投的也写上（那样不丢，不用 --replace）",
            );
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

/// 在已起好的后端上投票，输出写到 `sink`，返回退出码（直跑与执行方共用）
#[allow(clippy::too_many_arguments)]
pub fn execute<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &VoteParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
) -> u8 {
    match run_vote(b, stream, feed, p, sink, format, started, timeout) {
        Ok(code) => code,
        Err(f) => fail_line(sink, format, &f),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_vote<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &VoteParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
) -> Result<u8, Fail> {
    check(p)?;
    chats::ensure_ready(b)?;
    let chat_id = history::resolve_chat(b, chat_arg(&p.chat)?)?;
    history::load_chat(b, stream, feed, chat_id)?;
    let id = MsgId {
        chat_id,
        message_id: p.message_id,
    };
    let plan = bounded(
        b,
        vote::prepare_vote(b, id, &p.choices, p.replace, SendOrigin::Cli),
        QUERY_TIMEOUT,
    )
    .map_err(Fail::Core)?;
    let what = said(&plan);
    match format {
        Format::Json => emit_to(sink, &envelope("plan", send::vote_plan_json(&plan))),
        Format::Text => sink.out(&format!("将{what}；不用确认")),
    }
    let numbers: Vec<u32> = plan.choices().iter().map(|c| c.number).collect();
    let base = json!({"command": "vote", "chat_id": chat_id, "message_id": p.message_id,
        "choices": numbers});
    if p.dry_run {
        let mut d = base;
        d["dry_run"] = json!(true);
        d["elapsed_ms"] = json!(crate::elapsed_ms(started));
        result_line(sink, format, d, "只看了计划，没有投");
        return Ok(0);
    }
    let token = b.gate().issue(&plan, false).map_err(Fail::Core)?;
    let mode = vote::vote_within(b, &plan, token, timeout).map_err(Fail::Core)?;
    feed.drain(stream);
    let mut d = base;
    d["confirm"] = json!(mode);
    d["elapsed_ms"] = json!(crate::elapsed_ms(started));
    result_line(sink, format, d, &format!("已{what}"));
    Ok(0)
}

/// 「第 N 项，选项原文」逐项，「、」隔开；含剧透的选项只说「隐藏内容」
fn items(cs: &[vote::Choice]) -> String {
    let v: Vec<String> = cs
        .iter()
        .map(|c| match &c.text {
            Some(t) => format!("第 {} 项，{}", c.number, t.trim()),
            None => format!("第 {} 项，隐藏内容", c.number),
        })
        .collect();
    v.join("、")
}

/// 「在 私聊 · 林小雨 · 777 里 id 5 那条投票里投第 2 项，<选项原文>；公开投票，别人看得到你投了什么；投了能改」。
/// 会换掉现在投的就先说换掉哪几项；能不能改照这个投票的实际设置说，测验另说投后公布答案（人改定 6）
fn said(plan: &VotePlan) -> String {
    let mut notes = vec![
        if plan.anonymous() {
            "匿名投票"
        } else {
            "公开投票，别人看得到你投了什么"
        },
        if plan.revotable() {
            "投了能改"
        } else {
            "投了不能改"
        },
    ];
    if plan.quiz() {
        notes.push("这是测验，投后会公布答案");
    }
    let swap = if plan.replaces().is_empty() {
        String::new()
    } else {
        format!("（你现在投的{}会被撤掉）", items(plan.replaces()))
    };
    format!(
        "在 {} 里 id {} 那条投票里投{}{swap}；{}",
        send::target_zh(plan.target()),
        plan.message().message_id,
        items(plan.choices()),
        notes.join("；"),
    )
}

/// `vote` 直跑入口：档案被占就转给执行方（闸在执行方）
pub fn run(g: &Global, format: Format, a: &VoteArgs, started: Instant) -> ExitCode {
    // plan 行带会话名与选项文字，打码还没做：同 send，加 --redact 先拒
    if let Some(f) = crate::no_redact(g, "vote") {
        return out::fail_with(format, &f);
    }
    let p = VoteParams {
        chat: a.msg.chat.clone(),
        message_id: a.msg.message,
        choices: a.choices.clone(),
        replace: a.replace,
        dry_run: a.dry_run,
    };
    if let Err(f) = check(&p) {
        return out::fail_with(format, &f);
    }
    let params = serde_json::to_value(&p).unwrap_or_default();
    if let Some(code) = crate::forward::try_forward(g, format, "vote", &params) {
        return ExitCode::from(code);
    }
    ExitCode::from(crate::session::l2_run(
        g,
        format,
        "vote",
        started,
        &mut |b, stream, feed, sink| {
            execute(
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

/// 执行方：转来的 `vote`（参数绕过了 clap，检查在 [`execute`] 里照做）
pub(crate) fn serve<B: Backend>(
    ctx: crate::serve::Ctx<'_, B>,
    fa: crate::forward::FwdArgs,
    sink: &mut dyn Sink,
    started: Instant,
) -> (&'static str, u8) {
    let format = fa.format;
    let code = match serde_json::from_value::<VoteParams>(fa.params) {
        Err(_) => out::fail_with_to(
            sink,
            format,
            &Fail::Core(CoreError::Internal {
                what: "ipc_bad_request",
            }),
        ),
        Ok(p) => {
            let t = send::RESULT_TIMEOUT;
            execute(ctx.b, ctx.stream, ctx.feed, &p, sink, format, started, t)
        }
    };
    ("vote", code)
}

#[cfg(test)]
mod tests;
