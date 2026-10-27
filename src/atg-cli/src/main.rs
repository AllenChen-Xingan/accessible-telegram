//! `atg-cli`：无障碍 Telegram 的命令行入口。契约：`specs/contracts/cli.md`。
//!
//! 已做：`version` / `--version` / `--help`；登录这组 `login` · `auth *` · `logout`（第一刀第 4 块，T01）；
//! `chats`（第 5 块，T02）；`speech --fixture`（第 6 块，读法定标）；`history` / `links`（第 8 块，T03 T06）；
//! 本机接口（第 9 块）：`serve` 常驻当执行方，档案被占时 `chats` / `history` / `links` / `auth state` 自动转过去（`forward`）。真实后端要 `--features tdlib`。其余子命令输出一行
//! `not_implemented` 错误。

// 拍板 005 补丁 ③（`docs/plan/005_decisions-b3.md`）：同 `atg-core` 根上那行（另有 `scripts/check-no-debug.sh` 补 clippy 管不到的）
#![cfg_attr(
    not(test),
    deny(
        clippy::use_debug,
        clippy::dbg_macro,
        clippy::unwrap_used,
        clippy::expect_used
    )
)]

mod args;
// 列会话的流程只有真实后端调；不编 TDLib 时只有单测用它
#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
mod chats;
mod forward;
// 读消息的流程只有真实后端调；不编 TDLib 时只有单测用它
#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
mod history;
mod login;
mod out;
// 执行方只有真实后端起；不编 TDLib 时只有单测用它
#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
mod serve;
mod session;
mod signals;
mod speech;
// 终端读写只有真实后端用得到（假后端场景还没做）；不编 TDLib 时它们是死代码
#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
mod tty;
// 直跑只有真实后端起；不编 TDLib 时只有单测与执行方用它
#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
mod watch;

use args::{Cli, Command, Format};
use atg_core::status::AuthStep;
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use out::{Code, Fail, Sink, Stdio, emit, emit_to, envelope, fail, fail_with};
use serde_json::json;
use session::Job;
use std::ffi::OsString;
use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
    // panic 信息可能带出运行时的值：只说一句，不打印原文（`config-privacy.md` 第一节第 5 小节黑名单）
    std::panic::set_hook(Box::new(|_| eprintln!("atg-cli: 内部错误（panic）")));
    let started = Instant::now();
    let argv: Vec<OsString> = std::env::args_os().collect();

    let cli = match Cli::try_parse_from(&argv) {
        Ok(cli) => cli,
        Err(e) => return parse_failed(&e, format_from_raw(&argv)),
    };
    // 占位子命令把后面的参数整个收下，放在其后的 `--format` 会被吞进去；骨架阶段从原始参数补认。
    // 各块按契约定好真参数后，占位收参去掉，这行也跟着去掉
    let format = match format_from_raw(&argv) {
        Format::Text => Format::Text,
        Format::Json => cli.global.format,
    };

    let command = match (cli.version, cli.command) {
        (true, None) | (_, Some(Command::Version)) => Command::Version,
        (false, Some(c)) => c,
        (true, Some(_)) => {
            return fail(
                format,
                Code::Usage,
                "--version 不能和别的子命令一起用",
                json!({}),
            );
        }
        (false, None) => {
            eprintln!("{}", Cli::command().render_help());
            return fail(
                format,
                Code::Usage,
                "缺少子命令，用 --help 看用法",
                json!({}),
            );
        }
    };

    match command {
        Command::Version => version(format, started),
        Command::Login => run_job(&cli.global, format, Job::Login, started),
        Command::Auth { step } => run_job(&cli.global, format, Job::Auth(step.sub()), started),
        Command::Logout => run_job(&cli.global, format, Job::Logout, started),
        Command::Chats(a) => run_chats(&cli.global, format, &a, started),
        Command::History(a) => run_history(&cli.global, format, &a, started),
        Command::Links(a) => run_links(&cli.global, format, &a, started),
        Command::Serve => serve::run(&cli.global, format, started),
        Command::Watch(a) => run_watch(&cli.global, format, &a, started),
        // speech 只读夹具文件，不起后端；带 --fake 多半是用错了，不悄悄忽略
        Command::Speech(_) if cli.global.fake.is_some() => fail(
            format,
            Code::Usage,
            "speech 不用 --fake，夹具文件用 --fixture 给",
            json!({}),
        ),
        Command::Speech(a) => speech::run(&a, cli.global.tz.as_deref(), format, started),
        other => {
            let name = other.name();
            fail(
                format,
                Code::NotImplemented,
                &format!("「{name}」还没做（骨架阶段占位）"),
                json!({ "command": name }),
            )
        }
    }
}

