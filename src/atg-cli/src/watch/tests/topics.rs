//! 论坛话题：判到时还没设置的话题先去取、取不到或卡住时的期限与 Ctrl+C（#73）
use super::*;
use crate::deadlines::TOPIC_TIMEOUT;

/// 论坛群里一条话题消息（话题 7）
fn forum_setup(b: &FakeBackend) {
    b.push(chat(
        GROUP,
        json!({"@type": "chatTypeSupergroup", "supergroup_id": 1, "is_channel": false}),
        "周末爬山群",
    ));
    b.push_net("connectionStateReady");
}

fn in_topic_7(id: i64) -> Value {
    let mut m = new_msg(GROUP, id, plain("话题里"));
    m["message"]["topic_id"] = json!({"@type": "messageTopicForum", "forum_topic_id": 7});
    m
}

/// 取话题设置一直不应答（#73）：最多等 TOPIC_TIMEOUT，之后照常往下走，这个话题本次不再取
#[test]
fn hung_forum_topic_gives_up_after_deadline() {
    let b = Arc::new(backend());
    forum_setup(&b);
    b.fail_next(
        "forum_topic_get",
        CoreError::Internal {
            what: FakeBackend::HANG,
        },
    );
    b.push(in_topic_7(1));
    let pusher = {
        let b = b.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !b.calls().contains(&Call::ForumTopicGet(GROUP, 7)) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            b.push(in_topic_7(2));
        })
    };
    let t0 = Instant::now();
    let (code, ls) = run_watch(&b, Query { chat: None }, false, |v| {
        v["data"]["message_id"] == 2
    });
    pusher.join().unwrap();
    assert_eq!(code, 0);
    let took = t0.elapsed();
    assert!(took >= TOPIC_TIMEOUT, "{took:?}");
    assert!(took < TOPIC_TIMEOUT + Duration::from_secs(5), "{took:?}");
    let msgs = events(&ls, "new_message");
    assert_eq!(msgs[1]["data"]["notify"]["reason"], "topic_unknown");
    let gets = b
        .calls()
        .iter()
        .filter(|c| **c == Call::ForumTopicGet(GROUP, 7))
        .count();
    assert_eq!(gets, 1);
}

/// 取话题设置时按了 Ctrl+C：不等到期限，直接回主循环收尾
#[test]
fn stop_while_forum_topic_hangs_returns_at_once() {
    let b = backend();
    let p = profile("topic-stop");
    let mut stream = b.subscribe().unwrap();
    block_on(b.start(&p)).unwrap();
    forum_setup(&b);
    b.fail_next(
        "forum_topic_get",
        CoreError::Internal {
            what: FakeBackend::HANG,
        },
    );
    let mut feed = Feed::new(Instant::now());
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    feed.hub.add(Sub {
        out: Box::new(Cap {
            lines: lines.clone(),
            stop: Box::leak(Box::new(AtomicBool::new(false))),
            until: |_| false,
        }),
        watcher: Watcher::new(cx(false), Format::Json, None, clock),
        chat: None,
        dead: false,
    });
    b.push(in_topic_7(1));
    // 喂到这条话题消息被判过（判的时候记下要取的话题）
    let t0 = Instant::now();
    while !lines
        .lock()
        .unwrap()
        .iter()
        .any(|l| l.contains("new_message"))
        && t0.elapsed() < Duration::from_secs(5)
    {
        feed.drain(&mut stream);
        std::thread::sleep(Duration::from_millis(10));
    }
    // 等假后端记下取话题的请求（已交出、卡住）再立停止标志，不靠固定延时
    let stop = AtomicBool::new(false);
    let t1 = Instant::now();
    std::thread::scope(|s| {
        s.spawn(|| {
            while !b.calls().contains(&Call::ForumTopicGet(GROUP, 7))
                && t1.elapsed() < Duration::from_secs(5)
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            stop.store(true, Ordering::SeqCst);
        });
        fetch_topics(&b, &mut feed, &stop);
    });
    assert!(t1.elapsed() < TOPIC_TIMEOUT, "{:?}", t1.elapsed());
    assert!(
        b.calls().contains(&Call::ForumTopicGet(GROUP, 7)),
        "确实去取了、卡在等应答"
    );
    let _ = b.shutdown(Duration::from_secs(2));
}

#[test]
fn unknown_forum_topic_is_fetched_then_judged_by_topic() {
    let b = Arc::new(backend());
    b.push(chat(
        GROUP,
        json!({"@type": "chatTypeSupergroup", "supergroup_id": 1, "is_channel": false}),
        "周末爬山群",
    ));
    b.set_forum_topic(GROUP, 7, json!({"@type": "forumTopic", "last_read_inbox_message_id": 0,
        "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": false, "mute_for": 3600}}));
    b.push_net("connectionStateReady");
    let in_topic = |id: i64| {
        let mut m = new_msg(GROUP, id, plain("第一条"));
        m["message"]["topic_id"] = json!({"@type": "messageTopicForum", "forum_topic_id": 7});
        m
    };
    b.push(in_topic(1));
    // 取过话题之后再来一条
    let pusher = {
        let b = b.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !b.calls().contains(&Call::ForumTopicGet(GROUP, 7)) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            std::thread::sleep(Duration::from_millis(100));
            let mut m = new_msg(GROUP, 2, plain("第二条"));
            m["message"]["topic_id"] = json!({"@type": "messageTopicForum", "forum_topic_id": 7});
            b.push(m);
        })
    };
    let (code, ls) = run_watch(&b, Query { chat: None }, false, |v| {
        v["data"]["message_id"] == 2
    });
    pusher.join().unwrap();
    assert_eq!(code, 0);
    let msgs = events(&ls, "new_message");
    assert_eq!(msgs[0]["data"]["notify"]["reason"], "topic_unknown");
    assert_eq!(msgs[1]["data"]["notify"]["reason"], "muted_topic");
    assert_eq!(
        b.calls()
            .iter()
            .filter(|c| **c == Call::ForumTopicGet(GROUP, 7))
            .count(),
        1
    );
}
