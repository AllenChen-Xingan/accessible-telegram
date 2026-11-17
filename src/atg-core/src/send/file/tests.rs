//! `send::file` 的单测：许发的路径、快照、按图片还是按文件发、按进度等结果、防重复与留痕（数据全是编的）

use super::*;
use crate::backend::Upload;
use crate::config::{Profile, ServerKind};
use crate::error::Holder;
use crate::fake::{FakeAuth, FakeBackend, FakeSend};
use crate::gate::{DIRECT_LOG_FILE, Gate};
use crate::link::block_on;
use crate::status::AuthStep;
use std::path::{Path, PathBuf};

const ME: i64 = 4242;
const FRIEND: i64 = 777;

struct Rig {
    b: FakeBackend,
    stream: EventStream,
    dir: PathBuf,
    roots: FileRoots,
}

fn new_chat(id: i64, title: &str) -> Value {
    json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title,
        "type": {"@type": "chatTypePrivate", "user_id": id}, "positions": [], "notification_settings": {}}})
}

fn rig(tag: &str) -> Rig {
    let b = FakeBackend::with_gate(Gate::new(Holder::Cli)).unwrap();
    b.set_auth(FakeAuth {
        phone: "+15550001234".into(),
        logged_in: true,
        ..FakeAuth::default()
    });
    b.set_me(ME);
    b.set_chat(ME, vec![new_chat(ME, "我自己")]);
    b.set_chat(FRIEND, vec![new_chat(FRIEND, "林小雨")]);
    b.set_history(ME, vec![]);
    b.set_history(FRIEND, vec![]);
    let root = crate::testkit::temp_dir(&format!("file-{tag}"));
    // 本应用的数据根与家目录分开放（真机上一个在 Library 下，一个是许可文件夹的上一级）
    let data = root.join("data");
    let profile = Profile::create(&data, "p", ServerKind::Main).unwrap();
    let home = root.join("home");
    for d in ALLOWED_DIRS {
        std::fs::create_dir_all(home.join(d)).unwrap();
    }
    let stream = b.subscribe().unwrap();
    block_on(b.start(&profile)).unwrap();
    b.wait_auth(Duration::from_secs(5), &|s| s == AuthStep::Ready)
        .unwrap();
    let dir = profile.dir().to_path_buf();
    Rig {
        b,
        stream,
        roots: FileRoots {
            home,
            forbidden: vec![data, dir.clone()],
            outbox: dir.join("outbox"),
        },
        dir,
    }
}

impl Rig {
    /// 在家目录下写一个文件（`rel` 相对家目录）
    fn put(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.roots.home.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
        p
    }
    fn prepare(&self, path: &Path, caption: &str) -> Result<FilePlan, CoreError> {
        let src = FileSource {
            path: path.to_path_buf(),
            caption: caption.to_owned(),
            reply_to: None,
        };
        block_on(prepare_file(
            &self.b,
            FRIEND,
            &src,
            &self.roots,
            SendOrigin::Cli,
        ))
    }
    fn send(
        &mut self,
        plan: &FilePlan,
        again: bool,
    ) -> Result<(ConfirmMode, MsgId, Waited), CoreError> {
        self.send_with(plan, again, Duration::from_secs(5), Duration::from_secs(10))
    }
    fn send_with(
        &mut self,
        plan: &FilePlan,
        again: bool,
        idle: Duration,
        cap: Duration,
    ) -> Result<(ConfirmMode, MsgId, Waited), CoreError> {
        let token = self.b.gate().issue(plan, again)?;
        send_file_and_wait(
            &self.b,
            plan,
            token,
            &mut self.stream,
            Duration::from_secs(5),
            idle,
            cap,
            &mut |_| {},
        )
    }
}

fn why(e: CoreError) -> InvalidSendWhy {
    match e {
        CoreError::InvalidSend { why } => why,
        other => panic!("不是 invalid_input：{other:?}"),
    }
}

/// 一个宽 `w` 高 `h` 的 PNG 头（只要头，宽高从这里读）
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    v.extend(w.to_be_bytes());
    v.extend(h.to_be_bytes());
    v.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
    v
}

// ───────────────────────── 许发哪些文件 ─────────────────────────

#[test]
fn only_the_three_folders_are_allowed() {
    let r = rig("folders");
    for d in ALLOWED_DIRS {
        let p = r.put(&format!("{d}/a.txt"), b"hello");
        assert!(r.prepare(&p, "").is_ok(), "{d}");
    }
    let out = r.put("Library/a.txt", b"hello");
    assert_eq!(
        why(r.prepare(&out, "").unwrap_err()),
        InvalidSendWhy::FileNotAllowed
    );
}