/// 登录这组命令：每走到一步出一行 `auth`（只有步骤名），最后一行 `result` / `error`。
/// 档案被占时只有 `auth state` 转给执行方（只读、不带秘密）；其余不转（秘密不过接口），照旧报 `profile_busy`
fn run_job(g: &args::Global, format: Format, job: Job, started: Instant) -> ExitCode {
    if matches!(job, Job::Auth(login::AuthSub::State))
        && let Some(code) = forward::try_forward(g, format, "auth", &json!({ "sub": "state" }))
    {
        return ExitCode::from(code);
    }
    let r = session::run(g, job, &mut |step| emit_step(&mut Stdio, format, step));
    // 占着档案的是常驻进程（接口连得上），这条命令却不转：说清要先停掉它
    if let Err(Fail::Core(e @ atg_core::error::CoreError::ProfileBusy { .. })) = &r
        && let Some(m) = forward::busy_reason(g, false)
    {
        return ExitCode::from(out::busy_to(&mut Stdio, format, e, m));
    }
    ExitCode::from(job_output(&mut Stdio, format, job, r, started))
}

/// 授权走到的一步：JSON 一行 `auth`；文字一句
fn emit_step(sink: &mut dyn Sink, format: Format, step: AuthStep) {
    match format {
        Format::Json => emit_to(
            sink,
            &envelope(
                "auth",
                json!({ "step": serde_json::to_value(step).unwrap_or_default() }),
            ),
        ),
        Format::Text => sink.out(login::step_zh(step)),
    }
}

