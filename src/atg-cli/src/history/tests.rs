//! 假后端单测：名字只取 `src/atg-core/fixtures/names.txt`，正文是编的（「第N条」），不碰真实账号。
use super::*;
use crate::out::Sink;
use atg_core::config::ServerKind;
use atg_core::fake::{Call, FakeAuth, FakeBackend};
use serde_json::{Value, json};
use std::path::PathBuf;

const ME: i64 = 100;
const LIN: i64 = 101;
const ZHOU: i64 = 102;
const GROUP: i64 = -1001;
/// 2026-10-26T11:30:00Z（东八区 19:30）
const T0: i64 = 1_793_014_200;

fn temp_root(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "atg-cli-history-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn profile(tag: &str) -> Profile {
    Profile::create(&temp_root(tag), "main", ServerKind::Main).unwrap()
}

fn user(id: i64, name: &str) -> Value {
    json!({"@type": "updateUser", "user": {"@type": "user", "id": id, "first_name": name, "last_name": "",
        "usernames": null, "phone_number": "", "is_contact": true, "type": {"@type": "userTypeRegular"}}})
}

fn text(t: &str) -> Value {
    json!({"@type": "messageText", "text": {"@type": "formattedText", "text": t, "entities": []}})
}

/// 一条消息：`sender` 是自己就算自己发的；时间每条加一分钟
fn msg(chat: i64, id: i64, sender: i64, content: Value) -> Value {
    json!({"@type": "message", "id": id, "chat_id": chat,
        "sender_id": {"@type": "messageSenderUser", "user_id": sender},
        "is_outgoing": sender == ME, "is_pinned": false, "date": T0 + id * 60, "edit_date": 0,
        "media_album_id": "0", "content": content})
}

fn plain(chat: i64, id: i64, sender: i64) -> Value {
    msg(chat, id, sender, text(&format!("第{id}条")))
}

/// 群：读到 `last_read`，`unread` 条未读，最后一条 `last`
fn group(last_read: i64, unread: u32, last: i64) -> Vec<Value> {
    vec![
        json!({"@type": "updateOption", "name": "my_id", "value": {"@type": "optionValueInteger", "value": ME.to_string()}}),
        user(ME, "我自己"),
        user(LIN, "林小雨"),
        user(ZHOU, "周大山"),
        json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": GROUP, "title": "周末爬山群",
            "type": {"@type": "chatTypeSupergroup", "supergroup_id": 1, "is_channel": false},
            "positions": [], "unread_count": unread, "unread_mention_count": 0,
            "last_read_inbox_message_id": last_read, "last_read_outbox_message_id": 0,
            "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0},
            "last_message": plain(GROUP, last, LIN)}}),
    ]
}

/// 第 1–10 条：第 6 条是自己发的，其余林小雨、周大山轮流
fn ten() -> Vec<Value> {
    (1..=10)
        .map(|i| {
            let s = match i {
                6 => ME,
                _ if i % 2 == 0 => ZHOU,
                _ => LIN,
            };
            plain(GROUP, i, s)
        })
        .collect()
}

fn backend(last_read: i64, unread: u32) -> FakeBackend {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth {
        logged_in: true,
        ..FakeAuth::default()
    });
    b.push_net("connectionStateReady");
    b.set_chat(GROUP, group(last_read, unread, 10));
    b.set_history(GROUP, ten());
    b
}

fn q(anchor: Anchor, limit: usize) -> Query {
    Query {
        chat: ChatRef::Id(GROUP),
        anchor,
        limit,
        local_only: false,
    }
}

fn page(b: &FakeBackend, tag: &str, q: &Query) -> Page {
    let p = profile(tag);
    with_backend(b, &p, Instant::now(), |b, s, f| {
        collect(b, s, f, q, Duration::from_millis(300))
    })
    .unwrap_or_else(|_| panic!("取不到"))
}

fn ids(p: &Page) -> Vec<i64> {
    p.ids().collect()
}

fn fail_code(r: Result<Page, Fail>) -> String {
    match r {
        Err(Fail::Core(e)) => e.code().to_owned(),
        Err(_) => "other".into(),
        Ok(_) => "ok".into(),
    }
}

