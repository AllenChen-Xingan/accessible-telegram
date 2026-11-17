//! 投票与按钮的读法（#13，只读）。**句子是 AI 草稿，待人实听**（`specs/style/` 没定投票选项与按钮的说法）。
//! 分层（第九节：名称只放谁、说了什么、几点）：
//! - 名称（`brief`）：投票读「投票，题目」，有说明文字的接在后面（说明在正文里，同媒体说明）；按钮不进名称
//! - 详情（`detail`）：投票几个选项、你投了没有、能不能看结果、性质（匿名、多选、能否改票、测验）、为什么不能投；
//!   按钮逐个「文字，按下会怎样」
//! - 全文（`full`）：投票「题目，说明」再逐项「第 1 项，文字，5 票，42%，你选的，正确答案」+ 总人数 + 测验解析；看不到结果时不读各项票数。
//!   全文不读按钮（按钮在详情里）

use super::content::{glue, spans};
use crate::tree::{Attachment, ButtonAction, Keyboard, KeyboardKind, PollOption, VoteBlock};

/// 选项里有没有隐藏内容（剧透）：整条的按键提示要算上
pub(super) fn options_hidden(options: &[PollOption], reveal: bool) -> bool {
    options.iter().any(|o| spans(&o.text, reveal).1)
}

/// 看不到各项票数时说什么：只说推得出的事实。投了、还没结束却看不到 = 这个投票结束后才公布结果
/// （发起时设了 `hide_results_until_closes`，`td_api.tl:6788`）；别的情形不许诺「投了就看得到」
fn hidden_results(voted: bool, closed: bool) -> &'static str {
    if voted && !closed {
        "结束后才公布结果"
    } else {
        "现在看不到各项票数"
    }
}

/// 全文里的投票：题目、说明文字、逐项（看得到结果才读票数与百分比；你选的、正确答案标出）、总人数、测验解析
pub(super) fn poll_full(a: &Attachment, description: &str, reveal: bool) -> String {
    let Attachment::Poll {
        question,
        options,
        total,
        results_visible,
        closed,
        explanation,
        ..
    } = a
    else {
        return String::new();
    };
    let mut items = Vec::new();
    for (i, o) in options.iter().enumerate() {
        let mut p = format!("第 {} 项，{}", i + 1, spans(&o.text, reveal).0.trim());
        if *results_visible {
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
    let voted = options.iter().any(|o| o.chosen);
    items.push(if *results_visible {
        format!("共 {total} 人投票")
    } else {
        format!("共 {total} 人投票，{}", hidden_results(voted, *closed))
    });
    let e = spans(explanation, reveal).0;
    if !e.trim().is_empty() {
        items.push(format!("解析：{}", e.trim()));
    }
    let q = spans(question, reveal).0;
    let mut head = format!("投票，{}", q.trim());
    if !description.is_empty() {
        head = glue(&head, "，", description);
    }
    glue(&head, "；", &items.join("；"))
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
    }
    if !f.results_visible {
        parts.push(hidden_results(f.voted, f.closed).to_owned());
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
        parts.push("测验，有正确答案".to_owned());
    }
    // 能不能改只看 `allows_revoting`，测验也一样（TDLib `PollManager.cpp`：改票只看 `has_revoting_disabled_`，
    // 不看是不是测验，#13 自审第 4 轮）；已结束就不说
    if !f.closed {
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
        VoteBlock::MembershipRequired { .. } => "要先加入指定的会话满一天才能投",
        VoteBlock::Other | VoteBlock::Unknown { .. } => "你不能投",
    }
}

/// 详情里的按钮：「按钮 4 个：」再逐个「文字，按下会怎样」（按钮之间「；」），网址按钮读出地址（按钮文字常说不清去哪，
/// 是「链接只读文字」的例外，第九节写明）
pub(super) fn buttons_detail(k: &Keyboard) -> String {
    let head = match k.kind {
        KeyboardKind::Inline => "按钮",
        KeyboardKind::Reply => "键盘按钮",
    };
    let list: Vec<String> = k
        .rows
        .iter()
        .flatten()
        .map(|b| format!("{}，{}", b.text.trim(), action(&b.action)))
        .collect();
    // VoiceOver 不念括号（第九节）：按钮内用「，」，按钮之间用「；」，边界听得出来
    format!("{head} {} 个：{}", k.count(), list.join("；"))
}

/// 按下会怎样，说给人听。「通知机器人」= 只把按钮上看不见的数据交给机器人、聊天里不出现；
/// 「以你的名义发出」= 真发一条消息到会话里，别人看得到
fn action(a: &ButtonAction) -> String {
    match a {
        ButtonAction::Url { url } => format!("打开网址 {url}"),
        ButtonAction::LoginUrl { .. } => "以你的身份登录网站".to_owned(),
        ButtonAction::WebApp => "打开网页应用".to_owned(),
        ButtonAction::Callback => "通知机器人，聊天里不出现".to_owned(),
        ButtonAction::CallbackWithPassword => "通知机器人，要输入你账号的两步验证密码".to_owned(),
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
        ButtonAction::RequestManagedBot => "新建一个由这个机器人代管的机器人".to_owned(),
        ButtonAction::Unknown { .. } => "不认识的按钮".to_owned(),
    }
}
