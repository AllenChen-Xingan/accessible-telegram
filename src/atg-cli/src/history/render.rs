//! `history` 一页的读法与渲染：读法上下文（时区、要不要读出隐藏内容；CLI 没有按键）、文字 / JSON 两种输出、「以下为未读」那一行。
//! 取数在模块根 `history.rs`。

use super::*;

/// 读法上下文：CLI 没有按键；`--tz` 不给就跟系统时区
pub fn speech_ctx(tz: Option<&str>, reveal: bool) -> Result<SpeechCtx, Fail> {
    let (utc_offset, offset_at) = match tz {
        None => (
            atg_core::clock::local_offset_at(atg_core::clock::now_s()),
            Some(atg_core::clock::local_offset_at as fn(i64) -> i32),
        ),
        Some(s) => match crate::speech::parse_tz(s) {
            Some(o) => (o, None),
            None => {
                return Err(Fail::Usage(
                    "--tz 不对：写成 +08:00、-05:30、+0800、+8、Z 或 UTC",
                ));
            }
        },
    };
    Ok(SpeechCtx {
        utc_offset,
        offset_at,
        listen_key: None,
        expand_key: None,
        reveal,
    })
}

/// 把一页写到 `sink`，返回退出码（直跑与执行方共用这一段，输出才一致）。
/// JSON：每条一行 `kind:"message"` + 一行 `result`；文字：每条一句（`--full` 读全文，`--detail` 接「详情：」），
/// 第一条未读前另起一行「以下为未读」（F2），统计写标准错误
pub fn render(
    sink: &mut dyn Sink,
    format: Format,
    a: &HistoryArgs,
    cx: &SpeechCtx,
    page: &Page,
    started: Instant,
) -> u8 {
    let layers = Layers {
        detail: a.detail,
        full: a.full,
    };
    for d in &page.docs {
        match format {
            Format::Json => {
                let mut line = atg_core::json::value_with(d, cx, layers);
                if let Some(o) = line.as_object_mut() {
                    o.insert("at".into(), json!(out::now_rfc3339_ms()));
                }
                emit_to(sink, &line);
            }
            Format::Text => {
                if page.first_unread.is_some() && doc_id(d) == page.first_unread {
                    sink.out(UNREAD_BELOW);
                }
                // 文字模式一行一条：名称里的换行换成空格；全文本来就要保留换行，照原样
                let main = if a.full {
                    speech::full(d, cx)
                } else {
                    speech::brief(d, cx).replace('\n', " ")
                };
                let detail = if a.detail {
                    speech::detail(d, cx)
                } else {
                    String::new()
                };
                if detail.is_empty() {
                    sink.out(&main);
                } else {
                    sink.out(&format!("{main}。详情：{detail}"));
                }
            }
        }
    }
    let oldest = page.ids().next();
    let newest = page.ids().last();
    match format {
        Format::Json => {
            let mut data = json!({
                "command": "history",
                "chat_id": page.chat_id,
                "count": page.docs.len(),
                "more": page.more,
                "more_newer": page.more_newer,
                "first_unread": page.first_unread,
                "landing": page.landing,
                "synced": page.synced,
                "partial": page.partial,
                "elapsed_ms": crate::elapsed_ms(started),
            });
            if let Some(d) = data.as_object_mut() {
                if let (true, Some(o)) = (page.more, oldest) {
                    d.insert("next_cursor".into(), json!(o));
                }
                if let (true, Some(n)) = (page.more_newer, newest) {
                    d.insert("newer_cursor".into(), json!(n));
                }
            }
            emit_to(sink, &envelope("result", data));
        }
        Format::Text => {
            let where_ = match page.landing {
                "first_unread" if page.first_unread.is_some() => "从第一条未读起",
                "first_unread" | "latest" => "最新的",
                "before" => "更早的",
                _ => "更新的",
            };
            let mut s = format!("{where_} {} 条", page.docs.len());
            if page.more {
                s.push_str("；往前还有");
            }
            if page.more_newer {
                s.push_str("；往后还有");
            }
            if !page.synced {
                s.push_str("；没等到同步完，未读位置可能是旧的");
            }
            if page.partial {
                s.push_str("；Telegram 还没给全，可以稍后再试");
            }
            sink.err(&s);
        }
    }
    0
}

/// 文字模式里第一条未读上面那一行（F2「以下为未读」）
pub const UNREAD_BELOW: &str = "以下为未读";
