//! `forward`：把一个会话里的一条或几条消息转发到另一个会话（#10，T04）。契约：`specs/contracts/cli.md` `forward` 行，
//! 动作与闸：`specs/contracts/actions.md` 第一节第 3–5 小节；流程在 `atg_core::send::forward`。
//! （文件名不叫 `forward.rs`：那是本机接口的转发，撞名，照 `send/resending.rs` 的先例用动名词。）
//!
//! 输出（直跑与经本机接口转发同一段代码，同 `resend`）：
//! 1. 一行 `plan`：目标与来源会话（种类、名字、id）、每条的 id 与纯文字正文、发起方、确认方式、摘要
//! 2. `--dry-run` 到此为止（`result` 带 `dry_run:true`）
//! 3. 闸开凭据（测试服务器只许收藏夹；正式服务器直发不问，但查防重复，`--again` 放行）
//! 4. 逐条等结果（30 秒），成功的回显发出的那条；结尾 `result` / `error` 带每条的下落（同 `resend`）

use crate::args::Format;
use crate::chats::{self, Feed};
use crate::deadlines::{QUERY_TIMEOUT, bounded};
use crate::history;
use crate::out::{self, Fail, Sink, emit_to, envelope};
use crate::send::{chat_ref, confirm_zh, ctx, echo, fail_any, finish_line, l2_fail, resolve};
use atg_core::backend::Backend;
use atg_core::error::CoreError;
use atg_core::link::EventStream;
use atg_core::project;
use atg_core::send::forward::{self as fwd, ForwardPlan};
use atg_core::send::{self, SendOrigin, Waited};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// 一次 `forward` 要的东西（经本机接口转发时原样序列化过去）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardParams {
    /// 来源会话
    pub from: String,
    pub ids: Vec<i64>,
    /// 目标会话
    pub to: String,
    pub dry_run: bool,
    pub again: bool,
    #[serde(default)]
    pub tz: Option<String>,
}

/// 从参数组出请求：先查会话写法与条数，好在起后端之前报用法错误
pub fn params(a: &crate::args::ForwardArgs, tz: Option<&str>) -> Result<ForwardParams, Fail> {
    chat_ref(&a.from)?;
    chat_ref(&a.to)?;
    ctx(tz)?;
    check_ids(&a.ids)?;
    Ok(ForwardParams {
        from: a.from.clone(),
        ids: a.ids.clone(),
        to: a.to.clone(),
        dry_run: a.dry_run,
        again: a.again,
        tz: tz.map(str::to_owned),
    })
}

fn check_ids(ids: &[i64]) -> Result<(), Fail> {
    if ids.is_empty() {
        return Err(Fail::Usage("要转哪几条：至少给一个消息 id"));
    }
    Ok(())
}

