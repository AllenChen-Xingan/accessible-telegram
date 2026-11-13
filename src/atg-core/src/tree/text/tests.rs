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
