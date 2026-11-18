//! 正文与附件的读法（`docs/plan/010_complexity-budget.md` 定的 `speech/content.rs`）：时长的说法（视频、语音、音频、播放状态
//! 共用）、内容的全文（逐段渲染行内树）、语音的转写（#59），以及正文与附件的简短说法（行内树、表情合并）。
//! **转写的句子是 AI 草稿，待人实听**。
//!
//! 转写（`docs/experiments/exp008-local-stt/design.md` 第二节第 4 条、第三节第 2 条）：
//! - 名称（`brief`）**不变**；「已转文字」是状态，进详情（`specs/content-tree.md` 第九节：名称只放谁发的、说了什么、几点）
//! - 全文：语音的说法后面接转写全文；没转过接「还没转文字」。按键提示（如「按 T 转」）留给 GUI 那一块定
//! - 补标点只在这里做，树里存原始片段：片段之间，前一段末尾没有标点就补「，」（v0 只做这一条；英文转写也补全角「，」——
//!   读屏只拿它当停顿，v0 不按语种分）
//! - 隐藏内容（18+ 遮挡）没展开时：全文不接转写、详情不说「已转文字」，同 JSON 不输出转写（`tree/mask.rs`）

use super::{HIDDEN, UNSUPPORTED};
use crate::tree::*;

/// 时长：「12 秒」「1 分 5 秒」「1 小时 2 分」
pub(super) fn dur(secs: u32) -> String {
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    match (h, m) {
        (0, 0) => format!("{s} 秒"),
        (0, _) if s == 0 => format!("{m} 分"),
        (0, _) => format!("{m} 分 {s} 秒"),
        _ if m == 0 => format!("{h} 小时"),
        _ => format!("{h} 小时 {m} 分"),
    }
}

const TRANSCRIBED: &str = "已转文字";
const NOT_YET: &str = "还没转文字";

/// 详情里的那一段：有转写、且没被 18+ 遮着才有
pub(super) fn transcript_detail(m: &Message, reveal: bool) -> Option<String> {
    let hidden = m.content.sensitive && !reveal;
    m.transcript
        .as_ref()
        .filter(|_| !hidden)
        .map(|_| TRANSCRIBED.to_owned())
}

/// 消息的全文：内容的全文，是语音再接转写；遮着的（18+ 没展开）不接
pub(super) fn full(m: &Message, reveal: bool) -> String {
    let full = full_content(&m.content, reveal);
    if !matches!(m.content.attachment, Some(Attachment::Voice { .. }))
        || (m.content.sensitive && !reveal)
    {
        return full;
    }
    let tail = m
        .transcript
        .as_ref()
        .map_or_else(|| NOT_YET.to_owned(), punctuate);
    format!("{full}，{tail}")
}

/// 内容的全文：附件照名称的说法放在前面，正文原样（`super::full` 的正文部分）
pub(super) fn full_content(c: &Content, reveal: bool) -> String {
    let body = full_spans(&c.body, reveal);
    // 投票的全文自己排：题目、说明文字、逐项、总人数、解析（#13；说明紧跟题目，同名称）。18+ 遮挡的说法同下
    if let Some(a @ Attachment::Poll { .. }) = &c.attachment {
        let full = super::interactive::poll_full(a, body.trim(), reveal);
        return if c.sensitive && !reveal {
            format!("{full}，{HIDDEN}")
        } else {
            full
        };
    }
    let label = c.attachment.as_ref().map(|a| {
        let (l, spoiler) = attachment(a, reveal);
        if (spoiler || c.sensitive) && !reveal {
            format!("{l}，{HIDDEN}")
        } else {
            l
        }
    });
    match (label, body.trim().is_empty()) {
        (None, true) => "空消息".to_owned(),
        (None, false) => body,
        (Some(l), true) => l,
        (Some(l), false) => join(&l, &body),
    }
}

/// 句末这些标点之后不再接分隔号（免得念成「？，」「？；」，大总管定 2026-11-17）。名称、全文、投票共用这一份
pub(super) const ENDS: [char; 10] = ['？', '?', '！', '!', '。', '.', '…', '：', ':', '；'];

/// 「附件说法，正文」：附件说法自己以标点结尾（投票题目「几点出发？」）就不再接逗号
fn join(label: &str, body: &str) -> String {
    glue(label, "，", body)
}

/// 两段拼起来：前一段以标点（[`ENDS`] 或「，」）结尾就直接接，否则加 `sep`。名称、全文、投票共用这一条
pub(super) fn glue(a: &str, sep: &str, b: &str) -> String {
    if a.ends_with(ENDS) || a.ends_with('，') {
        format!("{a}{b}")
    } else {
        format!("{a}{sep}{b}")
    }
}

