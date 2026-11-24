//! `links`：列出一条消息里的链接（取、渲染）。起后端、找会话用模块根 `history.rs` 的。

use super::*;

// ───────────────────────── links ─────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct LinksQuery {
    pub chat: ChatRef,
    pub message_id: i64,
}

/// 一条消息里的链接：（文字，地址，是否在剧透里），按出现顺序
pub struct LinkList {
    pub id: MsgId,
    pub links: Vec<(String, String, bool)>,
}

pub fn links_query(a: &LinksArgs) -> Result<LinksQuery, Fail> {
    Ok(LinksQuery {
        chat: chat_arg(&a.chat)?,
        message_id: a.message,
    })
}

/// 在已起好的后端上取这条消息、列出链接（`getMessage`，不标已读）
pub fn collect_links<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    q: &LinksQuery,
) -> Result<LinkList, Fail> {
    chats::ensure_ready(b)?;
    let chat_id = resolve_chat(b, q.chat)?;
    load_chat(b, stream, feed, chat_id)?;
    let id = MsgId {
        chat_id,
        message_id: q.message_id,
    };
    let m = bounded(b, b.message_get(id), QUERY_TIMEOUT).map_err(Fail::Core)?;
    feed.drain(stream);
    // 有效内容（有临时内容就用它）；受限的存下来时已换成受限节点，里面没有正文，也就没有链接
    let links = match m.effective() {
        Body::Content(c) => links_marked(&c.body)
            .into_iter()
            .map(|(l, hidden)| (l.text, l.url, hidden))
            .collect(),
        Body::Service(_) => vec![],
    };
    Ok(LinkList { id, links })
}

/// 隐藏的链接文字换成这个（地址给 `null`）
const HIDDEN_LINK: &str = "隐藏内容";

pub fn render_links(
    sink: &mut dyn Sink,
    format: Format,
    reveal: bool,
    l: &LinkList,
    started: Instant,
) -> u8 {
    for (i, (text, url, hidden)) in l.links.iter().enumerate() {
        let masked = *hidden && !reveal;
        let (text, url): (&str, Value) = if masked {
            (HIDDEN_LINK, Value::Null)
        } else {
            (text, json!(url))
        };
        let index = i + 1;
        match format {
            Format::Json => {
                let line = envelope(
                    "link",
                    json!({ "index": index, "text": text, "url": url, "hidden": hidden }),
                );
                emit_to(sink, &line);
            }
            Format::Text => match url.as_str() {
                Some(u) if u == text => sink.out(&format!("链接 {index}，{u}")),
                Some(u) => sink.out(&format!("链接 {index}，{text}，{u}")),
                None => sink.out(&format!("链接 {index}，{text}")),
            },
        }
    }
    match format {
        Format::Json => emit_to(
            sink,
            &envelope(
                "result",
                json!({
                    "command": "links",
                    "chat_id": l.id.chat_id,
                    "message_id": l.id.message_id,
                    "count": l.links.len(),
                    "elapsed_ms": crate::elapsed_ms(started),
                }),
            ),
        ),
        Format::Text => sink.err(&if l.links.is_empty() {
            "这条消息里没有链接".to_owned()
        } else {
            format!("共 {} 个链接", l.links.len())
        }),
    }
    0
}
