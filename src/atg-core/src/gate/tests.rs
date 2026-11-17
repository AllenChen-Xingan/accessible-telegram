//! `gate` 的测试：开凭据、复核、防重复记录、直发留痕、限流冷却。

use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

struct Plan {
    target: ChatId,
    digest: [u8; 32],
    intent: [u8; 32],
}

impl Sealed for Plan {}
impl Gated for Plan {
    fn target(&self) -> ChatId {
        self.target
    }
    fn digest(&self) -> [u8; 32] {
        self.digest
    }
    fn intent(&self) -> [u8; 32] {
        self.intent
    }
    fn dedup(&self) -> bool {
        true
    }
    fn kind(&self) -> &'static str {
        "text"
    }
}

/// 不进防重复的计划（重发）：直发时不查、不写防重复记录，留痕照写
struct Resend(Plan);
impl Sealed for Resend {}
impl Gated for Resend {
    fn target(&self) -> ChatId {
        self.0.target
    }
    fn digest(&self) -> [u8; 32] {
        self.0.digest
    }
    fn intent(&self) -> [u8; 32] {
        self.0.intent
    }
    fn dedup(&self) -> bool {
        false
    }
    fn kind(&self) -> &'static str {
        "resend"
    }
}

#[test]
fn non_dedup_plans_skip_guard_but_keep_log() {
    let (g, dir) = gate(false);
    let p = Resend(plan(777, 4));
    for _ in 0..2 {
        let t = g.issue(&p, false).unwrap();
        let mode = g.verify(t, &p).unwrap();
        g.before_submit(mode, &p).unwrap();
        g.after_outcome(mode, &p, -1, Some(9), Outcome::Succeeded, None);
    }
    assert!(!dir.join(GUARD_FILE).exists());
    // 每次两行：交出前「已提交」+ 等到结果后一行
    let log = fs::read_to_string(dir.join(DIRECT_LOG_FILE)).unwrap();
    assert_eq!(log.lines().count(), 4);
    assert!(log.contains("\"kind\":\"resend\""));
}

#[test]
fn mode_names_match_serde() {
    for m in [ConfirmMode::TestExempt, ConfirmMode::DirectMode] {
        assert_eq!(
            serde_json::to_value(m).unwrap(),
            serde_json::json!(m.as_str())
        );
    }
}

