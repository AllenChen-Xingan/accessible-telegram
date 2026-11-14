//! 转写表（#59 3b；`docs/experiments/exp008-local-stt/design.md` 第二节第 3、4 条）：凭据与裁决、整理（删和迁）、
//! 投影（`transcript` 字段）、读法。规则只有一条：转写只对「同一个音频文件」作数。全是编的假数据。
//! design 第二节第 3 条要的「经假后端真实事件流、落在下载中 / 转写中两段时间」的测试要有 `transcribe` 命令，
//! 随 #59 第三段（3c，`atg-cli` 的 `transcribe`，见 `state/handoff/media.md`）补，这里没有。

#[allow(dead_code)] // 共用助手里本文件没用到的那些
mod common;
use atg_core::json;
use atg_core::model::*;
use atg_core::project;
use atg_core::speech::{self, SpeechCtx};
use atg_core::store::{Input, Stale, Store, StoreConfig, Transcribed, TranscriptTicket};
use atg_core::tree::{Doc, FileRef, Transcript, TranscriptSegment, TranscriptSource};
use common::*;

fn tr(parts: &[&str]) -> Transcript {
    Transcript {
        lang: "zh-CN".into(),
        source: TranscriptSource::Local,
        segments: parts
            .iter()
            .enumerate()
            .map(|(i, t)| TranscriptSegment {
                text: (*t).into(),
                start_s: i as f64,
                end_s: i as f64 + 1.0,
            })
            .collect(),
    }
}

fn voice_with(file: i32) -> Body {
    let Body::Content(mut c) = voice() else {
        unreachable!()
    };
    if let Some(Attachment::Voice { file: f, .. }) = &mut c.attachment {
        *f = FileRef { id: file };
    }
    Body::Content(c)
}

const V: MsgId = MsgId {
    chat_id: CHAT,
    message_id: 5,
};

/// 一条语音、取好凭据
fn setup() -> (Store, TranscriptTicket) {
    let s = feed([new(msg(CHAT, 5, voice()))]);
    let t = s.transcript_ticket(V).expect("语音消息该有凭据");
    (s, t)
}

fn commit(s: &mut Store, t: TranscriptTicket, parts: &[&str]) {
    s.apply(Input::Transcribed(Transcribed {
        ticket: t,
        transcript: tr(parts),
    }));
}

fn has(s: &Store, id: MsgId) -> bool {
    s.transcript(id).is_some()
}

#[test]
fn ticket_only_for_live_voice() {
    let s = feed([new(msg(CHAT, 5, voice())), new(msg(CHAT, 6, text("编的")))]);
    assert_eq!(s.transcript_ticket(V).map(|t| t.file.id), Some(7));
    assert!(s.transcript_ticket(id(CHAT, 6)).is_none(), "不是语音");
    assert!(s.transcript_ticket(id(CHAT, 9)).is_none(), "没有这条");
}

#[test]
fn accepted_is_written_and_projected() {
    let (mut s, t) = setup();
    assert_eq!(s.transcript_verdict(&t), Ok(V));
    commit(&mut s, t, &["编的一句"]);
    assert!(has(&s, V));
    let Some(Doc::Message(m)) = project::message(&s, V) else {
        panic!("投影不是消息")
    };
    assert!(m.transcript.is_some());
}

#[test]
fn listened_and_caption_edit_keep_it() {
    let (mut s, t) = setup();
    push(&mut s, CoreEvent::MessageContentOpened { id: V });
    // 标了已听，凭据照样作数
    assert_eq!(s.transcript_verdict(&t), Ok(V));
    commit(&mut s, t, &["编的"]);
    push(
        &mut s,
        CoreEvent::MessageEdited {
            id: V,
            edit_date: Time(1_792_755_500),
        },
    );
    assert!(has(&s, V), "已听、编辑时间都不碰音频");
}

#[test]
fn deleted_or_revoked_is_gone() {
    for permanent in [true, false] {
        let (mut s, t) = setup();
        commit(&mut s, t, &["编的"]);
        push(&mut s, deleted(CHAT, &[5], permanent, false));
        assert!(!has(&s, V), "删了还在（permanent={permanent}）");
        assert_eq!(s.transcript_verdict(&t), Err(Stale::Gone));
    }
}

