//! `send-file`：发一个本机文件或图片（#12，T04）。契约：`specs/contracts/cli.md` `send-file` 行，
//! 动作与闸：`specs/contracts/actions.md` `send_file_prepare` / `send_file`；流程在 `atg_core::send::file`。
//!
//! 输出：
//! 1. 一行 `plan`：目标、文件名、字节数、SHA-256 前 8 位、按图片还是按文件发（退回按文件发的写原因）、说明文字、回复哪条。
//!    **不带路径**
//! 2. `--dry-run` 到此为止（快照随即删掉）
//! 3. 闸开凭据（测试服务器只许收藏夹；正式服务器直发不问，但查防重复，`--again` 放行）
//! 4. 等结果：有上传进度就接着等，连续 60 秒没进度或总共 10 分钟按结果未知（退出码 10，不许重发）
//!
//! 不经本机接口代跑（v0，同 `download`）：档案被常驻进程占着 → `profile_busy`，说明请先停掉它。
//! 只许发「下载」「桌面」「文稿」里的文件，像密钥的文件名不许（人定，见 `atg_core::send::file`）。

use crate::args::{Format, Global};
use crate::chats::{self, Feed};
use crate::deadlines::{QUERY_TIMEOUT, bounded};
use crate::history;
use crate::out::{self, Fail, Sink, Stdio, emit_to, envelope};
use crate::send::{confirm_zh, ctx, echo, fail_any, finish_line, l2_fail};
use atg_core::backend::Backend;
use atg_core::error::CoreError;
use atg_core::link::EventStream;
use atg_core::project;
use atg_core::send::file::{self as file, FilePlan, FileRoots, FileSource, SendAs};
use atg_core::send::{self, SendOrigin, Waited};
use clap::Args;
use serde_json::json;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// `send-file` 的参数
#[derive(Args)]
pub struct SendFileArgs {
    /// 会话 id，或 saved（自己的收藏夹）
    #[arg(value_name = "CHAT_ID", allow_hyphen_values = true)]
    pub chat: String,
    /// 要发的文件：只许「下载」「桌面」「文稿」里的（有别的硬链接的不许），50 MB 以内。jpg / jpeg / png / webp 按图片发（会被压缩），其余按文件发
    #[arg(value_name = "PATH")]
    pub path: PathBuf,
    /// 说明文字（1024 个 UTF-16 码元以内：多数汉字算 1 个，emoji 常算 2 个）
    #[arg(long, value_name = "T")]
    pub caption: Option<String>,
    /// 从文件读说明文字（UTF-8）
    #[arg(long, value_name = "PATH")]
    pub caption_file: Option<PathBuf>,
    /// 回复本会话里的哪条
    #[arg(long, value_name = "MSG_ID")]
    pub reply_to: Option<i64>,
    /// 只输出要发什么，不发
    #[arg(long)]
    pub dry_run: bool,
    /// 最近向同一会话发过同一个文件会被拦下（按内容认，换路径、改名也算同一个；说明或回复不同的不算）：已发出的 10 分钟内，
    /// 结果不明的 24 小时内。确要再发一次就加它
    #[arg(long)]
    pub again: bool,
}

/// 一次 `send-file` 要的东西
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct SendFileParams {
    pub chat: String,
    pub path: PathBuf,
    pub caption: String,
    pub reply_to: Option<i64>,
    pub dry_run: bool,
    pub again: bool,
    pub tz: Option<String>,
}

pub fn params(a: &SendFileArgs, tz: Option<&str>) -> Result<SendFileParams, Fail> {
    history::chat_arg(&a.chat)?;
    ctx(tz)?;
    let caption = match (&a.caption, &a.caption_file) {
        (Some(t), None) => t.clone(),
        (None, Some(p)) => crate::send::read_text_file(p)?,
        (None, None) => String::new(),
        _ => {
            return Err(Fail::Usage(
                "说明文字用 --caption 或 --caption-file 给，二选一",
            ));
        }
    };
    Ok(SendFileParams {
        chat: a.chat.clone(),
        path: a.path.clone(),
        caption,
        reply_to: a.reply_to,
        dry_run: a.dry_run,
        again: a.again,
        tz: tz.map(str::to_owned),
    })
}

/// 入口：`--redact` 先拒 → 档案被常驻进程占着就说清要先停掉它（不代跑）→ 直跑
pub fn run(g: &Global, format: Format, a: &SendFileArgs, started: Instant) -> ExitCode {
    if let Some(f) = crate::no_redact(g, "send-file") {
        return out::fail_with(format, &f);
    }
    let p = match params(a, g.tz.as_deref()) {
        Ok(p) => p,
        Err(f) => return out::fail_with(format, &f),
    };
    // 假后端：`l2_run` 直接报 not_implemented，不碰本机档案
    let roots = if g.fake.is_some() {
        None
    } else {
        let profile = match crate::session::open_profile(g) {
            Ok(pr) => pr,
            Err(f) => return out::fail_with(format, &f),
        };
        if let Err(e @ CoreError::ProfileBusy { .. }) = profile.probe_lock()
            && let Some(m) = crate::forward::busy_reason(g, false)
        {
            return ExitCode::from(out::busy_to(&mut Stdio, format, &e, m));
        }
        match FileRoots::for_profile(&profile) {
            Ok(r) => Some(r),
            Err(e) => return out::fail_with(format, &Fail::Core(e)),
        }
    };
    let w = Wait {
        hand: send::RESULT_TIMEOUT,
        idle: file::UPLOAD_IDLE,
        cap: file::UPLOAD_CAP,
    };
    ExitCode::from(crate::session::l2_run(
        g,
        format,
        "send-file",
        started,
        &mut |b, stream, feed, sink| match &roots {
            Some(r) => execute(b, stream, feed, &p, r, sink, format, started, w),
            None => fail_any(
                sink,
                format,
                &Fail::Core(CoreError::Internal {
                    what: "file_roots_missing",
                }),
            ),
        },
    ))
}

