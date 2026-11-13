//! `play CHAT_ID MSG_ID [--mark-listened]`：前台放一条语音（M2.6，T07；契约 `specs/contracts/cli.md`）。
//!
//! 外壳只做三件事：把标准输入的文字解析成 [`Cmd`]（命令集合只在 `atg_audio::player::Cmd` 定义一次）、
//! 把播放状态做成树节点 `Doc::Playback` 输出（句子由 `atg_core::speech` 出）、管退出。组合动作在 [`crate::voice`]。
//! 语义正本 `docs/experiments/exp008-local-stt/design.md` 第三节第 5、5b 条：
//! - 播放中拒绝一条命令**不出 `error`**（`error` 表示命令结束），出非终结的 `control_rejected`，照常播
//! - 标准输入 EOF：播着就播完为止；暂停着就按 `stop`（再没人能恢复）
//! - 默认不标已听；`--mark-listened` 在出口停着打开之后、出声之前标；阅后即焚的拒绝（L2 确认还没做）
//! - Ctrl+C：停、出 `error(interrupted)`、退出码 130
//!
//! v0 不经本机接口转发，档案被占报 `profile_busy`（arch 定 2026-11-13）；`--redact` 报 `not_implemented`。

use crate::args::{Format, Global, PlayArgs};
use crate::history;
use crate::out::{self, Fail, Sink, Stdio, emit_to, envelope};
use crate::voice::VoiceFile;
use atg_audio::player::{Cmd, End, Output, Player, Reject, State, Status};
use atg_core::speech::SpeechCtx;
use atg_core::tree::{Doc, PlayState, Playback};
use serde_json::json;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// 多久看一次「放完没有」「Ctrl+C 了没有」
const TICK: Duration = Duration::from_millis(50);

pub fn run(g: &Global, format: Format, a: &PlayArgs, started: Instant) -> ExitCode {
    if let Some(f) = crate::no_redact(g, "play") {
        return out::fail_with(format, &f);
    }
    // 一开始就装：准备阶段（下载、解码、标已听）按 Ctrl+C 只立标志，准备照常收尾、后端正常关闭，
    // 出声前的第一次检查就以 interrupted 结束（同 watch 先装再起后端）
    crate::signals::install();
    let r = crate::voice::query(&a.at).and_then(|q| {
        let cx = history::speech_ctx(g.tz.as_deref(), false)?;
        let (player, v) = crate::session::play_prepare(g, &q, a.mark_listened, started)?;
        Ok(drive(
            &mut Stdio,
            format,
            &cx,
            player,
            &stdin_lines(),
            &crate::signals::STOP,
        )
        .map(|end| (end, v)))
    });
    let code = match r {
        Ok(Ok((end, v))) => finish(&mut Stdio, format, end, &v, a.mark_listened, started),
        Ok(Err(f)) | Err(f) => out::fail_with_to(&mut Stdio, format, &f),
    };
    ExitCode::from(code)
}

/// 一行控制命令 → [`Cmd`]。说明是参数用法的说明（守则第 3 条允许直接写），不带输入原文
pub fn parse(line: &str) -> Result<Cmd, &'static str> {
    let mut it = line.split_whitespace();
    let (word, arg, extra) = (it.next(), it.next(), it.next());
    if extra.is_some() {
        return Err("多了参数；命令是 pause、resume、seek +N、seek -N、speed X、stop");
    }
    let num = |s: Option<&str>| s.and_then(|x| x.parse::<f64>().ok());
    match (word, arg) {
        (Some("pause"), None) => Ok(Cmd::Pause),
        (Some("resume"), None) => Ok(Cmd::Resume),
        (Some("stop"), None) => Ok(Cmd::Stop),
        (Some("seek"), Some(n)) if n.starts_with('+') || n.starts_with('-') => num(Some(n))
            .map(Cmd::Seek)
            .ok_or("seek 后面写 +N 或 -N（秒）"),
        (Some("seek"), _) => Err("seek 后面写 +N 或 -N（秒）"),
        (Some("speed"), Some(x)) => num(Some(x))
            .map(Cmd::Speed)
            .ok_or("speed 后面写倍速，0.5 到 2"),
        (Some("speed"), None) => Err("speed 后面写倍速，0.5 到 2"),
        _ => Err("不认识的命令；命令是 pause、resume、seek +N、seek -N、speed X、stop"),
    }
}

fn reject_zh(r: Reject) -> &'static str {
    match r {
        Reject::SpeedOutOfRange => "倍速要在 0.5 到 2 之间",
        Reject::NotFinite => "数字不对",
    }
}

