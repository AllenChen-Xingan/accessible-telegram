//! 自审第 1 轮补的边界（#12）：硬链接、策略先于存在性、本机检查先于查会话、失败与交出时未知的快照去留、按图片发的请求形状

use super::*;

/// 硬链接：许可文件夹里的名字指向许可文件夹外（如 `~/.ssh` 下）的同一个文件实体，真实路径看不出来（自审第 1 轮 P2）
#[test]
fn hard_link_into_an_allowed_folder_is_refused() {
    let r = rig("hardlink");
    let outside = r.put("Library/key.txt", b"not for sending");
    let inside = r.roots.home.join("Downloads/a.txt");
    std::fs::hard_link(&outside, &inside).unwrap();
    assert_eq!(
        why(r.prepare(&inside, "").unwrap_err()),
        InvalidSendWhy::FileNotAllowed
    );
}

/// 许可文件夹外的路径：不管存不存在、是不是文件，一律先报不许（不让人借这条命令探别处有什么文件）
#[test]
fn outside_paths_are_refused_before_existence_is_told() {
    let r = rig("probe");
    let missing = r.roots.home.join(".ssh/id_none");
    assert_eq!(
        why(r.prepare(&missing, "").unwrap_err()),
        InvalidSendWhy::FileNotAllowed
    );
    std::fs::create_dir_all(r.roots.home.join("Library/dir")).unwrap();
    assert_eq!(
        why(r
            .prepare(&r.roots.home.join("Library/dir"), "")
            .unwrap_err()),
        InvalidSendWhy::FileNotAllowed
    );
    // 许可文件夹里找不到的照旧是 not_a_file
    let inside = r.roots.home.join("Downloads/none.txt");
    assert_eq!(
        why(r.prepare(&inside, "").unwrap_err()),
        InvalidSendWhy::NotAFile
    );
}

/// 只看本机就能拒的（路径、大小、说明）排在查会话之前：会话不存在时照样先报文件的问题
#[test]
fn local_checks_come_before_backend_queries() {
    let r = rig("order");
    let p = r.put("Library/a.txt", b"x");
    let src = FileSource {
        path: p,
        caption: String::new(),
        reply_to: None,
    };
    let e = block_on(prepare_file(&r.b, 999_999, &src, &r.roots, SendOrigin::Cli)).unwrap_err();
    assert_eq!(why(e), InvalidSendWhy::FileNotAllowed);
}

/// 失败且能重发：快照留着（`resend` 时 TDLib 要再读那个文件）；不能重发的照删
#[test]
fn failed_send_keeps_snapshot_only_when_it_can_be_resent() {
    let mut r = rig("failed");
    for (can_retry, kept) in [(true, true), (false, false)] {
        let p = r.put(
            &format!("Downloads/{can_retry}.txt"),
            format!("{can_retry}").as_bytes(),
        );
        let plan = r.prepare(&p, "").unwrap();
        let snap = plan.snapshot_path().to_path_buf();
        r.b.script_sends(vec![FakeSend::Fail {
            code: 400,
            message: "FILE_PART_INVALID".into(),
            can_retry,
        }]);
        let (_, _, w) = r.send(&plan, false).unwrap();
        assert!(matches!(w, Waited::Failed { .. }));
        drop(plan);
        assert_eq!(snap.exists(), kept, "can_retry={can_retry}");
    }
}

/// 交出这一步就结果未知（断线）：可能已交给 TDLib，快照留着
#[test]
fn unknown_at_hand_over_keeps_snapshot() {
    let mut r = rig("handunknown");
    let p = r.put("Downloads/a.txt", b"x");
    let plan = r.prepare(&p, "").unwrap();
    let snap = plan.snapshot_path().to_path_buf();
    r.b.script_sends(vec![FakeSend::Reject(CoreError::Disconnected)]);
    let e = r.send(&plan, false).unwrap_err();
    assert!(matches!(e, CoreError::SendOutcomeUnknown { .. }), "{e:?}");
    drop(plan);
    assert!(snap.exists());
}

/// 按图片发的请求形状（TDLib `inputMessagePhoto` 外套 `inputPhoto`，`td_api.tl:6435,6716`）：真机才验得到，这里钉住
#[test]
fn photo_request_has_the_tdlib_shape() {
    let mut r = rig("shape");
    let p = r.put("Desktop/a.png", &png(64, 48));
    let plan = r.prepare(&p, "说明").unwrap();
    r.send(&plan, false).unwrap();
    let c = r.b.submitted().pop().unwrap()["input_message_content"].clone();
    assert_eq!(c["@type"], "inputMessagePhoto");
    assert_eq!(c["photo"]["@type"], "inputPhoto");
    assert_eq!(c["photo"]["photo"]["@type"], "inputFileLocal");
    assert!(c["photo"]["thumbnail"].is_null());
    assert!(c["photo"]["video"].is_null());
    assert_eq!(c["photo"]["added_sticker_file_ids"], json!([]));
    assert_eq!(
        (c["photo"]["width"].clone(), c["photo"]["height"].clone()),
        (json!(64), json!(48))
    );
    assert_eq!(c["caption"]["text"], "说明");
    assert_eq!(c["show_caption_above_media"], false);
    assert!(c["self_destruct_type"].is_null());
    assert_eq!(c["has_spoiler"], false);
    let keys: Vec<&String> = c.as_object().unwrap().keys().collect();
    assert_eq!(keys.len(), 6, "{keys:?}");
}

/// 同一个文件改了扩展名（`.png` → `.bin`）：一个按图片发、一个按文件发，请求形状不同，但内容、说明、回复都一样，
/// 照样算同一意图被拦（codex 034 第 1 轮 #2）
#[test]
fn renaming_the_extension_does_not_escape_the_guard() {
    let mut r = rig("ext");
    let a = r.put("Downloads/a.png", &png(10, 10));
    let b = r.put("Desktop/a.bin", &png(10, 10));
    let plan = r.prepare(&a, "").unwrap();
    assert!(matches!(plan.send_as(), SendAs::Photo { .. }));
    r.send(&plan, false).unwrap();
    let plan2 = r.prepare(&b, "").unwrap();
    assert!(matches!(plan2.send_as(), SendAs::Document { .. }));
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
}

/// 等结果的总上限不被进度查询拖过头：一直有进度时，到总上限就收（codex 034 第 1 轮 #3）
#[test]
fn upload_wait_stops_at_the_cap() {
    let mut r = rig("capedge");
    let p = r.put("Downloads/a.bin", b"data");
    let plan = r.prepare(&p, "").unwrap();
    r.b.script_sends(vec![FakeSend::Silent]);
    r.b.set_uploads(
        (1..=2000)
            .map(|n| Upload {
                uploaded: n,
                done: false,
            })
            .collect(),
    );
    let t = Instant::now();
    let (_, _, w) = r
        .send_with(
            &plan,
            false,
            Duration::from_millis(200),
            Duration::from_millis(300),
        )
        .unwrap();
    assert!(matches!(w, Waited::Unknown(UnknownWhy::Timeout)));
    let el = t.elapsed();
    assert!(
        el >= Duration::from_millis(300) && el < Duration::from_millis(450),
        "{el:?}"
    );
}
