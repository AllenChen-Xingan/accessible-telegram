use super::*;
use atg_core::backend::Downloaded;
use atg_core::config::ServerKind;
use atg_core::fake::{Call, FakeAuth, FakeBackend};
use serde_json::{Value, json};
use std::path::PathBuf;

/// 这些测试不按 Ctrl+C
static NEVER: AtomicBool = AtomicBool::new(false);

const ME: i64 = 100;
const LIN: i64 = 101;
const GROUP: i64 = -1001;
const T0: i64 = 1_793_014_200;
/// 编的文件 id
const FILE: i32 = 42;

fn profile(tag: &str) -> Profile {
    let d = std::env::temp_dir().join(format!(
        "atg-cli-voice-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    Profile::create(&d, "main", ServerKind::Main).unwrap()
}

fn voice(file_id: i32) -> Value {
    json!({"@type": "messageVoiceNote", "is_listened": false,
        "caption": {"@type": "formattedText", "text": "", "entities": []},
        "voice_note": {"@type": "voiceNote", "duration": 12, "mime_type": "audio/ogg",
            "voice": {"@type": "file", "id": file_id, "size": 20480}}})
}

fn msg(id: i64, content: Value) -> Value {
    json!({"@type": "message", "id": id, "chat_id": GROUP,
        "sender_id": {"@type": "messageSenderUser", "user_id": LIN},
        "is_outgoing": false, "is_pinned": false, "date": T0 + id * 60, "edit_date": 0,
        "media_album_id": "0", "content": content})
}

fn backend() -> FakeBackend {
    let b = FakeBackend::new().unwrap();
    b.set_auth(FakeAuth {
        logged_in: true,
        ..FakeAuth::default()
    });
    b.push_net("connectionStateReady");
    b.set_chat(
        GROUP,
        vec![
            json!({"@type": "updateOption", "name": "my_id", "value": {"@type": "optionValueInteger", "value": ME.to_string()}}),
            json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": GROUP, "title": "周末爬山群",
                "type": {"@type": "chatTypeSupergroup", "supergroup_id": 1, "is_channel": false},
                "positions": [], "unread_count": 0, "unread_mention_count": 0,
                "last_read_inbox_message_id": 0, "last_read_outbox_message_id": 0,
                "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0}}}),
        ],
    );
    b.set_history(
        GROUP,
        vec![
            msg(1, voice(FILE)),
            msg(
                2,
                json!({"@type": "messageText", "text": {"@type": "formattedText", "text": "好的", "entities": []}}),
            ),
            msg(3, voice(0)),
        ],
    );
    b
}

fn q(message_id: i64) -> VoiceQuery {
    VoiceQuery {
        chat: ChatRef::Id(GROUP),
        message_id,
    }
}

fn downloaded() -> Downloaded {
    Downloaded {
        path: PathBuf::from("/编的/voice/file_42.oga"),
        size: 20480,
    }
}

fn code(r: Result<VoiceFile, Fail>) -> String {
    match r {
        Err(Fail::Core(e)) => e.code().to_owned(),
        Err(_) => "other".into(),
        Ok(_) => "ok".into(),
    }
}

#[test]
fn downloads_the_voice_file() {
    let b = backend();
    b.set_file(FILE, Some(downloaded()));
    let v = run_download(&b, &profile("ok"), &q(1), &NEVER, Instant::now())
        .unwrap_or_else(|_| panic!("没下到"));
    assert_eq!(v.id.message_id, 1);
    assert_eq!(v.duration, 12);
    assert_eq!(v.file, downloaded());
    // 只读 + 下载：不标已听（还没有 mark_listened 调用）、不开会话
    for c in b.calls() {
        assert!(
            matches!(
                c,
                Call::Start { .. }
                    | Call::Subscribe
                    | Call::Shutdown
                    | Call::ChatGet(_)
                    | Call::MessageGet(_)
                    | Call::FileDownload(FILE)
            ),
            "多了别的调用"
        );
    }
}

#[test]
fn not_a_voice_is_invalid_input_and_downloads_nothing() {
    let b = backend();
    b.set_file(FILE, Some(downloaded()));
    assert_eq!(
        code(run_download(
            &b,
            &profile("text"),
            &q(2),
            &NEVER,
            Instant::now()
        )),
        "invalid_input"
    );
    assert!(!b.calls().iter().any(|c| matches!(c, Call::FileDownload(_))));
}

#[test]
fn unfinished_download_is_an_error_not_half_a_file() {
    let b = backend();
    b.set_file(FILE, None);
    assert_eq!(
        code(run_download(
            &b,
            &profile("half"),
            &q(1),
            &NEVER,
            Instant::now()
        )),
        "internal"
    );
}

#[test]
fn missing_file_and_missing_message_are_not_found() {
    let b = backend();
    // 第 3 条的文件 id 是 0（TDLib 没给）；假后端没登记 → 找不到
    assert_eq!(
        code(run_download(
            &b,
            &profile("nofile"),
            &q(3),
            &NEVER,
            Instant::now()
        )),
        "not_found"
    );
    let b = backend();
    assert_eq!(
        code(run_download(
            &b,
            &profile("nomsg"),
            &q(99),
            &NEVER,
            Instant::now()
        )),
        "not_found"
    );
}

/// 编的合成语音夹具（atg-audio 的）：解码能过
fn real_ogg() -> Downloaded {
    Downloaded {
        path: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../atg-audio/fixtures/zh.ogg"),
        size: 4868,
    }
}

/// 假出口：把打开 / 释放记进共用的日志；可以让打开失败，或在打开时立起取消标志（模拟准备途中按了 Ctrl+C）
#[derive(Clone, Default)]
struct Rec {
    log: Arc<std::sync::Mutex<Vec<String>>>,
    fail_start: bool,
    cancel_on_start: Option<Arc<AtomicBool>>,
    fail_resume: bool,
    /// 恢复之后出口自报失效（模拟播放中设备没了）
    breaks_after_resume: bool,
    resumed: bool,
}
impl Rec {
    fn new(fail_start: bool, cancel_on_start: Option<Arc<AtomicBool>>) -> Self {
        Rec {
            log: Arc::default(),
            fail_start,
            cancel_on_start,
            fail_resume: false,
            breaks_after_resume: false,
            resumed: false,
        }
    }
    fn failing_resume(mut self) -> Self {
        self.fail_resume = true;
        self
    }
    fn breaking_after_resume(mut self) -> Self {
        self.breaks_after_resume = true;
        self
    }
    fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}
impl Drop for Rec {
    fn drop(&mut self) {
        self.log.lock().unwrap().push("drop".into());
    }
}
impl atg_audio::player::Output for Rec {
    fn start(
        &mut self,
        _: &Arc<atg_audio::Pcm>,
        _: usize,
        _: f64,
        paused: bool,
    ) -> Result<(), AudioError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("start paused={paused}"));
        if let Some(c) = &self.cancel_on_start {
            c.store(true, Ordering::SeqCst);
        }
        if self.fail_start {
            Err(AudioError::Output)
        } else {
            Ok(())
        }
    }
    fn pause(&mut self) -> Result<(), AudioError> {
        Ok(())
    }
    fn resume(&mut self) -> Result<(), AudioError> {
        self.log.lock().unwrap().push("resume".into());
        if self.fail_resume {
            return Err(AudioError::Output);
        }
        self.resumed = true;
        Ok(())
    }
    fn set_speed(&mut self, _: f64) -> Result<(), AudioError> {
        Ok(())
    }
    fn failed(&self) -> bool {
        self.breaks_after_resume && self.resumed
    }
    fn stop(&mut self) {
        self.log.lock().unwrap().push("stop".into());
    }
    fn played(&self) -> usize {
        0
    }
}

