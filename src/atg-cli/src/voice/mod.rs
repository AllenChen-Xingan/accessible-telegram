//! 语音的组合动作（#4、#59）：找到这条语音 → 下载 → 解码、放（以后：转写）。
//!
//! 落层（arch 定 2026-11-13，arch 分支 `docs/plan/010_complexity-budget.md` 第四节）：这里是**库代码**——吃
//! `Backend` 和参数，吐结果；不碰 clap、不写标准输出，渲染交给调用方。以后 GUI 链接 `atg-cli` 的库目标直接用。
//! 设计：`docs/experiments/exp008-local-stt/design.md`。
//!
//! 下载是按需的（大总管定 2026-11-13）：不自动下，`download` / `play` / `transcribe` 要用时才下。

use crate::chats::{self, Feed};
use crate::history::{self, ChatRef};
use crate::out::Fail;
use atg_audio::player::Output;
use atg_audio::{AudioError, Player};
use atg_core::backend::{Backend, Downloaded};
use atg_core::config::Profile;
use atg_core::error::CoreError;
use atg_core::link::{EventStream, block_on};
use atg_core::model::{Body, MsgId};
use atg_core::tree::{Attachment, Content, FileRef};
use std::sync::Arc;
use std::time::Instant;

/// 要哪条语音（命令行参数解析之后的样子）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceQuery {
    pub chat: ChatRef,
    pub message_id: i64,
}

/// 下好的语音：身份、时长（秒，TDLib 给的）、本地文件、是不是阅后即焚
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceFile {
    pub id: MsgId,
    pub duration: u32,
    pub file: Downloaded,
    pub self_destruct: bool,
}

/// 命令行的「会话 + 消息 id」→ 查询（`download`、`play` 共用这一份）
pub fn query(m: &crate::args::MsgRef) -> Result<VoiceQuery, Fail> {
    Ok(VoiceQuery {
        chat: ChatRef::parse(&m.chat).ok_or(Fail::Usage("会话写数字 id，或 saved（收藏夹）"))?,
        message_id: m.message,
    })
}

/// 直跑：起后端、找到并下载这条语音、关后端
pub fn run_download<B: Backend>(
    b: &B,
    profile: &Profile,
    q: &VoiceQuery,
    started: Instant,
) -> Result<VoiceFile, Fail> {
    history::with_backend(b, profile, started, |b, stream, feed| {
        fetch(b, stream, feed, q)
    })
}

/// 找到的语音（还没下载）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    pub id: MsgId,
    pub duration: u32,
    pub file: FileRef,
    pub self_destruct: bool,
}

/// 在已起好的后端上：取这条消息（`getMessage`，不标已读）、确认是语音。不是语音 → `invalid_input`
pub fn find<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    q: &VoiceQuery,
) -> Result<Found, Fail> {
    chats::ensure_ready(b)?;
    let chat_id = history::resolve(b, stream, feed, q.chat)?;
    history::load_chat(b, stream, feed, chat_id)?;
    let id = MsgId {
        chat_id,
        message_id: q.message_id,
    };
    let m = block_on(b.message_get(id)).map_err(Fail::Core)?;
    feed.drain(stream);
    // 有效内容（有临时内容就用它）；受限的存下来时已换成受限节点，不是语音
    match m.effective() {
        Body::Content(Content {
            attachment: Some(Attachment::Voice { duration, file, .. }),
            ..
        }) => Ok(Found {
            id,
            duration: *duration,
            file: *file,
            self_destruct: m.self_destruct,
        }),
        _ => Err(Fail::Core(CoreError::InvalidInput { td: None })),
    }
}

/// 下载找到的语音（同步等下完；只在下完时交路径）
pub fn download<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    f: Found,
) -> Result<VoiceFile, Fail> {
    let file = block_on(b.file_download(f.file)).map_err(Fail::Core)?;
    feed.drain(stream);
    Ok(VoiceFile {
        id: f.id,
        duration: f.duration,
        file,
        self_destruct: f.self_destruct,
    })
}

/// 找到并下载
pub fn fetch<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    q: &VoiceQuery,
) -> Result<VoiceFile, Fail> {
    let f = find(b, stream, feed, q)?;
    download(b, stream, feed, f)
}

// ───────────────────────── 播放（M2.6） ─────────────────────────

/// `atg-audio` 的错误 → CLI 的失败。**这个映射只写这一处**（arch 定 2026-11-13）；码见 `AudioError::what`
pub fn audio_fail(e: AudioError) -> Fail {
    Fail::Core(CoreError::Internal { what: e.what() })
}