fn plan(target: ChatId, d: u8) -> Plan {
    Plan {
        target,
        digest: [d; 32],
        intent: [d; 32],
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let d = std::env::temp_dir().join(format!(
        "atg-gate-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

const ME: Me = Me {
    user_id: 4242,
    saved_chat_id: 4242,
    test_number: true,
};

fn gate(test_dc: bool) -> (Gate, PathBuf) {
    let g = Gate::new(Holder::Cli);
    let dir = temp_dir(&format!("{test_dc}"));
    g.set_server(test_dc, &dir, derive_guard_key(b"k", "t"));
    g.set_me(ME);
    (g, dir)
}

fn reason(e: CoreError) -> SendRefusedReason {
    match e {
        CoreError::SendRefused { reason, .. } => reason,
        other => panic!("不是 send_refused：{other:?}"),
    }
}

#[test]
fn test_dc_saved_is_exempt() {
    let (g, _) = gate(true);
    let p = plan(ME.saved_chat_id, 1);
    let t = g.issue(&p, false).unwrap();
    assert_eq!(t.mode(), ConfirmMode::TestExempt);
    assert_eq!(g.verify(t, &p).unwrap(), ConfirmMode::TestExempt);
}

#[test]
fn test_dc_other_chat_refused() {
    let (g, _) = gate(true);
    let e = g.issue(&plan(777, 1), false).unwrap_err();
    assert_eq!(reason(e), SendRefusedReason::TestDcSavedOnly);
}

#[test]
fn test_dc_saved_needs_test_number() {
    let (g, _) = gate(true);
    g.set_me(Me {
        test_number: false,
        ..ME
    });
    let e = g.issue(&plan(ME.saved_chat_id, 1), false).unwrap_err();
    assert_eq!(reason(e), SendRefusedReason::TestDcSavedOnly);
}

#[test]
fn main_server_is_always_direct_without_asking() {
    // 人定 2026-11-16：Touch ID 确认整个去掉，正式服务器的一切目标（含收藏夹）都直发
    let (g, _) = gate(false);
    for target in [ME.saved_chat_id, 777] {
        let p = plan(target, 1);
        let t = g.issue(&p, false).unwrap();
        assert_eq!(t.mode(), ConfirmMode::DirectMode);
        assert_eq!(g.verify(t, &p).unwrap(), ConfirmMode::DirectMode);
    }
}

#[test]
fn token_binds_target_and_digest_once_within_ttl() {
    let (g, _) = gate(false);
    let p = plan(777, 1);
    // 目标不对
    let t = g.issue(&p, false).unwrap();
    assert_eq!(
        reason(g.verify(t, &plan(778, 1)).unwrap_err()),
        SendRefusedReason::TargetMismatch
    );
    // 内容不对
    let t = g.issue(&p, true).unwrap();
    assert_eq!(
        reason(g.verify(t, &plan(777, 2)).unwrap_err()),
        SendRefusedReason::ContentMismatch
    );
    // 过期
    let mut t = g.issue(&p, true).unwrap();
    t.age_by(TOKEN_TTL + Duration::from_secs(1));
    assert_eq!(
        reason(g.verify(t, &p).unwrap_err()),
        SendRefusedReason::TokenExpired
    );
    // 用过：同一个随机数第二次（凭据本身不可复制，只能在 crate 内伪造来测）
    let t = g.issue(&p, true).unwrap();
    let nonce = t.nonce;
    assert!(g.verify(t, &p).is_ok());
    let forged = ConfirmToken {
        target: 777,
        digest: [1; 32],
        nonce,
        issued: Instant::now(),
        mode: ConfirmMode::DirectMode,
        again: false,
    };
    assert_eq!(
        reason(g.verify(forged, &p).unwrap_err()),
        SendRefusedReason::TokenReused
    );
}

#[test]
fn direct_mode_blocks_duplicate_until_again_or_failure() {
    let (g, dir) = gate(false);
    let p = plan(777, 9);
    let t = g.issue(&p, false).unwrap();
    let mode = g.verify(t, &p).unwrap();
    g.before_submit(mode, &p).unwrap();
    // 还没结果（进程中途死掉也是这样）：拦
    let e = g.issue(&p, false).unwrap_err();
    match &e {
        CoreError::SendRefused {
            reason: SendRefusedReason::DuplicateRecent,
            last: Some(l),
        } => assert_eq!(l.outcome, Outcome::Submitted),
        other => panic!("{other:?}"),
    }
    g.after_outcome(mode, &p, -1, Some(5), Outcome::Succeeded, None);
    // 已发出：拦；--again 放行
    assert_eq!(
        reason(g.issue(&p, false).unwrap_err()),
        SendRefusedReason::DuplicateRecent
    );
    assert!(g.issue(&p, true).is_ok());
    // 别的会话、别的内容不受影响
    assert!(g.issue(&plan(778, 9), false).is_ok());
    assert!(g.issue(&plan(777, 8), false).is_ok());
    // 记录里没有内容：只有时间、会话、指纹、状态；权限 0600
    let raw = fs::read_to_string(dir.join(GUARD_FILE)).unwrap();
    let v: Vec<GuardEntry> = serde_json::from_str(&raw).unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].state, Outcome::Succeeded);
    use std::os::unix::fs::PermissionsExt;
    let mode_bits = fs::metadata(dir.join(GUARD_FILE))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode_bits & 0o777, 0o600);
    // 留痕两行：交出前「已提交」（证明写得进去，codex 016 第 1 轮 #5），等到结果后一行结果
    let log = fs::read_to_string(dir.join(DIRECT_LOG_FILE)).unwrap();
    assert_eq!(log.lines().count(), 2);
    assert!(
        log.lines()
            .next()
            .unwrap()
            .contains("\"result\":\"submitted\"")
    );
    assert!(
        log.lines()
            .nth(1)
            .unwrap()
            .contains("\"result\":\"succeeded\"")
    );

    // 明确失败的不拦
    let p2 = plan(777, 7);
    let t = g.issue(&p2, false).unwrap();
    let mode = g.verify(t, &p2).unwrap();
    g.before_submit(mode, &p2).unwrap();
    g.after_outcome(mode, &p2, -2, Some(-3), Outcome::Failed, None);
    assert!(g.issue(&p2, false).is_ok());
}

#[test]
fn duplicate_record_expires_after_window() {
    let dir = temp_dir("expire");
    let s = GuardStore::new(dir.join(GUARD_FILE));
    s.record(1000, 777, "ab", Outcome::Succeeded).unwrap();
    assert!(s.hit(1000 + DUP_WINDOW_S - 1, 777, "ab").unwrap().is_some());
    assert!(s.hit(1000 + DUP_WINDOW_S, 777, "ab").unwrap().is_none());
    // 结果未知的拦 24 小时：消息可能还在 TDLib 的待发队列里，下次启动会续发
    s.record(2000, 778, "ab", Outcome::Unknown).unwrap();
    assert!(s.hit(2000 + DUP_WINDOW_S + 1, 778, "ab").unwrap().is_some());
    assert!(
        s.hit(2000 + UNKNOWN_HOLD_S - 1, 778, "ab")
            .unwrap()
            .is_some()
    );
    assert!(s.hit(2000 + UNKNOWN_HOLD_S, 778, "ab").unwrap().is_none());
    // 写新的时把过期的清掉
    s.record(2000 + UNKNOWN_HOLD_S + 5, 777, "cd", Outcome::Submitted)
        .unwrap();
    assert_eq!(s.load().unwrap().len(), 1);
}

#[test]
fn clock_turned_back_does_not_lift_the_guard() {
    // codex 016 第 1 轮 #1：时钟往回拨，比「现在」还晚的记录照样拦，等时钟追上再过期
    let dir = temp_dir("rollback");
    let s = GuardStore::new(dir.join(GUARD_FILE));
    s.record(10_000, 777, "ab", Outcome::Unknown).unwrap();
    assert!(s.hit(10_000 - 3600, 777, "ab").unwrap().is_some());
    s.record(10_000 - 3600, 778, "cd", Outcome::Submitted)
        .unwrap();
    assert_eq!(s.load().unwrap().len(), 2);
}

#[test]
fn unreadable_guard_file_is_never_overwritten() {
    // codex 016 第 1 轮 #2：读不了（这里是文件坏了）就不写，免得拿空表抹掉别的记录；直发因此被拒
    let dir = temp_dir("corrupt");
    let path = dir.join(GUARD_FILE);
    fs::write(&path, b"{not json").unwrap();
    let s = GuardStore::new(path.clone());
    assert!(s.record(1000, 777, "ab", Outcome::Submitted).is_err());
    assert!(s.settle(1000, 777, "ab", Outcome::Succeeded).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"{not json");
}

#[test]
fn fingerprint_depends_on_key_and_hides_content() {
    let a = derive_guard_key(b"key-a", "main");
    let b = derive_guard_key(b"key-b", "main");
    let c = derive_guard_key(b"key-a", "test");
    let i = [3u8; 32];
    assert_ne!(fingerprint(&a, 1, &i), fingerprint(&b, 1, &i));
    assert_ne!(fingerprint(&a, 1, &i), fingerprint(&c, 1, &i));
    assert_ne!(fingerprint(&a, 1, &i), fingerprint(&a, 2, &i));
    // 派生出的钥匙不是库钥匙本身
    assert_ne!(a.expose(), b"key-a");
}

#[test]
fn account_limited_stops_everything() {
    let (g, _) = gate(false);
    g.limits().note(&CoreError::AccountLimited { td: None });
    assert_eq!(
        reason(g.issue(&plan(778, 2), false).unwrap_err()),
        SendRefusedReason::AccountLimited
    );
}

#[test]
fn account_limited_survives_into_the_next_process_until_removed() {
    let (g, dir) = gate(false);
    g.limits().note(&CoreError::AccountLimited { td: None });
    assert!(dir.join(ACCOUNT_LIMITED_FILE).exists());
    // 下一个进程（新的闸）开在同一个档案目录上：照样停
    let next = Gate::new(Holder::Cli);
    next.set_server(false, &dir, derive_guard_key(b"k", "t"));
    next.set_me(ME);
    assert_eq!(
        reason(next.issue(&plan(778, 2), false).unwrap_err()),
        SendRefusedReason::AccountLimited
    );
    // 人处理完、删掉标记：再开就恢复
    fs::remove_file(dir.join(ACCOUNT_LIMITED_FILE)).unwrap();
    let after = Gate::new(Holder::Cli);
    after.set_server(false, &dir, derive_guard_key(b"k", "t"));
    after.set_me(ME);
    assert!(after.issue(&plan(778, 2), false).is_ok());
}

#[test]
fn direct_sent_counts_handed_over_sends() {
    let (g, _) = gate(false);
    let p = plan(777, 3);
    assert_eq!(g.direct_sent(), 0);
    let t = g.issue(&p, false).unwrap();
    let mode = g.verify(t, &p).unwrap();
    g.before_submit(mode, &p).unwrap();
    assert_eq!(g.direct_sent(), 1);
}

#[test]
fn visible_escapes_tags_separators_and_variation_selectors() {
    // 标签字符藏的一段话、行分隔符、蒙古文元音分隔符、软连字符、变体选择符（含补充平面）
    assert_eq!(
        visible("好的\u{E0041}\u{E0042}"),
        "好的\\u{E0041}\\u{E0042}"
    );
    assert_eq!(
        visible("a\u{2028}b\u{180E}c\u{00AD}d"),
        "a\\u{2028}b\\u{180E}c\\u{00AD}d"
    );
    assert_eq!(visible("x\u{FE0F}\u{E0100}"), "x\\u{FE0F}\\u{E0100}");
    // 普通中文、标点、emoji 本身不动
    assert_eq!(visible("好的！👍"), "好的！👍");
    assert_eq!(hidden_count("好的\u{E0041}\u{200B}\n"), 2);
}

#[test]
fn visible_escapes_bidi_zero_width_and_controls() {
    assert_eq!(visible("a\u{202E}b"), "a\\u{202E}b");
    assert_eq!(visible("x\u{200B}y\u{1b}[31m"), "x\\u{200B}y\\u{001B}[31m");
    assert_eq!(visible("第一行\n第二行"), "第一行\n第二行");
}

#[test]
fn late_limit_from_the_receiver_stops_sending() {
    // codex 016 第 3 轮 #2 / 第 4 轮：收线程经同一份限制记下（等结果超时后才到的失败），下一次开凭据就拒
    let (g, dir) = gate(false);
    assert!(g.issue(&plan(777, 1), false).is_ok());
    g.limits().note(&CoreError::AccountLimited { td: None });
    assert!(dir.join(ACCOUNT_LIMITED_FILE).exists());
    assert_eq!(
        reason(g.issue(&plan(777, 2), false).unwrap_err()),
        SendRefusedReason::AccountLimited
    );
}

#[test]
fn flood_wait_blocks_the_next_send() {
    let (g, _) = gate(false);
    g.limits().note(&CoreError::RateLimited {
        retry_after_s: 60,
        td: None,
    });
    assert!(matches!(
        g.issue(&plan(778, 2), false),
        Err(CoreError::RateLimited { .. })
    ));
}

/// 查重与登记在交出前一步做完（#77）：两份同内容计划都在对方登记之前拿到凭据，第二份交出前被拒，
/// 什么都不写；带 `--again` 的放行。同一份计划先开过带 `--again`、没交出的凭据，再开不带的：不沿用那次放行
#[test]
fn duplicate_is_claimed_right_before_submit() {
    let (g, dir) = gate(false);
    let (a, b) = (plan(777, 6), plan(777, 6));
    let ta = g.issue(&a, false).unwrap();
    let tb = g.issue(&b, false).unwrap();
    let ma = g.verify(ta, &a).unwrap();
    g.before_submit(ma, &a).unwrap();
    let mb = g.verify(tb, &b).unwrap();
    assert_eq!(
        reason(g.before_submit(mb, &b).unwrap_err()),
        SendRefusedReason::DuplicateRecent
    );
    let entries = |d: &Path| -> usize {
        serde_json::from_str::<Vec<GuardEntry>>(&fs::read_to_string(d.join(GUARD_FILE)).unwrap())
            .unwrap()
            .len()
    };
    assert_eq!(entries(&dir), 1, "被拒的那份不登记");
    let log = fs::read_to_string(dir.join(DIRECT_LOG_FILE)).unwrap();
    assert_eq!(log.lines().count(), 1, "被拒的那份不留痕");
    assert_eq!(g.direct_sent(), 1);

    // --again：放行
    let c = plan(777, 6);
    let tc = g.issue(&c, true).unwrap();
    let mc = g.verify(tc, &c).unwrap();
    g.before_submit(mc, &c).unwrap();
    assert_eq!(entries(&dir), 2);

    // 同一摘要先开带 --again 的凭据、验过但没交出；再开不带的，交出前别的那份已登记：照拦，不沿用上一张的放行
    let f = plan(777, 5);
    let tf = g.issue(&f, true).unwrap();
    let _ = g.verify(tf, &f).unwrap();
    let tf2 = g.issue(&f, false).unwrap();
    // 同内容、摘要不同（真实里每次的 sending_id 不同）
    let other = Plan {
        target: 777,
        digest: [50; 32],
        intent: [5; 32],
    };
    let to = g.issue(&other, false).unwrap();
    let mo = g.verify(to, &other).unwrap();
    g.before_submit(mo, &other).unwrap();
    let mf2 = g.verify(tf2, &f).unwrap();
    assert_eq!(
        reason(g.before_submit(mf2, &f).unwrap_err()),
        SendRefusedReason::DuplicateRecent
    );
}

/// 一次请求动好几条的计划（批量重发）：留痕按条记（#83）
struct Batch(Plan, Vec<i64>);
impl Sealed for Batch {}
impl Gated for Batch {
    fn target(&self) -> ChatId {
        self.0.target
    }
    fn digest(&self) -> [u8; 32] {
        self.0.digest
    }
    fn intent(&self) -> [u8; 32] {
        self.0.intent
    }
    fn dedup(&self) -> bool {
        false
    }
    fn kind(&self) -> &'static str {
        "resend"
    }
    fn log_items(&self) -> Vec<i64> {
        self.1.clone()
    }
}