fn prep_with(
    b: &FakeBackend,
    tag: &str,
    id: i64,
    mark: bool,
    out: Rec,
    cancel: &AtomicBool,
) -> Result<VoiceFile, Fail> {
    history::with_backend(b, &profile(tag), Instant::now(), |b, s, f| {
        prepare_play(b, s, f, &q(id), mark, out, cancel).map(|(p, v)| {
            assert_eq!(p.status().state, atg_audio::State::Paused, "准备完是停着的");
            v
        })
    })
}

fn prep(b: &FakeBackend, tag: &str, id: i64, mark: bool) -> Result<VoiceFile, Fail> {
    prep_with(b, tag, id, mark, Rec::default(), &AtomicBool::new(false))
}

fn marked(b: &FakeBackend) -> bool {
    b.calls().iter().any(|c| matches!(c, Call::MarkListened(_)))
}

#[test]
fn play_prepare_marks_only_when_asked() {
    let b = backend();
    b.set_file(FILE, Some(real_ogg()));
    assert!(prep(&b, "nomark", 1, false).is_ok());
    assert!(!marked(&b), "默认不标已听");
    let b = backend();
    b.set_file(FILE, Some(real_ogg()));
    assert!(prep(&b, "mark", 1, true).is_ok());
    let calls = b.calls();
    let dl = calls
        .iter()
        .position(|c| matches!(c, Call::FileDownload(_)));
    let mk = calls
        .iter()
        .position(|c| matches!(c, Call::MarkListened(_)));
    assert!(dl < mk && mk.is_some(), "下载、解码、出口都好了才标");
}