/// 只调了只读的那几个：没有 openChat（后端有 `chat_open`，归 `watch --chat`，history 不许调）、也没有 viewMessages 这类
fn only_reads(b: &FakeBackend) {
    for c in b.calls() {
        assert!(
            matches!(
                c,
                Call::Start { .. }
                    | Call::Subscribe
                    | Call::Shutdown
                    | Call::ChatGet(_)
                    | Call::History(_)
                    | Call::MessageGet(_)
            ),
            "多了别的调用"
        );
    }
}

#[test]
fn lands_on_first_unread_skipping_own_messages() {
    // 读到第 5 条；第 6 条是自己发的，不算未读：落点是第 7 条
    let b = backend(5, 4);
    let p = page(&b, "land", &q(Anchor::Landing, 3));
    assert_eq!(ids(&p), vec![7, 8, 9]);
    assert_eq!(p.first_unread, Some(7));
    assert_eq!(p.landing, "first_unread");
    assert!(p.more && p.more_newer);
    only_reads(&b);
}

#[test]
fn lands_on_latest_without_unread() {
    let b = backend(10, 0);
    let p = page(&b, "latest", &q(Anchor::Landing, 3));
    assert_eq!(ids(&p), vec![8, 9, 10]);
    assert_eq!(p.first_unread, None);
    assert_eq!(p.landing, "latest");
    assert!(p.more);
    assert!(!p.more_newer);
}

#[test]
fn paging_back_is_contiguous_without_repeats() {
    let mut seen = vec![];
    let mut cursor = None;
    loop {
        let anchor = cursor.map_or(Anchor::Landing, Anchor::Before);
        let tag = format!("back{}", seen.len());
        // 一个假后端只起一次：每页一个（同直跑每次新起进程）
        let p = page(&backend(10, 0), &tag, &q(anchor, 3));
        let mut got = ids(&p);
        got.extend(seen);
        seen = got;
        if !p.more {
            break;
        }
        cursor = p.ids().next();
    }
    assert_eq!(seen, (1..=10).collect::<Vec<_>>());
}

#[test]
fn paging_forward_stops_at_last_message() {
    let p = page(&backend(10, 0), "fwd", &q(Anchor::After(4), 3));
    assert_eq!(ids(&p), vec![5, 6, 7]);
    assert!(p.more_newer);
    let p = page(&backend(10, 0), "fwd2", &q(Anchor::After(7), 5));
    assert_eq!(ids(&p), vec![8, 9, 10]);
    assert!(!p.more_newer);
    assert!(p.more);
}

#[test]
fn short_first_answers_are_filled_up() {
    // TDLib 首次只回起点那一条：循环补满，落点和翻页都不缺不重
    let b = backend(5, 4);
    b.short_first_history();
    let p = page(&b, "short", &q(Anchor::Landing, 4));
    assert_eq!(ids(&p), vec![7, 8, 9, 10]);
    let b = backend(10, 0);
    b.short_first_history();
    let p = page(&b, "short2", &q(Anchor::Landing, 4));
    assert_eq!(ids(&p), vec![7, 8, 9, 10]);
}

/// TDLib 连着好几次不给新的（停住）：不当成到头。拿到了消息 → 按「可能还有」交出并给游标，接着翻不缺；
/// 一条都没拿到 → 没有游标可给，more 为假，只看 partial
#[test]
fn stalled_answers_keep_more_or_report_partial() {
    // 往旧：第一次拿到最新一条（第 10 条）后，假后端故意连着只回起点（正常 TDLib 往旧不会，对应契约里「万一」那种）
    let b = backend(10, 0);
    b.short_first_history();
    b.stall_history(4);
    let p = page(&b, "stall", &q(Anchor::Landing, 4));
    assert!(ids(&p).len() < 4, "停住了，这页不满");
    assert!(p.more, "停住不等于到头");
    let oldest = p.ids().next().unwrap_or_else(|| panic!("至少有一条"));
    let p2 = page(&backend(10, 0), "stall2", &q(Anchor::Before(oldest), 4));
    assert_eq!(ids(&p2).last().copied(), Some(oldest - 1), "接着翻不缺");

    // 往新：一开始就停住、一条都没拿到
    let b = backend(10, 0);
    b.stall_history(4);
    let p = page(&b, "stall3", &q(Anchor::After(3), 4));
    assert!(p.partial, "停住了要说没拉全");
    // 一条都没拿到：没有游标可给，more_newer 为假，只看 partial（契约 cli.md）
    assert!(
        p.ids().next().is_none(),
        "备了 4 次停住，比上限 3 多，第 3 次就放弃了，一条都没拿到"
    );
    assert!(!p.more_newer);

    // 真到头（空应答）才说没有了：从最旧一条再往前
    let p = page(&backend(10, 0), "edge", &q(Anchor::Before(1), 4));
    assert!(ids(&p).is_empty());
    assert!(!p.more);
}