/// 片段拼成一句：前一段末尾没有标点就补「，」；全是空白的片段跳过
pub(super) fn punctuate(t: &Transcript) -> String {
    let mut out = String::new();
    for seg in t
        .segments
        .iter()
        .map(|s| s.text.trim())
        .filter(|s| !s.is_empty())
    {
        if !out.is_empty() && !out.ends_with(|c: char| is_punct(c)) {
            out.push('，');
        }
        out.push_str(seg);
    }
    out
}

fn is_punct(c: char) -> bool {
    matches!(
        c,
        '，' | '。' | '？' | '！' | '、' | '；' | '：' | '…' | ',' | '.' | '?' | '!' | ';' | ':'
    )
}

// ───────────────────────── 全文：逐段渲染 ─────────────────────────

/// 遮住处先放占位符，最后看两边：紧挨着别的字才补逗号（原文自己的空格、标点、换行照留）
const MASK: char = '\u{FFFC}';

fn full_spans(list: &[Span], reveal: bool) -> String {
    let mut raw = String::new();
    for s in list {
        full_span(&mut raw, s, reveal);
    }
    let pause = |c: char| c.is_whitespace() || "，、。；：！？,.;:!?「」（）()".contains(c);
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    for (i, &c) in chars.iter().enumerate() {
        if c != MASK {
            out.push(c);
            continue;
        }
        if out.chars().last().is_some_and(|p| !pause(p) && p != MASK) {
            out.push('，');
        }
        out.push_str(HIDDEN);
        if chars.get(i + 1).is_some_and(|&n| !pause(n)) {
            out.push('，');
        }
    }
    out
}

fn full_span(out: &mut String, s: &Span, reveal: bool) {
    // 相邻的几段剧透只说一次；套着剧透的链接整条遮（#70，同 [`span`]）
    // 全文只放占位符、不打标记，逗号最后看两边再补（与 brief 的 [`say_hidden`] 不同）
    let mark = |out: &mut String| {
        if !out.ends_with(MASK) {
            out.push(MASK);
        }
    };
    if !reveal && text::link_hidden(s) {
        return mark(out);
    }
    match s {
        Span::Text { text }
        | Span::Code { text }
        | Span::Pre { text, .. }
        | Span::DateTime { text, .. } => out.push_str(&text.replace(MASK, "")),
        Span::Spoiler { children } if reveal => {
            for c in children {
                full_span(out, c, reveal);
            }
        }
        Span::Spoiler { .. } => mark(out),
        Span::Bold { children }
        | Span::Italic { children }
        | Span::Underline { children }
        | Span::Strikethrough { children }
        | Span::Quote { children, .. }
        | Span::Link { children, .. }
        | Span::Url { children }
        | Span::Mention { children }
        | Span::MentionName { children, .. }
        | Span::Hashtag { children }
        | Span::Cashtag { children }
        | Span::BotCommand { children }
        | Span::Email { children }
        | Span::Phone { children }
        | Span::BankCard { children }
        | Span::TonAddress { children }
        | Span::CustomEmoji { children, .. }
        | Span::MediaTimestamp { children, .. }
        | Span::Unknown { children, .. } => {
            for c in children {
                full_span(out, c, reveal);
            }
        }
    }
}

// ───────────────────────── 正文与附件 ─────────────────────────

/// 正文 + 附件的简短说法，以及里面有没有隐藏内容（剧透、18+ 遮挡）。按键提示由调用方决定加不加
pub(super) fn content(c: &Content, reveal: bool) -> (String, bool) {
    let (body, body_hidden) = spans(&c.body, reveal);
    // 句号、逗号后面还要接「，时间」，去掉免得读成「。，」；问号叹号带语气、英文句点可能是省略号，留着
    let body = body.trim().trim_end_matches(['。', '，', ',']).trim_end();
    // V13（011 B18-5，#68）：投票题目里的隐藏内容也算进整条的按键提示；与正文走同一条 `spans`
    let poll_hidden = match &c.attachment {
        Some(Attachment::Poll {
            question,
            options,
            explanation,
            // 其余字段都不带行内树；写全不用 `..`：以后加了带文字的字段，这里编不过，逼着决定要不要遮（自审第 7 轮）
            total: _,
            results_visible: _,
            anonymous: _,
            multiple: _,
            revotable: _,
            kind: _,
            closed: _,
            blocked: _,
        }) => {
            spans(question, reveal).1
                || spans(explanation, reveal).1
                || super::interactive::options_hidden(options, reveal)
        }
        Some(_) | None => false,
    };
    let mut hidden = body_hidden || poll_hidden;
    let label = c.attachment.as_ref().map(|a| {
        let (l, spoiler) = attachment(a, reveal);
        // 剧透、18+ 遮挡同一种说法，两样都有也只读一次；已展开就不说
        if (spoiler || c.sensitive) && !reveal {
            hidden = true;
            format!("{l}，{HIDDEN}")
        } else {
            l
        }
    });
    let text = match (label, body.is_empty()) {
        (None, true) => "空消息".to_owned(),
        (None, false) => body.to_owned(),
        (Some(l), true) => l,
        (Some(l), false) => join(&l, body),
    };
    (text, hidden)
}