/// 标准输入一行一条，读到头送 `None`（另起线程读，主循环不被阻塞）
fn stdin_lines() -> Receiver<Option<String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lines() {
            match line {
                Ok(l) => {
                    if tx.send(Some(l)).is_err() {
                        return;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(None);
    });
    rx
}

fn node(s: Status) -> Doc {
    Doc::Playback(Playback {
        state: match s.state {
            State::Playing => PlayState::Playing,
            State::Paused => PlayState::Paused,
        },
        position_s: s.position_s,
        duration_s: s.duration_s,
        speed: s.speed,
    })
}

fn emit_status(sink: &mut dyn Sink, format: Format, cx: &SpeechCtx, s: Status) {
    let doc = node(s);
    match format {
        Format::Json => {
            let mut v = atg_core::json::value(&doc, cx);
            if let Some(o) = v.as_object_mut() {
                o.insert("at".into(), json!(out::now_rfc3339_ms()));
            }
            emit_to(sink, &v);
        }
        Format::Text => sink.out(&atg_core::speech::brief(&doc, cx)),
    }
}

fn emit_rejected(sink: &mut dyn Sink, format: Format, message_zh: &str) {
    match format {
        Format::Json => emit_to(
            sink,
            &envelope(
                "control_rejected",
                json!({ "code": "invalid_input", "message_zh": message_zh }),
            ),
        ),
        Format::Text => sink.err(message_zh),
    }
}

/// 放：先出声、出一行状态，然后边收命令边看放完没有。`Err` 只有 Ctrl+C（`interrupted`）
pub fn drive<O: Output>(
    sink: &mut dyn Sink,
    format: Format,
    cx: &SpeechCtx,
    mut player: Player<O>,
    lines: &Receiver<Option<String>>,
    stop: &AtomicBool,
) -> Result<End, Fail> {
    // 准备期间按过 Ctrl+C：不出声，直接以 interrupted 结束
    if stop.load(Ordering::SeqCst) {
        let _ = player.apply(Cmd::Stop);
        return Err(Fail::Interrupted);
    }
    // 准备时出口是停着打开的（design.md 第三节第 5b 条）；后端已关，现在出声
    if let Ok(Some(end)) = player.apply(Cmd::Resume) {
        return Ok(end);
    }
    emit_status(sink, format, cx, player.status());
    let mut eof = false;
    loop {
        if stop.load(Ordering::SeqCst) {
            let _ = player.apply(Cmd::Stop);
            return Err(Fail::Interrupted);
        }
        let got = if eof {
            std::thread::sleep(TICK);
            None
        } else {
            match lines.recv_timeout(TICK) {
                Ok(Some(l)) => Some(l),
                Ok(None) | Err(RecvTimeoutError::Disconnected) => {
                    eof = true;
                    // 暂停着又没人能再发命令：按 stop
                    if player.status().state == State::Paused
                        && let Ok(Some(end)) = player.apply(Cmd::Stop)
                    {
                        return Ok(end);
                    }
                    None
                }
                Err(RecvTimeoutError::Timeout) => None,
            }
        };
        if let Some(l) = got {
            if l.trim().is_empty() {
                continue;
            }
            match parse(&l) {
                Ok(cmd) => match player.apply(cmd) {
                    Ok(Some(end)) => return Ok(end),
                    Ok(None) => emit_status(sink, format, cx, player.status()),
                    Err(r) => emit_rejected(sink, format, reject_zh(r)),
                },
                Err(zh) => emit_rejected(sink, format, zh),
            }
        }
        if let Some(end) = player.tick() {
            return Ok(end);
        }
    }
}

/// 出声失败时给人的话：标了已听的要说清楚（已听无法撤回，设计第 5b 条：不假装没标）
const OUTPUT_FAILED_MARKED_ZH: &str = "声音放不出来；这条语音已经标为已听（无法撤回）";

/// 结尾行：放完或停 → `result`（`reason:"finished"|"stopped"`），退出码 0；
/// 出口坏了 → `error`（`internal{what:"audio_output_failed"}`，带 `marked_listened`）
pub fn finish(
    sink: &mut dyn Sink,
    format: Format,
    end: End,
    v: &VoiceFile,
    marked: bool,
    started: Instant,
) -> u8 {
    let reason = match end {
        End::Finished => "finished",
        End::Stopped => "stopped",
        End::OutputFailed => return output_failed(sink, format, marked),
    };
    match format {
        Format::Json => emit_to(
            sink,
            &envelope(
                "result",
                json!({
                    "command": "play",
                    "reason": reason,
                    "chat_id": v.id.chat_id,
                    "message_id": v.id.message_id,
                    "marked_listened": marked,
                    "elapsed_ms": crate::elapsed_ms(started),
                }),
            ),
        ),
        Format::Text => sink.err(reason),
    }
    0
}

fn output_failed(sink: &mut dyn Sink, format: Format, marked: bool) -> u8 {
    let e = crate::voice::audio_fail(atg_audio::AudioError::Output);
    let Fail::Core(core) = &e else {
        return out::fail_with_to(sink, format, &e);
    };
    let mut data = out::core_error_data(core);
    data["marked_listened"] = json!(marked);
    if marked {
        data["message_zh"] = json!(OUTPUT_FAILED_MARKED_ZH);
    }
    match format {
        Format::Json => emit_to(sink, &envelope("error", data)),
        Format::Text => sink.out(if marked {
            OUTPUT_FAILED_MARKED_ZH
        } else {
            core.message_zh()
        }),
    }
    out::core_exit(core)
}

#[cfg(test)]
mod tests {
    use super::*;
    use atg_audio::AudioError;
    use atg_audio::decode::{Pcm, SAMPLE_RATE};
    use std::sync::Arc;
    use std::sync::mpsc;

    /// 假出口：`played` 每被问一次就前进 `step` 帧（模拟时间在走）
    struct Ticking {
        played: std::cell::Cell<usize>,
        step: usize,
    }
    impl Output for Ticking {
        fn start(&mut self, _: &Arc<Pcm>, _: usize, _: f64, _: bool) -> Result<(), AudioError> {
            self.played.set(0);
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
        let pcm = Arc::new(Pcm {
            samples: vec![0.0; 2 * SAMPLE_RATE as usize],
        });
        Player::start_paused(
            Ticking {
                played: std::cell::Cell::new(0),
                step,
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
}
