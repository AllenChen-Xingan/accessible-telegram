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
        Ok(Err(f)) | Err(f) => fail(&mut Stdio, format, &f),
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
        // 空行只跳过解析，每轮照样看放完没有、出口坏没坏
        if let Some(l) = got.filter(|l| !l.trim().is_empty()) {
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

/// 准备阶段的失败：出口打不开时也按出口失效的形状出（带 `marked_listened:false`——标在出口好了之后，没标），
/// 别的照常
pub fn fail(sink: &mut dyn Sink, format: Format, f: &Fail) -> u8 {
    match f {
        Fail::Core(atg_core::error::CoreError::Internal { what })
            if *what == "audio_output_failed" =>
        {
            output_failed(sink, format, false)
        }
        _ => out::fail_with_to(sink, format, f),
    }
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
mod tests;
