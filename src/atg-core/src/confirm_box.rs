//! Touch ID 确认框放不放得下（`specs/contracts/actions.md` 第一节第 4 小节第 4 条）：人按下去之前必须看得到要发的
//! 全文，放不下就不弹框、拒发。系统框按比例字体排版、在断行机会处折行，这里只能估，所以每一步都往多里估（宁可多拒）。
//! 从 `gate.rs` 挪出来（超长基线只许变小，新逻辑进新文件）；`gate` 照旧再导出这几个名字。

use crate::gate::visible;

// Touch ID 确认框的说明区放得下多少（系统面板高度固定，放不下的被截掉）。下面三个数都是**估计值，没在真弹框上量过**：
// 待人在场时用 `localauth` 的刻度尺测试实测（`ruler_popup`，交接记录里叫「冒烟 B」，`state/handoff/send2.md`），
// 量了以后改这三个数，并把实测记进契约 `actions.md` 第一节第 4 小节第 4 条。取得保守些，宁可多拒：macOS 还会在
// 说明前面加一句「“atg-cli”想要……」，系统密码兜底框的版式也不同（自审第 8 轮 P1）。
// 「自审第 N 轮」= `atg-review` 多视角自审的轮次，记录在 `state/handoff/send.md` 与 `send2.md`。

/// 整段说明最多几个字（字符数）。估计值，待实测（见上）
pub const CONFIRM_MAX_CHARS: usize = 120;
/// 最多几个**视觉行**：系统框会把长的一行自动折成几行，按折完的算，不按换行符数（自审第 9 轮 P1：只数换行符时，
/// 表头几行一折，排在最后的正文先被截掉，人按下去等于确认了没看到的话）。估计值，待实测（见上）
pub const CONFIRM_MAX_LINES: usize = 6;
/// 一个视觉行估计能放几列（一个汉字 2 列，即约 20 个汉字）。估计值，待实测（见上）
pub const CONFIRM_LINE_COLS: usize = 40;

/// 一个字符占几列。**白名单**：空格 1 列；宽度能估、最宽也就一个汉字的文字区块 2 列（框里是比例字体，大写拉丁
/// 字母 W、M 也差不多一个汉字宽，所以不按「半角 1 列」算，自审第 10 轮 P1）；**其余一律按占满一整行算**——
/// 有些字形比汉字宽好几倍（如 U+FDFD 合字、楔形文字），按 2 列估就会把排在后面的正文挤出框（自审第 12 轮 P1）。
/// 多算不少算，宁可多拒
fn cols(c: char) -> usize {
    match c {
        ' ' => 1,
        c if width_known(c) => 2,
        _ => CONFIRM_LINE_COLS,
    }
}

/// 确认框里宽度可估（不超过一个汉字）的字符：ASCII 与拉丁、希腊、西里尔字母，常用标点与符号，中日韩文字与
/// 标点、假名、谚文，全角形式，常见 emoji，以及确认说明自己用到的 ⏎ ⇥ ·
fn width_known(c: char) -> bool {
    matches!(u32::from(c),
        0x21..=0x7E            // ASCII 可见字符
        | 0xA0..=0x24F         // 拉丁补充与扩展
        | 0x370..=0x3FF        // 希腊
        | 0x400..=0x4FF        // 西里尔
        | 0x2010..=0x205E      // 常用标点（零宽、双向控制已被 visible 转义）
        | 0x2100..=0x23FF      // 字母式符号、箭头、数学、技术符号（含 ⏎ ⇥）
        | 0x2460..=0x27BF      // 带圈数字、制表符号、几何图形、杂项符号、装饰符号
        | 0x3000..=0x30FF      // 中日韩标点、平假名、片假名
        | 0x3400..=0x4DBF      // 汉字扩展 A
        | 0x4E00..=0x9FFF      // 汉字
        | 0xAC00..=0xD7AF      // 谚文音节
        | 0xFF00..=0xFFEF      // 全角与半角形式
        | 0x1F300..=0x1FAFF    // 常见 emoji
    )
}

