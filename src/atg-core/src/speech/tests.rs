use super::*;

#[test]
fn durations() {
    assert_eq!(dur(0), "0 秒");
    assert_eq!(dur(12), "12 秒");
    assert_eq!(dur(60), "1 分");
    assert_eq!(dur(65), "1 分 5 秒");
    assert_eq!(dur(3600), "1 小时");
    assert_eq!(dur(3725), "1 小时 2 分");
}

/// V12 人定：连续 3 个及以上相同的才合并；不连续、不一样、只有两个的照原样
#[test]
fn emoji_runs() {
    assert_eq!(merge_emoji("😂😂😂😂😂"), "5 个😂");
    // 紧挨文字时加逗号；已有停顿不再加
    assert_eq!(merge_emoji("好的👍👍👍"), "好的，3 个👍");
    assert_eq!(merge_emoji("😂😂😂好的"), "3 个😂，好的");
    assert_eq!(merge_emoji("好的 👍👍👍！"), "好的 3 个👍！");
    assert_eq!(merge_emoji("👍👍👍😂😂😂"), "3 个👍，3 个😂");
    assert_eq!(merge_emoji("😀😀好消息"), "😀😀好消息");
    assert_eq!(merge_emoji("好😂😂哈哈🤣😂"), "好😂😂哈哈🤣😂");
    // 肤色、变体选择符、零宽连接、国旗算一个表情
    assert_eq!(merge_emoji("👍🏽👍🏽👍🏽"), "3 个👍🏽");
    assert_eq!(merge_emoji("👍👍🏽👍"), "👍👍🏽👍");
    assert_eq!(merge_emoji("❤️❤️❤️"), "3 个❤️");
    assert_eq!(merge_emoji("👨‍👩‍👧👨‍👩‍👧👨‍👩‍👧"), "3 个👨‍👩‍👧");
    assert_eq!(merge_emoji("🇨🇳🇨🇳🇨🇳"), "3 个🇨🇳");
    // 普通文字三连不动
    assert_eq!(merge_emoji("哈哈哈"), "哈哈哈");
    assert_eq!(merge_emoji(""), "");
}

/// 跟系统时区读时，每个钟点按它自己的时间点取偏移（`offset_at`）：夏令时前后两条当地都是 19:36，
/// 都读「19:36」；固定偏移（`offset_at` 为空）时一律按 `utc_offset` 算
#[test]
fn offset_at_is_per_timestamp() {
    // 假的「纽约」：2026-11-01T06:00Z 之前 -4 小时（夏令时），之后 -5 小时
    fn fake_ny(t: i64) -> i32 {
        if t < 1_793_512_800 {
            -4 * 3600
        } else {
            -5 * 3600
        }
    }
    let summer = Time(1_793_403_360); // 2026-10-30T23:36Z = 当地 19:36（-4）
    let winter = Time(1_793_579_760); // 2026-11-02T00:36Z = 当地 19:36（-5）
    let system = SpeechCtx {
        utc_offset: 0,
        offset_at: Some(fake_ny),
        listen_key: None,
        expand_key: None,
        reveal: false,
    };
    assert_eq!(hm(summer, &system), "19:36");
    assert_eq!(hm(winter, &system), "19:36");
    // 固定 -4 小时：冬天那条读成 20:36，`utc_offset` 不被 `offset_at` 以外的东西改
    let fixed = SpeechCtx {
        utc_offset: -4 * 3600,
        offset_at: None,
        ..system
    };
    assert_eq!(hm(summer, &fixed), "19:36");
    assert_eq!(hm(winter, &fixed), "20:36");
}

/// V22 人定：问号、叹号后面直接接，不再加逗号
#[test]
fn then_after_question_mark() {
    assert_eq!(then("几点出发？", "19:32"), "几点出发？19:32");
    assert_eq!(then("好！", "19:32"), "好！19:32");
    assert_eq!(then("why?", "19:32"), "why?19:32");
    assert_eq!(then("好的", "19:32"), "好的，19:32");
}

/// 「隐藏内容」后紧跟问号不加逗号，只认遮住处；原文里本来写着的「隐藏内容，？」不动（codex 020 第 1 轮 #2）
#[test]
fn question_mark_after_hidden_only_for_masks() {
    let t = |s: &str| Span::Text { text: s.into() };
    let masked = [
        t("几点"),
        Span::Spoiler {
            children: vec![t("丙")],
        },
        t("？"),
    ];
    assert_eq!(spans(&masked, false).0, "几点，隐藏内容？");
    assert_eq!(spans(&masked, true).0, "几点丙？");
    let literal = [t("原文写着隐藏内容，？")];
    assert_eq!(
        spans(&literal, false),
        ("原文写着隐藏内容，？".to_owned(), false)
    );
}
