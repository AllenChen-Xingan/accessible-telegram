//! 参数定义：子命令名与全局选项，照 `specs/contracts/cli.md` 第一节第 2 小节。
//!
//! 已做的子命令（`login` · `auth *` · `logout` · `chats` · `history` · `links` · `speech` · `version`）按契约定了参数；其余还只占名字，后面的参数原样收下不校验。

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
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

/// 全局选项：放在子命令前后都可以。各选项的含义见 `specs/contracts/cli.md` 的全局选项表；没做的在帮助里标了「还没做」。
/// 例外：`--redact` 在会输出名字、正文的命令上报 `not_implemented`（`main::no_redact`），免得以为打了码
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
    /// 打码输出，用于贴进 issue：只留类型、计数、时间、状态、错误码，名字和正文换成「类型 + 长度」。还没做：会输出名字、正文的命令加了它报 not_implemented（免得以为打了码），别的命令收下不用
    #[arg(long, global = true)]
    pub redact: bool,
    /// 旧参数，已无作用：发送一律直发（Touch ID 已去掉，人定 2026-11-16）。留着只为旧脚本不报错
    #[arg(long, global = true, hide = true)]
    pub unsafe_direct_send: bool,
    /// 遇到 Telegram 限流（要求过 N 秒再试）就等够再继续，默认立刻报 rate_limited。还没做
    #[arg(long, global = true)]
    pub wait: bool,
    /// 读法里报钟点用的时区偏移：+08:00、-05:30、+0800、+8、Z 或 UTC。默认取系统时区。读消息时间的命令用（会话项不读时间）
    #[arg(long, global = true, value_name = "OFFSET", allow_hyphen_values = true)]
    pub tz: Option<String>,
}

// 经本机接口转发时随请求带过去（`forward`）
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// 列出待发出的消息（本应用总开着本地库：上次没发完的会在任何一条命令启动时自动续发，跑这条也一样）；
    /// `pending cancel CHAT_ID` 尽力撤销这个会话里全部待发消息：请求已到服务器的照样送达、之后给所有人撤回。同 send 直发、不确认
    Pending(PendingArgs),
    /// 内核 / 授权 / 连接状态
    Status(Rest),
    /// 列会话，一行一个，顺序同官方客户端。会等服务器把离线期间漏掉的更新（新消息、未读数）补送完再出，
    /// 等不到（断网）照样从本地出，结尾行 synced 为 false
    Chats(ChatsArgs),
    /// 下一个有未读的会话
    NextUnread(Rest),
    /// 列某会话的消息，从旧到新（不标已读）。不给 --before / --after 时落在第一条未读，没有未读取最新的
    History(HistoryArgs),
    /// 列出一条消息里的链接（文字 + 地址）
    Links(LinksArgs),
    /// 下载一条语音（不标已听；档案被占时不转发）
    Download(DownloadArgs),
    /// 用系统打开链接或附件
    Open(Rest),
    /// 静音 / 取消静音
    Mute(Rest),
    /// 给所有人删除自己发的消息：先列出要删哪几条（kind:"plan"），然后立即删除，不再询问
    Delete(DeleteArgs),
    /// 改自己发的一条纯文字消息：先列出原文与新文（kind:"plan"），然后立即改，不再询问
    Edit(EditArgs),
    /// 给一条消息加上（或 --remove 去掉）一个 emoji 回应。先输出一行 plan，和 send 一样直接点、不确认。
    /// 测试服务器上只许收藏夹
    React(ReactArgs),
    /// 看谁点了这条消息的回应，一行一个（kind:"reactor"）
    Reactions(ReactionsArgs),
    /// 重发发送失败的消息：先输出一行要重发什么（kind:"plan"），然后直接重发、逐条等结果（同 send，不确认）
    Resend(ResendArgs),
    /// 一条消息的全部细节（不标已读）
    Show(Rest),
    /// 搜索：`search chats` / `search messages`
    Search(Rest),
    /// 发文字或回复某条。先输出一行要发什么（kind:"plan"：发给谁、全文、回复哪条），再确认：测试服务器发自己的收藏夹
    /// 不用确认；正式服务器直接发、不确认。
    /// 等到成功或失败才结束；30 秒没等到报 send_outcome_unknown（退出码 10）——消息可能已发出，不要重发
    Send(SendArgs),
    /// 订阅更新：一行一个事件，新消息带读法与提不提示的判定；Ctrl+C 结束。不标已读
    Watch(WatchArgs),
    /// 草稿：`draft set` / `draft clear`
    Draft(Rest),
    /// 前台放一条语音：标准输入收控制命令，标准输出逐行出状态（默认不标已听；档案被占时不转发）
    Play(PlayArgs),
    /// 本机把一条语音转成文字（不外发、不标已听；档案被占时不转发）
    Transcribe(TranscribeArgs),
    /// 只打印读法：夹具（TDLib 更新的 JSON 数组）→ 树 → 读法，不联网
    Speech(SpeechArgs),
    /// 机读自描述
    Schema(Rest),
    /// 常驻：占着档案、开本机接口，替后起的 atg-cli 执行命令（能转哪些见 specs/contracts/config-privacy.md），Ctrl+C 结束
    Serve,
    /// 版本
    Version,
}