/// 等多久：交出、没进度、总共
#[derive(Clone, Copy)]
pub struct Wait {
    pub hand: Duration,
    pub idle: Duration,
    pub cap: Duration,
}

/// 在已起好的后端上发一个文件，输出写到 `sink`，返回退出码
#[allow(clippy::too_many_arguments)]
pub fn execute<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &SendFileParams,
    roots: &FileRoots,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    wait: Wait,
) -> u8 {
    match run_inner(b, stream, feed, p, roots, sink, format, started, wait) {
        Ok(code) => code,
        Err(f) => fail_any(sink, format, &f),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_inner<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &SendFileParams,
    roots: &FileRoots,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    wait: Wait,
) -> Result<u8, Fail> {
    let chat = history::chat_arg(&p.chat)?;
    let cx = ctx(p.tz.as_deref())?;
    let src = FileSource {
        path: p.path.clone(),
        caption: p.caption.clone(),
        reply_to: p.reply_to,
    };
    // 文件的问题先报（会话不存在、网络不通时也是；自审第 2 轮）
    file::precheck(&src, roots).map_err(Fail::Core)?;
    chats::ensure_ready(b)?;
    let chat_id = history::resolve_chat(b, chat)?;
    history::load_chat(b, stream, feed, chat_id)?;
    let plan = bounded(
        b,
        file::prepare_file(b, chat_id, &src, roots, SendOrigin::Cli),
        QUERY_TIMEOUT,
    )
    .map_err(Fail::Core)?;
    feed.drain(stream);
    emit_plan(sink, format, &plan);
    if p.dry_run {
        finish_line(
            sink,
            format,
            json!({"command": "send-file", "dry_run": true, "chat_id": chat_id, "elapsed_ms": crate::elapsed_ms(started)}),
            "只看了计划，没有发",
        );
        return Ok(0);
    }

    let token = b.gate().issue(&plan, p.again).map_err(Fail::Core)?;
    let (mode, temp, waited) = file::send_file_and_wait(
        b,
        &plan,
        token,
        stream,
        wait.hand,
        wait.idle,
        wait.cap,
        &mut |env| feed.apply(env),
    )
    .map_err(Fail::Core)?;
    match waited {
        Waited::Succeeded(m) => {
            if let Some(d) = project::message(feed.store(), m.id) {
                echo(sink, format, &cx, &d);
            }
            let data = json!({
                "command": "send-file",
                "chat_id": chat_id,
                "message_id": m.id.message_id,
                "temp_message_id": temp.message_id,
                "send_as": plan.send_as().kind(),
                "size": plan.size(),
                "confirm": mode.as_str(),
                "elapsed_ms": crate::elapsed_ms(started),
            });
            finish_line(sink, format, data, "已发出");
            Ok(0)
        }
        Waited::Failed {
            message,
            error,
            can_retry,
        } => Ok(l2_fail(
            sink,
            format,
            &error,
            json!({
                "chat_id": chat_id,
                "message_id": message.id.message_id,
                "temp_message_id": temp.message_id,
                "can_resend": can_retry,
            }),
        )),
        Waited::Unknown(why) => Err(Fail::Core(CoreError::SendOutcomeUnknown {
            chat_id: Some(chat_id),
            temp_message_id: temp.message_id,
            why,
        })),
    }
}

fn emit_plan(sink: &mut dyn Sink, format: Format, plan: &FilePlan) {
    match format {
        Format::Json => emit_to(sink, &envelope("plan", file::file_plan_json(plan))),
        Format::Text => {
            let how = match plan.send_as() {
                SendAs::Photo { .. } => "按图片发（会被压缩）".to_owned(),
                SendAs::Document { fallback: None } => "按文件发".to_owned(),
                SendAs::Document {
                    fallback: Some(why),
                } => format!("按文件发（{}，不能按图片发）", file::fallback_zh(why)),
            };
            let caption = if plan.caption().is_empty() {
                String::new()
            } else {
                format!("；说明：{}", atg_core::gate::visible(plan.caption()))
            };
            let reply = plan
                .reply_to()
                .map(|r| format!("；回复第 {} 条", r.message_id))
                .unwrap_or_default();
            sink.out(&format!(
                "将发给 {}：文件「{}」，{} 字节，{how}{caption}{reply}。{}；{}",
                send::target_zh(plan.target()),
                atg_core::gate::visible(plan.name()),
                plan.size(),
                confirm_zh(plan.mode()),
                send::QUEUE_NOTE_ZH,
            ));
        }
    }
}

#[cfg(test)]
mod tests;
