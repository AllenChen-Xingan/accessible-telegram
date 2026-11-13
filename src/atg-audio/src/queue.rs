//! 真的声音出口：AudioToolbox 的 AudioQueue（C 接口，直接 FFI），实现 [`crate::player::Output`]。
//!
//! - 格式：48 kHz 单声道 f32（与 [`crate::decode`] 出的 PCM 一样，不再转换）
//! - 倍速：开 AudioQueue 自带的变速不变调（`EnableTimePitch` + 频谱算法），`PlayRate` 0.5–2
//! - 位置：每个缓冲区放完回到回调时才计入「已放完」——粒度一个缓冲区（约 85 毫秒），与倍速无关
//! - 跳转：由播放器先 `stop` 再从新位置 `start`（整个队列重建），旧的排队音频随之丢掉
//! - 「停着打开」（`start` 带 `paused`）：建好队列、排好缓冲区，但**不调 `AudioQueueStart`**——一个采样都不出声；
//!   `resume` 才第一次 Start（`play --mark-listened`：标成功之后才出声，设计第 5b 条）
//! - 出错：`resume` / 改倍速返回系统的失败；回调里重新排队失败记进 `failed`，播放器据此结束为 `OutputFailed`
//!
//! 回调在 AudioQueue 自己的线程上跑；它读写的状态在 [`Feed`] 里，用锁保护。队列先 `Dispose`（同步）再释放
//! `Feed`，回调不会摸到已释放的内存。

use crate::AudioError;
use crate::decode::Pcm;
use crate::player::Output;
use std::sync::{Arc, Mutex, PoisonError};

/// 每个缓冲区的帧数（约 85 毫秒）与缓冲区个数
const FRAMES_PER_BUFFER: usize = 4096;
const BUFFERS: usize = 3;

/// 回调与主线程共享的状态
struct Feed {
    pcm: Arc<Pcm>,
    /// 下一个要排进队列的帧
    next: usize,
    /// 已放完的帧（自本次 `start` 起）
    played: usize,
    /// 正在停：回调什么都不做
    stopping: bool,
    /// 回调里重新排队失败（出口在播放中坏了）
    failed: bool,
}

/// AudioQueue 声音出口。`muted` 为真时音量设 0（自动化测试用，不出声）
pub struct Queue {
    muted: bool,
    live: Option<imp::Live>,
}

impl Queue {
    pub fn new() -> Self {
        Queue {
            muted: false,
            live: None,
        }
    }