/// 落点：第一次往新取拿到两条后停住，跳过自己发的之后补的那一次顺利翻到最后一条——这页是全的，不报 partial
#[test]
fn landing_refill_that_reaches_the_end_is_not_partial() {
    // 读到第 5 条，第 6 条是自己发的；每次最多回 3 条；第一次照常（拿到 6、7），接着停住 3 次，之后照常
    let b = backend(5, 4);
    b.cap_history(3);
    b.stall_history_after(1, 3);
    let p = page(&b, "refill", &q(Anchor::Landing, 10));
    assert_eq!(ids(&p), vec![7, 8, 9, 10]);
    assert!(!p.partial, "补的那次翻到了最后一条，不该说没拉全");
    assert!(!p.more_newer);
    assert_eq!(p.first_unread, Some(7));
}

#[test]
fn saved_uses_own_chat() {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth {
        logged_in: true,
        ..FakeAuth::default()
    });
    b.push_net("connectionStateReady");
    b.push(json!({"@type": "updateOption", "name": "my_id", "value": {"@type": "optionValueInteger", "value": ME.to_string()}}));
    b.set_chat(
        ME,
        vec![user(ME, "我自己"), json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": ME,
            "title": "我自己", "type": {"@type": "chatTypePrivate", "user_id": ME}, "positions": [],
            "unread_count": 0, "unread_mention_count": 0, "last_read_inbox_message_id": 2,
            "last_read_outbox_message_id": 2,
            "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0},
            "last_message": plain(ME, 2, ME)}})],
    );
    b.set_history(ME, vec![plain(ME, 1, ME), plain(ME, 2, ME)]);
    let mut query = q(Anchor::Landing, 5);
    query.chat = ChatRef::Saved;
    let p = page(&b, "saved", &query);
    assert_eq!(p.chat_id, ME);
    assert_eq!(ids(&p), vec![1, 2]);
    assert!(!p.more && !p.more_newer);
    assert!(b.calls().contains(&Call::ChatGet(ME)));
}

#[test]
fn errors_not_logged_in_and_unknown_chat() {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth::default());
    let p = profile("nologin");
    let r = with_backend(&b, &p, Instant::now(), |b, s, f| {
        collect(b, s, f, &q(Anchor::Landing, 3), Duration::ZERO)
    });
    assert_eq!(fail_code(r), "auth_required");

    let b = backend(5, 4);
    let p = profile("unknown");
    let mut query = q(Anchor::Landing, 3);
    query.chat = ChatRef::Id(-999);
    let r = with_backend(&b, &p, Instant::now(), |b, s, f| {
        collect(b, s, f, &query, Duration::ZERO)
    });
    assert_eq!(fail_code(r), "not_found");
}

#[test]
fn chat_ref_parsing() {
    assert_eq!(ChatRef::parse("saved"), Some(ChatRef::Saved));
    assert_eq!(ChatRef::parse("-1001"), Some(ChatRef::Id(-1001)));
    assert_eq!(ChatRef::parse("x"), None);
}

// ───── 输出（快照）─────

#[derive(Default)]
struct Cap {
    out: Vec<String>,
    err: Vec<String>,
}
impl Sink for Cap {
    fn out(&mut self, line: &str) {
        self.out.push(line.to_owned());
    }
    fn err(&mut self, line: &str) {
        self.err.push(line.to_owned());
    }
}