#[test]
fn secret_like_names_are_refused_anywhere_below() {
    let r = rig("secret");
    for rel in [
        "Downloads/.env",
        "Downloads/.env.local",
        "Desktop/id_rsa",
        "Desktop/ID_ED25519.pub",
        "Documents/server.PEM",
        "Documents/a.key",
        "Documents/me.session",
        "Downloads/.ssh/notes.txt",
        "Downloads/credentials.json",
        "Documents/tdata/map0",
    ] {
        let p = r.put(rel, b"x");
        assert_eq!(
            why(r.prepare(&p, "").unwrap_err()),
            InvalidSendWhy::FileNotAllowed,
            "{rel}"
        );
    }
}

#[test]
fn symlink_is_judged_by_its_real_path() {
    let r = rig("link");
    let outside = r.put("Library/secret.txt", b"x");
    let link = r.roots.home.join("Downloads/looks-fine.txt");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    assert_eq!(
        why(r.prepare(&link, "").unwrap_err()),
        InvalidSendWhy::FileNotAllowed
    );
    // 反过来：许可文件夹里的真文件经外面的链接也照样许发（按真实路径）
    let inside = r.put("Documents/ok.txt", b"ok");
    let back = r.roots.home.join("elsewhere.txt");
    std::os::unix::fs::symlink(&inside, &back).unwrap();
    assert!(r.prepare(&back, "").is_ok());
}

#[test]
fn app_data_inside_an_allowed_folder_is_still_refused() {
    let mut r = rig("forbidden");
    let p = r.put("Documents/atg-data/profiles/p/db/x.txt", b"x");
    r.roots
        .forbidden
        .push(r.roots.home.join("Documents/atg-data"));
    assert_eq!(
        why(r.prepare(&p, "").unwrap_err()),
        InvalidSendWhy::FileNotAllowed
    );
}

#[test]
fn missing_or_directory_is_not_a_file() {
    let r = rig("notfile");
    let missing = r.roots.home.join("Downloads/none.txt");
    assert_eq!(
        why(r.prepare(&missing, "").unwrap_err()),
        InvalidSendWhy::NotAFile
    );
    let dir = r.roots.home.join("Downloads/sub");
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(
        why(r.prepare(&dir, "").unwrap_err()),
        InvalidSendWhy::NotAFile
    );
}

#[test]
fn over_50_mb_is_refused_and_leaves_no_snapshot() {
    let r = rig("big");
    let p = r.roots.home.join("Downloads/big.bin");
    let f = std::fs::File::create(&p).unwrap();
    f.set_len(FILE_MAX_BYTES + 1).unwrap();
    assert_eq!(
        why(r.prepare(&p, "").unwrap_err()),
        InvalidSendWhy::FileTooLarge
    );
    let left = std::fs::read_dir(&r.roots.outbox)
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(left, 0);
}

#[test]
fn caption_over_the_limit_is_refused() {
    let r = rig("caption");
    let p = r.put("Downloads/a.txt", b"x");
    let long = "字".repeat(CAPTION_MAX + 1);
    assert_eq!(
        why(r.prepare(&p, &long).unwrap_err()),
        InvalidSendWhy::CaptionTooLong
    );
    assert!(r.prepare(&p, &"字".repeat(CAPTION_MAX)).is_ok());
}

// ───────────────────────── 快照 ─────────────────────────

#[test]
fn what_is_sent_is_the_snapshot_not_the_original() {
    let mut r = rig("snapshot");
    let p = r.put("Downloads/a.txt", b"first");
    let plan = r.prepare(&p, "").unwrap();
    // 确认之后原文件被改：交出的仍是准备时那份
    std::fs::write(&p, b"changed").unwrap();
    let (_, _, w) = r.send(&plan, false).unwrap();
    assert!(matches!(w, Waited::Succeeded(_)));
    let req = r.b.submitted().pop().unwrap();
    let sent = req["input_message_content"]["document"]["document"]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(sent.starts_with(&r.roots.outbox.to_string_lossy().into_owned()));
    assert!(sent.ends_with("/a.txt"), "对方看到的是原名：{sent}");
}