/// `chats` 的参数（经本机接口转发时原样序列化过去）
#[derive(Args, Serialize, Deserialize)]
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
    /// 每个会话项显示的最后一条消息里的隐藏内容照出原文，等于人按了展开键。不加时：读法读「隐藏内容」，JSON 里剧透只留一个空占位（18+ 媒体的说明文字本来就照出；规则见 specs/content-tree.md 第八节）
    #[arg(long)]
    #[serde(default)]
    pub reveal: bool,
}

/// `history` 的参数（经本机接口转发时原样序列化过去，另带读法时区 `tz`）。契约 `specs/contracts/cli.md`
#[derive(Args, Serialize, Deserialize)]
pub struct HistoryArgs {
    /// 会话 id，或 saved（自己的收藏夹）
    #[arg(value_name = "CHAT_ID", allow_hyphen_values = true)]
    pub chat: String,
    /// 这一页最多几条
    #[arg(
        short = 'n',
        long = "limit",
        value_name = "N",
        default_value_t = 20,
        value_parser = clap::value_parser!(u32).range(1..=1000)
    )]
    pub limit: u32,
    /// 往前翻：比这条旧的（不含它）。填上一页 result 的 next_cursor
    #[arg(
        long,
        value_name = "MSG_ID",
        visible_alias = "from",
        conflicts_with = "after"
    )]
    pub before: Option<i64>,
    /// 往后翻：比这条新的（不含它）。填上一页 result 的 newer_cursor
    #[arg(long, value_name = "MSG_ID")]
    pub after: Option<i64>,
    /// 另带详情：日期、回复谁、转发自、已编辑、表情回应、话题名、送达状态
    #[arg(long)]
    pub detail: bool,
    /// 另带全文（保留换行与标点）；隐藏内容仍只读「隐藏内容」
    #[arg(long)]
    pub full: bool,
    /// 隐藏内容照出原文，等于人按了展开键。不加时：读法读「隐藏内容」，JSON 里剧透只留一个空占位（18+ 媒体的说明文字本来就照出；规则见 specs/content-tree.md 第八节）
    #[arg(long)]
    pub reveal: bool,
    /// 只读本地已有的，不联网
    #[arg(long)]
    pub local: bool,
    /// 读法时区：不是本子命令的参数，是全局选项 `--tz`，由发起方填进来随请求带给执行方
    #[arg(skip)]
    #[serde(default)]
    pub tz: Option<String>,
}

/// `watch` 的参数（经本机接口转发时原样序列化过去，另带读法时区 `tz`）。契约 `specs/contracts/cli.md`
#[derive(Args, Serialize, Deserialize)]
pub struct WatchArgs {
    /// 打开这个会话订阅（数字 id，或 saved）：超级群、频道只有打开的会话才推新消息；也是「当前会话」，对方正在输入只在它是私聊时播。不过滤：所有会话的事件照样输出。不标已读
    #[arg(long, value_name = "CHAT_ID", allow_hyphen_values = true)]
    pub chat: Option<String>,
    /// 隐藏内容照出原文，等于人按了展开键（规则同 history）
    #[arg(long)]
    pub reveal: bool,
    /// 读法时区：全局选项 `--tz`，由发起方填进来随请求带给执行方
    #[arg(skip)]
    #[serde(default)]
    pub tz: Option<String>,
}

