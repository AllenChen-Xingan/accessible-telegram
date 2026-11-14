//! `transcribe CHAT_ID MSG_ID [--lang zh|en] [--reveal]`：本机把一条语音转成文字（#59，取代 F4；契约 `specs/contracts/cli.md`）。
//!
//! 外壳只做：找辅助程序、选语种、把这一次的结果（树节点 `Doc::Transcription`）输出、管退出。组合动作在
//! [`crate::voice::transcribe`]。**L0**：不外发（Apple 端侧模型；断网未实测，待人验证）、不标已听、不标已读。
//! - 输出一行 `kind:"transcription"`（带读法）+ `result`；文字模式只出读法那一句
//! - 18+ 遮挡的语音不加 `--reveal`：照转、照写表，输出不带文字（`outcome:{type:"hidden"}`），标准错误提示加 `--reveal`
//!   （大总管定 2026-11-14，同 `history` 的遮挡口径）
//! - Ctrl+C：看门线程杀掉辅助程序，出 `error(interrupted)`、130
//!
//! v0 不经本机接口转发，档案被占报 `profile_busy`（arch 定 2026-11-13）；`--redact` 报 `not_implemented`（输出转写原文）。

use crate::args::{Format, Global, Lang, TranscribeArgs};
use crate::history;
use crate::out::{self, Sink, Stdio, emit_to, envelope};
use atg_core::speech::{self, SpeechCtx};
use atg_core::tree::{Doc, Outcome, Transcription};
use serde_json::json;
use std::process::ExitCode;
use std::time::Instant;

pub fn run(g: &Global, format: Format, a: &TranscribeArgs, started: Instant) -> ExitCode {
    if let Some(f) = crate::no_redact(g, "transcribe") {
        return out::fail_with(format, &f);
    }
    // 一开始就装：转写中按 Ctrl+C 只立标志，看门线程杀掉辅助程序，后端照常关（同 `play`）
    crate::signals::install();
    let r = crate::voice::query(&a.at).and_then(|q| {
        let cx = history::speech_ctx(g.tz.as_deref(), a.reveal)?;
        Ok((
            crate::session::transcribe(g, &q, locales(a.lang), started)?,
            cx,
        ))
    });
    let code = match r {
        Ok((t, cx)) => render(&mut Stdio, format, &cx, t, started),
        Err(f) => out::fail_with_to(&mut Stdio, format, &f),
    };
    ExitCode::from(code)
}

/// 要跑的语种：指定了就单跑，否则中文、英文双跑（相等取中文，排前面）
pub fn locales(lang: Option<Lang>) -> Vec<String> {
    let all = match lang {
        Some(Lang::Zh) => &["zh-CN"][..],
        Some(Lang::En) => &["en-US"][..],
        None => &["zh-CN", "en-US"][..],
    };
    all.iter().map(|s| (*s).to_owned()).collect()
}

/// 节点一行 + 结尾行（JSON）；文字模式只出读法。回退出码
pub fn render(
    sink: &mut dyn Sink,
    format: Format,
    cx: &SpeechCtx,
    t: Transcription,
    started: Instant,
) -> u8 {
    let hidden = t.sensitive && !cx.reveal;
    let (id, kind) = (t.id, outcome_kind(&t.outcome, hidden));
    let doc = Doc::Transcription(t);
    if hidden {
        sink.err("atg-cli transcribe: 这条语音被标为敏感内容，转写没有输出；要读出来加 --reveal");
    }
    match format {
        Format::Json => {
            let mut line = atg_core::json::value(&doc, cx);
            if let Some(o) = line.as_object_mut() {
                o.insert("at".into(), json!(out::now_rfc3339_ms()));
            }
            emit_to(sink, &line);
            emit_to(
                sink,
                &envelope(
                    "result",
                    json!({
                        "command": "transcribe",
                        "chat_id": id.chat_id,
                        "message_id": id.message_id,
                        "outcome": kind,
                        "elapsed_ms": crate::elapsed_ms(started),
                    }),
                ),
            );
        }
        Format::Text => sink.out(&speech::brief(&doc, cx)),
    }
    0
}

/// 结尾行里的 `outcome`（同节点里 `outcome.type`，遮着的是 `hidden`）
fn outcome_kind(o: &Outcome, hidden: bool) -> &'static str {
    match o {
        _ if hidden => "hidden",
        Outcome::Text(_) => "text",
        Outcome::NoSpeech => "no_speech",
        Outcome::Hidden => "hidden",
    }
}

#[cfg(test)]
mod tests;
