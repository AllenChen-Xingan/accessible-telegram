//! 解码：音频文件 → 48 kHz 单声道 32 位浮点 PCM（exp007：macOS 27 的 AudioToolbox 原生解 OGG/Opus，也解 MP3 / M4A）。
//!
//! 用 AudioToolbox 的 C 接口 `ExtAudioFile`：打开文件 → 把「客户端格式」设成 48 kHz 单声道 f32（系统负责解码、
//! 混成单声道、重采样）→ 一块块读完。直接 FFI，不经 Swift、不加依赖。
//!
//! ⚠️ 解码器分不出文件完不完整（exp007：截断的文件静默解出半截）。调用方必须只交**已确认下完**的文件
//! （`atg-core` 的 `file_download` 只在 TDLib 报下完时交路径）。

use crate::AudioError;
use std::path::Path;

/// 输出采样率：Opus 本身就是 48 kHz，播放与转写都吃这个（转写那边由系统转成 16 kHz）
pub const SAMPLE_RATE: u32 = 48_000;

/// 解出来的声音：单声道，[`SAMPLE_RATE`]，每个采样一个 f32（-1..1）
#[derive(Clone, PartialEq)]
pub struct Pcm {
    pub samples: Vec<f32>,
}

impl Pcm {
    /// 时长（秒）
    pub fn duration_s(&self) -> f64 {
        self.samples.len() as f64 / f64::from(SAMPLE_RATE)
    }
}

// 声音内容不打印（虽然不是文字，也只给形状）
impl std::fmt::Debug for Pcm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pcm {{ frames: {} }}", self.samples.len())
    }
}

