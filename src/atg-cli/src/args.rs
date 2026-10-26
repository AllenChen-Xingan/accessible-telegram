//! 参数定义：子命令名与全局选项，照 `specs/contracts/cli.md` 第一节第 2 小节。
//!
//! 已做的子命令（`login` · `auth *` · `logout` · `chats` · `speech` · `version`）按契约定了参数；其余还只占名字，后面的参数原样收下不校验。

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::ffi::OsString;

#[derive(Parser)]
#[command(
    name = "atg-cli",
    about = "无障碍 Telegram 的命令行入口。标准输出一行一个 JSON；给人看加 --format text",
    disable_version_flag = true
)]
pub struct Cli {
    /// 输出版本（与子命令 `version` 相同）
    #[arg(long)]
    pub version: bool,

    #[command(flatten)]
    pub global: Global,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// 全局选项：放在子命令前后都可以。现在起作用的：`--format`、`--test-dc`、`--profile`（登录这组与 `chats`）、
/// `--tz`（`speech`）。
/// 其余收下不用（帮助里标了「还没做」）
#[derive(Args)]
pub struct Global {
    /// 选测试服务器档案，并断言它是测试服务器
    #[arg(long, global = true)]
    pub test_dc: bool,
    /// 本地档案名
    #[arg(long, global = true, value_name = "NAME", default_value = "main")]
    pub profile: String,
    /// 用假后端（夹具场景名或路径），不联网。还没做：场景目录未建，现在一律报 not_implemented
    #[arg(long, global = true, value_name = "SCENARIO")]
    pub fake: Option<OsString>,
    /// 输出格式
    #[arg(long, global = true, value_enum, default_value_t = Format::Json)]
    pub format: Format,
    /// 打码输出，用于贴进 issue：只留类型、计数、时间、状态、错误码，名字和正文换成「类型 + 长度」。还没做
    #[arg(long, global = true)]
    pub redact: bool,
    /// 直发模式：本次运行发消息免确认（细则见 `specs/contracts/actions.md` 第一节第 5 小节）
    #[arg(long, global = true)]
    pub unsafe_direct_send: bool,
    /// 遇到 Telegram 限流（要求过 N 秒再试）就等够再继续，默认立刻报 rate_limited。还没做
    #[arg(long, global = true)]
    pub wait: bool,
    /// 读法里报钟点用的时区偏移：+08:00、-05:30、+0800、+8、Z 或 UTC。默认取系统时区。现在只有 speech 用（会话项不读时间）
    #[arg(long, global = true, value_name = "OFFSET", allow_hyphen_values = true)]
    pub tz: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Json,
    Text,
}

/// 子命令占位：只收名字，后面的参数原样收下
#[derive(Args)]
pub struct Rest {
    #[arg(num_args = 0.., allow_hyphen_values = true, hide = true)]
    pub args: Vec<OsString>,
}

#[derive(Subcommand)]
pub enum Command {
    /// 交互登录（给人用）：手机号 → 验证码 → 二步验证密码，都从终端读、不回显
    Login,
    /// 分步登录（给 AI 和脚本）：每步一行 `auth`。秘密值从终端读；测试档案上号码取 `.env.local` 的 ATG_TEST_PHONE
    // 缺分步名是用法错误（一行 `usage`），不是给人看的帮助
    #[command(arg_required_else_help = false)]
    Auth {
        #[command(subcommand)]
        step: AuthCmd,
    },
    /// 登出本机（须在终端当场确认）
    Logout,
    /// 自己是谁
    Me(Rest),
    /// 待发出的消息；`pending cancel CHAT_ID MSG_ID` 撤销一条
    Pending(Rest),
    /// 内核 / 授权 / 连接状态
    Status(Rest),
    /// 列会话，一行一个，顺序同官方客户端。会等服务器把离线期间漏掉的更新（新消息、未读数）补送完再出，
    /// 等不到（断网）照样从本地出，结尾行 synced 为 false
    Chats(ChatsArgs),
    /// 下一个有未读的会话
    NextUnread(Rest),
    /// 列某会话的消息（不标已读）
    History(Rest),
    /// 列出一条消息里的链接
    Links(Rest),
    /// 下载一条消息的附件
    Download(Rest),
    /// 用系统打开链接或附件
    Open(Rest),
    /// 静音 / 取消静音
    Mute(Rest),
    /// 删除消息（给所有人）
    Delete(Rest),
    /// 重发失败的消息
    Resend(Rest),
    /// 一条消息的全部细节（不标已读）
    Show(Rest),
    /// 搜索：`search chats` / `search messages`
    Search(Rest),
    /// 发消息（须确认）
    Send(Rest),
    /// 订阅更新
    Watch(Rest),
    /// 草稿：`draft set` / `draft clear`
    Draft(Rest),
    /// 播放语音
    Play(Rest),
    /// 只打印读法：夹具（TDLib 更新的 JSON 数组）→ 树 → 读法，不联网
    Speech(SpeechArgs),
    /// 机读自描述
    Schema(Rest),
    /// 版本
    Version,
}

/// `chats` 的参数
#[derive(Args)]
pub struct ChatsArgs {
    /// 哪个列表：主列表或归档
    #[arg(long, value_enum, default_value_t = ListArg::Main)]
    pub list: ListArg,
    /// 这一页最多几个
    #[arg(
        short = 'n',
        long = "limit",
        value_name = "N",
        default_value_t = 100,
        value_parser = clap::value_parser!(u32).range(1..=100_000)
    )]
    pub limit: u32,
    /// 只列有未读的
    #[arg(long)]
    pub unread: bool,
    /// 接着上一页列：填上一页最后一行（kind 为 result）里的 next_cursor
    #[arg(long, value_name = "C")]
    pub cursor: Option<String>,
}