fn attachment(a: &Attachment, reveal: bool) -> (String, bool) {
    let label = match a {
        Attachment::Photo { spoiler } => return ("图片".to_owned(), *spoiler),
        Attachment::Video { duration, spoiler } => {
            return (format!("视频 {}", dur(*duration)), *spoiler);
        }
        Attachment::Voice { duration, .. } => format!("语音 {}", dur(*duration)),
        Attachment::VideoNote { duration, .. } => format!("圆形视频 {}", dur(*duration)),
        Attachment::Audio {
            duration,
            title,
            performer,
        } => {
            let mut parts = vec!["音频".to_owned()];
            parts.extend(
                [title, performer]
                    .into_iter()
                    .filter(|x| !x.is_empty())
                    .cloned(),
            );
            parts.push(dur(*duration));
            parts.join("，")
        }
        Attachment::Animation { .. } => "动图".to_owned(),
        Attachment::Document { file_name, .. } => {
            if file_name.is_empty() {
                "文件".to_owned()
            } else {
                format!("文件，{file_name}")
            }
        }
        Attachment::Sticker { emoji } => labeled("贴纸", emoji),
        Attachment::AnimatedEmoji { emoji } => emoji.clone(),
        Attachment::Location => "位置".to_owned(),
        Attachment::Venue { title } => labeled("地点", title),
        Attachment::Contact { name } => labeled("联系人", name),
        // 名称只读题目（第九节：名称只放谁、说了什么、几点；说明文字在正文里，照常接在后面）；选项、票数、性质在详情与全文里
        Attachment::Poll { question, .. } => labeled("投票", &spans(question, reveal).0),
        Attachment::Call {
            video,
            duration,
            missed,
        } => {
            let kind = if *video {
                "视频通话"
            } else {
                "语音通话"
            };
            if *missed {
                format!("未接{kind}")
            } else {
                format!("{kind} {}", dur(*duration))
            }
        }
        Attachment::Expired { what } => {
            let w = match what {
                Expirable::Photo => "图片",
                Expirable::Video => "视频",
                Expirable::VideoNote => "圆形视频",
                Expirable::Voice => "语音",
            };
            format!("已过期的{w}")
        }
        Attachment::Restricted { .. } => "受限内容，无法查看".to_owned(),
        Attachment::Other { .. } | Attachment::Unknown { .. } => UNSUPPORTED.to_owned(),
    };
    (label, false)
}

/// 「类型，内容」；内容为空只读类型
fn labeled(kind: &str, what: &str) -> String {
    let what = what.trim();
    if what.is_empty() {
        kind.to_owned()
    } else {
        format!("{kind}，{what}")
    }
}

/// 行内树的简短说法，以及有没有剧透：格式不读；剧透只读「隐藏内容」（R1）；链接只读文字不读地址；
/// 连续 3 个及以上相同的表情合并成「5 个😂」（V12 人定）
pub(super) fn spans(list: &[Span], reveal: bool) -> (String, bool) {
    let mut out = String::new();
    let mut hidden = false;
    for s in list {
        span(&mut out, s, &mut hidden, reveal);
    }
    // 「隐藏内容」两边加的逗号可能和原文的空格、逗号、开头撞上
    let mut out = out.replace(" ，", "，");
    while out.contains("，，") {
        out = out.replace("，，", "，");
    }
    // 遮住处后面紧跟问号、叹号就不加逗号（V22；message.md 第三节「隐藏内容？按 D 展开」）。只认占位符，原文不动
    for p in ['？', '！', '?', '!'] {
        out = out.replace(&format!("{MASK}，{p}"), &format!("{MASK}{p}"));
    }
    let out = out.replace(MASK, HIDDEN);
    (merge_emoji(out.trim_start_matches('，')), hidden)
}

