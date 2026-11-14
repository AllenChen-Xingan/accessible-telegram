//! 用法错误写进标准错误的那段话：只说哪类错、哪个参数，**绝不带用户写的值**（#64）。
//!
//! clap 的原文会把认不出的参数、不合法的值原样印出来：人把验证码当参数写（`auth code 40419`）、
//! `send --text` 的正文（`.42cog/real.md`「隐私」）就会进终端和 AI 抓的日志。这里只用命令定义里本来就有的名字
//! （参数名、子命令名、建议、可选值），用户的原文一个字都不拿。所有命令都走这一处。

use crate::args::Cli;
use clap::CommandFactory;
use clap::error::{ContextKind, ContextValue, ErrorKind};
use std::ffi::OsString;

/// 标准错误上的那段话（末尾带换行）
pub fn usage_text(e: &clap::Error, argv: &[OsString]) -> String {
    let mut s = format!("atg-cli: 参数不对：{}", kind_zh(e.kind()));
    // 认不出的参数、值、子命令：`InvalidArg` / `InvalidValue` / `InvalidSubcommand` 里是用户的原文，不印
    if e.kind() != ErrorKind::UnknownArgument
        && let Some(arg) = text(e.get(ContextKind::InvalidArg))
    {
        s += &format!("（{arg}）");
    }
    s.push('\n');
    if let Some(prior) = text(e.get(ContextKind::PriorArg)) {
        s += &format!("  与 {prior} 不能同用\n");
    }
    for kind in [ContextKind::SuggestedArg, ContextKind::SuggestedSubcommand] {
        if let Some(hint) = text(e.get(kind)) {
            s += &format!("  是不是想写 {hint}\n");
        }
    }
    if let Some(valid) = text(e.get(ContextKind::ValidValue)) {
        s += &format!("  可以写：{valid}\n");
    }
    s += &format!("用 `{} --help` 看用法（写的值不回显）\n", path(argv));
    s
}

fn kind_zh(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::UnknownArgument => "有认不出的参数",
        ErrorKind::InvalidSubcommand => "没有这个子命令",
        ErrorKind::InvalidValue | ErrorKind::ValueValidation => "值不对",
        ErrorKind::MissingRequiredArgument => "缺必填参数",
        ErrorKind::MissingSubcommand => "缺子命令",
        ErrorKind::ArgumentConflict => "参数冲突",
        ErrorKind::TooManyValues | ErrorKind::TooFewValues | ErrorKind::WrongNumberOfValues => {
            "值的个数不对"
        }
        ErrorKind::NoEquals => "这个参数要用 = 连写值",
        ErrorKind::InvalidUtf8 => "参数不是 UTF-8",
        _ => "写法不对",
    }
}

/// 上下文里的名字（来自命令定义）；多个用顿号连
fn text(v: Option<&ContextValue>) -> Option<String> {
    let s = match v? {
        ContextValue::String(s) => s.clone(),
        ContextValue::Strings(v) if !v.is_empty() => v.join("、"),
        _ => return None,
    };
    (!s.is_empty()).then_some(s)
}

/// 用户写到了哪一层子命令：只认定义里有的名字，所以印出来的都是命令自己的词
fn path(argv: &[OsString]) -> String {
    let mut cmd = Cli::command();
    let mut out = String::from("atg-cli");
    for a in argv.iter().skip(1) {
        let Some(a) = a.to_str() else { continue };
        if a == "--" {
            break;
        }
        let Some(sub) = cmd.find_subcommand(a).cloned() else {
            continue;
        };
        out.push(' ');
        out.push_str(sub.get_name());
        cmd = sub;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::usage_text;
    use crate::args::Cli;
    use clap::Parser;
    use std::ffi::OsString;

    fn text_for(args: &[&str]) -> String {
        let argv: Vec<OsString> = std::iter::once("atg-cli")
            .chain(args.iter().copied())
            .map(OsString::from)
            .collect();
        let e = Cli::try_parse_from(&argv).err().expect("该是用法错误");
        usage_text(&e, &argv)
    }

    /// 秘密、正文写错地方：哪种错都不带原文（登录组与 send 两类）
    #[test]
    fn user_values_never_appear() {
        for (args, secret) in [
            (&["auth", "code", "40419"][..], "40419"),
            (&["auth", "code", "-40419"][..], "40419"),
            (&["login", "--phone", "+99966123", "多余"][..], "+99966123"),
            (&["login", "--phonee=+99966123"][..], "+99966123"),
            (
                &["send", "1", "--text", "私信正文", "--txt", "私信正文二"][..],
                "私信正文",
            ),
            (
                &["send", "1", "--text", "私信正文", "--text-file", "/x"][..],
                "私信正文",
            ),
            (&["send", "私信正文"][..], "私信正文"),
            (&["--format", "私信正文", "me"][..], "私信正文"),
            (&["私信正文"][..], "私信正文"),
        ] {
            let s = text_for(args);
            assert!(!s.contains(secret), "{args:?}: {s}");
            assert!(s.starts_with("atg-cli: 参数不对"), "{args:?}: {s}");
        }
    }

    /// 命令自己的词照常说：参数名、可选值、子命令路径
    #[test]
    fn definition_names_are_kept() {
        let s = text_for(&["--format", "xml", "me"]);
        assert!(s.contains("--format") && s.contains("json"), "{s}");
        let s = text_for(&["auth", "code", "40419"]);
        assert!(s.contains("atg-cli auth code --help"), "{s}");
    }
}