#[test]
fn content_replaced_is_changed() {
    let (mut s, t) = setup();
    commit(&mut s, t, &["编的"]);
    push(
        &mut s,
        CoreEvent::MessageContentChanged {
            id: V,
            content: voice_with(8),
        },
    );
    assert!(!has(&s, V), "换成别的语音，旧转写还在");
    assert_eq!(s.transcript_verdict(&t), Err(Stale::Changed));
    // 迟到的结果不许复活
    commit(&mut s, t, &["编的"]);
    assert!(!has(&s, V));
}

#[test]
fn ephemeral_appearing_or_going_drops_it() {
    let (mut s, t) = setup();
    commit(&mut s, t, &["编的"]);
    push(
        &mut s,
        CoreEvent::MessageEphemeralChanged {
            id: V,
            ephemeral: Some(text("编的临时内容")),
        },
    );
    assert!(!has(&s, V));
    assert_eq!(s.transcript_verdict(&t), Err(Stale::Changed));
    // 临时内容消失：有效内容又是那段语音，凭据重新作数（转写的对象就是那段音频）；
    // 但表里那条已经删了，不会自己回来，要再提交一次
    push(
        &mut s,
        CoreEvent::MessageEphemeralChanged {
            id: V,
            ephemeral: None,
        },
    );
    assert_eq!(s.transcript_verdict(&t), Ok(V));
    assert!(!has(&s, V), "删了的不自己回来");
    commit(&mut s, t, &["编的"]);
    assert!(has(&s, V));
}

/// 历史重拉、整条快照替换但内容没变：转写留着（按序号判会误杀的三种之一）
#[test]
fn history_reload_same_content_keeps_it() {
    let (mut s, t) = setup();
    commit(&mut s, t, &["编的"]);
    s.apply(Input::OpenChat { chat_id: CHAT });
    let ticket = s.ticket(CHAT);
    s.apply(loaded(ticket, vec![msg(CHAT, 5, voice())]));
    assert!(has(&s, V));
    assert_eq!(s.transcript_verdict(&t), Ok(V));
}

/// 受限信息只随整条消息快照到（`restriction_info` 是消息对象的字段，没有单独的更新），所以用历史应答带进来
#[test]
fn restricted_drops_it() {
    let (mut s, t) = setup();
    commit(&mut s, t, &["编的"]);
    s.apply(Input::OpenChat { chat_id: CHAT });
    let ticket = s.ticket(CHAT);
    let mut m = msg(CHAT, 5, voice());
    m.restriction = Some("编的原因".into());
    s.apply(loaded(ticket, vec![m]));
    assert!(!has(&s, V), "受限了，转写还在");
    assert_eq!(s.transcript_verdict(&t), Err(Stale::Changed));
}

#[test]
fn send_succeeded_moves_to_new_id() {
    let mut s = Store::new();
    let mut pending = msg(CHAT, 5, voice());
    pending.outgoing = true;
    pending.sending = SendingState::Pending;
    push(&mut s, new(pending.clone()));
    let t = s.transcript_ticket(V).unwrap();
    let mut sent = pending;
    sent.id = id(CHAT, 50);
    sent.sending = SendingState::Sent;
    push(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: V,
            message: sent,
        },
    );
    // 换 id 之后提交：接受，记在新 id 上
    assert_eq!(s.transcript_verdict(&t), Ok(id(CHAT, 50)));
    commit(&mut s, t, &["编的"]);
    assert!(has(&s, id(CHAT, 50)));
    assert!(!has(&s, V));
}

fn pending_voice(message_id: i64, file: i32) -> Message {
    let mut m = msg(CHAT, message_id, voice_with(file));
    m.outgoing = true;
    m.sending = SendingState::Pending;
    m
}

fn sent_as(m: &Message, new_id: i64, file: i32) -> Message {
    let mut s = m.clone();
    s.id = id(CHAT, new_id);
    s.content = voice_with(file);
    s.sending = SendingState::Sent;
    s
}