fn span(out: &mut String, s: &Span, hidden: &mut bool, reveal: bool) {
    // #70：链接文字里套着剧透，地址也算隐藏，整条只读「隐藏内容」（与 data、links 同一条判定）
    if !reveal && text::link_hidden(s) {
        return say_hidden(out, hidden);
    }
    match s {
        Span::Text { text }
        | Span::Code { text }
        | Span::Pre { text, .. }
        | Span::DateTime { text, .. } => out.push_str(&text.replace(MASK, "")),
        // 已展开：照读里面的字
        Span::Spoiler { children } if reveal => {
            for c in children {
                span(out, c, hidden, reveal);
            }
        }
        Span::Spoiler { .. } => say_hidden(out, hidden),
        Span::Bold { children }
        | Span::Italic { children }
        | Span::Underline { children }
        | Span::Strikethrough { children }
        | Span::Quote { children, .. }
        | Span::Link { children, .. }
        | Span::Url { children }
        | Span::Mention { children }
        | Span::MentionName { children, .. }
        | Span::Hashtag { children }
        | Span::Cashtag { children }
        | Span::BotCommand { children }
        | Span::Email { children }
        | Span::Phone { children }
        | Span::BankCard { children }
        | Span::TonAddress { children }
        | Span::CustomEmoji { children, .. }
        | Span::MediaTimestamp { children, .. }
        | Span::Unknown { children, .. } => {
            for c in children {
                span(out, c, hidden, reveal);
            }
        }
    }
}

/// VoiceOver 不念括号，用逗号隔开才听得出这是标注。先放占位符，[`spans`] 收尾时换成「隐藏内容」
fn say_hidden(out: &mut String, hidden: &mut bool) {
    *hidden = true;
    out.push('，');
    out.push(MASK);
    out.push('，');
}

/// 连续 3 个及以上相同的表情 → 「N 个😂」（V12 人定）。只改名称，复制全文、阅读视图照原样。
/// 合并后紧挨着文字时两边加逗号（「好的，3 个👍」），免得听成「好的三个」——这一点 style 稿没写，
/// AI 先定的默认，**待人定**（另一选项：紧挨文字的表情不合并）；题面已交 `state/coord.md`「待定清单」
pub(super) fn merge_emoji(s: &str) -> String {
    // 已经有停顿的地方不再加逗号
    let pause = |c: char| c.is_whitespace() || "，、。；：！？,.;:!?「」（）()".contains(c);
    let clusters = emoji_clusters(s);
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < clusters.len() {
        let (text, is_emoji) = clusters[i];
        let mut n = 1;
        while is_emoji && i + n < clusters.len() && clusters[i + n] == (text, true) {
            n += 1;
        }
        if is_emoji && n >= 3 {
            if out.chars().last().is_some_and(|c| !pause(c)) {
                out.push('，');
            }
            out.push_str(&format!("{n} 个{text}"));
            if let Some(&(next, _)) = clusters.get(i + n)
                && next.chars().next().is_some_and(|c| !pause(c))
            {
                out.push('，');
            }
        } else {
            for _ in 0..n {
                out.push_str(text);
            }
        }
        i += n;
    }
    out
}

/// 把文字切成「表情」与「别的字」两种片段。表情 = 一个图形符号，后面跟着的变体选择符、肤色、
/// 键帽符、零宽连接（及它连上的下一个符号）、成对的国旗字母都算同一个。不引字素切分库（与
/// `specs/content-tree.md` 第四节同一取舍），覆盖常见表情就够
fn emoji_clusters(s: &str) -> Vec<(&str, bool)> {
    let pictographic = |c: char| {
        matches!(c as u32,
            0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2B00..=0x2BFF | 0x2190..=0x21FF
            | 0x2300..=0x23FF | 0x3030 | 0x303D | 0x3297 | 0x3299 | 0x00A9 | 0x00AE | 0x203C | 0x2049)
    };
    let flag = |c: char| matches!(c as u32, 0x1F1E6..=0x1F1FF);
    let extend =
        |c: char| matches!(c as u32, 0xFE0F | 0x20E3 | 0x1F3FB..=0x1F3FF | 0xE0020..=0xE007F);
    let mut out = Vec::new();
    let mut it = s.char_indices().peekable();
    while let Some((start, c)) = it.next() {
        if !pictographic(c) {
            let end = start + c.len_utf8();
            out.push((&s[start..end], false));
            continue;
        }
        let mut end = start + c.len_utf8();
        // 国旗是两个国旗字母成对
        if flag(c)
            && let Some(&(j, d)) = it.peek()
            && flag(d)
        {
            end = j + d.len_utf8();
            it.next();
        }
        while let Some(&(j, d)) = it.peek() {
            if extend(d) {
                end = j + d.len_utf8();
                it.next();
            } else if d == '\u{200D}' {
                end = j + d.len_utf8();
                it.next();
                if let Some(&(k, e)) = it.peek() {
                    end = k + e.len_utf8();
                    it.next();
                }
            } else {
                break;
            }
        }
        out.push((&s[start..end], true));
    }
    out
}
