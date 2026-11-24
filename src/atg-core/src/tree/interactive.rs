//! 能互动的那部分（#13）：投票的选项（[`PollOption`]，挂在 `Attachment::Poll` 里）与消息上的按钮（[`Keyboard`]）。
//! 只放数据，怎么读在 `speech/interactive.rs`。按钮的回调数据（`data`）不进树：它不给人看，按下时现取那条消息

use super::Span;
use serde::Serialize;

/// 投票的一个选项（`pollOption`，`td_api.tl:475`）。投票里的选项已按 TDLib 要求的显示顺序（`option_order`）排好
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct PollOption {
    /// TDLib 的 0 起编号（`options` 数组里的位置；投票 `setPollAnswer` 用它），与显示顺序不一定相同
    pub index: u32,
    pub text: Vec<Span>,
    /// 投它的人数。看不到结果（投票的 `results_visible` 为假：没投、或设了结束后才公布）时 TDLib 给 0，不是真没人投
    pub votes: u32,
    /// 占总票数的百分比（0–100，TDLib 算好的；看不到结果时同样是 0）
    pub percent: u32,
    /// 我选了它
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub chosen: bool,
    /// 测验的正确答案（`pollTypeQuiz.correct_option_ids`；别人发的、还没作答的测验 TDLib 不给，都为假）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub correct: bool,
    /// 我刚投的、还在提交（`is_being_chosen`）：这时 TDLib 把我原来投着的也报成没选，「我投了哪几项」拿不准（#13 13b-1）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub pending: bool,
}

/// 投票类型（`PollType`，`td_api.tl:484-494`）。开放集合：TDLib 以后加的类型进 `Unknown`；读法在详情与全文里多说一句
/// 「暂不支持的投票类型」，不说它是不是测验，投票自己的字段（匿名、多选、能否改）照读
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PollKind {
    /// `pollTypeRegular`
    Regular,
    /// `pollTypeQuiz`：有正确答案
    Quiz,
    Unknown {
        td_type: String,
    },
}

/// 我为什么不能投（`pollVoteRestrictionReason*`，`td_api.tl:513-529`）。开放集合，认不出的进 `Unknown`
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VoteBlock {
    /// 已结束
    Closed,
    /// 投票还没发出去（自己刚发的、还在待发）
    YetUnsent,
    /// 定时消息里的投票，发出前不能投
    Scheduled,
    /// 所在国家或地区的用户不许投
    CountryRestricted {
        country_code: String,
    },
    /// 只许投票发出前已在 `chat_id` 那个会话里满一天的人投（TDLib：入会时间早于投票发出时间一天以上；现在加入也投不了）。
    /// 会话不一定是投票所在的那个，如转发来的投票
    MembershipRequired {
        chat_id: i64,
    },
    /// 别的原因
    Other,
    Unknown {
        td_type: String,
    },
}

/// 消息上的按钮（`reply_markup`，`td_api.tl:4294-4314`）。有按钮的两种：消息下面的（`inline`）与机器人换掉输入框的
/// 键盘（`reply`）；「收起键盘」「强制回复」没有按钮，不建；认不出的种类进 [`KeyboardKind::Unknown`]（按钮读不出，只说有）
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Keyboard {
    pub kind: KeyboardKind,
    /// 一行一行（TDLib 的排法），空行已去掉
    pub rows: Vec<Vec<Button>>,
}

/// 开放集合（`ReplyMarkup`，`td_api.tl:4294-4314`）：认不出的带按钮种类进 `Unknown`（按钮读不出，只说有）
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KeyboardKind {
    /// `replyMarkupInlineKeyboard`：消息下面的按钮
    Inline,
    /// `replyMarkupShowKeyboard`：机器人换掉输入框的键盘，按下就是发那句文字（或交出手机号、位置……）
    Reply,
    /// 不认识的 `ReplyMarkup` 子类（TDLib 以后加的）；`rows` 为空
    Unknown { td_type: String },
}

impl Keyboard {
    /// 一共几个按钮
    pub fn count(&self) -> usize {
        self.rows.iter().map(Vec::len).sum()
    }
}

/// 一个按钮：上面的字 + 按下会怎样
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Button {
    pub text: String,
    pub action: ButtonAction,
}

/// 按下会怎样（内联 `inlineKeyboardButtonType*` `td_api.tl:4230-4266`、键盘 `keyboardButtonType*` `:4167-4215`）。
/// 开放集合：TDLib 以后会加新种类，认不出的进 `Unknown`
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ButtonAction {
    /// 打开网址
    Url {
        url: String,
    },
    /// 以你的身份登录一个网站
    LoginUrl {
        url: String,
    },
    /// 打开机器人的网页应用
    WebApp,
    /// 把按钮上的数据发给机器人（机器人收到后做什么由它定）
    Callback,
    /// 同上，但要先输入你账号的两步验证密码（`td_api.tl:4244-4246`）
    CallbackWithPassword,
    /// 开一个游戏
    Game,
    /// 在输入框里换成对这个机器人的内联查询
    SwitchInline,
    /// 付款
    Buy,
    /// 打开一个用户的资料
    User {
        user_id: i64,
    },
    /// 把一段文字复制到剪贴板
    CopyText {
        text: String,
    },
    /// 按不了
    Disabled,
    /// 键盘按钮：把按钮上的字当一条消息发出去
    Text,
    /// 键盘按钮：把你的手机号发给机器人
    RequestPhone,
    /// 键盘按钮：把你的位置发给机器人
    RequestLocation,
    /// 键盘按钮：新建一个投票发出去
    RequestPoll,
    /// 键盘按钮：选几个用户发给机器人
    RequestUsers,
    /// 键盘按钮：选一个会话发给机器人
    RequestChat,
    /// 键盘按钮：新建一个由这个机器人代管的机器人（`keyboardButtonTypeRequestManagedBot`，`td_api.tl:4212`）
    RequestManagedBot,
    Unknown {
        td_type: String,
    },
}
