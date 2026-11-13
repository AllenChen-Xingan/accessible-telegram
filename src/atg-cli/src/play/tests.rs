use super::*;
use atg_audio::AudioError;
use atg_audio::decode::{Pcm, SAMPLE_RATE};
use std::sync::Arc;
use std::sync::mpsc;

/// 假出口：`played` 每被问一次就前进 `step` 帧（模拟时间在走）
struct Ticking {
    played: std::cell::Cell<usize>,
    step: usize,
    fail_resume: bool,
}
impl Output for Ticking {
    fn start(&mut self, _: &Arc<Pcm>, _: usize, _: f64, _: bool) -> Result<(), AudioError> {
        self.played.set(0);
        Ok(())
    }
    fn pause(&mut self) -> Result<(), AudioError> {
        Ok(())
    }
    fn resume(&mut self) -> Result<(), AudioError> {
        if self.fail_resume {
            Err(AudioError::Output)
        } else {
            Ok(())
        }
    }
    fn set_speed(&mut self, _: f64) -> Result<(), AudioError> {
        Ok(())
    }
    fn failed(&self) -> bool {
        false
    }
    fn stop(&mut self) {}
    fn played(&self) -> usize {
        let p = self.played.get();
        self.played.set(p + self.step);
        p
    }
}

#[derive(Default)]
struct Buf {
    out: Vec<String>,
    err: Vec<String>,
}
impl Sink for Buf {
    fn out(&mut self, l: &str) {
        self.out.push(l.to_owned());
    }
    fn err(&mut self, l: &str) {
        self.err.push(l.to_owned());
    }
}

fn cx() -> SpeechCtx {
    history::speech_ctx(Some("+08:00"), false).unwrap_or_else(|_| panic!("时区"))
}

fn player(step: usize) -> Player<Ticking> {
    player_with(step, false)
}

fn player_with(step: usize, fail_resume: bool) -> Player<Ticking> {
    let pcm = Arc::new(Pcm {
        samples: vec![0.0; 2 * SAMPLE_RATE as usize],
    });
    Player::start_paused(
        Ticking {
            played: std::cell::Cell::new(0),
            step,
            fail_resume,
        },
        pcm,
    )
    .unwrap()
}