/// 登录这组命令的结尾行，返回退出码。`started`：直跑是进程启动，经本机接口是执行方收到请求
fn job_output(
    sink: &mut dyn Sink,
    format: Format,
    job: Job,
    r: Result<serde_json::Value, Fail>,
    started: Instant,
) -> u8 {
    match r {
        Ok(mut data) => {
            match format {
                Format::Json => {
                    if let Some(d) = data.as_object_mut() {
                        d.insert("elapsed_ms".into(), json!(elapsed_ms(started)));
                    }
                    emit_to(sink, &envelope("result", data));
                }
                Format::Text => sink.out(match (job, data["already_signed_out"].as_bool()) {
                    (Job::Logout, Some(true)) => "本来就没有登录",
                    (Job::Logout, _) => "已登出",
                    (Job::Login, _) => "已登录",
                    (Job::Auth(_), _) => "完成",
                }),
            }
            0
        }
        Err(f) => out::fail_with_to(sink, format, &f),
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// `chats` 的查询；`--cursor` 形状不对是用法错误
fn chats_query(a: &args::ChatsArgs) -> Result<chats::Query, Fail> {
    let after = match a.cursor.as_deref().map(chats::Cursor::parse) {
        None => None,
        Some(Some(c)) => Some(c),
        Some(None) => {
            return Err(Fail::Usage(
                "--cursor 不对：用上一页 result 里的 next_cursor",
            ));
        }
    };
    Ok(chats::Query {
        list: a.list.list(),
        limit: usize::try_from(a.limit).unwrap_or(usize::MAX),
        unread: a.unread,
        after,
    })
}

/// `chats`：一行一个会话（`kind:"chat"`，`data` 是 `ChatItem`，`speech.brief` 是会话项读法）+ 一行 `result`。
/// 文字模式每行一句读法，统计写标准错误。档案被占就转给执行方
fn run_chats(g: &args::Global, format: Format, a: &args::ChatsArgs, started: Instant) -> ExitCode {
    // 会话项带名字和最后一条的正文：同 history / links，打码没做之前不悄悄出原文
    if let Some(f) = no_redact(g, "chats") {
        return fail_with(format, &f);
    }
    let q = match chats_query(a) {
        Ok(q) => q,
        Err(f) => return fail_with(format, &f),
    };
    let params = serde_json::to_value(a).unwrap_or_default();
    if let Some(code) = forward::try_forward(g, format, "chats", &params) {
        return ExitCode::from(code);
    }
    let code = match session::chats(g, &q, started) {
        Ok(l) => render_chats(&mut Stdio, format, a, &l, started),
        Err(f) => out::fail_with_to(&mut Stdio, format, &f),
    };
    ExitCode::from(code)
}

/// 把列出来的会话写到 `sink`，返回退出码（直跑与执行方共用这一段，输出才一致）
fn render_chats(
    sink: &mut dyn Sink,
    format: Format,
    a: &args::ChatsArgs,
    listing: &chats::Listing,
    started: Instant,
) -> u8 {
    // 会话项读法不读时间，时区不影响；`--tz` 留给读消息的命令
    let cx = atg_core::speech::SpeechCtx {
        utc_offset: 0,
        offset_at: None,
        listen_key: None,
        expand_key: None,
        reveal: a.reveal,
    };
    for item in &listing.items {
        let doc = atg_core::tree::Doc::ChatItem(item.clone());
        match format {
            Format::Json => {
                let mut line = atg_core::json::value(&doc, &cx);
                if let Some(o) = line.as_object_mut() {
                    o.insert("at".into(), json!(out::now_rfc3339_ms()));
                }
                emit_to(sink, &line);
            }
            Format::Text => sink.out(&atg_core::speech::chat_brief_with(item, a.reveal)),
        }
    }
    match format {
        Format::Json => {
            let mut data = json!({
                "command": "chats",
                "list": a.list.name(),
                "count": listing.items.len(),
                "more": listing.more,
                "synced": listing.synced,
                "load_timed_out": listing.load_timed_out,
                "first_chat_ms": listing.first_chat_ms,
                "elapsed_ms": elapsed_ms(started),
            });
            if let (Some(d), Some(c)) = (data.as_object_mut(), &listing.next_cursor) {
                d.insert("next_cursor".into(), json!(c));
            }
            emit_to(sink, &envelope("result", data));
        }
        Format::Text => {
            let first = listing.first_chat_ms.map_or_else(
                || "列表是空的".to_owned(),
                |ms| format!("冷启动到第一条会话 {ms} 毫秒"),
            );
            let synced = if listing.synced {
                ""
            } else {
                "；没等到同步完，未读数可能是旧的"
            };
            let more = match (listing.load_timed_out, listing.more) {
                (true, _) => "；加载超时（可能断网），只列出了本地已有的",
                (false, true) => "；还有更多",
                (false, false) => "",
            };
            sink.err(&format!(
                "共 {} 个会话；{first}{synced}{more}",
                listing.items.len()
            ));
        }
    }
    0
}

/// `--redact`（打码）还没做：吐名字、正文、链接的命令（`chats`、`history`、`links`）不能悄悄忽略它照样出原文，
/// 先报 `not_implemented`
fn no_redact(g: &args::Global, command: &'static str) -> Option<Fail> {
    g.redact.then_some(Fail::NotImplemented {
        command,
        message_zh: "--redact 打码还没做；这条命令会输出名字和正文，请先不加 --redact",
    })
}

/// `history`：一行一条消息（`kind:"message"`，从旧到新）+ 一行 `result`。不标已读。档案被占就转给执行方
fn run_history(
    g: &args::Global,
    format: Format,
    a: &args::HistoryArgs,
    started: Instant,
) -> ExitCode {
    if let Some(f) = no_redact(g, "history") {
        return fail_with(format, &f);
    }
    // 用法错误在本地就报（与直跑一样）
    let (q, cx) = match history::query(a)
        .and_then(|q| history::speech_ctx(g.tz.as_deref(), a.reveal).map(|cx| (q, cx)))
    {
        Ok(x) => x,
        Err(f) => return fail_with(format, &f),
    };
    // 读法时区是全局选项，执行方那边没有：填进参数随请求带过去（`config-privacy.md` 协议 v0）
    let mut params = serde_json::to_value(a).unwrap_or_default();
    if let Some(o) = params.as_object_mut() {
        o.insert("tz".into(), json!(g.tz));
    }
    if let Some(code) = forward::try_forward(g, format, "history", &params) {
        return ExitCode::from(code);
    }
    let code = match session::history(g, &q, started) {
        Ok(p) => history::render(&mut Stdio, format, a, &cx, &p, started),
        Err(f) => out::fail_with_to(&mut Stdio, format, &f),
    };
    ExitCode::from(code)
}

/// `watch`：一行 `status`、每个事件一行、Ctrl+C 后一行 `result`（退出码 0）。不标已读。档案被占就经本机接口流式订阅
fn run_watch(g: &args::Global, format: Format, a: &args::WatchArgs, started: Instant) -> ExitCode {
    if let Some(f) = no_redact(g, "watch") {
        return fail_with(format, &f);
    }
    let (q, cx) = match watch::query(a)
        .and_then(|q| history::speech_ctx(g.tz.as_deref(), a.reveal).map(|cx| (q, cx)))
    {
        Ok(x) => x,
        Err(f) => return fail_with(format, &f),
    };
    let mut params = serde_json::to_value(a).unwrap_or_default();
    if let Some(o) = params.as_object_mut() {
        o.insert("tz".into(), json!(g.tz));
    }
    signals::install();
    if let Some(code) =
        forward::try_forward_stream(g, format, "watch", &params, &signals::STOP, started)
    {
        return ExitCode::from(code);
    }
    match session::watch(g, &q, cx, format, started) {
        Ok(code) => ExitCode::from(code),
        Err(f) => fail_with(format, &f),
    }
}

/// `links`：一行一个链接（`kind:"link"`）+ 一行 `result`。不标已读。档案被占就转给执行方
fn run_links(g: &args::Global, format: Format, a: &args::LinksArgs, started: Instant) -> ExitCode {
    if let Some(f) = no_redact(g, "links") {
        return fail_with(format, &f);
    }
    let q = match history::links_query(a) {
        Ok(q) => q,
        Err(f) => return fail_with(format, &f),
    };
    let params = serde_json::to_value(a).unwrap_or_default();
    if let Some(code) = forward::try_forward(g, format, "links", &params) {
        return ExitCode::from(code);
    }
    let code = match session::links(g, &q, started) {
        Ok(l) => history::render_links(&mut Stdio, format, a.reveal, &l, started),
        Err(f) => out::fail_with_to(&mut Stdio, format, &f),
    };
    ExitCode::from(code)
}

/// `version`：一行 `kind:"version"` + 一行 `result`
fn version(format: Format, started: Instant) -> ExitCode {
    let cli_version = env!("CARGO_PKG_VERSION");
    match format {
        Format::Json => {
            emit(&envelope(
                "version",
                json!({
                    "atg_cli": cli_version,
                    "v": atg_core::tree::SCHEMA_V,
                    // 没编进 TDLib（不带 `tdlib` 特性）时是 null
                    "tdlib": tdlib_version(),
                }),
            ));
            let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            emit(&envelope(
                "result",
                json!({ "command": "version", "elapsed_ms": elapsed_ms }),
            ));
        }
        Format::Text => {
            let td =
                tdlib_version().map_or_else(|| "没编进 TDLib".to_owned(), |v| format!("TDLib {v}"));
            println!(
                "atg-cli {cli_version}，外壳版本 {}，{td}",
                atg_core::tree::SCHEMA_V
            );
        }
    }
    ExitCode::SUCCESS
}

/// 链进来的 TDLib 版本；没编进来是 `None`
fn tdlib_version() -> Option<String> {
    #[cfg(feature = "tdlib")]
    {
        atg_core::td::tdlib_version()
    }
    #[cfg(not(feature = "tdlib"))]
    {
        None
    }
}

/// 参数解析失败：`--help` 照常给人看；其余按 `usage` 出一行错误，人看的说明走标准错误
fn parse_failed(e: &clap::Error, format: Format) -> ExitCode {
    match e.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            let _ = e.print();
            ExitCode::SUCCESS
        }
        _ => {
            // clap 的原文可能带用户输入，只进标准错误，不进标准输出
            eprint!("{}", e.render());
            fail(format, Code::Usage, "参数不对，用 --help 看用法", json!({}))
        }
    }
}

/// 解析失败时还拿不到 `--format`，从原始参数里认一下
fn format_from_raw(argv: &[OsString]) -> Format {
    let mut it = argv.iter().skip(1);
    while let Some(a) = it.next() {
        if a == "--format=text" || (a == "--format" && it.next().is_some_and(|v| v == "text")) {
            return Format::Text;
        }
    }
    Format::Json
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }

    #[test]
    fn raw_format_detection() {
        assert!(matches!(
            format_from_raw(&os(&["atg-cli", "--format", "text", "x"])),
            Format::Text
        ));
        assert!(matches!(
            format_from_raw(&os(&["atg-cli", "x", "--format=text"])),
            Format::Text
        ));
        assert!(matches!(
            format_from_raw(&os(&["atg-cli", "--format"])),
            Format::Json
        ));
    }

    #[test]
    fn clap_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