/// 先写了转写、再换 id（整理时挪键）：新 id 上原文还在、旧 id 上没了；换两次（失败换 id 再成功换 id）也跟着走；
/// 换 id 时文件变了就删
#[test]
fn committed_then_renamed_moves_key() {
    let mut s = Store::new();
    let p = pending_voice(5, 7);
    push(&mut s, new(p.clone()));
    let t = s.transcript_ticket(V).unwrap();
    commit(&mut s, t, &["编的原文"]);
    let failed_as = {
        let mut f = sent_as(&p, 50, 7);
        f.sending = SendingState::Failed;
        f
    };
    push(
        &mut s,
        CoreEvent::SendFailed {
            old_id: V,
            message: failed_as.clone(),
            error: atg_core::error::CoreError::Internal { what: "编的" },
            can_retry: true,
        },
    );
    assert!(!has(&s, V));
    assert!(
        full_of_id(&s, id(CHAT, 50)).ends_with("编的原文"),
        "第一次换 id 没挪过去"
    );
    push(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: id(CHAT, 50),
            message: sent_as(&failed_as, 51, 7),
        },
    );
    assert!(!has(&s, id(CHAT, 50)));
    assert!(
        full_of_id(&s, id(CHAT, 51)).ends_with("编的原文"),
        "第二次换 id 没挪过去"
    );

    // 换 id 时文件变了：删
    let mut s = Store::new();
    let p = pending_voice(5, 7);
    push(&mut s, new(p.clone()));
    let t = s.transcript_ticket(V).unwrap();
    commit(&mut s, t, &["编的原文"]);
    push(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: V,
            message: sent_as(&p, 50, 8),
        },
    );
    assert!(!has(&s, V) && !has(&s, id(CHAT, 50)));
}

fn failed(message_id: i64) -> Message {
    let mut m = msg(CHAT, message_id, voice());
    m.outgoing = true;
    m.sending = SendingState::Failed;
    m
}

/// 重发：失败那条 F 与新那条 N 一段时间里同时是活消息、共用稳定键——两条的转写各归各的，删 F 不连累 N
#[test]
fn resent_old_and_new_are_separate() {
    let mut s = feed([new(failed(5))]);
    let tf = s.transcript_ticket(V).unwrap();
    let n = id(CHAT, 6);
    let mut pending = failed(6);
    pending.sending = SendingState::Pending;
    push(&mut s, new(pending));
    s.apply(Input::Resent { old: V, new: n });
    let tn = s.transcript_ticket(n).unwrap();
    commit(&mut s, tn, &["新那条"]);
    commit(&mut s, tf, &["失败那条"]); // 迟到的、失败那条的
    assert!(has(&s, n) && has(&s, V), "两条各归各的");
    push(&mut s, deleted(CHAT, &[5], false, false));
    assert!(!has(&s, V));
    assert!(
        full_of_id(&s, n).ends_with("新那条"),
        "删失败那条不连累新那条"
    );
}

/// 重发应答晚于新那条的换 id 才到（`tests/reducer.rs` 的 `resent_vs_send_succeeded_any_order` 支持这个先后）：
/// 其间提交的转写，重发应答到了之后照样查得到
#[test]
fn resent_after_send_succeeded_keeps_it() {
    let mut s = feed([new(failed(5))]);
    let b = id(CHAT, 6);
    let c = id(CHAT, 60);
    let mut pending = failed(6);
    pending.sending = SendingState::Pending;
    push(&mut s, new(pending.clone()));
    let mut sent = pending;
    sent.id = c;
    sent.sending = SendingState::Sent;
    push(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: b,
            message: sent,
        },
    );
    let t = s.transcript_ticket(c).unwrap();
    commit(&mut s, t, &["编的"]);
    s.apply(Input::Resent { old: V, new: b });
    assert!(has(&s, c), "重发应答改了稳定键，转写不该丢");
    push(&mut s, deleted(CHAT, &[5], false, false));
    assert!(has(&s, c));
}

#[test]
fn later_commit_overwrites() {
    let (mut s, t) = setup();
    commit(&mut s, t, &["第二次"]);
    commit(&mut s, t, &["第一次"]);
    assert_eq!(full_of(&s), "语音 3 秒，第一次", "后到的覆盖先到的");
}

#[test]
fn pruned_drops_it() {
    let mut s = Store::with_config(StoreConfig { keep_per_chat: 1 });
    push(
        &mut s,
        CoreEvent::ChatAdded {
            chat: chat(CHAT, 1),
            last_message: None,
        },
    );
    push(&mut s, new(msg(CHAT, 5, voice())));
    let t = s.transcript_ticket(V).unwrap();
    commit(&mut s, t, &["编的"]);
    assert!(has(&s, V));
    // 非当前会话按上限删旧：来了更新的一条，这条被删掉，转写跟着删
    push(&mut s, new(msg(CHAT, 6, text("编的"))));
    push(&mut s, new(msg(CHAT, 7, text("编的"))));
    assert!(s.message(V).is_none(), "前提：这条该被删旧");
    assert!(!has(&s, V));
}

