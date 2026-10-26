//! `atg-cli`：无障碍 Telegram 的命令行入口。契约：`specs/contracts/cli.md`。
//!
//! 已做：`version` / `--version` / `--help`；登录这组 `login` · `auth *` · `logout`（第一刀第 4 块，T01）；
//! `chats`（第 5 块，T02）。真实后端要 `--features tdlib`。其余子命令输出一行 `not_implemented` 错误。

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
mod login;
mod out;
mod session;
// 终端读写只有真实后端用得到（假后端场景还没做）；不编 TDLib 时它们是死代码
#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
mod tty;

use args::{Cli, Command, Format};
use atg_core::status::AuthStep;
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use out::{Code, emit, envelope, fail, fail_with, text_line};
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

/// 登录这组命令：每走到一步出一行 `auth`（只有步骤名），最后一行 `result` / `error`
fn run_job(g: &args::Global, format: Format, job: Job, started: Instant) -> ExitCode {
    let mut on_step = |step: AuthStep| match format {
        Format::Json => emit(&envelope(
            "auth",
            json!({ "step": serde_json::to_value(step).unwrap_or_default() }),
        )),
        Format::Text => text_line(login::step_zh(step)),
    };
    match session::run(g, job, &mut on_step) {
        Ok(mut data) => {
            match format {
                Format::Json => {
                    let elapsed_ms =
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    if let Some(d) = data.as_object_mut() {
                        d.insert("elapsed_ms".into(), json!(elapsed_ms));
                    }
                    emit(&envelope("result", data));
                }
                Format::Text => text_line(match (job, data["already_signed_out"].as_bool()) {
                    (Job::Logout, Some(true)) => "本来就没有登录",
                    (Job::Logout, _) => "已登出",
                    (Job::Login, _) => "已登录",
                    (Job::Auth(_), _) => "完成",
                }),
            }
            ExitCode::SUCCESS
        }
        Err(f) => fail_with(format, &f),
    }
}

/// `chats`：一行一个会话（`kind:"chat"`，`data` 是 `ChatItem`，`speech.brief` 是会话项读法）+ 一行 `result`。
/// 文字模式每行一句读法，统计写标准错误
fn run_chats(g: &args::Global, format: Format, a: &args::ChatsArgs, started: Instant) -> ExitCode {
    let after = match a.cursor.as_deref().map(chats::Cursor::parse) {
        None => None,
        Some(Some(c)) => Some(c),
        Some(None) => {
            return fail(
                format,
                Code::Usage,
                "--cursor 不对：用上一页 result 里的 next_cursor",
                json!({}),
            );
        }
    };
    let q = chats::Query {
        list: a.list.list(),
        limit: usize::try_from(a.limit).unwrap_or(usize::MAX),
        unread: a.unread,
        after,
    };
    let listing = match session::chats(g, &q, started) {
        Ok(l) => l,
        Err(f) => return fail_with(format, &f),
    };
    // 会话项读法不读时间，时区不影响；`--tz` 留给读消息的命令
    let cx = atg_core::speech::SpeechCtx {
        utc_offset: 0,
        listen_key: None,
    };
    for item in &listing.items {
        let doc = atg_core::tree::Doc::ChatItem(item.clone());
        match format {
            Format::Json => {
                let mut line = atg_core::json::value(&doc, &cx);
                if let Some(o) = line.as_object_mut() {
                    o.insert("at".into(), json!(out::now_rfc3339_ms()));
                }
                emit(&line);
            }
            Format::Text => text_line(&atg_core::speech::chat_brief(item)),
        }
    }
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
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
                "elapsed_ms": elapsed_ms,
            });
            if let (Some(d), Some(c)) = (data.as_object_mut(), &listing.next_cursor) {
                d.insert("next_cursor".into(), json!(c));
            }
            emit(&envelope("result", data));
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
            eprintln!("共 {} 个会话；{first}{synced}{more}", listing.items.len());
        }
    }
    ExitCode::SUCCESS
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