/// 去掉每次都变的时间字段
fn norm(lines: &[String]) -> String {
    lines
        .iter()
        .map(|l| {
            let mut v: Value = serde_json::from_str(l).unwrap();
            if let Some(o) = v.as_object_mut() {
                o.insert("at".into(), json!("…"));
            }
            if let Some(d) = v["data"].as_object_mut()
                && d.contains_key("elapsed_ms")
            {
                d.insert("elapsed_ms".into(), json!(0));
            }
            v.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn args(detail: bool, full: bool, reveal: bool) -> HistoryArgs {
    HistoryArgs {
        chat: GROUP.to_string(),
        limit: 20,
        before: None,
        after: None,
        detail,
        full,
        reveal,
        local: false,
        tz: None,
    }
}

fn cx(reveal: bool) -> SpeechCtx {
    speech_ctx(Some("+08:00"), reveal).unwrap_or_else(|_| panic!("时区"))
}

/// 详情、全文、隐藏内容要用的几条：回复（带引用）、转发自频道、已编辑、表情回应、论坛话题、剧透、遮挡的图片
fn rich() -> FakeBackend {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth {
        logged_in: true,
        ..FakeAuth::default()
    });
    b.push_net("connectionStateReady");
    let mut chat = group(2, 4, 6);
    chat.push(json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": -1002, "title": "示例频道",
        "type": {"@type": "chatTypeSupergroup", "supergroup_id": 2, "is_channel": true}, "positions": [],
        "unread_count": 0, "unread_mention_count": 0, "last_read_inbox_message_id": 0, "last_read_outbox_message_id": 0,
        "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0}}}));
    b.set_chat(GROUP, chat);
    let mut reply = msg(GROUP, 3, LIN, text("收到"));
    reply["reply_to"] = json!({"@type": "messageReplyToMessage", "chat_id": GROUP, "message_id": 2,
        "quote": {"@type": "textQuote", "text": {"@type": "formattedText", "text": "周六出发", "entities": []}, "position": 0, "is_manual": true}});
    reply["edit_date"] = json!(T0 + 3 * 60 + 600);
    reply["interaction_info"] = json!({"@type": "messageInteractionInfo", "view_count": 0, "forward_count": 0,
        "reactions": {"@type": "messageReactions", "reactions": [
            {"@type": "messageReaction", "type": {"@type": "reactionTypeEmoji", "emoji": "👍"}, "total_count": 3, "is_chosen": true},
            {"@type": "messageReaction", "type": {"@type": "reactionTypeEmoji", "emoji": "❤️"}, "total_count": 1, "is_chosen": false}]}});
    reply["topic_id"] = json!({"@type": "messageTopicForum", "forum_topic_id": 3});
    let mut fwd = msg(GROUP, 4, ZHOU, text("周六全天晴。\n注意防晒补水！"));
    fwd["forward_info"] = json!({"@type": "messageForwardInfo",
        "origin": {"@type": "messageOriginChannel", "chat_id": -1002, "message_id": 9, "author_signature": ""},
        "date": T0 - 86_400});
    // 「答案是42，别说出去」：「42」是剧透
    let spoiler = msg(
        GROUP,
        5,
        LIN,
        json!({"@type": "messageText", "text": {"@type": "formattedText",
        "text": "答案是42，别说出去", "entities": [{"@type": "textEntity", "offset": 3, "length": 2,
        "type": {"@type": "textEntityTypeSpoiler"}}]}}),
    );
    let photo = msg(
        GROUP,
        6,
        ME,
        json!({"@type": "messagePhoto", "photo": {"@type": "photo", "has_stickers": false, "minithumbnail": null, "sizes": []},
        "caption": {"@type": "formattedText", "text": "山顶的风景", "entities": []}, "has_spoiler": true, "show_caption_above_media": false, "is_secret": false}),
    );
    b.set_history(
        GROUP,
        vec![
            plain(GROUP, 1, ZHOU),
            plain(GROUP, 2, ZHOU),
            reply,
            fwd,
            spoiler,
            photo,
        ],
    );
    b
}

fn render_rich(tag: &str, format: Format, a: &HistoryArgs) -> Cap {
    let b = rich();
    let p = page(&b, tag, &q(Anchor::Landing, 20));
    let mut cap = Cap::default();
    render(&mut cap, format, a, &cx(a.reveal), &p, Instant::now());
    cap
}

#[test]
fn json_brief_detail_full_snapshot() {
    let cap = render_rich("snapj", Format::Json, &args(true, true, false));
    insta::assert_snapshot!("history_json_detail_full", norm(&cap.out));
    // 不加 --reveal：data 树里也没有剧透的字（大总管授权 2026-10-26）
    // 只比文字节点：原始行里的 `at` 时刻也可能含「42」
    let secret = r#""text":"42""#;
    assert!(!cap.out.join("").contains(secret), "剧透漏进 data");
    assert!(
        cap.out
            .join("")
            .contains(r#""type":"spoiler","children":[]"#)
    );
    let shown = render_rich("snapjr", Format::Json, &args(false, false, true));
    assert!(
        shown.out.join("").contains(secret),
        "--reveal 后 data 该带原文"
    );
}

#[test]
fn text_snapshots_with_unread_marker() {
    let plain_text = render_rich("snapt", Format::Text, &args(false, false, false));
    let detail = render_rich("snapd", Format::Text, &args(true, false, false));
    let full = render_rich("snapf", Format::Text, &args(false, true, false));
    let revealed = render_rich("snapr", Format::Text, &args(false, true, true));
    insta::assert_snapshot!(
        "history_text",
        [
            "# brief",
            &plain_text.out.join("\n"),
            "# detail",
            &detail.out.join("\n"),
            "# full",
            &full.out.join("\n"),
            "# full --reveal",
            &revealed.out.join("\n"),
            "# stderr",
            &plain_text.err.join("\n"),
        ]
        .join("\n")
    );
    // 第一条未读（第 3 条）上面一行是「以下为未读」
    assert_eq!(plain_text.out[0], UNREAD_BELOW);
    // 不加 --reveal：剧透的字一个都不出
    for l in plain_text.out.iter().chain(&detail.out).chain(&full.out) {
        assert!(!l.contains("42"), "剧透漏出来了");
    }
    assert!(revealed.out.iter().any(|l| l.contains("42")));
}

// ───── links ─────

fn links_backend() -> FakeBackend {
    let b = backend(10, 0);
    // 「看下路线图 https://example.invalid/a 秘密」：「路线图」是文字链接，网址照写，「秘密」是剧透里的文字链接
    let t = "看下路线图 https://example.invalid/a 秘密";
    let utf16 = |s: &str| s.encode_utf16().count();
    let url_at = utf16("看下路线图 ");
    let secret_at = utf16("看下路线图 https://example.invalid/a ");
    let m = msg(
        GROUP,
        11,
        LIN,
        json!({"@type": "messageText", "text": {"@type": "formattedText", "text": t, "entities": [
        {"@type": "textEntity", "offset": 2, "length": 3, "type": {"@type": "textEntityTypeTextUrl", "url": "https://example.invalid/route"}},
        {"@type": "textEntity", "offset": url_at, "length": utf16("https://example.invalid/a"), "type": {"@type": "textEntityTypeUrl"}},
        {"@type": "textEntity", "offset": secret_at, "length": 2, "type": {"@type": "textEntityTypeSpoiler"}},
        {"@type": "textEntity", "offset": secret_at, "length": 2, "type": {"@type": "textEntityTypeTextUrl", "url": "https://example.invalid/secret"}}]}}),
    );
    let mut all = ten();
    all.push(m);
    b.set_history(GROUP, all);
    b
}

fn links_of(tag: &str, message_id: i64) -> Result<LinkList, Fail> {
    let b = links_backend();
    let p = profile(tag);
    let q = LinksQuery {
        chat: ChatRef::Id(GROUP),
        message_id,
    };
    let r = with_backend(&b, &p, Instant::now(), |b, s, f| collect_links(b, s, f, &q));
    only_reads(&b);
    r
}

#[test]
fn links_masked_and_revealed() {
    let Ok(l) = links_of("links", 11) else {
        panic!("取不到")
    };
    assert_eq!(l.links.len(), 3);
    let mut masked = Cap::default();
    render_links(&mut masked, Format::Json, false, &l, Instant::now());
    let mut shown = Cap::default();
    render_links(&mut shown, Format::Text, true, &l, Instant::now());
    let mut hidden_text = Cap::default();
    render_links(&mut hidden_text, Format::Text, false, &l, Instant::now());
    insta::assert_snapshot!(
        "links",
        [
            norm(&masked.out),
            shown.out.join("\n"),
            hidden_text.out.join("\n"),
        ]
        .join("\n")
    );
    assert!(
        !masked.out.join("").contains("secret"),
        "隐藏的地址漏出来了"
    );
    assert!(!hidden_text.out.join("").contains("秘密"));
}

#[test]
fn links_none_and_missing_message() {
    let Ok(l) = links_of("nolinks", 3) else {
        panic!("取不到")
    };
    assert!(l.links.is_empty());
    let code = match links_of("missing", 999) {
        Err(Fail::Core(e)) => e.code(),
        _ => "?",
    };
    assert_eq!(code, "not_found");
}