/// 播放前的准备（`design.md` 第三节第 5b 条），前一步失败后一步都不做：
/// 找到 →（要标而它是阅后即焚 → 拒绝，确认还没做）→ 下载 → 解码 → 出口**停着**打开 →（要标就）标已听。
/// 返回停着的播放器；调用方关掉后端后 `Cmd::Resume` 开始出声
pub fn prepare_play<B: Backend, O: Output>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    q: &VoiceQuery,
    mark: bool,
    out: O,
) -> Result<(Player<O>, VoiceFile), Fail> {
    let f = find(b, stream, feed, q)?;
    if mark && f.self_destruct {
        return Err(Fail::NotImplemented {
            command: "play",
            message_zh: "阅后即焚的语音标已听会开始倒计时，要先确认（L2），确认还没做；不加 --mark-listened 可以照常听",
        });
    }
    let v = download(b, stream, feed, f)?;
    let pcm = Arc::new(atg_audio::decode(&v.file.path).map_err(audio_fail)?);
    let player = Player::start_paused(out, pcm).map_err(audio_fail)?;
    if mark {
        // 失败时 player 随之释放出口，不出声（不说「已标」）
        block_on(b.mark_listened(v.id)).map_err(Fail::Core)?;
        feed.drain(stream);
    }
    Ok((player, v))
}

/// 直跑 `play` 的准备：起后端、准备好、关后端（声音不需要后端，关了再出声）
pub fn run_prepare_play<B: Backend, O: Output>(
    b: &B,
    profile: &Profile,
    q: &VoiceQuery,
    mark: bool,
    out: O,
    started: Instant,
) -> Result<(Player<O>, VoiceFile), Fail> {
    history::with_backend(b, profile, started, |b, stream, feed| {
        prepare_play(b, stream, feed, q, mark, out)
    })
}

// 测试数据全是编的
#[cfg(test)]
mod tests {
    use super::*;
    use atg_core::backend::Downloaded;
    use atg_core::config::ServerKind;
    use atg_core::fake::{Call, FakeAuth, FakeBackend};
    use serde_json::{Value, json};
    use std::path::PathBuf;

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
        let v = run_download(&b, &profile("ok"), &q(1), Instant::now())
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
            code(run_download(&b, &profile("text"), &q(2), Instant::now())),
            "invalid_input"
        );
        assert!(!b.calls().iter().any(|c| matches!(c, Call::FileDownload(_))));
    }

    #[test]
    fn unfinished_download_is_an_error_not_half_a_file() {
        let b = backend();
        b.set_file(FILE, None);
        assert_eq!(
            code(run_download(&b, &profile("half"), &q(1), Instant::now())),
            "internal"
        );
    }

    #[test]
    fn missing_file_and_missing_message_are_not_found() {
        let b = backend();
        // 第 3 条的文件 id 是 0（TDLib 没给）；假后端没登记 → 找不到
        assert_eq!(
            code(run_download(&b, &profile("nofile"), &q(3), Instant::now())),
            "not_found"
        );
        let b = backend();
        assert_eq!(
            code(run_download(&b, &profile("nomsg"), &q(99), Instant::now())),
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

    /// 假出口：什么都不做
    struct Mute;
    impl atg_audio::player::Output for Mute {
        fn start(
            &mut self,
            _: &Arc<atg_audio::Pcm>,
            _: usize,
            _: f64,
            _: bool,
        ) -> Result<(), AudioError> {
            Ok(())
        }
        fn pause(&mut self) {}
        fn resume(&mut self) -> Result<(), AudioError> {
            Ok(())
        }
        fn set_speed(&mut self, _: f64) -> Result<(), AudioError> {
            Ok(())
        }
        fn failed(&self) -> bool {
            false
        }
        fn stop(&mut self) {}
        fn played(&self) -> usize {
            0
        }
    }

    fn prep(b: &FakeBackend, tag: &str, id: i64, mark: bool) -> Result<VoiceFile, Fail> {
        history::with_backend(b, &profile(tag), Instant::now(), |b, s, f| {
            prepare_play(b, s, f, &q(id), mark, Mute).map(|(p, v)| {
                assert_eq!(p.status().state, atg_audio::State::Paused, "准备完是停着的");
                v
            })
        })
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
        // 标记本身失败 → 报标记的错
        let b = backend();
        b.set_file(FILE, Some(real_ogg()));
        b.fail_next("mark_listened", CoreError::Forbidden { td: None });
        assert_eq!(code(prep(&b, "markfail", 1, true)), "forbidden");
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
}