// ───── 投影与读法 ─────

fn cx() -> SpeechCtx {
    SpeechCtx {
        utc_offset: 8 * 3600,
        offset_at: None,
        listen_key: None,
        expand_key: None,
        reveal: false,
    }
}

fn doc(s: &Store) -> Doc {
    project::message(s, V).unwrap()
}

fn full_of(s: &Store) -> String {
    full_of_id(s, V)
}

fn full_of_id(s: &Store, i: MsgId) -> String {
    speech::full(&project::message(s, i).unwrap(), &cx())
}

#[test]
fn readings() {
    let (mut s, t) = setup();
    assert_eq!(full_of(&s), "语音 3 秒，还没转文字");
    assert!(!speech::detail(&doc(&s), &cx()).contains("已转文字"));
    let brief_before = speech::brief(&doc(&s), &cx());
    // 片段间补逗号：前一段末尾已有标点就不补；空白片段跳过
    commit(&mut s, t, &["你好", "  ", "这是编的。", "再见"]);
    assert_eq!(full_of(&s), "语音 3 秒，你好，这是编的。再见");
    assert!(speech::detail(&doc(&s), &cx()).ends_with("已转文字"));
    assert_eq!(speech::brief(&doc(&s), &cx()), brief_before, "名称不变");
}

#[test]
fn json_has_transcript_only_when_present() {
    let (mut s, t) = setup();
    let before = json::value(&doc(&s), &cx());
    assert!(before["data"].get("transcript").is_none(), "没有转写不输出");
    commit(&mut s, t, &["编的"]);
    let after = json::value(&doc(&s), &cx());
    assert_eq!(
        before["v"], 1,
        "v 不加一：新增可选字段（固定值，不和同一个序列化器自己比）"
    );
    assert_eq!(after["v"], 1);
    let j = &after["data"]["transcript"];
    assert_eq!(j["lang"], "zh-CN");
    assert_eq!(j["source"], "local");
    assert_eq!(j["segments"][0]["text"], "编的");
    assert_eq!(j["segments"][0]["start_s"], 0.0);
}

/// 18+ 遮挡的语音：没展开时转写既不读、也不进 JSON；展开了照常
#[test]
fn sensitive_voice_hides_transcript_until_revealed() {
    // 18+ 标记在消息上（`restrictionInfo.has_sensitive_content`），store 写入时落到内容的 `sensitive`
    let mut m = msg(CHAT, 5, voice());
    m.sensitive = true;
    let mut s = feed([new(m)]);
    let t = s.transcript_ticket(V).unwrap();
    commit(&mut s, t, &["编的暗句"]);
    assert!(!full_of(&s).contains("编的暗句"));
    assert!(
        !speech::detail(&doc(&s), &cx()).contains("已转文字"),
        "遮着时连有没有转写都不说"
    );
    // 整个字段不输出（不只是没有原文：lang、source、空的 segments 也不许留）
    assert!(
        json::value(&doc(&s), &cx())["data"]
            .get("transcript")
            .is_none()
    );
    assert!(!json::line(&doc(&s), &cx()).contains("transcript"));
    let open = SpeechCtx {
        reveal: true,
        ..cx()
    };
    assert!(speech::full(&doc(&s), &open).contains("编的暗句"));
    assert!(speech::detail(&doc(&s), &open).ends_with("已转文字"));
    assert!(json::line(&doc(&s), &open).contains("编的暗句"));
}

/// 会话项的最后一条（`chat_item.last`）是同一份消息投影：18+ 遮着时转写同样整个不输出，展开了才有
#[test]
fn chat_item_last_hides_sensitive_transcript() {
    let mut m = msg(CHAT, 5, voice());
    m.sensitive = true;
    let mut s = feed([CoreEvent::ChatAdded {
        chat: chat(CHAT, 1),
        last_message: Some(m),
    }]);
    let t = s.transcript_ticket(V).expect("前提：最后一条进了 store");
    commit(&mut s, t, &["编的暗句"]);
    let items = project::chat_list(&s, &ChatList::Main);
    assert_eq!(items.len(), 1);
    let hidden = json::line(&items[0], &cx());
    assert!(!hidden.contains("transcript") && !hidden.contains("编的暗句"));
    let open = SpeechCtx {
        reveal: true,
        ..cx()
    };
    assert!(
        json::line(&items[0], &open).contains("编的暗句"),
        "前提：会话项确实带转写"
    );
}
