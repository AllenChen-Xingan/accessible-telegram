//! 发送前本地就能判定的「这样发不行」的原因（[`InvalidSendWhy`]）与它们的中文说明。从 `error.rs` 拆出（只挪不改，#13 13b-2）

use serde::Serialize;

/// 发送前本地就能判定的「这样发不行」（机读码仍是 `invalid_input`，`why` 平铺进 `error.data`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidSendWhy {
    /// 话题群里不回复任何一条的新消息，第一阶段拒发（`actions.md` `send_text_prepare`）
    ForumNeedsReply,
    /// 被回复的那条不能回复（`messageProperties.can_be_replied` 为假）
    ReplyNotAllowed,
    /// 正文是空的
    EmptyText,
    /// 正文超过上限（`send::TEXT_MAX_BYTES`，64 KB）
    TooLong,
    /// 要重发的那条不是发送失败的消息
    NotFailed,
    /// TDLib 说这条不能重发（`can_retry` 为假）
    CannotRetry,
    /// 重发要换发送身份或换引用（第一阶段不做）
    NeedsOther,
    /// 重发要付星（付费发送不做）
    Paid,
    /// 表情回应给的不是一个 emoji（空、太长、全是 ASCII、带空白或除 U+FE0F / U+200D 外看不见的字符；#9）
    ReactionInvalid,
    /// 要去掉的回应我没点过（#9）
    ReactionNotChosen,
    /// 加上这个回应可能把我原先点的别的回应挤掉（每条能点几个有上限），没给 `--replace`（#9）
    ReactionWouldReplace,
    /// 拿不准这条上我已点了哪些回应（没等到同步完，或私聊、加密聊天以外的会话里服务器没给全名单），加上可能把它挤掉（#9）
    ReactionStateUnknown,
    /// 要撤销的那条不是自己的待发消息（已发出、失败了、或本来就不是自己发的）
    NotPending,
    /// 要撤销的待发消息里有不是纯文字的（带附件）：正文给不了人确认、也绑不进凭据，不撤（codex 029 #3）
    NotText,
    /// 会话里待发的超过一次能撤的上限（`delete::MAX_IDS`）：不截断、不分批，整批不撤（codex 029 #1）
    TooMany,
    /// 一条都没转出去（#10）：准备时有一条不能转（`messageProperties.can_be_forwarded` 为假，如来源会话开了禁止转发），
    /// 整批不交出；或交出后 TDLib 说每一条都转不了（应答全是 null）
    NotForwardable,
    /// 一次要转的超过上限（`send::forward::MAX_IDS`）：不截断、不分批，整批不转（#10）
    ForwardTooMany,
    /// 目标是加密聊天：TDLib 转进加密聊天一律改成复制（`send_copy` 恒为真，`td_api.tl:13177`），对方看到的消息不带原作者，
    /// 像是本人写的；本应用只做带原发送人的转发，所以整批不转（#10）
    ForwardToSecretChat,
    /// 交给 TDLib 之后它说其中几条转不了（应答里是 null），别的各有结果（#10；一条都没转出是 `NotForwardable`）
    NotForwarded,
    /// 要删 / 要改的那条在准备计划之后、交出之前变了（被编辑过）：计划里的正文已不对，什么都不动（#8）
    Changed,
    /// 要改的不是纯文字消息（带附件）：说明文字的编辑（`editMessageCaption`）第二批再做（#8 B，大总管定 2026-11-17）。
    /// 机读码与撤销的同名（`not_text`），说明不同
    #[serde(rename = "not_text")]
    EditNotText,
    /// 新的文字和原来一样：本地先拦，不让服务器报 `MESSAGE_NOT_MODIFIED`（#8 B）
    Unchanged,
    /// 这个文件不许发（#12 人定）：不在「下载」「桌面」「文稿」里（按解析符号链接后的真实路径；这三个文件夹以外的路径
    /// 不存在也报它），文件名像密钥，在本应用的数据目录里，或有别的硬链接（链接数大于 1）
    FileNotAllowed,
    /// 文件超过 50 MB（#12 人定）
    FileTooLarge,
    /// 在三个许可文件夹里，但不是普通文件（目录、设备……）、不存在或读不了（文件夹以外的报 `FileNotAllowed`）
    NotAFile,
    /// 说明文字超过上限（`send::file::CAPTION_MAX`）
    CaptionTooLong,
    /// 投票（#13）：这条显示的不是投票 · 已结束或不让投 · 投过且不许改 · 投过、这次会整组换掉而没给 `--replace` · 选号不对
    NotAPoll,
    VoteBlocked,
    VoteNotRevotable,
    VoteWouldReplace,
    VoteBadChoice,
    /// 按按钮（#13 13b-2）：这条消息上没有第 N 个按钮（没有按钮、编号越界，或显示的是临时内容）
    ButtonNotFound,
    /// 这种按钮本应用不按（带密码的回调、游戏、付款、网页应用、内联切换、交手机号 / 位置……、按不了的、认不出的）
    ButtonNotSupported,
}

