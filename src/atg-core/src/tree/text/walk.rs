//! 遍历建好的行内树：列出链接、判断剧透、还原原文。建树规则见模块根 `text.rs` 的头注释。

use crate::tree::Span;

// ───────────────────────── 遍历 ─────────────────────────

/// 正文里的一个链接：给「列出本条链接」用
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub struct LinkRef {
    /// 链接文字
    pub text: String,
    /// 地址。文本里直接写的网址，地址就是文字本身
    pub url: String,
}

/// 按出现顺序列出所有链接（`url` 与 `link`，含引用块、格式里套着的）
pub fn links(spans: &[Span]) -> Vec<LinkRef> {
    let mut out = Vec::new();
    collect_links(spans, &mut out);
    out
}

/// 同 [`links`]，另标每个链接是否隐藏（读法 R1）。规则与 [`crate::tree::mask`] 遮 `data`、`speech` 读时同一条：链接落在剧透里，
/// 或链接文字里套着剧透（[`link_hidden`]），都算隐藏
pub fn links_marked(spans: &[Span]) -> Vec<(LinkRef, bool)> {
    let mut out = Vec::new();
    walk_marked(spans, false, &mut out);
    out
}

fn walk_marked(spans: &[Span], inside: bool, out: &mut Vec<(LinkRef, bool)>) {
    for sp in spans {
        match sp {
            Span::Url { .. } | Span::Link { .. } => {
                let hidden = inside || link_hidden(sp);
                let mut one = Vec::new();
                collect_links(std::slice::from_ref(sp), &mut one);
                out.extend(one.into_iter().map(|l| (l, hidden)));
            }
            Span::Text { .. } | Span::Code { .. } | Span::Pre { .. } | Span::DateTime { .. } => {}
            Span::Spoiler { children } => walk_marked(children, true, out),
            Span::Bold { children }
            | Span::Italic { children }
            | Span::Underline { children }
            | Span::Strikethrough { children }
            | Span::Quote { children, .. }
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
            | Span::Unknown { children, .. } => walk_marked(children, inside, out),
        }
    }
}

/// 链接（`link` / `url`）的文字里套着剧透：地址也算隐藏的一部分，整条遮、整条读「隐藏内容」。
/// `data`（[`crate::tree::mask`]）、`links`（[`links_marked`]）、读法（`speech`）三处只认这一条（`specs/content-tree.md` 第八节；
/// 拍板 008 H2'）
pub fn link_hidden(sp: &Span) -> bool {
    matches!(sp, Span::Link { .. } | Span::Url { .. }) && contains_spoiler(sp)
}

/// 这段里（含它自己）有没有剧透
pub fn contains_spoiler(sp: &Span) -> bool {
    match sp {
        Span::Spoiler { .. } => true,
        Span::Text { .. } | Span::Code { .. } | Span::Pre { .. } | Span::DateTime { .. } => false,
        Span::Bold { children }
        | Span::Italic { children }
        | Span::Underline { children }
        | Span::Strikethrough { children }
        | Span::Quote { children, .. }
        | Span::Url { children }
        | Span::Link { children, .. }
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
        | Span::Unknown { children, .. } => children.iter().any(contains_spoiler),
    }
}

fn collect_links(spans: &[Span], out: &mut Vec<LinkRef>) {
    for sp in spans {
        match sp {
            Span::Url { children } => {
                let t = plain_text(children);
                out.push(LinkRef {
                    text: t.clone(),
                    url: t,
                });
            }
            Span::Link { url, children } => out.push(LinkRef {
                text: plain_text(children),
                url: url.clone(),
            }),
            Span::Text { .. } | Span::Code { .. } | Span::Pre { .. } | Span::DateTime { .. } => {}
            Span::Bold { children }
            | Span::Italic { children }
            | Span::Underline { children }
            | Span::Strikethrough { children }
            | Span::Spoiler { children }
            | Span::Quote { children, .. }
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
            | Span::Unknown { children, .. } => collect_links(children, out),
        }
    }
}

/// 还原原文（含剧透里的字）。`build` 的逆：用来测「一个字都不丢」，也给要原文的渲染器用。
pub fn plain_text(spans: &[Span]) -> String {
    let mut s = String::new();
    for sp in spans {
        append_plain(&mut s, sp);
    }
    s
}

fn append_plain(out: &mut String, sp: &Span) {
    match sp {
        Span::Text { text }
        | Span::Code { text }
        | Span::Pre { text, .. }
        | Span::DateTime { text, .. } => out.push_str(text),
        Span::Bold { children }
        | Span::Italic { children }
        | Span::Underline { children }
        | Span::Strikethrough { children }
        | Span::Spoiler { children }
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
                append_plain(out, c);
            }
        }
    }
}
