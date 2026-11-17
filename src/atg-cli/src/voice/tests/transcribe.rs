//! `transcribe` 的组合动作（#59 3c）：假后端 + 假辅助程序（`atg-audio/fixtures/stt/*.sh`）+ 入库的合成语音。内容全是编的。
//! 撤回、内容替换、换 id 都经假后端的事件流进来（不直接调 reducer），落在「转写中」那段时间里：
//! 辅助程序收完音频后停在闸门前（[`Gated`]），更新推进去、确认已进事件流（`FakeBackend::settle`）才放行，不靠时序。
use super::*;
use crate::voice::transcribe::{SttOpts, transcribe};
use atg_core::tree::{Outcome, Transcript, Transcription};
use std::time::Duration;

fn helper(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../atg-audio/fixtures/stt/{name}.sh"))
}

fn opts(name: &str) -> SttOpts {
    opts_at(helper(name))
}

fn opts_at(helper: PathBuf) -> SttOpts {
    SttOpts {
        helper,
        locales: vec!["zh-CN".into(), "en-US".into()],
    }
}

/// 带闸门的假辅助程序（每条测试一份）：收完音频先留个 `started`，等到 `go` 出现才出结果（同 `ok-zh`）
struct Gated {
    dir: PathBuf,
}

impl Gated {
    fn new(tag: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("atg-stt-gate-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let d = dir.display();
        let script = format!(
            "#!/bin/sh\necho '{{\"kind\":\"hello\",\"proto\":1}}'\ncat > /dev/null\ntouch '{d}/started'\n\
             while [ ! -e '{d}/go' ]; do sleep 0.01; done\n\
             echo '{{\"kind\":\"locale\",\"locale\":\"zh-CN\",\"status\":\"text\",\"conf\":0.9,\"segments\":[{{\"text\":\"今天下午开会\",\"start_s\":0.0,\"end_s\":1.5}}]}}'\n\
             echo '{{\"kind\":\"locale\",\"locale\":\"en-US\",\"status\":\"empty\"}}'\n\
             echo '{{\"kind\":\"done\",\"count\":2}}'\n"
        );
        let h = dir.join("helper.sh");
        std::fs::write(&h, script).unwrap();
        std::fs::set_permissions(&h, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self { dir }
    }

    fn opts(&self) -> SttOpts {
        opts_at(self.dir.join("helper.sh"))
    }

    /// 另起线程：等辅助程序收完音频（下载、解码都已完），推更新、等它进了事件流，再放辅助程序出结果
    fn push_while_transcribing(
        &self,
        b: &Arc<FakeBackend>,
        update: Value,
    ) -> std::thread::JoinHandle<()> {
        let (b, dir) = (b.clone(), self.dir.clone());
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !dir.join("started").exists() {
                assert!(Instant::now() < deadline, "辅助程序没起来");
                std::thread::sleep(Duration::from_millis(5));
            }
            b.push(update);
            b.settle();
            std::fs::write(dir.join("go"), "").unwrap();
        })
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
    go_with(b, tag, id, &opts(name), cancel)
}

fn go_with(
    b: &FakeBackend,
    tag: &str,
    id: i64,
    o: &SttOpts,
    cancel: &AtomicBool,
) -> (Result<Transcription, Fail>, Option<Transcript>) {
    let p = profile(tag);
    let mut kept = None;
    let r = history::with_backend(b, &p, Instant::now(), |b, stream, feed| {
        let r = transcribe(b, stream, feed, &q(id), o, cancel);
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
    let g = Gated::new("t-revoked");
    let h = g.push_while_transcribing(&b, up);
    let (r, _) = go_with(&b, "t-revoked", 1, &g.opts(), &NEVER);
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
    let g = Gated::new("t-replaced");
    let h = g.push_while_transcribing(&b, up);
    let (r, _) = go_with(&b, "t-replaced", 1, &g.opts(), &NEVER);
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
    let g = Gated::new("t-renamed");
    let h = g.push_while_transcribing(&b, up);
    let (r, kept) = go_with(&b, "t-renamed", 1, &g.opts(), &NEVER);
    h.join().unwrap();
    let t = r.unwrap_or_else(|f| panic!("{}", err(&Err(f))));
    assert_eq!(t.id.message_id, 7);
    assert!(kept.is_some());
}

/// 转写中被标为 18+（换 id 时带来的新整条）→ 照转、照写表，节点 `sensitive`；不加 `--reveal` 时 JSON 与文字都不带字
#[test]
fn marked_sensitive_while_transcribing_is_hidden() {
    let b = Arc::new(backend());
    b.set_file(FILE, Some(real_ogg()));
    let mut m = msg(7, voice(FILE));
    m["is_outgoing"] = json!(true);
    m["restriction_info"] = json!({"@type": "restrictionInfo", "has_sensitive_content": true});
    let up = json!({"@type": "updateMessageSendSucceeded", "old_message_id": 1, "message": m});
    let g = Gated::new("t-sensitive");
    let h = g.push_while_transcribing(&b, up);
    let (r, kept) = go_with(&b, "t-sensitive", 1, &g.opts(), &NEVER);
    h.join().unwrap();
    let t = r.unwrap_or_else(|f| panic!("{}", err(&Err(f))));
    assert!(t.sensitive && t.hidden(false) && !t.hidden(true));
    assert!(kept.is_some(), "照写表");
    let cx = history::speech_ctx(Some("+08:00"), false).unwrap_or_else(|_| panic!("时区"));
    let doc = atg_core::tree::Doc::Transcription(t);
    let line = atg_core::json::value(&doc, &cx).to_string();
    let said = atg_core::speech::brief(&doc, &cx);
    for out in [&line, &said] {
        assert!(!out.contains("今天下午开会"), "不带文字：{out}");
    }
    assert_eq!(
        atg_core::json::value(&doc, &cx)["data"]["outcome"]["type"],
        "hidden"
    );
}

/// 租约守卫与别的使用者共用一张表（design 第二节第 3 条的三种情形）：两项转写先结束一项不关；
/// 别人（`watch` 订阅者）仍开着时转写结束不关；panic 展开也放回。只有最后一份放手才真关
#[test]
fn guard_shares_the_table_and_only_last_release_closes() {
    use crate::lease::{self, Guard};
    let b = backend();
    let p = profile("t-guard");
    let r = history::with_backend(&b, &p, Instant::now(), |b, stream, feed| {
        // 先像 `transcribe` 那样认会话：等到授权就绪，之后的开关才不会被拒
        crate::voice::chat(b, stream, feed, &q(1))?;
        let leases = feed.hub.leases().clone();
        let n = |c: Call| b.calls().iter().filter(|x| **x == c).count();
        let (open, close) = (Call::ChatOpen(GROUP), Call::ChatClose(GROUP));
        // 两项转写
        let g1 = Guard::take(b, leases.clone(), GROUP, "t").map_err(Fail::Core)?;
        let g2 = Guard::take(b, leases.clone(), GROUP, "t").map_err(Fail::Core)?;
        assert_eq!(n(open.clone()), 1, "第二份不再开");
        drop(g1);
        assert_eq!(n(close.clone()), 0, "先结束的一项不关");
        drop(g2);
        assert_eq!(n(close.clone()), 1, "最后一份关");
        // 订阅者先开着，转写结束不关；订阅者放手才关
        lease::open(b, &leases, GROUP).map_err(Fail::Core)?;
        drop(Guard::take(b, leases.clone(), GROUP, "t").map_err(Fail::Core)?);
        assert_eq!((n(open.clone()), n(close.clone())), (2, 1));
        assert!(leases.give(GROUP));
        lease::close(b, vec![GROUP], "t", &mut crate::out::Stdio);
        assert_eq!(n(close.clone()), 2);
        // panic 展开：守卫照样放回、真关
        let l2 = leases.clone();
        let hit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = Guard::take(b, l2, GROUP, "t");
            panic!("转写途中出错");
        }));
        assert!(hit.is_err());
        assert_eq!((n(open), n(close)), (3, 3));
        assert_eq!(leases.held(GROUP), 0);
        Ok(())
    });
    if let Err(f) = r {
        panic!("{}", err(&Err(f)));
    }
}