/// 入口：`--redact` 先拒（`plan` 带会话名与正文）→ 档案被占就转给执行方 → 否则直跑
pub fn run(
    g: &crate::args::Global,
    format: Format,
    a: &crate::args::ForwardArgs,
    started: Instant,
) -> ExitCode {
    if let Some(f) = crate::no_redact(g, "forward") {
        return out::fail_with(format, &f);
    }
    let p = match params(a, g.tz.as_deref()) {
        Ok(p) => p,
        Err(f) => return out::fail_with(format, &f),
    };
    let wire = serde_json::to_value(&p).unwrap_or_default();
    if let Some(code) = crate::forward::try_forward(g, format, "forward", &wire) {
        return ExitCode::from(code);
    }
    ExitCode::from(crate::session::l2_run(
        g,
        format,
        "forward",
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

/// 在已起好的后端上转发，输出写到 `sink`，返回退出码（直跑与执行方共用）。全部成功 0；否则同 `resend`：
/// 有结果未知的报它（10，最要紧的是别重转），没有就报第一条没成功的
#[allow(clippy::too_many_arguments)]
pub fn execute<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &ForwardParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
) -> u8 {
    match run_inner(b, stream, feed, p, sink, format, started, timeout) {
        Ok(code) => code,
        Err(f) => fail_any(sink, format, &f),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_inner<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &ForwardParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
) -> Result<u8, Fail> {
    let (from, to) = (chat_ref(&p.from)?, chat_ref(&p.to)?);
    check_ids(&p.ids)?;
    let cx = ctx(p.tz.as_deref())?;
    chats::ensure_ready(b)?;
    let (from, to) = (resolve(b, from)?, resolve(b, to)?);
    // 目标会话装进 store：成功的那几条要回显（读法同 `history`）
    history::load_chat(b, stream, feed, to)?;
    let plan = bounded(
        b,
        fwd::prepare_forward(b, to, from, &p.ids, SendOrigin::Cli),
        QUERY_TIMEOUT,
    )
    .map_err(Fail::Core)?;
    emit_plan(sink, format, &plan);
    if p.dry_run {
        finish_line(
            sink,
            format,
            json!({"command": "forward", "dry_run": true, "chat_id": to, "from_chat_id": from,
                "elapsed_ms": crate::elapsed_ms(started)}),
            "只看了计划，没有转发",
        );
        return Ok(0);
    }
    let token = b.gate().issue(&plan, p.again).map_err(Fail::Core)?;
    let (mode, outcomes) =
        fwd::forward_and_wait(b, &plan, token, stream, timeout, &mut |env| feed.apply(env))
            .map_err(Fail::Core)?;

    let mut items = Vec::new();
    let mut first: Option<CoreError> = None;
    let mut unknown: Option<CoreError> = None;
    for o in outcomes {
        let src = o.source;
        let Some((temp, w)) = o.result else {
            items.push(json!({"message_id": src, "outcome": "not_forwarded"}));
            // 交出之后才知道转不了：不能借准备时整批拒的说法（那句是「什么都没转」）；全都转不了的在下面换回整批那句
            first.get_or_insert(CoreError::InvalidSend {
                why: atg_core::error::InvalidSendWhy::NotForwarded,
            });
            continue;
        };
        match w {
            Waited::Succeeded(m) => {
                if let Some(d) = project::message(feed.store(), m.id) {
                    echo(sink, format, &cx, &d);
                }
                items.push(json!({"message_id": src, "outcome": "succeeded", "new_message_id": m.id.message_id}));
            }
            Waited::Failed {
                message,
                error,
                can_retry,
            } => {
                items.push(json!({"message_id": src, "outcome": "failed", "new_message_id": message.id.message_id,
                    "code": error.code(), "can_resend": can_retry}));
                first.get_or_insert(error);
            }
            Waited::Unknown(why) => {
                items.push(json!({"message_id": src, "outcome": "unknown", "temp_message_id": temp.message_id, "why": why}));
                unknown.get_or_insert(CoreError::SendOutcomeUnknown {
                    chat_id: Some(to),
                    temp_message_id: temp.message_id,
                    why,
                });
            }
        }
    }
    let zh = summary_zh(&items);
    // 一条都没转出（全是 null）：说「什么都没转」，不说「别的各自的结果」（核心已按「失败」收尾，同一批再转不被拦）
    if items.iter().all(|i| i["outcome"] == "not_forwarded") {
        first = Some(CoreError::InvalidSend {
            why: atg_core::error::InvalidSendWhy::NotForwardable,
        });
    }
    let Some(e) = unknown.or(first) else {
        finish_line(
            sink,
            format,
            json!({"command": "forward", "chat_id": to, "from_chat_id": from, "confirm": mode.as_str(),
                "items": items, "elapsed_ms": crate::elapsed_ms(started)}),
            &zh,
        );
        return Ok(0);
    };
    let msg = format!("{zh}；{}", e.message_zh());
    Ok(l2_fail(
        sink,
        format,
        &e,
        json!({"message_zh": msg, "chat_id": to, "from_chat_id": from, "confirm": mode.as_str(), "items": items}),
    ))
}

fn emit_plan(sink: &mut dyn Sink, format: Format, plan: &ForwardPlan) {
    match format {
        Format::Json => emit_to(sink, &envelope("plan", plan.json())),
        Format::Text => {
            let texts: Vec<String> = plan
                .items()
                .iter()
                .map(|i| match &i.text {
                    Some(t) => format!("id {} 那条：{}", i.id, atg_core::gate::visible(t)),
                    None => format!("id {} 那条：不是纯文字", i.id),
                })
                .collect();
            let forum = if plan.target().is_forum {
                "（论坛群：落进 General 话题）"
            } else {
                ""
            };
            sink.out(&format!(
                "将把 {} 里的 {} 条转发给 {}{forum}：{}。{}；{}",
                send::target_zh(plan.from()),
                plan.items().len(),
                send::target_zh(plan.target()),
                texts.join("；"),
                confirm_zh(plan.mode()),
                send::QUEUE_NOTE_ZH,
            ));
        }
    }
}

/// 结尾一句：几条、成功几条、失败几条、结果未知的点名（不要再转）
fn summary_zh(items: &[serde_json::Value]) -> String {
    let ids = |o: &str| -> Vec<String> {
        items
            .iter()
            .filter(|i| i["outcome"] == o)
            .map(|i| i["message_id"].to_string())
            .collect()
    };
    let mut zh = format!(
        "要转 {} 条，成功 {} 条",
        items.len(),
        ids("succeeded").len()
    );
    // 文字模式没有 items：失败、转不了的也点名 id，补转要列的就是它们
    for (o, word) in [("failed", "失败"), ("not_forwarded", "转不了")] {
        let got = ids(o);
        if !got.is_empty() {
            zh.push_str(&format!(
                "，{word} {} 条（来源里的 id {}）",
                got.len(),
                got.join("、")
            ));
        }
    }
    let unsure = ids("unknown");
    if !unsure.is_empty() {
        zh.push_str(&format!(
            "，结果未知 {} 条（来源里的 id {}：可能已转出，网络恢复后 Telegram 会继续送出；这几条不要再转，补转也别列进去）",
            unsure.len(),
            unsure.join("、")
        ));
    }
    zh
}

#[cfg(test)]
mod tests;
