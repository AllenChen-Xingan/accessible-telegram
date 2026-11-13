//! `download CHAT_ID MSG_ID`：下载一条语音，输出本地路径（M2.5，T07；契约 `specs/contracts/cli.md`）。
//!
//! 按需下（大总管定 2026-11-13）：只在这条命令（以及以后的 `play` / `transcribe`）要时下。**不标已听**。
//! v0 不出进度行（语音很小；进度留给附件打开 M3.7 的大文件），不经本机接口转发：档案被常驻执行方占着就报
//! `profile_busy`（arch 定 2026-11-13；以后只转发下载这一步）。组合动作在 [`crate::voice`]。
//!
//! 下载的文件是明文、在档案目录里（`config-privacy.md` 第一节「库加密不覆盖下载的文件」）；路径只出到标准输出，
//! 不进日志。

use crate::args::{DownloadArgs, Format, Global};
use crate::history::ChatRef;
use crate::out::{self, Fail, Sink, Stdio, emit_to, envelope};
use crate::session;
use crate::voice::{VoiceFile, VoiceQuery};
use serde_json::json;
use std::process::ExitCode;
use std::time::Instant;

pub fn run(g: &Global, format: Format, a: &DownloadArgs, started: Instant) -> ExitCode {
    // 输出带本机路径（含用户名），打码还没做：不悄悄输出原文（同 `history` 的现行口径）
    if let Some(f) = crate::no_redact(g, "download") {
        return out::fail_with(format, &f);
    }
    let code = match query(a).and_then(|q| session::download(g, &q, started)) {
        Ok(v) => render(&mut Stdio, format, &v, started),
        Err(f) => out::fail_with_to(&mut Stdio, format, &f),
    };
    ExitCode::from(code)
}

pub fn query(a: &DownloadArgs) -> Result<VoiceQuery, Fail> {
    Ok(VoiceQuery {
        chat: ChatRef::parse(&a.chat).ok_or(Fail::Usage("会话写数字 id，或 saved（收藏夹）"))?,
        message_id: a.message,
    })
}

/// 结尾行：JSON 是 `result`（路径、字节数、时长）；文字模式只打印路径一行（数据，不是句子）
pub fn render(sink: &mut dyn Sink, format: Format, v: &VoiceFile, started: Instant) -> u8 {
    match format {
        Format::Json => emit_to(
            sink,
            &envelope(
                "result",
                json!({
                    "command": "download",
                    "chat_id": v.id.chat_id,
                    "message_id": v.id.message_id,
                    "path": v.file.path.to_string_lossy(),
                    "size": v.file.size,
                    "duration_s": v.duration,
                    "elapsed_ms": crate::elapsed_ms(started),
                }),
            ),
        ),
        Format::Text => sink.out(&v.file.path.to_string_lossy()),
    }
    0
}