/// 把整个文件解成 PCM
pub fn decode(path: &Path) -> Result<Pcm, AudioError> {
    imp::decode(path)
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{AudioError, Path, Pcm, SAMPLE_RATE};
    use std::ffi::c_void;
    use std::os::unix::ffi::OsStrExt;

    type OSStatus = i32;
    type ExtAudioFileRef = *mut c_void;
    type CFURLRef = *const c_void;
    type CFAllocatorRef = *const c_void;

    /// `AudioStreamBasicDescription`（CoreAudioTypes）
    #[repr(C)]
    struct Asbd {
        sample_rate: f64,
        format_id: u32,
        format_flags: u32,
        bytes_per_packet: u32,
        frames_per_packet: u32,
        bytes_per_frame: u32,
        channels_per_frame: u32,
        bits_per_channel: u32,
        reserved: u32,
    }

    /// `AudioBuffer`
    #[repr(C)]
    struct AudioBuffer {
        channels: u32,
        byte_size: u32,
        data: *mut c_void,
    }

    /// 只放一个缓冲区的 `AudioBufferList`（单声道交错格式只要一个）
    #[repr(C)]
    struct AudioBufferList1 {
        count: u32,
        buffers: [AudioBuffer; 1],
    }

    const fn fourcc(s: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*s)
    }
    const K_AUDIO_FORMAT_LINEAR_PCM: u32 = fourcc(b"lpcm");
    /// `kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked`
    const FLOAT_PACKED: u32 = 1 | 8;
    /// `kExtAudioFileProperty_ClientDataFormat`
    const PROP_CLIENT_FORMAT: u32 = fourcc(b"cfmt");

    #[link(name = "AudioToolbox", kind = "framework")]
    unsafe extern "C" {
        fn ExtAudioFileOpenURL(url: CFURLRef, out: *mut ExtAudioFileRef) -> OSStatus;
        fn ExtAudioFileSetProperty(
            f: ExtAudioFileRef,
            id: u32,
            size: u32,
            data: *const c_void,
        ) -> OSStatus;
        fn ExtAudioFileRead(
            f: ExtAudioFileRef,
            frames: *mut u32,
            data: *mut AudioBufferList1,
        ) -> OSStatus;
        fn ExtAudioFileDispose(f: ExtAudioFileRef) -> OSStatus;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFURLCreateFromFileSystemRepresentation(
            alloc: CFAllocatorRef,
            buf: *const u8,
            len: isize,
            is_dir: u8,
        ) -> CFURLRef;
        fn CFRelease(cf: *const c_void);
    }

    /// 打开的文件；离开作用域就关
    struct File(ExtAudioFileRef);
    impl Drop for File {
        fn drop(&mut self) {
            // SAFETY: 句柄由 ExtAudioFileOpenURL 成功返回，只关一次
            unsafe {
                ExtAudioFileDispose(self.0);
            }
        }
    }

    fn open(path: &Path) -> Result<File, AudioError> {
        let bytes = path.as_os_str().as_bytes();
        let len = isize::try_from(bytes.len()).map_err(|_| AudioError::Decode)?;
        // SAFETY: 指针与长度来自同一切片；返回的 URL 用完就 CFRelease
        let url = unsafe {
            CFURLCreateFromFileSystemRepresentation(std::ptr::null(), bytes.as_ptr(), len, 0)
        };
        if url.is_null() {
            return Err(AudioError::Decode);
        }
        let mut f: ExtAudioFileRef = std::ptr::null_mut();
        // SAFETY: url 有效；f 是出参
        let st = unsafe { ExtAudioFileOpenURL(url, &mut f) };
        // SAFETY: url 由上面 Create 得来，只释放一次
        unsafe { CFRelease(url) };
        if st != 0 || f.is_null() {
            return Err(AudioError::Decode);
        }
        Ok(File(f))
    }

    pub fn decode(path: &Path) -> Result<Pcm, AudioError> {
        let file = open(path)?;
        let client = Asbd {
            sample_rate: f64::from(SAMPLE_RATE),
            format_id: K_AUDIO_FORMAT_LINEAR_PCM,
            format_flags: FLOAT_PACKED,
            bytes_per_packet: 4,
            frames_per_packet: 1,
            bytes_per_frame: 4,
            channels_per_frame: 1,
            bits_per_channel: 32,
            reserved: 0,
        };
        // SAFETY: 文件句柄有效；client 是按 C 布局的完整结构体
        let st = unsafe {
            ExtAudioFileSetProperty(
                file.0,
                PROP_CLIENT_FORMAT,
                std::mem::size_of::<Asbd>() as u32,
                (&raw const client).cast(),
            )
        };
        if st != 0 {
            return Err(AudioError::Decode);
        }
        const CHUNK: u32 = 8192;
        let mut samples: Vec<f32> = Vec::new();
        let mut buf = vec![0f32; CHUNK as usize];
        loop {
            let mut list = AudioBufferList1 {
                count: 1,
                buffers: [AudioBuffer {
                    channels: 1,
                    byte_size: CHUNK * 4,
                    data: buf.as_mut_ptr().cast(),
                }],
            };
            let mut frames = CHUNK;
            // SAFETY: list 指向 buf（CHUNK 个 f32），frames 不超过它的容量
            let st = unsafe { ExtAudioFileRead(file.0, &mut frames, &mut list) };
            if st != 0 {
                return Err(AudioError::Decode);
            }
            if frames == 0 {
                break;
            }
            samples.extend_from_slice(&buf[..frames as usize]);
        }
        if samples.is_empty() {
            return Err(AudioError::Decode);
        }
        Ok(Pcm { samples })
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::{AudioError, Path, Pcm};
    pub fn decode(_: &Path) -> Result<Pcm, AudioError> {
        Err(AudioError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn decodes_ogg_opus_with_priming_removed() {
        let p = decode(&fixture("zh.ogg")).unwrap();
        // afinfo：107129 个有效帧（312 个预跳过与尾部填充已去掉）
        assert_eq!(p.samples.len(), 107_129);
        assert!((p.duration_s() - 2.2318).abs() < 0.001);
        let peak = p.samples.iter().fold(0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.05 && peak <= 1.0, "应有声音、且在范围内");
    }

    #[test]
    fn truncated_file_decodes_part_without_crashing() {
        // 解码器分不出没下完（exp007）：只断言不崩、解出的比整份短
        let full = decode(&fixture("zh.ogg")).unwrap();
        if let Ok(part) = decode(&fixture("zh-truncated.ogg")) {
            assert!(part.samples.len() < full.samples.len());
        }
    }

    #[test]
    fn missing_or_garbage_file_is_decode_error() {
        assert_eq!(
            decode(&fixture("没有这个.ogg")).err(),
            Some(AudioError::Decode)
        );
        assert_eq!(decode(&fixture("make.sh")).err(), Some(AudioError::Decode));
    }

    #[test]
    fn debug_shows_only_shape() {
        let p = Pcm {
            samples: vec![0.5; 3],
        };
        assert_eq!(format!("{p:?}"), "Pcm { frames: 3 }");
    }
}