/// 交出前每条一行「已提交」；结果按条记（含 TDLib 说不能重发的那条）；请求整个失败 / 到点时每条一行
#[test]
fn batch_plans_log_one_line_per_item() {
    let (g, dir) = gate(false);
    let p = Batch(plan(777, 3), vec![11, 12, 13]);
    let t = g.issue(&p, false).unwrap();
    let mode = g.verify(t, &p).unwrap();
    g.before_submit(mode, &p).unwrap();
    g.after_item(mode, &p, 11, -21, Some(31), Outcome::Succeeded, None);
    g.after_item(mode, &p, 12, 0, None, Outcome::Failed, Some("not_resent"));
    g.after_item(mode, &p, 13, -23, None, Outcome::Unknown, None);
    let lines = |d: &Path| -> Vec<serde_json::Value> {
        fs::read_to_string(d.join(DIRECT_LOG_FILE))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    };
    let v = lines(&dir);
    assert_eq!(v.len(), 6);
    for (i, old) in [11, 12, 13].into_iter().enumerate() {
        assert_eq!(v[i]["old_id"], old);
        assert_eq!(v[i]["result"], "submitted");
        assert_eq!(v[i + 3]["old_id"], old);
    }
    assert_eq!(v[3]["final_id"], 31);
    assert_eq!(v[4]["error"], "not_resent");
    assert_eq!(v[5]["result"], "unknown");

    // 请求整个没等到（没有临时 id）：每条各一行
    let t = g.issue(&p, false).unwrap();
    let mode = g.verify(t, &p).unwrap();
    g.before_submit(mode, &p).unwrap();
    g.after_outcome(mode, &p, 0, None, Outcome::Unknown, None);
    let v = lines(&dir);
    assert_eq!(v.len(), 12);
    assert!(v[9..].iter().all(|l| l["result"] == "unknown"));
}
