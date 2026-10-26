//! `logline`：本项目自己的日志，只记形状，不记内容（`specs/contracts/config-privacy.md` 第一节第 5 小节）。
//!
//! 白名单靠类型拦：字段只能是 [`Field`] 的几种——写死的名字（`&'static str`）、数字、清洗过的 TDLib 类型名。
//! `String`、`serde_json::Value`、错误对象都进不来；事件名本身也必须是 `&'static str`。
//! 输出走 `log` crate（目标 `atg`），由进程入口决定写到哪（`~/Library/Logs/atg/`）。

/// 一个日志字段
pub enum Field<'a> {
    /// 数字：id、序号、计数、长度、耗时、错误码
    Num(&'static str, i64),
    /// 写死的取值：状态名、错误机读码、事件名
    Name(&'static str, &'static str),
    /// TDLib 类型名（运行时来的）：只认标识符字符、最长 64，否则记 `<invalid>`
    TdType(&'static str, &'a str),
}

/// 运行时拿到的 TDLib 类型名 → 能进日志的样子
pub fn safe_td_type(t: &str) -> &str {
    if !t.is_empty() && t.len() <= 64 && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        t
    } else {
        "<invalid>"
    }
}

/// 拼成一行：`event k=v k=v`
pub fn render(event: &'static str, fields: &[Field<'_>]) -> String {
    let mut line = String::from(event);
    for f in fields {
        line.push(' ');
        match f {
            Field::Num(k, v) => line.push_str(&format!("{k}={v}")),
            Field::Name(k, v) => line.push_str(&format!("{k}={v}")),
            Field::TdType(k, v) => line.push_str(&format!("{k}={}", safe_td_type(v))),
        }
    }
    line
}

pub fn info(event: &'static str, fields: &[Field<'_>]) {
    log::info!(target: "atg", "{}", render(event, fields));
}

pub fn warn(event: &'static str, fields: &[Field<'_>]) {
    log::warn!(target: "atg", "{}", render(event, fields));
}
