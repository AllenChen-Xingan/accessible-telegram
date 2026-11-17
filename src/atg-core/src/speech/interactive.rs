//! 投票与按钮的读法（#13，只读）。**句子是 AI 草稿，待人实听**（`specs/style/` 没定投票选项与按钮的说法）。
//! 分层（第九节：名称只放谁、说了什么、几点）：
//! - 名称（`brief`）：投票只读「投票，题目」（同改动前）；按钮不进名称
//! - 详情（`detail`）：投票几个选项、你投了没有、能不能看结果、性质（匿名、多选、能否改票、测验）、为什么不能投；
//!   按钮逐个「文字（按下会怎样）」
//! - 全文（`full`）：投票逐项「第 1 项，文字，5 票，42%，你选的，正确答案」+ 总人数 + 测验解析；看不到结果时不读各项票数。
//!   全文不读按钮（按钮在详情里）

use super::content::spans;
use crate::tree::{ButtonAction, Keyboard, KeyboardKind, PollOption, Span, VoteBlock};

/// 题目末尾这些标点之后不再接分隔号（免得念成「？；」，大总管定 2026-11-17）
const ENDS: [char; 10] = ['？', '?', '！', '!', '。', '.', '…', '：', ':', '；'];

/// 选项里有没有隐藏内容（剧透）：整条的按键提示要算上
pub(super) fn options_hidden(options: &[PollOption], reveal: bool) -> bool {
    options.iter().any(|o| spans(&o.text, reveal).1)
}

/// 全文里的投票：逐项读文字；看得到结果才读票数与百分比；你选的、正确答案标出；最后总人数与测验解析
pub(super) fn poll_full(
    question: &[Span],
    options: &[PollOption],
    total: u32,
    results_visible: bool,
    explanation: &[Span],
    reveal: bool,
) -> String {
    let mut items = Vec::new();
    for (i, o) in options.iter().enumerate() {
        let mut p = format!("第 {} 项，{}", i + 1, spans(&o.text, reveal).0.trim());
        if results_visible {
            p.push_str(&format!("，{} 票，{}%", o.votes, o.percent));
        }
        if o.chosen {
            p.push_str("，你选的");
        }
        if o.correct {
            p.push_str("，正确答案");
        }
        items.push(p);
    }
    items.push(if results_visible {
        format!("共 {total} 人投票")
    } else {
        // TDLib 这时给各项 0 票，不是真没人投（`td_api.tl:468`）
        format!("共 {total} 人投票，投了才看得到各项票数")
    });
    let e = spans(explanation, reveal).0;
    if !e.trim().is_empty() {
        items.push(format!("解析：{}", e.trim()));
    }
    let items = items.join("；");
    let q = spans(question, reveal).0;
    let q = q.trim();
    let sep = if q.is_empty() || q.ends_with(ENDS) {
        ""
    } else {
        "；"
    };
    format!("投票，{q}{sep}{items}")
}

/// 详情要的那几样投票性质
pub(super) struct PollFacts<'a> {
    pub options: usize,
    pub voted: bool,
    pub results_visible: bool,
    pub anonymous: bool,
    pub multiple: bool,
    pub revotable: bool,
    pub quiz: bool,
    pub closed: bool,
    pub blocked: Option<&'a VoteBlock>,
}

/// 详情里的投票：几个选项、你投了没有，再说性质；已结束就不说「能改」；不能投说原因
pub(super) fn poll_detail(f: &PollFacts<'_>) -> String {
    let mut parts = vec![format!("{} 个选项", f.options)];
    if f.voted {
        parts.push("你已投".to_owned());
    } else if !f.results_visible {
        parts.push("投了才看得到结果".to_owned());
    }
    parts.push(
        if f.anonymous {
            "匿名投票"
        } else {
            "公开投票，别人看得到你投了什么"
        }
        .to_owned(),
    );
    if f.multiple {
        parts.push("可以多选".to_owned());
    }
    if f.quiz {
        parts.push("测验，有正确答案，投了不能改".to_owned());
    } else if !f.closed {
        parts.push(
            if f.revotable {
                "投了能改"
            } else {
                "投了不能改"
            }
            .to_owned(),
        );
    }
    if let Some(b) = f.blocked {
        parts.push(blocked(b).to_owned());
    }
    parts.join("，")
}

fn blocked(b: &VoteBlock) -> &'static str {
    match b {
        VoteBlock::Closed => "已结束",
        VoteBlock::YetUnsent => "还没发出去，发出后才能投",
        VoteBlock::Scheduled => "定时消息，发出后才能投",
        VoteBlock::CountryRestricted { .. } => "你所在的地区不能投",
        VoteBlock::MembershipRequired => "要先加入这个会话满一天才能投",
        VoteBlock::Other | VoteBlock::Unknown { .. } => "你不能投",
    }
}

/// 详情里的按钮：「按钮 4 个：」再逐个「文字（按下会怎样）」，网址按钮带网址
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
    format!("{head} {} 个：{}", k.count(), list.join("、"))
}

/// 按下会怎样，说给人听。「通知机器人」= 只把按钮上看不见的数据交给机器人、聊天里不出现；
/// 「以你的名义发出」= 真发一条消息到会话里，别人看得到
fn action(a: &ButtonAction) -> String {
    match a {
        ButtonAction::Url { url } => format!("打开网址 {url}"),
        ButtonAction::LoginUrl { .. } => "以你的身份登录网站".to_owned(),
        ButtonAction::WebApp => "打开网页应用".to_owned(),
        ButtonAction::Callback => "通知机器人，聊天里不出现".to_owned(),
        ButtonAction::CallbackWithPassword => "通知机器人，要输入密码".to_owned(),
        ButtonAction::Game => "开游戏".to_owned(),
        ButtonAction::SwitchInline => "在输入框里调用这个机器人".to_owned(),
        ButtonAction::Buy => "付款".to_owned(),
        ButtonAction::User { .. } => "打开用户资料".to_owned(),
        ButtonAction::CopyText { .. } => "复制文字".to_owned(),
        ButtonAction::Disabled => "按不了".to_owned(),
        ButtonAction::Text => "以你的名义发出这句话".to_owned(),
        ButtonAction::RequestPhone => "把你的手机号发给机器人".to_owned(),
        ButtonAction::RequestLocation => "把你的位置发给机器人".to_owned(),
        ButtonAction::RequestPoll => "新建投票".to_owned(),
        ButtonAction::RequestUsers => "选用户发给机器人".to_owned(),
        ButtonAction::RequestChat => "选会话发给机器人".to_owned(),
        ButtonAction::Unknown { .. } => "不认识的按钮".to_owned(),
    }
}
