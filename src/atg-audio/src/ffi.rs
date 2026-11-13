//! AudioToolbox 的共用 C 布局与常量（解码 `decode` 与出声 `queue` 共用，**只在这里定义一次**）。
//! 名字后面括号里是苹果头文件里的原名，方便对照 `CoreAudioTypes.h`、`AudioQueue.h`、`ExtendedAudioFile.h`。

use crate::decode::SAMPLE_RATE;

/// `OSStatus`：0 = 成功
pub type OSStatus = i32;

/// 音频格式描述（`AudioStreamBasicDescription`，CoreAudioTypes.h）
#[repr(C)]
pub struct Asbd {
    pub sample_rate: f64,
    pub format_id: u32,
    pub format_flags: u32,
    pub bytes_per_packet: u32,
    pub frames_per_packet: u32,
    pub bytes_per_frame: u32,
    pub channels_per_frame: u32,
    pub bits_per_channel: u32,
    pub reserved: u32,
}

/// 四字符码（苹果的 `'lpcm'` 这类写法）→ 整数
pub const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}

/// 线性 PCM（`kAudioFormatLinearPCM`）
pub const LINEAR_PCM: u32 = fourcc(b"lpcm");
/// 浮点、紧排（`kAudioFormatFlagIsFloat` = 1 | `kAudioFormatFlagIsPacked` = 8）
pub const FLOAT_PACKED: u32 = 1 | 8;

/// 本 crate 唯一的 PCM 格式：[`SAMPLE_RATE`] Hz、单声道、32 位浮点、紧排。解码出它，出声吃它
pub fn pcm_f32_mono() -> Asbd {
    Asbd {
        sample_rate: f64::from(SAMPLE_RATE),
        format_id: LINEAR_PCM,
        format_flags: FLOAT_PACKED,
        bytes_per_packet: 4,
        frames_per_packet: 1,
        bytes_per_frame: 4,
        channels_per_frame: 1,
        bits_per_channel: 32,
        reserved: 0,
    }
}
