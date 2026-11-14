//! `atg-audio`：本机音频能力（#4 听语音、#59 语音转文字；F8 = A，`docs/plan/003_decisions-b2.md`）。
//!
//! 只认文件路径和 PCM，不认识消息、会话——不依赖 `atg-core`，也不依赖界面框架；`atg-cli` 与以后的 GUI 共用。
//! **解码只做一份**：[`decode`] 出的同一份 48 kHz 单声道 f32 PCM，播放与转写都用它。
//! 设计正本：`docs/experiments/exp008-local-stt/design.md`；依据：exp007（系统 AudioToolbox 原生解 OGG/Opus）。
//!
//! 只在 macOS 上有实现（AudioToolbox）；别的平台编得过，解码、播放调用时报 [`AudioError::Unsupported`]，
//! 转写找不到辅助程序、报 [`stt::SttError::Unavailable`]。

// 与 atg-core 同一行（拍板 005 补丁 ③，`scripts/check-no-debug.sh` 检查每个 crate 根都带着）
#![cfg_attr(
    not(test),
    deny(
        clippy::use_debug,
        clippy::dbg_macro,
        clippy::unwrap_used,
        clippy::expect_used
    )
)]

pub mod decode;
mod error;
#[cfg(target_os = "macos")]
mod ffi;
pub mod player;
pub mod queue;
pub mod stt;

pub use decode::{Pcm, SAMPLE_RATE, decode};
pub use error::AudioError;
pub use player::{Cmd, End, Player, Reject, State, Status};
pub use queue::Queue;