#[test]
fn tampered_snapshot_is_refused_before_hand_over() {
    let mut r = rig("tamper");
    let p = r.put("Downloads/a.txt", b"first");
    let plan = r.prepare(&p, "").unwrap();
    std::fs::write(plan.snapshot_path(), b"evil").unwrap();
    let e = r.send(&plan, false).unwrap_err();
    assert!(
        matches!(
            e,
            CoreError::SendRefused {
                reason: SendRefusedReason::ContentMismatch,
                ..
            }
        ),
        "{e:?}"
    );
    assert!(r.b.submitted().is_empty());
}

#[test]
fn snapshot_is_removed_after_a_result_and_kept_when_unknown() {
    let mut r = rig("cleanup");
    let p = r.put("Downloads/a.txt", b"ok");
    let plan = r.prepare(&p, "").unwrap();
    let snap = plan.snapshot_path().to_path_buf();
    r.send(&plan, false).unwrap();
    drop(plan);
    assert!(!snap.exists(), "发出后快照删掉");

    let q = r.put("Downloads/b.txt", b"unknown");
    let plan = r.prepare(&q, "").unwrap();
    let snap = plan.snapshot_path().to_path_buf();
    r.b.script_sends(vec![FakeSend::Silent]);
    let (_, _, w) = r
        .send_with(
            &plan,
            false,
            Duration::from_millis(60),
            Duration::from_millis(300),
        )
        .unwrap();
    assert!(matches!(w, Waited::Unknown(UnknownWhy::Timeout)));
    drop(plan);
    assert!(snap.exists(), "结果未知的快照留着给 TDLib 续发");

    // 只看了计划（--dry-run）：丢掉计划就删
    let plan = r.prepare(&p, "").unwrap();
    let snap = plan.snapshot_path().to_path_buf();
    drop(plan);
    assert!(!snap.exists());
}

#[test]
fn old_snapshots_are_swept() {
    let r = rig("sweep");
    let stale = r.roots.outbox.join("old");
    std::fs::create_dir_all(&stale).unwrap();
    local::sweep(&r.roots.outbox, 0);
    assert!(!stale.exists());
}

// ───────────────────────── 按图片还是按文件发 ─────────────────────────

#[test]
fn photos_by_extension_and_fallback_to_document() {
    let r = rig("photo");
    let as_of = |rel: &str, bytes: &[u8]| {
        let p = r.put(rel, bytes);
        r.prepare(&p, "").unwrap().send_as()
    };
    assert_eq!(
        as_of("Desktop/a.png", &png(640, 480)),
        SendAs::Photo {
            width: 640,
            height: 480
        }
    );
    assert_eq!(
        as_of("Desktop/tall.PNG", &png(100, 3000)),
        SendAs::Document {
            fallback: Some("photo_too_narrow")
        }
    );
    assert_eq!(
        as_of("Desktop/huge.png", &png(9000, 2000)),
        SendAs::Document {
            fallback: Some("photo_too_big")
        }
    );
    assert_eq!(
        as_of("Desktop/broken.jpg", b"not really a jpeg"),
        SendAs::Document {
            fallback: Some("photo_unreadable")
        }
    );
    assert_eq!(
        as_of("Desktop/a.gif", b"GIF89a"),
        SendAs::Document { fallback: None }
    );
}

#[test]
fn jpeg_and_webp_headers_are_read() {
    let r = rig("headers");
    // JPEG：SOI，APP0（长 16），SOF0（高 300、宽 400）
    let mut j = vec![0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10];
    j.extend([0u8; 14]);
    j.extend([0xff, 0xc0, 0x00, 0x11, 0x08, 0x01, 0x2c, 0x01, 0x90, 0x03]);
    let p = r.put("Documents/a.jpg", &j);
    assert_eq!(local::image_size(&p), Some((400, 300)));
    // WebP VP8X：宽高各减 1 存成 24 位
    let mut w = b"RIFF\0\0\0\0WEBPVP8X".to_vec();
    w.extend([0u8; 8]);
    w.extend([0x1f, 0x03, 0x00, 0xdf, 0x01, 0x00]);
    let p = r.put("Documents/a.webp", &w);
    assert_eq!(local::image_size(&p), Some((800, 480)));
}

// ───────────────────────── 等结果 ─────────────────────────

