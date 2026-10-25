//! 「纯文本 + 实体（UTF-16 偏移）」→ 行内树 [`Span`]。
//!
//! 规则见 `specs/content-tree.md`「建树规则」。要点：
//! 1. 偏移和长度是 UTF-16 码元（`td_api.tl:110-111`），先换成字节下标；落在代理对中间的往外取整
//! 2. 外层先排：起点小的在前；起点相同时长的在外；还一样就按种类排（引用块 > 其余实体 > 格式 > 叶子）
//! 3. 嵌套规则照 `td_api.tl:118-121`；违反的不报错，**拆掉那一层实体、文字照留**，记一笔 [`Repair`]
//! 4. 交叉的实体在外层的边界处切成两段
//! 5. **文字一个字都不丢**：`plain_text(&build(t, e).spans) == t` 对任何输入成立
//!
//! [`Repair`] 只记种类名和原因，不记文字（日志不出现聊天内容）。

use super::{Span, Time};
use std::cmp::Reverse;

/// 一个实体，形状与 TDLib `textEntity` 相同，但种类是本项目自己的枚举。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    /// UTF-16 码元
    pub offset: i64,
    /// UTF-16 码元
    pub length: i64,
    pub kind: EntityKind,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntityKind {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Spoiler,
    Code,
    Pre { language: Option<String> },
    DateTime { unix_time: i64 },
    Quote { expandable: bool },
    Link { url: String },
    Url,
    Mention,
    MentionName { user_id: i64 },
    Hashtag,
    Cashtag,
    BotCommand,
    Email,
    Phone,
    BankCard,
    TonAddress,
    CustomEmoji { custom_emoji_id: String },
    MediaTimestamp { seconds: u32 },
    Unknown { td_type: String },
}

