//! 投票与按钮的读法（#13，只读）。**句子是 AI 草稿，待人实听**（`specs/style/` 没定投票选项与按钮的说法）。
//! - 名称（`brief`）：投票「投票，题目，3 个选项，你已投 / 已结束」；带按钮的消息末尾「，3 个按钮」
//! - 详情（`detail`）：投票的性质（匿名、多选、能否改票、测验、你不能投）；按钮逐个「文字（种类）」
//! - 全文（`full`）：投票逐项「第 1 项，文字，5 票，42%，你选的」+ 总票数

use super::content::spans;
use crate::tree::{ButtonAction, Keyboard, KeyboardKind, PollOption, Span};

/// 名称里的投票：题目 + 几个选项 + 你已投 / 已结束
pub(super) fn poll_label(
    question: &[Span],
    options: &[PollOption],
    closed: bool,
    reveal: bool,
) -> String {
    let mut rest = vec![format!("{} 个选项", options.len())];
    if options.iter().any(|o| o.chosen) {
        rest.push("你已投".to_owned());
    }
    if closed {
        rest.push("已结束".to_owned());
    }
    let rest = rest.join("，");
    let q = spans(question, reveal).0;
    let q = q.trim();
    if q.is_empty() {
        return format!("投票，{rest}");
    }
    // 题目自己以标点结尾（「几点出发？」）就不再接逗号，免得念成「？，」（大总管定 2026-11-17）
    let sep = if q.ends_with(ENDS) { "" } else { "，" };
    format!("投票，{q}{sep}{rest}")
}

/// 题目末尾这些标点之后不再接「，」
const ENDS: [char; 10] = ['？', '?', '！', '!', '。', '.', '…', '：', ':', '；'];

/// 选项里有没有隐藏内容（剧透）：整条的按键提示要算上
pub(super) fn options_hidden(options: &[PollOption], reveal: bool) -> bool {
    options.iter().any(|o| spans(&o.text, reveal).1)
}

/// 全文里的投票：逐项读文字、票数、百分比，你选的标出；最后总票数
pub(super) fn poll_full(
    question: &[Span],
    options: &[PollOption],
    total: u32,
    reveal: bool,
) -> String {
    let mut items = Vec::new();
    for (i, o) in options.iter().enumerate() {
        let mut p = format!(
            "第 {} 项，{}，{} 票，{}%",
            i + 1,
            spans(&o.text, reveal).0.trim(),
            o.votes,
            o.percent
        );
        if o.chosen {
            p.push_str("，你选的");
        }
        items.push(p);
    }
    items.push(format!("共 {total} 人投票"));
    let items = items.join("；");
    let q = spans(question, reveal).0;
    let q = q.trim();
    // 同名称：题目以标点结尾就不再接分隔号
    let sep = if q.is_empty() || q.ends_with(ENDS) {
        ""
    } else {
        "；"
    };
    format!("投票，{q}{sep}{items}")
}

/// 详情里的投票性质：有哪项说哪项
pub(super) fn poll_detail(
    anonymous: bool,
    multiple: bool,
    revotable: bool,
    quiz: bool,
    closed: bool,
    can_vote: bool,
) -> String {
    let mut parts = vec![if anonymous {
        "匿名投票"
    } else {
        "公开投票，别人看得到你投了什么"
    }];
    if quiz {
        parts.push("测验，投了不能改");
    } else if revotable {
        parts.push("投了能改");
    } else {
        parts.push("投了不能改");
    }
    if multiple {
        parts.push("可以多选");
    }
    if closed {
        parts.push("已结束");
    } else if !can_vote {
        parts.push("你不能投");
    }
    parts.join("，")
}

/// 名称末尾：「3 个按钮」（机器人换掉输入框的键盘说「3 个键盘按钮」）
pub(super) fn buttons_brief(k: &Keyboard) -> String {
    match k.kind {
        KeyboardKind::Inline => format!("{} 个按钮", k.count()),
        KeyboardKind::Reply => format!("{} 个键盘按钮", k.count()),
    }
}

/// 详情里的按钮：逐个「文字（种类）」，网址按钮带网址
pub(super) fn buttons_detail(k: &Keyboard) -> String {
    let head = match k.kind {
        KeyboardKind::Inline => "按钮",
        KeyboardKind::Reply => "键盘按钮",
    };
    let list: Vec<String> = k
        .rows
        .iter()
        .flatten()
        .map(|b| format!("{}（{}）", b.text.trim(), action(&b.action)))
        .collect();
    format!("{head}：{}", list.join("、"))
}

/// 按下会怎样，说给人听
fn action(a: &ButtonAction) -> String {
    match a {
        ButtonAction::Url { url } => format!("打开网址 {url}"),
        ButtonAction::LoginUrl { .. } => "以你的身份登录网站".to_owned(),
        ButtonAction::WebApp => "打开网页应用".to_owned(),
        ButtonAction::Callback => "发给机器人".to_owned(),
        ButtonAction::CallbackWithPassword => "发给机器人，要输入密码".to_owned(),
        ButtonAction::Game => "开游戏".to_owned(),
        ButtonAction::SwitchInline => "内联查询".to_owned(),
        ButtonAction::Buy => "付款".to_owned(),
        ButtonAction::User { .. } => "打开用户资料".to_owned(),
        ButtonAction::CopyText { .. } => "复制文字".to_owned(),
        ButtonAction::Disabled => "按不了".to_owned(),
        ButtonAction::Text => "发出这句话".to_owned(),
        ButtonAction::RequestPhone => "发出你的手机号".to_owned(),
        ButtonAction::RequestLocation => "发出你的位置".to_owned(),
        ButtonAction::RequestPoll => "新建投票".to_owned(),
        ButtonAction::RequestUsers => "选用户发给机器人".to_owned(),
        ButtonAction::RequestChat => "选会话发给机器人".to_owned(),
        ButtonAction::Unknown { .. } => "不认识的按钮".to_owned(),
    }
}
