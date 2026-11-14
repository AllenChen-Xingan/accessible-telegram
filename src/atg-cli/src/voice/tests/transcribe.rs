//! `transcribe` 的组合动作（#59 3c）：假后端 + 假辅助程序（`atg-audio/fixtures/stt/*.sh`）+ 入库的合成语音。内容全是编的。
//! 撤回、内容替换、换 id 都经假后端的事件流进来（不直接调 reducer），落在「转写中」那段时间里。
use super::*;
use crate::voice::transcribe::{SttOpts, transcribe};
use atg_core::tree::{Outcome, Transcript, Transcription};
use std::time::Duration;

fn helper(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../atg-audio/fixtures/stt/{name}.sh"))
}

fn opts(name: &str) -> SttOpts {
    SttOpts {
        helper: helper(name),
        locales: vec!["zh-CN".into(), "en-US".into()],
    }
}

/// 转第 `id` 条；回（结果，提交后 store 里这条——现在的 id——的转写）
fn go(
    b: &FakeBackend,
    tag: &str,
    id: i64,
    name: &str,
    cancel: &AtomicBool,
) -> (Result<Transcription, Fail>, Option<Transcript>) {
    let p = profile(tag);
    let mut kept = None;
    let r = history::with_backend(b, &p, Instant::now(), |b, stream, feed| {
        let r = transcribe(b, stream, feed, &q(id), &opts(name), cancel);
        if let Ok(t) = &r {
            kept = feed.store().transcript(t.id).cloned();
        }
        r
    });
    (r, kept)
}

fn err(r: &Result<Transcription, Fail>) -> String {
    match r {
        Err(Fail::Core(e)) => {
            let what = match e {
                CoreError::Internal { what }
                | CoreError::NotFound { what, .. }
                | CoreError::SttUnavailable { what } => format!(":{what}"),
                _ => String::new(),
            };
            format!("{}{what}", e.code())
        }
        Err(Fail::Interrupted) => "interrupted".into(),
        Err(_) => "other".into(),
        Ok(_) => "ok".into(),
    }
}

/// 会话恰好开了一次、关了一次（租约在所有退出路径上都放）
fn opened_and_closed_once(b: &FakeBackend) {
    let calls = b.calls();
    let n = |c: Call| calls.iter().filter(|x| **x == c).count();
    assert_eq!(
        (n(Call::ChatOpen(GROUP)), n(Call::ChatClose(GROUP))),
        (1, 1),
        "{calls:?}"
    );
    let open = calls.iter().position(|c| *c == Call::ChatOpen(GROUP));
    let get = calls.iter().position(|c| matches!(c, Call::MessageGet(_)));
    assert!(open < get, "取消息之前就要打开会话：{calls:?}");
}

/// 等到假后端收到下载请求，再推一条更新（落在下载之后、提交之前）
fn push_after_download(b: &Arc<FakeBackend>, update: Value) -> std::thread::JoinHandle<()> {
    let b = b.clone();
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !b.calls().contains(&Call::FileDownload(FILE)) {
            assert!(Instant::now() < deadline, "没等到下载");
            std::thread::sleep(Duration::from_millis(5));
        }
        b.push(update);
    })
}

#[test]
fn transcribes_writes_table_and_releases_lease() {
    let b = backend();
    b.set_file(FILE, Some(real_ogg()));
    let (r, kept) = go(&b, "t-ok", 1, "ok-zh", &NEVER);
    let t = r.unwrap_or_else(|_| panic!("{}", "转写失败"));
    assert_eq!(
        t.id,
        MsgId {
            chat_id: GROUP,
            message_id: 1
        }
    );
    let Outcome::Text(tr) = &t.outcome else {
        panic!("应有字");
    };
    assert_eq!(tr.lang, "zh-CN");
    assert_eq!(tr.segments[0].text, "今天下午开会");
    assert_eq!(kept.as_ref(), Some(tr), "写进了转写表");
    assert!(!t.sensitive);
    opened_and_closed_once(&b);
    assert!(
        !b.calls().iter().any(|c| matches!(c, Call::MarkListened(_))),
        "不标已听"
    );
}