#[test]
fn play_prepare_failures_never_mark() {
    // 解码失败（文件不是音频）→ 不标
    let b = backend();
    b.set_file(
        FILE,
        Some(Downloaded {
            path: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
            size: 1,
        }),
    );
    assert_eq!(code(prep(&b, "baddecode", 1, true)), "internal");
    assert!(!marked(&b));
    // 出口打开失败 → 不标（证明「出口好了才标」），报 audio_output_failed
    let b = backend();
    b.set_file(FILE, Some(real_ogg()));
    let out = Rec::new(true, None);
    let r = prep_with(&b, "outfail", 1, true, out.clone(), &AtomicBool::new(false));
    assert!(
        matches!(&r, Err(Fail::Core(CoreError::Internal { what })) if *what == "audio_output_failed")
    );
    assert!(!marked(&b));
    // 标记本身失败 → 报标记的错、出口已释放、没出声
    let b = backend();
    b.set_file(FILE, Some(real_ogg()));
    b.fail_next("mark_listened", CoreError::Forbidden { td: None });
    let out = Rec::default();
    assert_eq!(
        code(prep_with(
            &b,
            "markfail",
            1,
            true,
            out.clone(),
            &AtomicBool::new(false)
        )),
        "forbidden"
    );
    let log = out.log();
    assert_eq!(log.first().map(String::as_str), Some("start paused=true"));
    assert!(log.iter().any(|l| l == "drop"), "出口已释放");
    assert!(!log.iter().any(|l| l == "resume"), "没出声");
}

/// 输出收集（JSON 行）
#[derive(Default)]
struct Lines(Vec<String>);
impl crate::out::Sink for Lines {
    fn out(&mut self, l: &str) {
        self.0.push(l.to_owned());
    }
    fn err(&mut self, _: &str) {}
}