/// 一个逻辑行折成几个视觉行。框按词折行：放不下的「词」整个挪到下一行，单个词比一行还长才按字断。空行也占一行。
/// `fine` 为假时只在空格处分词；为真时连字符、斜杠、标点、汉字这类字后面也算断行机会（系统框在这些地方也会折行，
/// codex 016 第 7 轮 #2：只按空格分词时，`MMM-MMM-MMM` 这种被当成一个长词逐字填满，比按连字符折少算一行）
fn wrapped_at(line: &str, fine: bool) -> usize {
    let (mut lines, mut cur) = (1, 0);
    for (i, word) in line.split(' ').enumerate() {
        // 每个空格都占 1 列，行首、连续的也算（codex 016 第 2 轮 #1：行首一串空格曾被算成零宽）；
        // 放不下就折到下一行，并把它算在新行里（多算不少算）
        if i > 0 {
            if cur + 1 > CONFIRM_LINE_COLS {
                lines += 1;
                cur = 0;
            }
            cur += 1;
        }
        for piece in pieces(word, fine) {
            place(piece, &mut lines, &mut cur);
        }
    }
    lines
}

/// 把一个空格分出来的词再按断行机会切开（`fine` 为假就不切）。切在断行字**之后**，字跟着前一段走
fn pieces(word: &str, fine: bool) -> Vec<&str> {
    if !fine {
        return vec![word];
    }
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in word.char_indices() {
        if !joins(c) {
            let end = i + c.len_utf8();
            out.push(&word[start..end]);
            start = end;
        }
    }
    if start < word.len() {
        out.push(&word[start..]);
    }
    out
}

/// 词中间连着、不在它后面断行的字：拉丁 / 希腊 / 西里尔字母与数字。其余（连字符、斜杠、标点、符号、汉字……）
/// 后面都可能断行
fn joins(c: char) -> bool {
    c.is_alphanumeric() && u32::from(c) < 0x2000
}

/// 把一段（中间不能断的）排进当前行：放得下接着排；放不下挪到下一行；比一行还长从新的一行起逐字排、
/// 放不下整个字就换行（字不能拆；codex 016 第 3 轮 #1：按总宽度除以列宽会把占满一行的宽字形和前后的字算进同一行）
fn place(piece: &str, lines: &mut usize, cur: &mut usize) {
    let w: usize = piece.chars().map(cols).sum();
    if w == 0 || *cur + w <= CONFIRM_LINE_COLS {
        *cur += w;
        return;
    }
    if *cur > 0 {
        *lines += 1;
    }
    if w <= CONFIRM_LINE_COLS {
        *cur = w;
        return;
    }
    *cur = 0;
    for c in piece.chars() {
        let cw = cols(c);
        if *cur > 0 && *cur + cw > CONFIRM_LINE_COLS {
            *lines += 1;
            *cur = 0;
        }
        *cur += cw;
    }
}

/// 一段说明在确认框里折成几个视觉行：每个逻辑行按「只在空格处断」和「断行机会处都可断」各估一遍，取多的
/// （系统的断行规则介于两者之间、又没实测，两种都可能更多；精确规则待实测，#89）
pub fn confirm_visual_lines(text: &str) -> usize {
    text.lines()
        .map(|l| wrapped_at(l, false).max(wrapped_at(l, true)))
        .sum()
}

/// 整段确认说明放不放得下：视觉行数与字数都不超（会话名、来源那几行也算，不只算正文）。还含制表符的一律放不下：
/// 制表符在框里跳到下一个制表位，比一个汉字宽得多，越过最后一个还会折行，估不准（自审第 11 轮 P1）。
/// 正常路径上 [`confirm_visible`] 已把它换成 ⇥，这里是兜底
pub fn fits_confirm_box(text: &str) -> bool {
    !text.contains('\t')
        && confirm_visual_lines(text) <= CONFIRM_MAX_LINES
        && text.chars().count() <= CONFIRM_MAX_CHARS
}