fn kinds(b: &Buf) -> Vec<String> {
    b.out
        .iter()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[test]
fn parses_commands() {
    assert_eq!(parse("pause"), Ok(Cmd::Pause));
    assert_eq!(parse(" resume "), Ok(Cmd::Resume));
    assert_eq!(parse("stop"), Ok(Cmd::Stop));
    assert_eq!(parse("seek +5"), Ok(Cmd::Seek(5.0)));
    assert_eq!(parse("seek -2.5"), Ok(Cmd::Seek(-2.5)));
    assert_eq!(parse("speed 1.5"), Ok(Cmd::Speed(1.5)));
    for bad in [
        "seek 5",
        "seek",
        "speed",
        "speed 快",
        "jump",
        "pause now",
        "seek +x",
    ] {
        assert!(parse(bad).is_err(), "{bad}");
    }
    // NaN 能解析成数，留给播放器拒绝（不在这里判范围）
    assert!(matches!(parse("speed NaN"), Ok(Cmd::Speed(x)) if x.is_nan()));
}

#[test]
fn plays_to_the_end_with_rejections_in_between() {
    let (tx, rx) = mpsc::channel();
    for l in ["speed 9", "jump", "speed 1.5"] {
        tx.send(Some(l.to_owned())).unwrap();
    }
    let mut b = Buf::default();
    let end = drive(
        &mut b,
        Format::Json,
        &cx(),
        player(SAMPLE_RATE as usize / 4),
        &rx,
        &AtomicBool::new(false),
    );
    assert!(matches!(end, Ok(End::Finished)));
    assert_eq!(
        kinds(&b),
        vec![
            "playback",
            "control_rejected",
            "control_rejected",
            "playback"
        ]
    );
    let first: serde_json::Value = serde_json::from_str(&b.out[0]).unwrap();
    assert_eq!(first["data"]["type"], "playback");
    assert_eq!(first["data"]["state"], "playing");
    assert!(
        first["speech"]["brief"]
            .as_str()
            .unwrap()
            .starts_with("播放中")
    );
    drop(tx);
}

#[test]
fn eof_while_paused_stops_eof_while_playing_finishes() {
    let (tx, rx) = mpsc::channel();
    tx.send(Some("pause".into())).unwrap();
    tx.send(None).unwrap();
    let mut b = Buf::default();
    let end = drive(
        &mut b,
        Format::Json,
        &cx(),
        player(1000),
        &rx,
        &AtomicBool::new(false),
    );
    assert!(matches!(end, Ok(End::Stopped)));

    let (tx, rx) = mpsc::channel::<Option<String>>();
    drop(tx); // 一开始就 EOF，播着
    let mut b = Buf::default();
    let end = drive(
        &mut b,
        Format::Json,
        &cx(),
        player(SAMPLE_RATE as usize),
        &rx,
        &AtomicBool::new(false),
    );
    assert!(matches!(end, Ok(End::Finished)));
}

#[test]
fn stop_and_ctrl_c() {
    let (tx, rx) = mpsc::channel();
    tx.send(Some("stop".into())).unwrap();
    let mut b = Buf::default();
    assert!(matches!(
        drive(
            &mut b,
            Format::Json,
            &cx(),
            player(1),
            &rx,
            &AtomicBool::new(false)
        ),
        Ok(End::Stopped)
    ));
    let (_tx, rx) = mpsc::channel::<Option<String>>();
    let mut b = Buf::default();
    assert!(matches!(
        drive(
            &mut b,
            Format::Json,
            &cx(),
            player(1),
            &rx,
            &AtomicBool::new(true)
        ),
        Err(Fail::Interrupted)
    ));
}

#[test]
fn output_failed_after_mark_is_an_error_that_says_marked() {
    let v = VoiceFile {
        id: atg_core::model::MsgId {
            chat_id: 1,
            message_id: 2,
        },
        duration: 3,
        file: atg_core::backend::Downloaded {
            path: "/编的".into(),
            size: 1,
        },
        self_destruct: false,
    };
    let mut b = Buf::default();
    let code = finish(
        &mut b,
        Format::Json,
        End::OutputFailed,
        &v,
        true,
        Instant::now(),
    );
    assert_ne!(code, 0);
    let l: serde_json::Value = serde_json::from_str(&b.out[0]).unwrap();
    assert_eq!(l["kind"], "error");
    assert_eq!(l["data"]["code"], "internal");
    assert_eq!(l["data"]["what"], "audio_output_failed");
    assert_eq!(l["data"]["marked_listened"], true);
    assert!(
        l["data"]["message_zh"]
            .as_str()
            .unwrap()
            .contains("已经标为已听")
    );
    let mut b = Buf::default();
    finish(
        &mut b,
        Format::Json,
        End::OutputFailed,
        &v,
        false,
        Instant::now(),
    );
    let l: serde_json::Value = serde_json::from_str(&b.out[0]).unwrap();
    assert_eq!(l["data"]["marked_listened"], false);
}

/// 标了已听之后出声失败，从放到结尾整条走一遍：只有一行结尾、是 error、注明已标；准备阶段出口打不开也同形（没标）
#[test]
fn resume_failure_end_to_end_one_error_line() {
    let (_tx, rx) = mpsc::channel::<Option<String>>();
    let mut b = Buf::default();
    let end = drive(
        &mut b,
        Format::Json,
        &cx(),
        player_with(1, true),
        &rx,
        &AtomicBool::new(false),
    );
    assert!(matches!(end, Ok(End::OutputFailed)));
    assert!(b.out.is_empty(), "出声失败前不出状态行");
    let v = VoiceFile {
        id: atg_core::model::MsgId {
            chat_id: 1,
            message_id: 2,
        },
        duration: 3,
        file: atg_core::backend::Downloaded {
            path: "/编的".into(),
            size: 1,
        },
        self_destruct: false,
    };
    let code = finish(
        &mut b,
        Format::Json,
        End::OutputFailed,
        &v,
        true,
        Instant::now(),
    );
    assert_ne!(code, 0);
    assert_eq!(kinds(&b), vec!["error"]);
    let l: serde_json::Value = serde_json::from_str(&b.out[0]).unwrap();
    assert_eq!(l["data"]["marked_listened"], true);

    let mut b = Buf::default();
    let f = crate::voice::audio_fail(AudioError::Output);
    assert_ne!(fail(&mut b, Format::Json, &f), 0);
    assert_eq!(kinds(&b), vec!["error"]);
    let l: serde_json::Value = serde_json::from_str(&b.out[0]).unwrap();
    assert_eq!(l["data"]["what"], "audio_output_failed");
    assert_eq!(l["data"]["marked_listened"], false);
}

/// 标准输入一直送空行：照样看得到放完（空行不跳过检查）
#[test]
fn blank_lines_do_not_block_finishing() {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for _ in 0..10_000 {
            if tx.send(Some(String::new())).is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    let mut b = Buf::default();
    let end = drive(
        &mut b,
        Format::Json,
        &cx(),
        player(SAMPLE_RATE as usize / 2),
        &rx,
        &AtomicBool::new(false),
    );
    assert!(matches!(end, Ok(End::Finished)));
    assert_eq!(kinds(&b), vec!["playback"]);
}

#[test]
fn text_mode_prints_sentences_and_rejections_to_stderr() {
    let (tx, rx) = mpsc::channel();
    tx.send(Some("speed 3".into())).unwrap();
    tx.send(Some("stop".into())).unwrap();
    let mut b = Buf::default();
    let _ = drive(
        &mut b,
        Format::Text,
        &cx(),
        player(1),
        &rx,
        &AtomicBool::new(false),
    );
    assert_eq!(b.out, vec!["播放中，0 秒，共 2 秒"]);
    assert_eq!(b.err, vec!["倍速要在 0.5 到 2 之间"]);
}