/// 建树时做过的修补。只带种类名，不带文字。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Repair {
    /// 长度 ≤ 0 或偏移为负
    DroppedInvalid { kind: &'static str },
    /// 起点在文本之外
    DroppedOutOfBounds { kind: &'static str },
    /// 换成字节后长度为 0
    DroppedEmpty { kind: &'static str },
    /// 与另一个实体完全相同（同范围同种类）
    DroppedDuplicate { kind: &'static str },
    /// 终点超出文本，截到末尾
    Clamped { kind: &'static str },
    /// 边界落在代理对中间，往外取整
    Snapped { kind: &'static str },
    /// 与外层交叉，在外层边界处切开
    Split { kind: &'static str },
    /// 违反嵌套规则，拆掉这一层（文字留着）
    Unwrapped {
        kind: &'static str,
        inside: &'static str,
    },
    /// 嵌套超过 [`MAX_DEPTH`] 层，更深的拆平（文字留着）
    TooDeep { kind: &'static str },
}

/// 最多嵌几层。正常消息不过三五层；防恶意输入把栈撑爆
pub const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub spans: Vec<Span>,
    pub repairs: Vec<Repair>,
}

/// 嵌套规则里的四类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// 粗斜下删剧透：可以包任何东西，也可以被任何东西包
    Style,
    /// Code / Pre / DateTime：里面不许有实体
    Leaf,
    /// 引用块：不许套引用块
    Quote,
    /// 其余实体：里面只许有格式
    Other,
}

impl EntityKind {
    fn class(&self) -> Class {
        use EntityKind as K;
        match self {
            K::Bold | K::Italic | K::Underline | K::Strikethrough | K::Spoiler => Class::Style,
            K::Code | K::Pre { .. } | K::DateTime { .. } => Class::Leaf,
            K::Quote { .. } => Class::Quote,
            K::Link { .. }
            | K::Url
            | K::Mention
            | K::MentionName { .. }
            | K::Hashtag
            | K::Cashtag
            | K::BotCommand
            | K::Email
            | K::Phone
            | K::BankCard
            | K::TonAddress
            | K::CustomEmoji { .. }
            | K::MediaTimestamp { .. }
            | K::Unknown { .. } => Class::Other,
        }
    }

    /// 同范围时谁在外：数小的在外
    /// 类小的在外：引用块 > 其余实体 > 格式 > 叶子。同类再按种类本身比（见 [`sort`]）
    fn rank(&self) -> u8 {
        match self.class() {
            Class::Quote => 0,
            Class::Other => 1,
            Class::Style => 2,
            Class::Leaf => 3,
        }
    }

    /// 给 [`Repair`] 用的种类名（与 JSON 里的 `type` 一致）
    pub fn name(&self) -> &'static str {
        use EntityKind as K;
        match self {
            K::Bold => "bold",
            K::Italic => "italic",
            K::Underline => "underline",
            K::Strikethrough => "strikethrough",
            K::Spoiler => "spoiler",
            K::Code => "code",
            K::Pre { .. } => "pre",
            K::DateTime { .. } => "date_time",
            K::Quote { .. } => "quote",
            K::Link { .. } => "link",
            K::Url => "url",
            K::Mention => "mention",
            K::MentionName { .. } => "mention_name",
            K::Hashtag => "hashtag",
            K::Cashtag => "cashtag",
            K::BotCommand => "bot_command",
            K::Email => "email",
            K::Phone => "phone",
            K::BankCard => "bank_card",
            K::TonAddress => "ton_address",
            K::CustomEmoji { .. } => "custom_emoji",
            K::MediaTimestamp { .. } => "media_timestamp",
            K::Unknown { .. } => "unknown",
        }
    }
}

fn class_name(c: Class) -> &'static str {
    match c {
        Class::Style => "style",
        Class::Leaf => "leaf",
        Class::Quote => "quote",
        Class::Other => "entity",
    }
}

/// `ctx` = 最近一层「非格式」祖先的类（格式层透明）
fn allowed(ctx: Option<Class>, child: Class) -> bool {
    match ctx {
        None | Some(Class::Style) => true,
        Some(Class::Leaf) => false,
        Some(Class::Quote) => child != Class::Quote,
        Some(Class::Other) => child == Class::Style,
    }
}

/// UTF-16 码元位置 ↔ 字节下标。只在字符边界上有记录。
struct Offsets {
    /// 第 i 个字符边界的 UTF-16 位置
    u16: Vec<usize>,
    /// 第 i 个字符边界的字节下标
    byte: Vec<usize>,
}

impl Offsets {
    fn new(text: &str) -> Self {
        let mut u16 = Vec::with_capacity(text.len() + 1);
        let mut byte = Vec::with_capacity(text.len() + 1);
        let mut pos = 0;
        for (b, ch) in text.char_indices() {
            u16.push(pos);
            byte.push(b);
            pos += ch.len_utf16();
        }
        u16.push(pos);
        byte.push(text.len());
        Self { u16, byte }
    }

    fn total(&self) -> usize {
        *self.u16.last().unwrap_or(&0)
    }

    /// 起点往前取整。返回 (字节下标, 是否取过整)
    fn floor(&self, u: usize) -> (usize, bool) {
        match self.u16.binary_search(&u) {
            Ok(i) => (self.byte[i], false),
            Err(i) => (self.byte[i - 1], true),
        }
    }

    /// 终点往后取整
    fn ceil(&self, u: usize) -> (usize, bool) {
        match self.u16.binary_search(&u) {
            Ok(i) => (self.byte[i], false),
            Err(i) => (self.byte[i], true),
        }
    }
}

/// 换成字节范围后的实体
#[derive(Debug, Clone)]
struct Ent {
    s: usize,
    e: usize,
    /// 在输入里的序号：种类在 `entities[idx]`，也用来让排序稳定
    idx: usize,
}

/// 建树。永不 panic；不合规的输入按模块文档里的规则修补。
pub fn build(text: &str, entities: &[Entity]) -> Built {
    let mut repairs = Vec::new();
    let map = Offsets::new(text);
    let total = map.total();
    let mut ents = Vec::with_capacity(entities.len());

    for (idx, ent) in entities.iter().enumerate() {
        let kind = ent.kind.name();
        if ent.offset < 0 || ent.length <= 0 {
            repairs.push(Repair::DroppedInvalid { kind });
            continue;
        }
        let start = ent.offset as usize;
        if start >= total {
            repairs.push(Repair::DroppedOutOfBounds { kind });
            continue;
        }
        let mut end = ent.offset.saturating_add(ent.length) as usize;
        if end > total {
            repairs.push(Repair::Clamped { kind });
            end = total;
        }
        let (s, snap_s) = map.floor(start);
        let (e, snap_e) = map.ceil(end);
        if snap_s || snap_e {
            repairs.push(Repair::Snapped { kind });
        }
        if s >= e {
            repairs.push(Repair::DroppedEmpty { kind });
            continue;
        }
        ents.push(Ent { s, e, idx });
    }

    sort(&mut ents, entities);
    // 去掉完全相同的（排序后相邻）
    let mut deduped: Vec<Ent> = Vec::with_capacity(ents.len());
    for x in ents {
        let dup = deduped.last().is_some_and(|p| {
            p.s == x.s && p.e == x.e && entities[p.idx].kind == entities[x.idx].kind
        });
        if dup {
            repairs.push(Repair::DroppedDuplicate {
                kind: entities[x.idx].kind.name(),
            });
        } else {
            deduped.push(x);
        }
    }

    let spans = build_range(
        text,
        (0, text.len()),
        deduped,
        None,
        0,
        entities,
        &mut repairs,
    );
    Built { spans, repairs }
}

/// 排序键：起点 → 终点（长的在前）→ 类 → 种类本身（含参数；格式按 粗 > 斜 > 下划线 > 删除线 > 剧透）→ 输入顺序。
/// 前四项都相同的实体内容完全一样，所以结果与输入顺序无关，相同的也必然相邻
fn sort(ents: &mut [Ent], entities: &[Entity]) {
    ents.sort_by(|a, b| {
        let (ka, kb) = (&entities[a.idx].kind, &entities[b.idx].kind);
        (a.s, Reverse(a.e), ka.rank())
            .cmp(&(b.s, Reverse(b.e), kb.rank()))
            .then_with(|| ka.cmp(kb))
            .then(a.idx.cmp(&b.idx))
    });
}

/// `(start, end)` 是这一层覆盖的字节范围
fn build_range(
    text: &str,
    (start, end): (usize, usize),
    ents: Vec<Ent>,
    ctx: Option<Class>,
    depth: usize,
    entities: &[Entity],
    repairs: &mut Vec<Repair>,
) -> Vec<Span> {
    if depth >= MAX_DEPTH {
        for x in &ents {
            repairs.push(Repair::TooDeep {
                kind: entities[x.idx].kind.name(),
            });
        }
        let mut out = Vec::new();
        push_text(&mut out, &text[start..end]);
        return out;
    }
    let inside = ctx.map_or("root", class_name);
    let mut queue: Vec<Ent> = ents
        .into_iter()
        .filter(|x| {
            let kind = &entities[x.idx].kind;
            let ok = allowed(ctx, kind.class());
            if !ok {
                repairs.push(Repair::Unwrapped {
                    kind: kind.name(),
                    inside,
                });
            }
            ok
        })
        .collect();
    sort(&mut queue, entities);

    let mut out = Vec::new();
    let mut cur = start;
    while !queue.is_empty() {
        let mut first = queue.remove(0);
        // 外层是格式、和它交叉的是链接一类的实体：切格式，不切链接（切开的网址就坏了）
        if entities[first.idx].kind.class() == Class::Style {
            let cut = queue
                .iter()
                .filter(|x| {
                    x.s > first.s
                        && x.s < first.e
                        && x.e > first.e
                        && entities[x.idx].kind.class() != Class::Style
                })
                .map(|x| x.s)
                .min();
            if let Some(cut) = cut {
                repairs.push(Repair::Split {
                    kind: entities[first.idx].kind.name(),
                });
                queue.push(Ent {
                    s: cut,
                    ..first.clone()
                });
                sort(&mut queue, entities);
                first.e = cut;
            }
        }
        let mut children = Vec::new();
        let mut rest = Vec::new();
        for x in queue.drain(..) {
            if x.s >= first.e {
                rest.push(x);
            } else if x.e <= first.e {
                children.push(x);
            } else {
                repairs.push(Repair::Split {
                    kind: entities[x.idx].kind.name(),
                });
                children.push(Ent {
                    e: first.e,
                    ..x.clone()
                });
                rest.push(Ent { s: first.e, ..x });
            }
        }
        sort(&mut rest, entities);
        queue = rest;

        push_text(&mut out, &text[cur..first.s]);
        let kind = &entities[first.idx].kind;
        let class = kind.class();
        let kids = if class == Class::Leaf {
            for c in &children {
                repairs.push(Repair::Unwrapped {
                    kind: entities[c.idx].kind.name(),
                    inside: class_name(Class::Leaf),
                });
            }
            Vec::new()
        } else {
            let child_ctx = if class == Class::Style {
                ctx
            } else {
                Some(class)
            };
            build_range(
                text,
                (first.s, first.e),
                children,
                child_ctx,
                depth + 1,
                entities,
                repairs,
            )
        };
        out.push(make_span(kind, kids, &text[first.s..first.e]));
        cur = first.e;
    }
    push_text(&mut out, &text[cur..end]);
    out
}

fn push_text(out: &mut Vec<Span>, s: &str) {
    if s.is_empty() {
        return;
    }
    if let Some(Span::Text { text }) = out.last_mut() {
        text.push_str(s);
    } else {
        out.push(Span::Text { text: s.to_owned() });
    }
}

fn make_span(kind: &EntityKind, children: Vec<Span>, raw: &str) -> Span {
    use EntityKind as K;
    match kind {
        K::Bold => Span::Bold { children },
        K::Italic => Span::Italic { children },
        K::Underline => Span::Underline { children },
        K::Strikethrough => Span::Strikethrough { children },
        K::Spoiler => Span::Spoiler { children },
        K::Code => Span::Code {
            text: raw.to_owned(),
        },
        K::Pre { language } => Span::Pre {
            language: language.clone().filter(|l| !l.is_empty()),
            text: raw.to_owned(),
        },
        K::DateTime { unix_time } => Span::DateTime {
            unix_time: Time(*unix_time),
            text: raw.to_owned(),
        },
        K::Quote { expandable } => Span::Quote {
            expandable: *expandable,
            children,
        },
        K::Link { url } => Span::Link {
            url: url.clone(),
            children,
        },
        K::Url => Span::Url { children },
        K::Mention => Span::Mention { children },
        K::MentionName { user_id } => Span::MentionName {
            user_id: *user_id,
            children,
        },
        K::Hashtag => Span::Hashtag { children },
        K::Cashtag => Span::Cashtag { children },
        K::BotCommand => Span::BotCommand { children },
        K::Email => Span::Email { children },
        K::Phone => Span::Phone { children },
        K::BankCard => Span::BankCard { children },
        K::TonAddress => Span::TonAddress { children },
        K::CustomEmoji { custom_emoji_id } => Span::CustomEmoji {
            custom_emoji_id: custom_emoji_id.clone(),
            children,
        },
        K::MediaTimestamp { seconds } => Span::MediaTimestamp {
            seconds: *seconds,
            children,
        },
        K::Unknown { td_type } => Span::Unknown {
            td_type: td_type.clone(),
            children,
        },
    }
}

/// 正文里的一个链接：给「列出本条链接」用
#[derive(Debug, Clone, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use EntityKind as K;

    fn e(offset: i64, length: i64, kind: EntityKind) -> Entity {
        Entity {
            offset,
            length,
            kind,
        }
    }
    fn t(s: &str) -> Span {
        Span::Text { text: s.into() }
    }

    #[test]
    fn no_entities() {
        let b = build("你好", &[]);
        assert_eq!(b.spans, vec![t("你好")]);
        assert!(b.repairs.is_empty());
    }

    #[test]
    fn empty_text() {
        let b = build("", &[e(0, 1, K::Bold)]);
        assert!(b.spans.is_empty());
        assert_eq!(b.repairs, vec![Repair::DroppedOutOfBounds { kind: "bold" }]);
    }

    #[test]
    fn utf16_offsets_after_emoji() {
        // 👍 占 2 个 UTF-16 码元、4 个字节；「好」从 UTF-16 第 2 位开始
        let b = build("👍好的", &[e(2, 1, K::Bold)]);
        assert_eq!(
            b.spans,
            vec![
                t("👍"),
                Span::Bold {
                    children: vec![t("好")]
                },
                t("的")
            ]
        );
        assert!(b.repairs.is_empty());
    }

    #[test]
    fn offset_inside_surrogate_pair_snaps_outward() {
        // 偏移 1 落在 👍 的两半之间：起点往前到 0
        let b = build("👍好", &[e(1, 2, K::Italic)]);
        assert_eq!(
            b.spans,
            vec![Span::Italic {
                children: vec![t("👍好")]
            }]
        );
        assert_eq!(b.repairs, vec![Repair::Snapped { kind: "italic" }]);
    }

    #[test]
    fn nested_bold_italic() {
        // 「加粗里有斜体」：bold 0..6，italic 2..4
        let b = build("ab斜体cd", &[e(0, 6, K::Bold), e(2, 2, K::Italic)]);
        assert_eq!(
            b.spans,
            vec![Span::Bold {
                children: vec![
                    t("ab"),
                    Span::Italic {
                        children: vec![t("斜体")]
                    },
                    t("cd")
                ]
            }]
        );
    }

    #[test]
    fn same_range_order_is_by_kind_not_input() {
        // 同范围的 italic 与 bold：不论输入顺序，bold 都在外
        let a = build("xy", &[e(0, 2, K::Italic), e(0, 2, K::Bold)]);
        let b = build("xy", &[e(0, 2, K::Bold), e(0, 2, K::Italic)]);
        assert_eq!(a.spans, b.spans);
        assert!(matches!(a.spans[0], Span::Bold { .. }));
    }

    #[test]
    fn crossing_entities_split_at_outer_boundary() {
        // bold 0..4，italic 2..6 交叉：italic 切成 2..4（在 bold 里）和 4..6
        let b = build("abcdef", &[e(0, 4, K::Bold), e(2, 4, K::Italic)]);
        assert_eq!(
            b.spans,
            vec![
                Span::Bold {
                    children: vec![
                        t("ab"),
                        Span::Italic {
                            children: vec![t("cd")]
                        }
                    ]
                },
                Span::Italic {
                    children: vec![t("ef")]
                }
            ]
        );
        assert_eq!(b.repairs, vec![Repair::Split { kind: "italic" }]);
    }

    #[test]
    fn entity_inside_code_is_unwrapped() {
        let b = build("let x", &[e(0, 5, K::Code), e(0, 3, K::Bold)]);
        assert_eq!(
            b.spans,
            vec![Span::Code {
                text: "let x".into()
            }]
        );
        assert_eq!(
            b.repairs,
            vec![Repair::Unwrapped {
                kind: "bold",
                inside: "leaf"
            }]
        );
    }

    #[test]
    fn quote_in_quote_is_unwrapped_but_text_kept() {
        let b = build(
            "外层内层",
            &[
                e(0, 4, K::Quote { expandable: false }),
                e(2, 2, K::Quote { expandable: true }),
            ],
        );
        assert_eq!(
            b.spans,
            vec![Span::Quote {
                expandable: false,
                children: vec![t("外层内层")]
            }]
        );
        assert_eq!(b.repairs.len(), 1);
    }

    #[test]
    fn mention_inside_link_is_unwrapped_but_bold_kept() {
        // 链接里只许有格式：mention 拆掉，mention 里的 bold 留着
        let b = build(
            "see @bob",
            &[
                e(
                    0,
                    8,
                    K::Link {
                        url: "https://example.invalid".into(),
                    },
                ),
                e(4, 4, K::Mention),
                e(5, 3, K::Bold),
            ],
        );
        assert_eq!(
            b.spans,
            vec![Span::Link {
                url: "https://example.invalid".into(),
                children: vec![
                    t("see @"),
                    Span::Bold {
                        children: vec![t("bob")]
                    }
                ]
            }]
        );
    }

    #[test]
    fn style_is_transparent_for_nesting_rules() {
        // link > bold > hashtag：hashtag 的最近非格式祖先是 link，拆掉
        let b = build(
            "#tag",
            &[
                e(0, 4, K::Link { url: "u".into() }),
                e(0, 4, K::Bold),
                e(0, 4, K::Hashtag),
            ],
        );
        assert_eq!(plain_text(&b.spans), "#tag");
        assert!(b.repairs.iter().any(|r| matches!(
            r,
            Repair::Unwrapped {
                kind: "hashtag",
                ..
            }
        )));
    }

    #[test]
    fn invalid_and_out_of_range_entities() {
        let b = build(
            "abc",
            &[
                e(-1, 2, K::Bold),
                e(0, 0, K::Bold),
                e(5, 1, K::Bold),
                e(1, 99, K::Italic),
            ],
        );
        assert_eq!(
            b.spans,
            vec![
                t("a"),
                Span::Italic {
                    children: vec![t("bc")]
                }
            ]
        );
        assert_eq!(
            b.repairs,
            vec![
                Repair::DroppedInvalid { kind: "bold" },
                Repair::DroppedInvalid { kind: "bold" },
                Repair::DroppedOutOfBounds { kind: "bold" },
                Repair::Clamped { kind: "italic" },
            ]
        );
    }

    #[test]
    fn links_lists_bare_and_hidden_urls_in_order() {
        let b = build(
            "看 路线 和 https://a.invalid",
            &[
                e(0, 4, K::Quote { expandable: false }),
                e(
                    2,
                    2,
                    K::Link {
                        url: "https://b.invalid".into(),
                    },
                ),
                e(7, 17, K::Url),
            ],
        );
        assert_eq!(
            links(&b.spans),
            vec![
                LinkRef {
                    text: "路线".into(),
                    url: "https://b.invalid".into()
                },
                LinkRef {
                    text: "https://a.invalid".into(),
                    url: "https://a.invalid".into()
                },
            ]
        );
    }

    #[test]
    fn style_crossing_a_url_is_split_not_the_url() {
        // 粗体 0..7 与网址 4..15 交叉：切粗体，网址保持完整
        let b = build("see example.com", &[e(0, 7, K::Bold), e(4, 11, K::Url)]);
        assert_eq!(
            b.spans,
            vec![
                Span::Bold {
                    children: vec![t("see ")]
                },
                Span::Url {
                    children: vec![
                        Span::Bold {
                            children: vec![t("exa")]
                        },
                        t("mple.com")
                    ]
                }
            ]
        );
        assert_eq!(links(&b.spans)[0].url, "example.com");
    }

    #[test]
    fn same_range_non_style_order_is_deterministic() {
        let a = build("xy", &[e(0, 2, K::Url), e(0, 2, K::Mention)]);
        let b = build("xy", &[e(0, 2, K::Mention), e(0, 2, K::Url)]);
        assert_eq!(a.spans, b.spans);
        let q1 = K::Quote { expandable: true };
        let q2 = K::Quote { expandable: false };
        let a = build("xy", &[e(0, 2, q1.clone()), e(0, 2, q2.clone())]);
        let b = build("xy", &[e(0, 2, q2), e(0, 2, q1)]);
        assert_eq!(a.spans, b.spans);
    }

    #[test]
    fn deep_nesting_is_capped_without_losing_text() {
        let text = "x".repeat(200);
        let ents: Vec<Entity> = (0..200).map(|i| e(i, 200 - i, K::Bold)).collect();
        let b = build(&text, &ents);
        assert_eq!(plain_text(&b.spans), text);
        assert!(
            b.repairs
                .iter()
                .any(|r| matches!(r, Repair::TooDeep { .. }))
        );
    }

    #[test]
    fn non_adjacent_duplicates_dropped() {
        let b = build(
            "ab",
            &[
                e(0, 2, K::Link { url: "a".into() }),
                e(0, 2, K::Link { url: "b".into() }),
                e(0, 2, K::Link { url: "a".into() }),
            ],
        );
        assert!(
            b.repairs
                .contains(&Repair::DroppedDuplicate { kind: "link" })
        );
    }

    #[test]
    fn huge_length_does_not_overflow() {
        let b = build("ab", &[e(1, i64::MAX, K::Bold)]);
        assert_eq!(plain_text(&b.spans), "ab");
    }

    #[test]
    fn duplicates_dropped() {
        let b = build("ab", &[e(0, 2, K::Bold), e(0, 2, K::Bold)]);
        assert_eq!(
            b.spans,
            vec![Span::Bold {
                children: vec![t("ab")]
            }]
        );
        assert_eq!(b.repairs, vec![Repair::DroppedDuplicate { kind: "bold" }]);
    }

    #[test]
    fn zwj_family_emoji_length() {
        // 👨‍👩‍👧 = 3 个人（各 2 码元）+ 2 个 ZWJ（各 1）= 8 个 UTF-16 码元
        let text = "👨\u{200d}👩\u{200d}👧来了";
        let b = build(text, &[e(8, 2, K::Bold)]);
        assert_eq!(
            b.spans,
            vec![
                t("👨\u{200d}👩\u{200d}👧"),
                Span::Bold {
                    children: vec![t("来了")]
                }
            ]
        );
    }

    /// 「一个字都不丢」：随便造一批乱七八糟的实体，原文必须原样还原，且不 panic
    #[test]
    fn never_loses_text_on_messy_input() {
        let text = "a👍b😀c中文\u{200d}d🏳️‍🌈e";
        let kinds = [
            K::Bold,
            K::Italic,
            K::Code,
            K::Quote { expandable: false },
            K::Mention,
            K::Link { url: "u".into() },
            K::Pre { language: None },
            K::Spoiler,
        ];
        let total = text.encode_utf16().count() as i64;
        // 小型伪随机（固定种子，结果可复现）
        let mut seed: u64 = 0x5eed;
        let mut next = |m: i64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 33) as i64).rem_euclid(m.max(1))
        };
        for _ in 0..500 {
            let n = next(6) as usize;
            let ents: Vec<Entity> = (0..n)
                .map(|_| {
                    e(
                        next(total + 3) - 1,
                        next(total + 2) - 1,
                        kinds[next(kinds.len() as i64) as usize].clone(),
                    )
                })
                .collect();
            let b = build(text, &ents);
            assert_eq!(plain_text(&b.spans), text, "实体：{ents:?}");
        }
    }
}