/// `links` 的参数（经本机接口转发时原样序列化过去）
#[derive(Args, Serialize, Deserialize)]
pub struct LinksArgs {
    /// 会话 id，或 saved（自己的收藏夹）
    #[arg(value_name = "CHAT_ID", allow_hyphen_values = true)]
    pub chat: String,
    /// 消息 id
    #[arg(value_name = "MSG_ID")]
    pub message: i64,
    /// 隐藏的链接（落在剧透里、或链接文字里套着剧透）也给出文字和地址，等于人按了展开键；不加时只说「隐藏内容」、地址为 null（规则见 specs/content-tree.md 第八节）
    #[arg(long)]
    pub reveal: bool,
}

/// `react` 的参数
#[derive(Args)]
pub struct ReactArgs {
    #[command(flatten)]
    pub msg: MsgRef,
    /// 一个 emoji，如 👍（v0 只收 emoji；自定义表情、付费星星不做）
    #[arg(value_name = "EMOJI")]
    pub emoji: String,
    /// 去掉自己点过的这个回应
    #[arg(long)]
    pub remove: bool,
    /// 这条上你已点了别的回应时照样点：每条能点几个有上限（非 Telegram Premium 会员 1 个），超了 Telegram 会去掉你最早点的那个
    #[arg(long, conflicts_with = "remove")]
    pub replace: bool,
    /// 只输出要点什么，不点
    #[arg(long)]
    pub dry_run: bool,
}

/// `reactions` 的参数
#[derive(Args)]
pub struct ReactionsArgs {
    #[command(flatten)]
    pub msg: MsgRef,
    /// 只看这一个 emoji 的
    #[arg(long, value_name = "EMOJI")]
    pub emoji: Option<String>,
    /// 一页最多几个（1–100）
    #[arg(short = 'n', long = "limit", default_value_t = 50)]
    pub limit: u32,
    /// 接着上一页（上一页结尾行的 next_cursor）
    #[arg(long, value_name = "C")]
    pub cursor: Option<String>,
}

/// 一条消息：会话 + 消息 id（`download`、`play`、`transcribe`、`react`、`reactions` 共用）
#[derive(Args)]
pub struct MsgRef {
    /// 会话 id，或 saved（自己的收藏夹）
    #[arg(value_name = "CHAT_ID", allow_hyphen_values = true)]
    pub chat: String,
    /// 消息 id
    #[arg(value_name = "MSG_ID")]
    pub message: i64,
}

/// `download` 的参数。v0 只下语音（#4）；以后附件打开（M3.7）扩到别的附件
#[derive(Args)]
pub struct DownloadArgs {
    #[command(flatten)]
    pub at: MsgRef,
}

/// `play` 的参数
#[derive(Args)]
pub struct PlayArgs {
    #[command(flatten)]
    pub at: MsgRef,
    /// 标为已听（对方看得到）：出口准备好之后、出声之前标；阅后即焚的语音会拒绝（确认还没做）
    #[arg(long)]
    pub mark_listened: bool,
}

/// `transcribe` 的参数
#[derive(Args)]
pub struct TranscribeArgs {
    #[command(flatten)]
    pub at: MsgRef,
    /// 只用这一种语言的模型（不给时中文、英文都跑，取置信度高的）
    #[arg(long, value_enum)]
    pub lang: Option<Lang>,
    /// 被标为敏感内容（18+）的语音也输出转写文字；不加时照转，但只说「隐藏内容」
    #[arg(long)]
    pub reveal: bool,
}

/// `transcribe --lang` 的取值
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Lang {
    Zh,
    En,
}

/// `send` 的参数。正文只从 `--text` / `--text-file` 来，不从标准输入读（标准输入可能是 AI 拼进来的管道）
#[derive(Args)]
#[command(group(clap::ArgGroup::new("body").required(true).args(["text", "text_file"])))]
pub struct SendArgs {
    /// 会话 id，或 saved（自己的收藏夹）
    #[arg(value_name = "CHAT_ID", allow_hyphen_values = true)]
    pub chat: String,
    /// 要发的文字
    #[arg(long, value_name = "T")]
    pub text: Option<String>,
    /// 从文件读要发的文字（UTF-8，64 KB 以内；Telegram 一条最多 4096 字，超了会被拒，不自动拆条）
    #[arg(long, value_name = "PATH")]
    pub text_file: Option<std::path::PathBuf>,
    /// 回复本会话里的哪条（话题群里话题取被回复那条的）
    #[arg(long, value_name = "MSG_ID")]
    pub reply_to: Option<i64>,
    /// 只输出要发什么，不发
    #[arg(long)]
    pub dry_run: bool,
    /// 最近向同一会话发过同样的内容会被拦下（已发出的 10 分钟内；结果不明的 24 小时内）；确要再发一次就加它
    #[arg(long)]
    pub again: bool,
}

