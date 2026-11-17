//! `send_limits` 的单测（从 `send_limits.rs` 挪出）
use super::*;

fn temp_dir(tag: &str) -> PathBuf {
    crate::testkit::temp_dir(&format!("limits-{tag}"))
}

#[test]
fn account_limited_is_written_at_once_and_read_by_the_next_process() {
    // codex 016 第 4 轮 #1：收到就落盘，不等下一次判开法；进程一退、重启照样停
    let dir = temp_dir("limited");
    let l = SendLimits::new();
    l.load(&dir);
    l.note(&CoreError::AccountLimited { td: None });
    assert!(dir.join(ACCOUNT_LIMITED_FILE).exists());
    let next = SendLimits::new();
    next.load(&dir);
    assert!(next.check(unix_now()).is_err());
}

#[test]
fn noted_before_the_dir_is_known_is_written_when_it_is() {
    let dir = temp_dir("early");
    let l = SendLimits::new();
    l.note(&CoreError::AccountLimited { td: None });
    l.note(&CoreError::RateLimited {
        retry_after_s: 300,
        td: None,
    });
    l.load(&dir);
    assert!(dir.join(ACCOUNT_LIMITED_FILE).exists());
    assert!(dir.join(FLOOD_FILE).exists());
}

#[test]
fn flood_wait_is_kept_across_processes_and_expires() {
    // codex 016 第 4 轮 #2：限流的等待期写进档案目录，新进程也遵守；到点自然失效
    let dir = temp_dir("flood");
    let l = SendLimits::new();
    l.load(&dir);
    l.note(&CoreError::RateLimited {
        retry_after_s: 120,
        td: None,
    });
    let next = SendLimits::new();
    next.load(&dir);
    let now = unix_now();
    match next.check(now) {
        Err(CoreError::RateLimited { retry_after_s, .. }) => assert!(retry_after_s > 100),
        other => panic!("应当限流：{other:?}"),
    }
    assert!(next.check(now + 121).is_ok());
    // 短的不会把长的冲掉
    next.note(&CoreError::RateLimited {
        retry_after_s: 5,
        td: None,
    });
    assert!(next.check(now + 60).is_err());
}

#[test]
fn unreadable_marks_fail_closed() {
    // codex 016 第 5 轮 #2：截止文件内容坏了 → 不当成「没有记录」，拒发
    let dir = temp_dir("bad");
    fs::write(dir.join(FLOOD_FILE), b"not a number").unwrap();
    let l = SendLimits::new();
    l.load(&dir);
    assert!(matches!(
        l.check(unix_now()),
        Err(CoreError::Internal {
            what: "send_limits_unreadable"
        })
    ));
    // 读不了（这里是同名目录）也一样
    let dir2 = temp_dir("dir");
    fs::create_dir_all(dir2.join(FLOOD_FILE)).unwrap();
    let l2 = SendLimits::new();
    l2.load(&dir2);
    assert!(l2.check(unix_now()).is_err());
}

#[test]
fn unsaved_limit_keeps_the_pending_mark_for_the_next_process() {
    // codex 016 第 6 轮 #2：限制写不进盘（这里用同名目录挡住）时，交出未完成标记留着，下一个进程照样停
    let dir = temp_dir("unsaved");
    let l = SendLimits::new();
    l.load(&dir);
    // 加载之后才把截止文件的位置占成非空目录：之后写截止时间（临时文件改名过去）必失败
    fs::create_dir_all(dir.join(FLOOD_FILE).join("x")).unwrap();
    let guard = l.begin().unwrap();
    l.note(&CoreError::RateLimited {
        retry_after_s: 60,
        td: None,
    });
    drop(guard);
    assert!(dir.join(PENDING_FILE).exists());
    let next = SendLimits::new();
    next.load(&dir);
    assert!(next.check(unix_now()).is_err());
    // 干净的一次交出：标记删掉
    let ok = temp_dir("clean");
    let c = SendLimits::new();
    c.load(&ok);
    drop(c.begin().unwrap());
    assert!(!ok.join(PENDING_FILE).exists());
    let after = SendLimits::new();
    after.load(&ok);
    assert!(after.check(unix_now()).is_ok());
}