#[test]
fn upload_progress_keeps_waiting_and_silence_times_out() {
    let mut r = rig("progress");
    let p = r.put("Downloads/a.bin", b"data");
    // 一直有进度：等到总上限才算结果未知
    let plan = r.prepare(&p, "").unwrap();
    r.b.script_sends(vec![FakeSend::Silent]);
    r.b.set_uploads(
        (1..=400)
            .map(|n| Upload {
                uploaded: n * 10,
                done: false,
            })
            .collect(),
    );
    let t = Instant::now();
    let (_, _, w) = r
        .send_with(
            &plan,
            false,
            Duration::from_millis(100),
            Duration::from_millis(500),
        )
        .unwrap();
    assert!(matches!(w, Waited::Unknown(UnknownWhy::Timeout)));
    assert!(
        t.elapsed() >= Duration::from_millis(450),
        "{:?}",
        t.elapsed()
    );

    // 没有进度：没动静的时限一到就算结果未知
    let q = r.put("Downloads/b.bin", b"data2");
    let plan = r.prepare(&q, "").unwrap();
    r.b.script_sends(vec![FakeSend::Silent]);
    r.b.set_uploads(vec![Upload {
        uploaded: 10,
        done: false,
    }]);
    let t = Instant::now();
    let (_, _, w) = r
        .send_with(
            &plan,
            false,
            Duration::from_millis(100),
            Duration::from_secs(5),
        )
        .unwrap();
    assert!(matches!(w, Waited::Unknown(UnknownWhy::Timeout)));
    assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
}

// ───────────────────────── 防重复与留痕 ─────────────────────────

#[test]
fn same_file_from_another_path_is_held_and_again_releases() {
    let mut r = rig("dedup");
    let a = r.put("Downloads/a.txt", b"same bytes");
    let b = r.put("Desktop/copy.txt", b"same bytes");
    let plan = r.prepare(&a, "说明").unwrap();
    r.send(&plan, false).unwrap();
    let plan2 = r.prepare(&b, "说明").unwrap();
    let e = r.send(&plan2, false).unwrap_err();
    assert!(
        matches!(
            e,
            CoreError::SendRefused {
                reason: SendRefusedReason::DuplicateRecent,
                ..
            }
        ),
        "{e:?}"
    );
    // 换了说明就不算同一意图
    let plan3 = r.prepare(&b, "另一句").unwrap();
    assert!(r.send(&plan3, false).is_ok());
    // --again 放行
    let plan4 = r.prepare(&b, "说明").unwrap();
    assert!(r.send(&plan4, true).is_ok());
}

#[test]
fn log_has_kind_and_size_but_no_path_or_name() {
    let mut r = rig("log");
    let p = r.put("Desktop/私人照片.png", &png(10, 10));
    let plan = r.prepare(&p, "一句说明").unwrap();
    r.send(&plan, false).unwrap();
    let log = std::fs::read_to_string(r.dir.join(DIRECT_LOG_FILE)).unwrap();
    let last: Value = serde_json::from_str(log.lines().last().unwrap()).unwrap();
    assert_eq!(last["kind"], "file");
    assert_eq!(last["file"]["kind"], "photo");
    assert_eq!(last["file"]["size"], png(10, 10).len());
    assert!(!log.contains("私人照片"));
    assert!(!log.contains("outbox"));
    assert!(!log.contains("一句说明"));
}

#[test]
fn plan_line_has_no_path() {
    let r = rig("planjson");
    let p = r.put("Documents/报告.pdf", b"%PDF-1.4");
    let plan = r.prepare(&p, "").unwrap();
    let v = file_plan_json(&plan);
    assert_eq!(v["file_name"], "报告.pdf");
    assert_eq!(v["send_as"], "document");
    assert_eq!(v["size"], 8);
    let text = v.to_string();
    assert!(!text.contains("outbox"));
    assert!(!text.contains("Documents"));
}

#[test]
fn upload_progress_is_read_from_document_and_photo() {
    let remote = |n: i64, done: bool| json!({"uploaded_size": n, "is_uploading_completed": done});
    let doc = json!({"content": {"@type": "messageDocument",
        "document": {"document": {"id": 3, "remote": remote(4096, false)}}}});
    assert_eq!(
        crate::tdjson::file::upload(&doc),
        Some(Upload {
            uploaded: 4096,
            done: false
        })
    );
    let photo = json!({"content": {"@type": "messagePhoto", "photo": {"sizes": [
        {"photo": {"id": 1, "remote": remote(100, true)}},
        {"photo": {"id": 2, "remote": remote(9000, false)}},
    ]}}});
    assert_eq!(
        crate::tdjson::file::upload(&photo),
        Some(Upload {
            uploaded: 9100,
            done: false
        })
    );
    let text = json!({"content": {"@type": "messageText"}});
    assert_eq!(crate::tdjson::file::upload(&text), None);
}