#[test]
fn no_speech_is_success_and_not_written() {
    let b = backend();
    b.set_file(FILE, Some(real_ogg()));
    let (r, kept) = go(&b, "t-empty", 1, "both-empty", &NEVER);
    assert!(matches!(r.map(|t| t.outcome), Ok(Outcome::NoSpeech)));
    assert!(kept.is_none());
    opened_and_closed_once(&b);
}

#[test]
fn failures_map_to_codes_and_release_lease() {
    for (helper, want) in [
        ("both-unavailable", "stt_unavailable:unavailable"),
        // 版本不对也是「不可用」（人要重跑 build.sh）
        ("bad-proto", "stt_unavailable:unavailable"),
        ("garbage", "internal:stt_helper_failed"),
    ] {
        let b = backend();
        b.set_file(FILE, Some(real_ogg()));
        let (r, _) = go(&b, &format!("t-{helper}"), 1, helper, &NEVER);
        assert_eq!(err(&r), want);
        opened_and_closed_once(&b);
    }
    // 不是语音：不下载、照样放租约
    let b = backend();
    let (r, _) = go(&b, "t-text", 2, "ok-zh", &NEVER);
    assert_eq!(err(&r), "invalid_input");
    assert!(!b.calls().iter().any(|c| matches!(c, Call::FileDownload(_))));
    opened_and_closed_once(&b);
}

#[test]
fn ctrl_c_during_transcription_is_interrupted_and_releases_lease() {
    let b = backend();
    b.set_file(FILE, Some(real_ogg()));
    let cancel = Arc::new(AtomicBool::new(false));
    let c = cancel.clone();
    let bb = Arc::new(b);
    let b2 = bb.clone();
    let flip = std::thread::spawn(move || {
        while !b2.calls().contains(&Call::FileDownload(FILE)) {
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(200));
        c.store(true, Ordering::SeqCst);
    });
    let (r, _) = go(&bb, "t-cancel", 1, "hang", &cancel);
    flip.join().unwrap();
    assert_eq!(err(&r), "interrupted");
    opened_and_closed_once(&bb);
}

/// 转写中被撤回 → 作废，`not_found{message}`，不写表
#[test]
fn revoked_while_transcribing_is_not_found() {
    let b = Arc::new(backend());
    b.set_file(FILE, Some(real_ogg()));
    let up = json!({"@type": "updateDeleteMessages", "chat_id": GROUP, "message_ids": [1],
        "is_permanent": true, "from_cache": false});
    let h = push_after_download(&b, up);
    let (r, _) = go(&b, "t-revoked", 1, "ok-zh-slow", &NEVER);
    h.join().unwrap();
    assert_eq!(err(&r), "not_found:message");
    opened_and_closed_once(&b);
}

/// 转写中内容换成了另一段语音 → 作废，`internal{transcript_stale}`
#[test]
fn content_replaced_while_transcribing_is_stale() {
    let b = Arc::new(backend());
    b.set_file(FILE, Some(real_ogg()));
    let up = json!({"@type": "updateMessageContent", "chat_id": GROUP, "message_id": 1,
        "new_content": voice(FILE + 1)});
    let h = push_after_download(&b, up);
    let (r, _) = go(&b, "t-replaced", 1, "ok-zh-slow", &NEVER);
    h.join().unwrap();
    assert_eq!(err(&r), "internal:transcript_stale");
    opened_and_closed_once(&b);
}

/// 转写中换了 id（同一个音频文件）→ 接受，节点用新 id，表挂在新 id 上
#[test]
fn renamed_while_transcribing_uses_new_id() {
    let b = Arc::new(backend());
    b.set_file(FILE, Some(real_ogg()));
    let mut m = msg(7, voice(FILE));
    m["is_outgoing"] = json!(true);
    let up = json!({"@type": "updateMessageSendSucceeded", "old_message_id": 1, "message": m});
    let h = push_after_download(&b, up);
    let (r, kept) = go(&b, "t-renamed", 1, "ok-zh-slow", &NEVER);
    h.join().unwrap();
    let t = r.unwrap_or_else(|f| panic!("{}", err(&Err(f))));
    assert_eq!(t.id.message_id, 7);
    assert!(kept.is_some());
}