/// 模拟磁盘满：在 `write_atomic` 的临时文件位置放一个目录，写就失败，而目标文件本身不存在（新进程看不出异样）
fn jam(dir: &Path, name: &str) {
    let tmp = dir
        .join(name)
        .with_extension(format!("tmp{}", std::process::id()));
    fs::create_dir_all(tmp.join("x")).unwrap();
}

#[test]
fn abandoned_wait_keeps_the_mark_until_the_answer_is_noted() {
    // #88 ①：调用方等不及丢掉等待（它的守卫先放），交出线程还在等应答；晚到的限流写不进盘、补写标记也写不进
    // （磁盘满）时，标记必须还在——交出线程自己的守卫没放，标记就不删
    let dir = temp_dir("abandoned");
    let l = SendLimits::new();
    l.load(&dir);
    let caller = l.begin().unwrap();
    let answer = l.hold();
    drop(caller);
    assert!(dir.join(PENDING_FILE).exists());
    jam(&dir, FLOOD_FILE);
    jam(&dir, PENDING_FILE);
    l.note(&CoreError::RateLimited {
        retry_after_s: 60,
        td: None,
    });
    drop(answer);
    let next = SendLimits::new();
    next.load(&dir);
    assert!(next.check(unix_now()).is_err());
}

#[test]
fn accepted_send_keeps_the_mark_until_its_outcome() {
    // #88 ②：交出成功（进了 TDLib 的待发队列）不等于结束，真正的成败晚到；到之前标记留着
    let dir = temp_dir("accepted");
    let l = SendLimits::new();
    l.load(&dir);
    let id = MsgId {
        chat_id: 7,
        message_id: 1_048_577,
    };
    let g = l.begin().unwrap();
    l.await_outcome(&[id]);
    drop(g);
    assert!(dir.join(PENDING_FILE).exists());
    // 别的消息的下落不算
    l.outcome(MsgId {
        chat_id: 7,
        message_id: 9,
    });
    assert!(dir.join(PENDING_FILE).exists());
    l.outcome(id);
    assert!(!dir.join(PENDING_FILE).exists());
}

#[test]
fn clean_close_drops_the_mark_for_outcomes_still_out() {
    // #88 暂定（待人定，题面在 issue #88）：正常关闭时还没回来的下落不留人工活——标记删掉，
    // 下个进程启动时 TDLib 续发、失败由它登记。但有限制没写进盘的，照样留着
    let dir = temp_dir("close");
    let l = SendLimits::new();
    l.load(&dir);
    let id = MsgId {
        chat_id: 7,
        message_id: 1_048_577,
    };
    drop({
        let g = l.begin().unwrap();
        l.await_outcome(&[id]);
        g
    });
    l.closed(Duration::ZERO);
    assert!(!dir.join(PENDING_FILE).exists());
    let dir2 = temp_dir("close-unsaved");
    let u = SendLimits::new();
    u.load(&dir2);
    drop({
        let g = u.begin().unwrap();
        u.await_outcome(&[id]);
        g
    });
    jam(&dir2, FLOOD_FILE);
    u.note(&CoreError::RateLimited {
        retry_after_s: 60,
        td: None,
    });
    u.closed(Duration::ZERO);
    assert!(dir2.join(PENDING_FILE).exists());
}

#[test]
fn inherited_mark_survives_a_clean_close() {
    // codex 029 第 3 轮 #1：上一个进程留下的标记，本进程正常关闭也不许删（只跑了列表、没交出也一样）
    let dir = temp_dir("inherited");
    fs::write(dir.join(PENDING_FILE), b"1").unwrap();
    let l = SendLimits::new();
    l.load(&dir);
    l.closed(Duration::ZERO);
    assert!(dir.join(PENDING_FILE).exists());
    let next = SendLimits::new();
    next.load(&dir);
    next.closed(Duration::ZERO);
    assert!(dir.join(PENDING_FILE).exists());
    assert!(next.check(unix_now()).is_err());
}

