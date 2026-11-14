//! `atg-audio` 自己的小错误枚举。映射到 `CoreError` / CLI 的 `Fail` **只写一处**，在 `atg-cli`（arch 定 2026-11-13）。
//! 只带码，不带路径、内容或系统给的说明文字（`config-privacy.md` 黑名单：文件名、正文）。

/// 音频出错的种类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioError {
    /// 文件打不开、不是认得的音频、解不出来（文件坏或本项目的 bug）
    Decode,
    /// 声音出口打不开或出错（没有声音设备、系统拒绝）
    Output,
    /// 这个平台没有实现（非 macOS）
    Unsupported,
}

impl AudioError {
    /// 码（进 `internal{what}` 的那个字符串；转写的码在 [`crate::stt::SttError::what`]）
    pub fn what(self) -> &'static str {
        match self {
            AudioError::Decode => "audio_decode_failed",
            AudioError::Output => "audio_output_failed",
            AudioError::Unsupported => "audio_unsupported",
        }
    }
}