/// `speech` 的参数（`specs/contracts/cli.md`）。`--fixture` 是单个读法夹具文件。`--scenario` 现在**没有**，名字预留给以后的场景目录
/// （假后端用的一整套数据：会话、用户、历史、时间线，约定在 `src/atg-core/fixtures/scenarios/<名>/`、目录还没建，`testing.md` 第一节第 3 小节）
#[derive(Args)]
pub struct SpeechArgs {
    /// 夹具文件：TDLib 更新对象的 JSON 数组，按顺序喂进来（如 src/atg-core/fixtures/speech/basic.json）
    #[arg(long, value_name = "PATH", required = true)]
    pub fixture: std::path::PathBuf,
    /// 读哪一层：brief = 按到达顺序每条消息一句；chat_brief = 主列表（不含归档）每个会话一句
    #[arg(long, value_enum, default_value_t = SpeechLevel::Brief)]
    pub level: SpeechLevel,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SpeechLevel {
    Brief,
    #[value(name = "chat_brief")]
    ChatBrief,
}

impl SpeechLevel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Brief => "brief",
            Self::ChatBrief => "chat_brief",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ListArg {
    Main,
    Archive,
}

impl ListArg {
    pub fn list(self) -> atg_core::model::ChatList {
        match self {
            Self::Main => atg_core::model::ChatList::Main,
            Self::Archive => atg_core::model::ChatList::Archive,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Archive => "archive",
        }
    }
}

/// `auth` 的分步子命令
#[derive(Subcommand, Clone, Copy)]
pub enum AuthCmd {
    /// 现在走到哪一步
    State,
    /// 交手机号
    Phone,
    /// 交验证码
    Code,
    /// 交二步验证密码
    Password,
    /// 交登录邮箱
    Email,
    /// 交邮箱里收到的验证码
    EmailCode,
    /// 换一种方式重发验证码
    Resend,
}

impl AuthCmd {
    pub fn sub(self) -> crate::login::AuthSub {
        use crate::login::AuthSub as S;
        match self {
            Self::State => S::State,
            Self::Phone => S::Phone,
            Self::Code => S::Code,
            Self::Password => S::Password,
            Self::Email => S::Email,
            Self::EmailCode => S::EmailCode,
            Self::Resend => S::Resend,
        }
    }
}

impl Command {
    /// 契约里的子命令名
    pub fn name(&self) -> &'static str {
        match self {
            Command::Login => "login",
            Command::Auth { .. } => "auth",
            Command::Logout => "logout",
            Command::Me(_) => "me",
            Command::Pending(_) => "pending",
            Command::Status(_) => "status",
            Command::Chats(_) => "chats",
            Command::NextUnread(_) => "next-unread",
            Command::History(_) => "history",
            Command::Links(_) => "links",
            Command::Download(_) => "download",
            Command::Open(_) => "open",
            Command::Mute(_) => "mute",
            Command::Delete(_) => "delete",
            Command::Resend(_) => "resend",
            Command::Show(_) => "show",
            Command::Search(_) => "search",
            Command::Send(_) => "send",
            Command::Watch(_) => "watch",
            Command::Draft(_) => "draft",
            Command::Play(_) => "play",
            Command::Speech(_) => "speech",
            Command::Schema(_) => "schema",
            Command::Version => "version",
        }
    }
}