#[test]
fn close_waits_for_the_submit_thread_then_keeps_hands_off() {
    // codex 029 第 3 轮 #2：关闭先等交出线程登记完；关闭之后晚到的登记与收尾不再碰标记（锁已放，可能是新进程的）
    let dir = temp_dir("close-wait");
    let l = SendLimits::new();
    l.load(&dir);
    let id = MsgId {
        chat_id: 7,
        message_id: 1_048_577,
    };
    let caller = l.begin().unwrap();
    let answer = l.hold();
    drop(caller);
    let t = {
        let l = l.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            l.await_outcome(&[id]);
            drop(answer);
        })
    };
    l.closed(Duration::from_secs(5));
    t.join().unwrap();
    // 等到了线程：它登记的那条随关闭清掉，标记删了
    assert!(!dir.join(PENDING_FILE).exists());
    // 关闭后：新进程写了自己的标记；本进程晚到的登记、守卫收尾都不碰它
    fs::write(dir.join(PENDING_FILE), b"2").unwrap();
    l.await_outcome(&[id]);
    drop(l.hold());
    l.outcome(id);
    assert!(dir.join(PENDING_FILE).exists());
    // 等不到（到点还在途）：标记留着
    let dir2 = temp_dir("close-late");
    let k = SendLimits::new();
    k.load(&dir2);
    let g = k.begin().unwrap();
    k.closed(Duration::from_millis(50));
    drop(g);
    assert!(dir2.join(PENDING_FILE).exists());
}

#[test]
fn outcome_before_the_answer_is_not_waited_for_again() {
    // 成败（收线程）比应答（交出线程）先到：之后登记那条时跳过，标记照常删（不然永远等不到）
    let dir = temp_dir("outcome-first");
    let l = SendLimits::new();
    l.load(&dir);
    let id = MsgId {
        chat_id: 7,
        message_id: 1_048_577,
    };
    let g = l.begin().unwrap();
    l.outcome(id);
    l.await_outcome(&[id]);
    drop(g);
    assert!(!dir.join(PENDING_FILE).exists());
}

#[test]
fn only_permanent_deletions_settle_an_outcome() {
    // 清缓存（from_cache）、暂时不可访问（非 is_permanent）的删除不算成败到了：请求还在，标记留着
    let dir = temp_dir("deleted");
    let l = SendLimits::new();
    l.load(&dir);
    let id = MsgId {
        chat_id: 7,
        message_id: 1_048_577,
    };
    drop({
        let g = l.begin().unwrap();
        l.await_outcome(&[id]);
        g
    });
    let del = |is_permanent, from_cache| CoreEvent::MessagesDeleted {
        chat_id: 7,
        message_ids: vec![1_048_577],
        is_permanent,
        from_cache,
    };
    l.observe(&del(true, true));
    l.observe(&del(false, false));
    assert!(dir.join(PENDING_FILE).exists());
    l.observe(&del(true, false));
    assert!(!dir.join(PENDING_FILE).exists());
}

#[test]
fn nothing_is_written_after_close() {
    // codex 029 新身份第 2 轮 #1：关闭等到点还有在途的交出线程，它晚到的限流只记内存、不写盘（锁已放，目录里可能
    // 已是新进程的文件）；留着的标记兜底，下一个进程照样拒
    let dir = temp_dir("after-close");
    let l = SendLimits::new();
    l.load(&dir);
    let caller = l.begin().unwrap();
    let answer = l.hold();
    drop(caller);
    l.closed(Duration::from_millis(20));
    l.note(&CoreError::RateLimited {
        retry_after_s: 60,
        td: None,
    });
    drop(answer);
    assert!(!dir.join(FLOOD_FILE).exists());
    assert!(dir.join(PENDING_FILE).exists());
    // 关闭后再交出：拒，且不写标记
    let dir2 = temp_dir("after-close-begin");
    let k = SendLimits::new();
    k.load(&dir2);
    k.closed(Duration::ZERO);
    assert!(matches!(
        k.begin(),
        Err(CoreError::Internal {
            what: "not_started"
        })
    ));
    assert!(!dir2.join(PENDING_FILE).exists());
    // 同一进程再起（`load`）：解封，关闭前记在内存的限流补写下来
    l.load(&dir);
    assert!(dir.join(FLOOD_FILE).exists());
}

#[test]
fn abandoned_or_overlapping_submits_clean_up_themselves() {
    // 调用方等不及把交出丢掉（守卫随之 drop）也自己清；两次交错时最后一个收尾才删
    let dir = temp_dir("overlap");
    let l = SendLimits::new();
    l.load(&dir);
    let a = l.begin().unwrap();
    let b = l.begin().unwrap();
    drop(a);
    assert!(dir.join(PENDING_FILE).exists());
    drop(b);
    assert!(!dir.join(PENDING_FILE).exists());
}
