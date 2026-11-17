//! 命令行解析（只测 clap，不起后端、不连 Telegram）。
//!
//! #153：可变长的 `MSG_ID` 不能把后面的选项吞成 id。按契约顺序写 `resend 777 5 --dry-run`，`--dry-run` 必须是选项；
//! 被吞的话，本该只看计划的命令会变成真重发。会话 id 是单值参数，负数照旧能用（群、频道的 id 是负数）。

use super::*;
use clap::Parser;

fn parse(args: &[&str]) -> Cli {
    let argv = std::iter::once("atg-cli").chain(args.iter().copied());
    match Cli::try_parse_from(argv) {
        Ok(c) => c,
        Err(e) => panic!("该能解析：{args:?}\n{e}"),
    }
}

fn resend(args: &[&str]) -> ResendArgs {
    match parse(args).command {
        Some(Command::Resend(a)) => a,
        _ => panic!("该是 resend：{args:?}"),
    }
}

fn delete(args: &[&str]) -> DeleteArgs {
    match parse(args).command {
        Some(Command::Delete(a)) => a,
        _ => panic!("该是 delete：{args:?}"),
    }
}

#[test]
fn resend_dry_run_after_ids_is_an_option() {
    let a = resend(&["resend", "777", "5", "--dry-run"]);
    assert_eq!(a.chat, "777");
    assert_eq!(a.ids, vec![5]);
    assert!(a.dry_run, "--dry-run 被吞成了 MSG_ID");

    let a = resend(&["resend", "777", "5", "6", "--dry-run"]);
    assert_eq!(a.ids, vec![5, 6]);
    assert!(a.dry_run);
}

#[test]
fn resend_global_options_after_ids_are_options() {
    let c = parse(&["resend", "777", "5", "--format", "text", "--dry-run"]);
    assert!(c.global.format == Format::Text);
    match c.command {
        Some(Command::Resend(a)) => {
            assert_eq!(a.ids, vec![5]);
            assert!(a.dry_run);
        }
        _ => panic!("该是 resend"),
    }
}

#[test]
fn resend_negative_chat_id_still_works() {
    let a = resend(&["resend", "-1001234567", "5", "--dry-run"]);
    assert_eq!(a.chat, "-1001234567");
    assert_eq!(a.ids, vec![5]);
    assert!(a.dry_run);
}

#[test]
fn resend_without_dry_run_is_not_dry_run() {
    let a = resend(&["resend", "777", "5"]);
    assert!(!a.dry_run);
}

#[test]
fn delete_dry_run_after_ids_is_an_option() {
    let a = delete(&["delete", "777", "5", "6", "--dry-run"]);
    assert_eq!(a.chat, "777");
    assert_eq!(a.ids, vec![5, 6]);
    assert!(a.dry_run, "--dry-run 被吞成了 MSG_ID");
}

#[test]
fn delete_negative_chat_id_still_works() {
    let a = delete(&["delete", "-1001234567", "5", "--dry-run"]);
    assert_eq!(a.chat, "-1001234567");
    assert_eq!(a.ids, vec![5]);
    assert!(a.dry_run);
}

#[test]
fn send_negative_chat_with_reply_to_dry_run_again() {
    let c = parse(&[
        "send",
        "-1001234567",
        "--text",
        "你好",
        "--reply-to",
        "5",
        "--dry-run",
        "--again",
    ]);
    match c.command {
        Some(Command::Send(a)) => {
            assert_eq!(a.chat, "-1001234567");
            assert_eq!(a.reply_to, Some(5));
            assert!(a.dry_run);
            assert!(a.again);
        }
        _ => panic!("该是 send"),
    }
}