/// 放进确认框的文字：[`visible`] 之外再把制表符换成看得见、宽度可估的 ⇥（终端与计划行里的制表符照常保留）
pub fn confirm_visible(s: &str) -> String {
    visible(s).replace('\t', "⇥")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_box_counts_wrapped_lines_not_newlines() {
        // 自审第 9 轮 P1：4 个逻辑行、总字数也不超，但每行都长到要折——按视觉行算就放不下
        let long_line = "字".repeat(CONFIRM_LINE_COLS / 2 + 1);
        let text = [long_line.as_str(); 4].join("\n");
        assert!(text.chars().count() <= CONFIRM_MAX_CHARS);
        assert_eq!(confirm_visual_lines(&text), 8);
        assert!(!fits_confirm_box(&text));
        // 拉丁字母与汉字同按 2 列；空行也占一行
        assert_eq!(confirm_visual_lines(&"字".repeat(CONFIRM_LINE_COLS / 2)), 1);
        assert_eq!(
            confirm_visual_lines(&"W".repeat(CONFIRM_LINE_COLS / 2 + 1)),
            2
        );
        assert_eq!(confirm_visual_lines("一\n\n二"), 3);
        // 按词折：一行剩的放不下下一个词，整个词挪下去（字符一个个塞是 2 行，按词折要 3 行）
        let word = "W".repeat(CONFIRM_LINE_COLS / 2 - 2);
        let three = [word.as_str(); 3].join(" ");
        assert_eq!(confirm_visual_lines(&three), 3);
        // 比一行还长的词按字断
        assert_eq!(confirm_visual_lines(&"x".repeat(CONFIRM_LINE_COLS * 2)), 4);
        assert!(fits_confirm_box("发给 收藏夹 · id 1\n好的"));
    }

    #[test]
    fn tabs_never_reach_the_confirm_box() {
        // 自审第 11 轮 P1：一串制表符看着是空白、却能把后半句挤出框；换成 ⇥ 后按 2 列估，兜底见到制表符就拒
        assert_eq!(confirm_visible("好的\t\t后半句"), "好的⇥⇥后半句");
        assert!(!fits_confirm_box("好的\t后半句"));
        assert!(fits_confirm_box(&confirm_visible("好的\t后半句")));
    }

    #[test]
    fn glyphs_of_unknown_width_take_a_whole_line() {
        // 自审第 12 轮 P1：比汉字宽好几倍的字形不能按 2 列估；白名单外的一个字就占满一行
        let wide = format!("好{}把这句藏到框外", "\u{FDFD}".repeat(6));
        assert!(confirm_visual_lines(&wide) > CONFIRM_MAX_LINES);
        assert!(!fits_confirm_box(&wide));
        assert!(!fits_confirm_box(&format!("好{}", "\u{1242B}".repeat(6))));
        // 常见文字照常
        assert!(fits_confirm_box("好的，周末见 OK 👍 Привет"));
    }

    #[test]
    fn every_space_takes_a_column() {
        // codex 016 第 2 轮 #1：行首、连续的空格都占位，一串空格能把正文推到下一行
        assert_eq!(confirm_visual_lines(&" ".repeat(CONFIRM_LINE_COLS)), 1);
        assert_eq!(confirm_visual_lines(&" ".repeat(CONFIRM_LINE_COLS + 1)), 2);
        let pushed = format!("{}转账", " ".repeat(CONFIRM_LINE_COLS - 1));
        assert_eq!(confirm_visual_lines(&pushed), 2);
        // 完整的确认说明：会话名 + 来源 + 字数行 + 一串空格加两个字的正文
        let text = format!(
            "发给 私聊 · 林小雨周末爬山 · id 777\n来自命令行，可能是 AI，网络断了也会在恢复后发出\n共 52 字 1 行：\n{}转账",
            " ".repeat(50)
        );
        assert!(
            confirm_visual_lines(&text) >= 7,
            "{}",
            confirm_visual_lines(&text)
        );
        assert!(!fits_confirm_box(&text));
    }

    #[test]
    fn wide_glyph_inside_a_long_word_gets_its_own_line() {
        // codex 016 第 3 轮 #1：A + 占满一行的字形 + B，中间没空格，逐字排是 3 行
        assert_eq!(confirm_visual_lines("A\u{FDFD}B"), 3);
        assert_eq!(confirm_visual_lines(&"x".repeat(CONFIRM_LINE_COLS * 2)), 4);
    }

    #[test]
    fn hyphens_and_other_break_chances_are_counted() {
        // codex 016 第 7 轮 #2：三段各 12 个 M、用连字符连起来（38 字），按连字符折是 3 行
        let m = "M".repeat(12);
        let body = [m.as_str(); 3].join("-");
        assert_eq!(body.chars().count(), 38);
        assert_eq!(confirm_visual_lines(&body), 3);
        // 完整的确认说明：表头 4 行 + 这段正文，按连字符折要 7 行，放不下
        let text = format!(
            "发给 私聊 · 林小雨 · id 777\n来自命令行，可能是 AI，网络断了也会在恢复后发出\n共 38 字 1 行：\n{body}"
        );
        assert!(confirm_visual_lines(&text) > CONFIRM_MAX_LINES);
        assert!(!fits_confirm_box(&text));
        // 斜杠同理
        let slashed = [m.as_str(); 3].join("/");
        assert_eq!(confirm_visual_lines(&slashed), 3);
    }
}