/// `resend` 的参数
#[derive(Args)]
pub struct ResendArgs {
    /// 会话 id，或 saved（自己的收藏夹）
    #[arg(value_name = "CHAT_ID", allow_hyphen_values = true)]
    pub chat: String,
    // `allow_hyphen_values` = 以「-」开头的词也当作值收，不当作选项。会话 id 要开（群、频道的 id 是负数）；这里不开：
    // 可变长参数开了它会把后面的 `--dry-run` 等选项吞成 id（#153）。消息 id 都是正数，用不着
    /// 要重发的失败消息 id（一条或几条）。用 send 失败时那行 error 里的 message_id——失败后 TDLib 会给它换一个新
    /// id，不是最初的那个，也不是 temp_message_id
    #[arg(value_name = "MSG_ID", required = true, num_args = 1..)]
    pub ids: Vec<i64>,
    /// 只输出要重发什么，不重发
    #[arg(long)]
    pub dry_run: bool,
}

/// `edit` 的参数
#[derive(Args)]
pub struct EditArgs {
    /// 会话 id，或 saved（自己的收藏夹；收藏夹里的不限时）
    #[arg(value_name = "CHAT_ID", allow_hyphen_values = true)]
    pub chat: String,
    /// 要改的消息 id（history 输出里的 message_id；只能是自己发的、已发出的纯文字消息）
    #[arg(value_name = "MSG_ID")]
    pub message_id: i64,
    /// 改成的文字（只有纯文字；原文带的格式会丢）
    #[arg(long, value_name = "T")]
    pub text: Option<String>,
    /// 从文件读改成的文字（UTF-8，64 KB 以内；Telegram 一条最多 4096 字，超了交出后被拒）
    #[arg(long, value_name = "PATH")]
    pub text_file: Option<std::path::PathBuf>,
    /// 只输出要怎么改，不改
    #[arg(long)]
    pub dry_run: bool,
}

/// `delete` 的参数
#[derive(Args)]
pub struct DeleteArgs {
    /// 会话 id（数字；收藏夹里的消息不能给所有人删）
    #[arg(value_name = "CHAT_ID", allow_hyphen_values = true)]
    pub chat: String,
    /// 要删的消息 id（history 输出里的 message_id；只能是自己发的、已发出的；一次最多 100 条）
    #[arg(value_name = "MSG_ID", required = true, num_args = 1..=100)]
    pub ids: Vec<i64>,
    /// 只输出要删什么，不删
    #[arg(long)]
    pub dry_run: bool,
}

/// `pending` 的参数
#[derive(Args)]
pub struct PendingArgs {
    #[command(subcommand)]
    pub sub: Option<PendingCmd>,
}

#[derive(Subcommand)]
pub enum PendingCmd {
    /// 尽力撤销这个会话里此刻全部待发消息（临时 id 每次启动都重编，不按 id 选）：先输出一行要撤销哪几条、
    /// 每条正文（kind:"plan"），随即交出，等这几条的下落
    Cancel {
        /// 会话 id，或 saved（自己的收藏夹）
        #[arg(value_name = "CHAT_ID", allow_hyphen_values = true)]
        chat: String,
        /// 只输出要撤销什么，不撤销
        #[arg(long)]
        dry_run: bool,
    },
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
    /// 隐藏内容照出原文，等于人按了展开键。不加时：读法读「隐藏内容」，JSON 里剧透只留一个空占位（18+ 媒体的说明文字本来就照出；规则见 specs/content-tree.md 第八节）
    #[arg(long)]
    pub reveal: bool,
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

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
            Command::Edit(_) => "edit",
            Command::React(_) => "react",
            Command::Reactions(_) => "reactions",
            Command::Resend(_) => "resend",
            Command::Show(_) => "show",
            Command::Search(_) => "search",
            Command::Send(_) => "send",
            Command::Watch(_) => "watch",
            Command::Draft(_) => "draft",
            Command::Play(_) => "play",
            Command::Transcribe(_) => "transcribe",
            Command::Speech(_) => "speech",
            Command::Schema(_) => "schema",
            Command::Serve => "serve",
            Command::Version => "version",
        }
    }
}

#[cfg(test)]
mod tests;