impl InvalidSendWhy {
    /// 给人看的中文说明（[`super::CoreError::message_zh`] 里 `invalid_input` 的发送类）
    pub fn message_zh(self) -> &'static str {
        match self {
            InvalidSendWhy::ForumNeedsReply => "话题群里请先选一条消息回复",
            InvalidSendWhy::ReplyNotAllowed => "这条消息不能回复",
            InvalidSendWhy::EmptyText => "要发的内容是空的",
            InvalidSendWhy::TooLong => "要发的内容太长（超过 64 KB）",
            InvalidSendWhy::NotFailed => "这条消息不是发送失败的，不用重发",
            InvalidSendWhy::NotText => {
                "待发的消息里有不是纯文字的（带附件），给不出全文让你确认，本应用不撤；什么都没撤销"
            }
            InvalidSendWhy::TooMany => {
                "这个会话里待发的超过 100 条，一次撤不完，本应用不分批撤；什么都没撤销"
            }
            InvalidSendWhy::NotPending => {
                "没有可撤销的待发消息：这个会话里此刻没有待发的，或确认之后它已经发出、失败或内容变了；什么都没撤销"
            }
            InvalidSendWhy::Changed => {
                "那条消息在列出计划之后、真正动手之前变了（被编辑过，或投票的选项、性质、你投的票变了，或按钮被机器人换了），计划已经不对；什么都没动，请重新发起"
            }
            InvalidSendWhy::EditNotText => {
                "这条不是纯文字消息（带附件），本应用暂时只改纯文字消息的正文；什么都没改"
            }
            InvalidSendWhy::Unchanged => {
                "新的文字和原来一样，不用改（本应用只改文字，不能只去掉格式）；什么都没改"
            }
            InvalidSendWhy::CannotRetry => "这条消息 Telegram 说不能重发",
            InvalidSendWhy::NeedsOther => {
                "这条要换个身份发（比如以频道名义）或换引用才能重发，本应用暂不支持"
            }
            InvalidSendWhy::Paid => {
                "这条重发要付 Telegram Stars（Telegram 的付费星星），本应用不做付费发送"
            }
            InvalidSendWhy::ReactionInvalid => "表情回应请给一个 emoji，如 👍",
            InvalidSendWhy::ReactionNotChosen => {
                "这条消息上你没点过这个回应；emoji 写法可能不同（如 ❤ 与 ❤️），照消息详情里显示的抄"
            }
            InvalidSendWhy::ReactionWouldReplace => {
                "这条消息上你已点了别的回应，再点可能把它挤掉；确要换请明确说要替换"
            }
            InvalidSendWhy::ReactionStateUnknown => {
                "拿不准这条消息上你已点了哪些回应，再点可能把原来的挤掉；确要点请明确说要替换"
            }
            InvalidSendWhy::NotForwardable => {
                "要转的消息里有不能转发的（来源会话可能禁止转发）；什么都没转"
            }
            InvalidSendWhy::ForwardTooMany => "一次最多转 20 条，本应用不分批转；什么都没转",
            InvalidSendWhy::ForwardToSecretChat => {
                "不能转进加密聊天：Telegram 会把它变成复制、不带原作者，对方会以为是你写的；什么都没转"
            }
            InvalidSendWhy::NotForwarded => {
                "有的消息 Telegram 说转不了（结尾说明点了是哪几条），别的各有结果；不要整批加 --again 重转，那会把已转出的再转一遍"
            }
            InvalidSendWhy::FileNotAllowed => {
                "这个文件不许发：只许发「下载」「桌面」「文稿」三个文件夹里的；文件名像密钥（如 .env、id_rsa、.pem）的、本应用数据目录里的、有别的硬链接的也不许（硬链接的可复制一份到这三个文件夹再发）。路径写错、引号里的 ~ 没展开也会这样报，请写完整路径，如 /Users/你的用户名/Downloads/文件名；什么都没发"
            }
            InvalidSendWhy::FileTooLarge => "文件超过 50 MB，本应用不发；什么都没发",
            InvalidSendWhy::NotAFile => {
                "找不到这个文件，或它不是普通文件（比如是文件夹），或读不了；什么都没发"
            }
            InvalidSendWhy::CaptionTooLong => {
                "说明文字太长：最多 1024 个 UTF-16 码元（多数汉字算 1 个，emoji 常算 2 个）；什么都没发"
            }
            InvalidSendWhy::NotAPoll => "这条消息不是投票；什么都没投",
            InvalidSendWhy::VoteBlocked => {
                "这个投票已结束、你不能投、类型暂不支持，或你上一次投的还在提交中（history --detail 看那条投票会说原因）；什么都没投"
            }
            InvalidSendWhy::VoteNotRevotable => "你已投过这个投票，它不许改票；什么都没投",
            InvalidSendWhy::VoteWouldReplace => {
                "你已投过这个投票，这次会把你现在投的换掉（不是加上）；什么都没投"
            }
            InvalidSendWhy::ButtonNotFound => {
                "这条消息上没有这个按钮：写详情里读的第几个按钮（从 1 起）；什么都没按"
            }
            InvalidSendWhy::ButtonNotSupported => {
                "这种按钮本应用不按（如要密码、付款、交出手机号或位置的），请在手机上按；什么都没按"
            }
            InvalidSendWhy::VoteBadChoice => {
                "选项不对：写全文里读的第几项（从 1 起），不能重复；这个投票不许多选时只能选一项；什么都没投"
            }
        }
    }
}