    /// 音量 0 的出口：走完整的系统播放路径但不出声（测试用）
    pub fn muted() -> Self {
        Queue {
            muted: true,
            live: None,
        }
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Output for Queue {
    fn start(
        &mut self,
        pcm: &Arc<Pcm>,
        from: usize,
        speed: f64,
        paused: bool,
    ) -> Result<(), AudioError> {
        self.stop();
        self.live = Some(imp::Live::start(
            pcm.clone(),
            from,
            speed,
            paused,
            self.muted,
        )?);
        Ok(())
    }

    fn pause(&mut self) {
        if let Some(l) = &self.live {
            l.pause();
        }
    }

    fn resume(&mut self) -> Result<(), AudioError> {
        self.live
            .as_ref()
            .map_or(Err(AudioError::Output), imp::Live::resume)
    }

    fn set_speed(&mut self, speed: f64) -> Result<(), AudioError> {
        self.live
            .as_ref()
            .map_or(Err(AudioError::Output), |l| l.set_speed(speed))
    }

    fn stop(&mut self) {
        // Live 的 Drop 做：标记停、同步 Dispose、再释放共享状态
        self.live = None;
    }

    fn played(&self) -> usize {
        self.live.as_ref().map_or(0, imp::Live::played)
    }

    fn failed(&self) -> bool {
        self.live.as_ref().is_some_and(imp::Live::failed)
    }
}

fn lock(f: &Mutex<Feed>) -> std::sync::MutexGuard<'_, Feed> {
    f.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{AudioError, BUFFERS, FRAMES_PER_BUFFER, Feed, Pcm, lock};
    use crate::ffi::{Asbd, OSStatus, fourcc, pcm_f32_mono};
    use std::ffi::c_void;
    use std::sync::{Arc, Mutex};

    type AudioQueueRef = *mut c_void;

    /// `AudioQueueBuffer`（AudioQueue.h）
    #[repr(C)]
    struct AudioQueueBuffer {
        capacity: u32,
        data: *mut c_void,
        byte_size: u32,
        user_data: *mut c_void,
        packet_desc_capacity: u32,
        packet_descs: *mut c_void,
        packet_desc_count: u32,
    }
    type AudioQueueBufferRef = *mut AudioQueueBuffer;

    type Callback = extern "C" fn(*mut c_void, AudioQueueRef, AudioQueueBufferRef);

    /// 开变速不变调（`kAudioQueueProperty_EnableTimePitch`）
    const PROP_ENABLE_TIME_PITCH: u32 = fourcc(b"q_tp");
    /// 变速算法（`kAudioQueueProperty_TimePitchAlgorithm`）
    const PROP_TIME_PITCH_ALGORITHM: u32 = fourcc(b"qtpa");
    /// 频谱算法，适合人声（`kAudioQueueTimePitchAlgorithm_Spectral`）
    const ALGORITHM_SPECTRAL: u32 = fourcc(b"spec");
    /// 音量（`kAudioQueueParam_Volume`）
    const PARAM_VOLUME: u32 = 1;
    /// 播放速率（`kAudioQueueParam_PlayRate`）
    const PARAM_PLAY_RATE: u32 = 2;

    #[link(name = "AudioToolbox", kind = "framework")]
    unsafe extern "C" {
        fn AudioQueueNewOutput(
            format: *const Asbd,
            callback: Callback,
            user: *mut c_void,
            run_loop: *const c_void,
            run_loop_mode: *const c_void,
            flags: u32,
            out: *mut AudioQueueRef,
        ) -> OSStatus;
        fn AudioQueueAllocateBuffer(
            q: AudioQueueRef,
            bytes: u32,
            out: *mut AudioQueueBufferRef,
        ) -> OSStatus;
        fn AudioQueueEnqueueBuffer(
            q: AudioQueueRef,
            b: AudioQueueBufferRef,
            n: u32,
            descs: *const c_void,
        ) -> OSStatus;
        fn AudioQueueSetProperty(
            q: AudioQueueRef,
            id: u32,
            data: *const c_void,
            size: u32,
        ) -> OSStatus;
        fn AudioQueueSetParameter(q: AudioQueueRef, id: u32, value: f32) -> OSStatus;
        fn AudioQueueStart(q: AudioQueueRef, at: *const c_void) -> OSStatus;
        fn AudioQueuePause(q: AudioQueueRef) -> OSStatus;
        fn AudioQueueDispose(q: AudioQueueRef, immediate: u8) -> OSStatus;
    }

    /// 填一块的结果
    enum Fill {
        Queued,
        NoMoreData,
        EnqueueFailed,
    }

    /// 把下一段 PCM 填进缓冲区并排队；没有数据了就不排
    ///
    /// # Safety
    /// `q`、`b` 是同一个活着的队列与它分配的缓冲区
    unsafe fn refill(feed: &mut Feed, q: AudioQueueRef, b: AudioQueueBufferRef) -> Fill {
        let total = feed.pcm.samples.len();
        if feed.next >= total {
            return Fill::NoMoreData;
        }
        let n = FRAMES_PER_BUFFER.min(total - feed.next);
        let src = &feed.pcm.samples[feed.next..feed.next + n];
        // SAFETY: 缓冲区容量是 FRAMES_PER_BUFFER 个 f32，n 不超过它
        unsafe {
            std::ptr::copy_nonoverlapping(src.as_ptr(), (*b).data.cast::<f32>(), n);
            (*b).byte_size = (n * 4) as u32;
            // 记下这块有几帧，放完回来时计入「已放完」
            (*b).user_data = n as *mut c_void;
        }
        feed.next += n;
        // SAFETY: 同上
        if unsafe { AudioQueueEnqueueBuffer(q, b, 0, std::ptr::null()) } == 0 {
            Fill::Queued
        } else {
            Fill::EnqueueFailed
        }
    }

    extern "C" fn on_buffer_done(user: *mut c_void, q: AudioQueueRef, b: AudioQueueBufferRef) {
        // SAFETY: user 是 Live 持有的 Box<Mutex<Feed>> 的地址，队列 Dispose 之前一直有效
        let feed = unsafe { &*(user as *const Mutex<Feed>) };
        let mut f = lock(feed);
        if f.stopping {
            return;
        }
        // SAFETY: b 是这个队列交回的缓冲区
        let frames = unsafe { (*b).user_data } as usize;
        f.played += frames;
        // SAFETY: 同上
        if let Fill::EnqueueFailed = unsafe { refill(&mut f, q, b) } {
            f.failed = true;
        }
    }

    pub struct Live {
        q: AudioQueueRef,
        feed: Box<Mutex<Feed>>,
    }

    impl Live {
        pub fn start(
            pcm: Arc<Pcm>,
            from: usize,
            speed: f64,
            paused: bool,
            muted: bool,
        ) -> Result<Self, AudioError> {
            let feed = Box::new(Mutex::new(Feed {
                pcm,
                next: from,
                played: 0,
                stopping: false,
                failed: false,
            }));
            let format: Asbd = pcm_f32_mono();
            let mut q: AudioQueueRef = std::ptr::null_mut();
            let user = (&raw const *feed).cast_mut().cast::<c_void>();
            // SAFETY: 格式结构完整；run loop 传空 = 回调在队列自己的线程上；user 在 Live 活着时一直有效
            let st = unsafe {
                AudioQueueNewOutput(
                    &format,
                    on_buffer_done,
                    user,
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    &mut q,
                )
            };
            if st != 0 || q.is_null() {
                return Err(AudioError::Output);
            }
            // 先建好 Live：之后任何一步失败，Drop 都会 Dispose
            let live = Live { q, feed };
            let one: u32 = 1;
            let alg = ALGORITHM_SPECTRAL;
            // SAFETY: q 有效；属性值是按值传的 u32
            let ok = unsafe {
                AudioQueueSetProperty(q, PROP_ENABLE_TIME_PITCH, (&raw const one).cast(), 4) == 0
                    && AudioQueueSetProperty(
                        q,
                        PROP_TIME_PITCH_ALGORITHM,
                        (&raw const alg).cast(),
                        4,
                    ) == 0
                    && AudioQueueSetParameter(q, PARAM_PLAY_RATE, speed as f32) == 0
                    && AudioQueueSetParameter(q, PARAM_VOLUME, if muted { 0.0 } else { 1.0 }) == 0
            };
            if !ok {
                return Err(AudioError::Output);
            }
            {
                let mut f = lock(&live.feed);
                for _ in 0..BUFFERS {
                    let mut b: AudioQueueBufferRef = std::ptr::null_mut();
                    // SAFETY: q 有效；b 是出参
                    let st = unsafe {
                        AudioQueueAllocateBuffer(q, (FRAMES_PER_BUFFER * 4) as u32, &mut b)
                    };
                    if st != 0 || b.is_null() {
                        return Err(AudioError::Output);
                    }
                    // SAFETY: b 是刚从 q 分配的
                    match unsafe { refill(&mut f, q, b) } {
                        Fill::Queued => {}
                        Fill::NoMoreData => break,
                        Fill::EnqueueFailed => return Err(AudioError::Output),
                    }
                }
            }
            // 停着打开：不 Start，一个采样都不出声，等 resume
            if !paused {
                live.resume()?;
            }
            Ok(live)
        }

        pub fn pause(&self) {
            // SAFETY: q 有效
            unsafe { AudioQueuePause(self.q) };
        }

        /// 第一次开始、或暂停后继续（都是 `AudioQueueStart`）
        pub fn resume(&self) -> Result<(), AudioError> {
            // SAFETY: q 有效
            match unsafe { AudioQueueStart(self.q, std::ptr::null()) } {
                0 => Ok(()),
                _ => Err(AudioError::Output),
            }
        }

        pub fn set_speed(&self, speed: f64) -> Result<(), AudioError> {
            // SAFETY: q 有效
            match unsafe { AudioQueueSetParameter(self.q, PARAM_PLAY_RATE, speed as f32) } {
                0 => Ok(()),
                _ => Err(AudioError::Output),
            }
        }

        pub fn played(&self) -> usize {
            lock(&self.feed).played
        }

        pub fn failed(&self) -> bool {
            lock(&self.feed).failed
        }
    }

    impl Drop for Live {
        fn drop(&mut self) {
            lock(&self.feed).stopping = true;
            // SAFETY: q 有效；immediate = 同步停下并释放，返回后回调不会再跑，之后才释放 feed
            unsafe { AudioQueueDispose(self.q, 1) };
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::{AudioError, Pcm};
    use std::sync::Arc;

    pub struct Live;
    impl Live {
        pub fn start(_: Arc<Pcm>, _: usize, _: f64, _: bool, _: bool) -> Result<Self, AudioError> {
            Err(AudioError::Unsupported)
        }
        pub fn pause(&self) {}
        pub fn resume(&self) -> Result<(), AudioError> {
            Err(AudioError::Unsupported)
        }
        pub fn set_speed(&self, _: f64) -> Result<(), AudioError> {
            Err(AudioError::Unsupported)
        }
        pub fn played(&self) -> usize {
            0
        }
        pub fn failed(&self) -> bool {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{Cmd, End, Player};
    use std::time::{Duration, Instant};

    /// 走真实的系统播放路径，**音量 0、不出声**；要有声音设备。0.3 秒的静音，2 倍速，应在 1 秒内放完
    #[test]
    #[ignore = "要声音设备；手动跑：cargo test -p atg-audio -- --ignored"]
    fn muted_queue_plays_to_the_end_at_double_speed() {
        let pcm = Arc::new(Pcm {
            samples: vec![0.0; (crate::decode::SAMPLE_RATE as usize) * 3 / 10],
        });
        let mut p = Player::start(Queue::muted(), pcm).unwrap();
        p.apply(Cmd::Speed(2.0)).unwrap();
        let t0 = Instant::now();
        while p.tick().is_none() {
            assert!(t0.elapsed() < Duration::from_secs(2), "没放完");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(p.ended(), Some(End::Finished));
    }
}
