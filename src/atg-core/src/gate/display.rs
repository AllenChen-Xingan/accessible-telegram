//! 确认框与计划行里给人看的文字：把看不见、会改写显示的字符换成可见的 `\u{…}`。

// ───────────────────────── 给人看的文字 ─────────────────────────

/// 把会改写终端显示、读屏也不读的字符换成可见的 `\u{…}`：控制字符（换行、制表符除外）与 [`invisible`] 列的
/// 全部看不见的字符（零宽、双向控制、标签字符、变体选择符……；`actions.md` 第一节第 4 小节第 4 条）
pub fn visible(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if hides(c) {
            out.push_str(&format!("\\u{{{:04X}}}", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}

/// [`visible`] 会把几个字符换成 `\u{…}`（计划行里报给 AI：正文里藏了几个看不见的字符）
pub fn hidden_count(s: &str) -> usize {
    s.chars().filter(|&c| hides(c)).count()
}

fn hides(c: char) -> bool {
    c != '\n' && c != '\t' && (c.is_control() || invisible(c))
}

/// 看不见、读屏也不读、却能藏东西的字符：Unicode 的 Default_Ignorable_Code_Point 全集（Unicode 15.1
/// `DerivedCoreProperties.txt`），外加格式类（Cf）与行 / 段分隔符（Zl / Zp）里不在其中的那几个。
/// 包括零宽字符、双向控制、软连字符、蒙古文变体选择符、变体选择符（含补充平面）、标签字符（U+E0000–E007F，
/// 能藏一整段只有程序读得出的文字）。宁可多转（emoji 的 U+FE0F、U+200D 也会显示成 `\u{…}`），不可漏
/// （自审第 3 轮 P1：人按 Touch ID 时看到的必须就是要发出去的全文）
fn invisible(c: char) -> bool {
    matches!(
        u32::from(c),
        0x00AD
            | 0x034F
            | 0x061C
            | 0x0600..=0x0605
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x115F..=0x1160
            | 0x17B4..=0x17B5
            | 0x180B..=0x180F
            | 0x200B..=0x200F
            | 0x2028..=0x202E
            | 0x2060..=0x206F
            | 0x3164
            | 0xFE00..=0xFE0F
            | 0xFEFF
            | 0xFFA0
            | 0xFFF0..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0000..=0xE0FFF
    )
}
