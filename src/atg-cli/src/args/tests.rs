//! 命令行解析（只测 clap，不起后端、不连 Telegram）。
//!
//! #153：可变长的 `MSG_ID` 不能把后面的选项吞成 id。按契约顺序写 `resend 777 5 --dry-run`，`--dry-run` 必须是选项；
//! 被吞的话 `--dry-run` 会被当成一个 id 去解析，报用法错误（退出码 2），按契约的写法就用不了只看计划。
//! 会话 id 是单值参数，负数照旧能用（群、频道的 id 是负数）。
//!
//! 例子测 `resend`（这次修的）；`delete` 的 `MSG_ID` 本来就没开 `allow_hyphen_values`，留作对照、防以后回退；`send` 那条
//! 确认负数会话 id 加各个选项的组合照常能用。最后一条按结构查全部子命令：可变长的位置参数都不许开 `allow_hyphen_values`。

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

/// 结构检查：任何子命令里，能收多个值的位置参数都不许开 `allow_hyphen_values`（会把后面的选项吞掉，#153）。
/// 例外只有子命令占位 `Rest`（参数原样收下，交给子命令自己再解析）
#[test]
fn no_variadic_positional_swallows_options() {
    use clap::CommandFactory;
    fn walk(cmd: &clap::Command, path: &str, bad: &mut Vec<String>) {
        for a in cmd.get_positionals() {
            let many = a.get_num_args().is_some_and(|r| r.max_values() > 1);
            if many && a.is_allow_hyphen_values_set() && a.get_id() != "args" {
                bad.push(format!("{path} {}", a.get_id()));
            }
        }
        for sub in cmd.get_subcommands() {
            walk(sub, &format!("{path} {}", sub.get_name()), bad);
        }
    }
    let mut bad = Vec::new();
    let mut cmd = Cli::command();
    cmd.build();
    walk(&cmd, "atg-cli", &mut bad);
    assert!(bad.is_empty(), "这些可变长参数会吞掉后面的选项：{bad:?}");
}
