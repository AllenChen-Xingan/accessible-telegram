//! `atg-cli`：无障碍 Telegram 的命令行入口。契约：`specs/contracts/cli.md`。
//!
//! 骨架阶段：`version` / `--version` / `--help` 能用；其余子命令输出一行 `not_implemented` 错误。

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
mod out;

use args::{Cli, Command, Format};
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use out::{Code, emit, envelope, fail};
use serde_json::json;
use std::ffi::OsString;
use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
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
                    // 骨架阶段还没接 TDLib
                    "tdlib": null,
                }),
            ));
            let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            emit(&envelope(
                "result",
                json!({ "command": "version", "elapsed_ms": elapsed_ms }),
            ));
        }
        Format::Text => {
            println!(
                "atg-cli {cli_version}，外壳版本 {}，还没接 TDLib",
                atg_core::tree::SCHEMA_V
            );
        }
    }
    ExitCode::SUCCESS
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