/// 从准备一路接到放、到结尾行：标了已听之后出口出事，结尾只有一行 error、注明已标、出口已停
fn mark_then_fail(tag: &str, out: Rec) -> (Vec<String>, Vec<String>, FakeBackend) {
    let b = backend();
    b.set_file(FILE, Some(real_ogg()));
    let no = AtomicBool::new(false);
    let (player, v) = history::with_backend(&b, &profile(tag), Instant::now(), |b, s, f| {
        prepare_play(b, s, f, &q(1), true, out.clone(), &no)
    })
    .unwrap_or_else(|_| panic!("准备失败"));
    let cx = history::speech_ctx(Some("+08:00"), false).unwrap_or_else(|_| panic!("时区"));
    let (_tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    let mut sink = Lines::default();
    let end = crate::play::drive(&mut sink, crate::args::Format::Json, &cx, player, &rx, &no)
        .unwrap_or_else(|_| panic!("不该中断"));
    assert_eq!(end, atg_audio::End::OutputFailed);
    crate::play::finish(
        &mut sink,
        crate::args::Format::Json,
        end,
        &v,
        true,
        Instant::now(),
    );
    (sink.0, out.log(), b)
}

#[test]
fn marked_then_resume_fails_end_to_end() {
    let (lines, log, b) = mark_then_fail("e2e-resume", Rec::new(false, None).failing_resume());
    assert!(marked(&b));
    assert_eq!(lines.len(), 1, "只有一行结尾");
    let l: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(l["kind"], "error");
    assert_eq!(l["data"]["what"], "audio_output_failed");
    assert_eq!(l["data"]["marked_listened"], true);
    assert!(log.iter().any(|x| x == "stop"), "出口已释放");
}

#[test]
fn marked_then_output_breaks_while_playing_end_to_end() {
    let (lines, log, b) =
        mark_then_fail("e2e-broken", Rec::new(false, None).breaking_after_resume());
    assert!(marked(&b));
    // 先一行开始播放的状态，再一行结尾 error
    assert_eq!(lines.len(), 2);
    let last: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
    assert_eq!(last["kind"], "error");
    assert_eq!(last["data"]["marked_listened"], true);
    assert!(log.iter().any(|x| x == "stop"));
}

#[test]
fn cancel_wins_over_a_failing_step() {
    // 打开出口时按了 Ctrl+C，而这一步本身也失败了：仍报 interrupted、不标
    let b = backend();
    b.set_file(FILE, Some(real_ogg()));
    let cancel = Arc::new(AtomicBool::new(false));
    let out = Rec::new(true, Some(cancel.clone()));
    let r = prep_with(&b, "cancelfail", 1, true, out, &cancel);
    assert!(matches!(r, Err(Fail::Interrupted)));
    assert!(!marked(&b));
}

#[test]
fn cancel_during_prepare_never_marks() {
    // 一开始就取消：不下载、不标
    let b = backend();
    b.set_file(FILE, Some(real_ogg()));
    let r = prep_with(
        &b,
        "cancel0",
        1,
        true,
        Rec::default(),
        &AtomicBool::new(true),
    );
    assert!(matches!(r, Err(Fail::Interrupted)));
    assert!(!b.calls().iter().any(|c| matches!(c, Call::FileDownload(_))));
    assert!(!marked(&b));
    // 打开出口时（下载、解码之后、标之前）按了 Ctrl+C：不标、不出声、出口释放
    let b = backend();
    b.set_file(FILE, Some(real_ogg()));
    let cancel = Arc::new(AtomicBool::new(false));
    let out = Rec::new(false, Some(cancel.clone()));
    let r = prep_with(&b, "cancel1", 1, true, out.clone(), &cancel);
    assert!(matches!(r, Err(Fail::Interrupted)));
    assert!(!marked(&b), "取消后不标已听");
    assert!(!out.log().iter().any(|l| l == "resume"));
    assert!(out.log().iter().any(|l| l == "drop"));
}

#[test]
fn self_destruct_with_mark_is_refused_before_download() {
    let b = backend();
    let mut m = msg(4, voice(FILE));
    m["self_destruct_type"] = json!({"@type": "messageSelfDestructTypeImmediately"});
    b.set_history(GROUP, vec![m]);
    b.set_file(FILE, Some(real_ogg()));
    assert!(matches!(
        prep(&b, "sd", 4, true),
        Err(Fail::NotImplemented { .. })
    ));
    assert!(!b.calls().iter().any(|c| matches!(c, Call::FileDownload(_))));
    assert!(!marked(&b));
    // 不标就照常能听
    let b = backend();
    let mut m = msg(4, voice(FILE));
    m["self_destruct_type"] = json!({"@type": "messageSelfDestructTypeImmediately"});
    b.set_history(GROUP, vec![m]);
    b.set_file(FILE, Some(real_ogg()));
    assert!(prep(&b, "sd2", 4, false).is_ok());
}

#[test]
fn bad_chat_is_usage() {
    let m = crate::args::MsgRef {
        chat: "群".into(),
        message: 1,
    };
    assert!(matches!(query(&m), Err(Fail::Usage(_))));
}

/// 下载一直不应答（#78）：调用方立起停止标志（`play` 的 Ctrl+C、以后 GUI 的取消）之后很快以 `interrupted` 结束，
/// 后端照常关掉，不用等 `DOWNLOAD_TIMEOUT`。`download` 命令自己不装信号处理，传的是永远立不起来的标志
#[test]
fn ctrl_c_while_download_hangs_is_interrupted() {
    let b = backend();
    b.set_file(FILE, Some(downloaded()));
    b.fail_next(
        "file_download",
        CoreError::Internal {
            what: FakeBackend::HANG,
        },
    );
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    let h = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        s.store(true, Ordering::SeqCst);
    });
    let t0 = Instant::now();
    let r = run_download(&b, &profile("hang"), &q(1), &stop, t0);
    h.join().unwrap();
    assert!(matches!(r, Err(Fail::Interrupted)));
    assert!(t0.elapsed() < crate::deadlines::QUERY_TIMEOUT);
    assert!(
        b.calls()
            .iter()
            .any(|c| matches!(c, Call::FileDownload(FILE)))
    );
    assert!(matches!(b.calls().last(), Some(Call::Shutdown)));
}
