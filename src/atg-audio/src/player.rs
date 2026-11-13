//! 播放状态机（M2.6 / M3.8 共用）：纯逻辑，声音从哪出由 [`Output`] 决定（真的是 AudioQueue，测试用假的）。
//!
//! **控制命令只在 [`Cmd`] 定义一次**（arch 定 2026-11-13）：CLI 把标准输入的文字解析成它，GUI 按键直接给它。
//! 语义正本：`docs/experiments/exp008-local-stt/design.md` 第三节第 5 条——
//! - 倍速必须是有限数且在 [`SPEED_MIN`]–[`SPEED_MAX`]：越界、非有限数 → 拒绝这一条（[`Reject`]），照常播，不截断
//! - 跳转：秒数必须是有限数；结果位置截到 [0, 时长]；截到时长 = 直接播完（不论播着还是暂停）
//! - 跳转时声音出口按新位置重开（不先放完已排队的旧音频）；暂停中跳转（没到末尾）、改倍速：仍是暂停
//! - `Stop`：立刻停
//! - **出口坏了**（`resume` / 改倍速 / 跳转重开失败、出口自报失效、在播却连着 [`STALL`] 没前进）→ 结束为
//!   [`End::OutputFailed`]，不当作正常的停（第 5b 条：标了已听之后出声失败要报错，不假装成功）

use crate::AudioError;
use crate::decode::{Pcm, SAMPLE_RATE};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const SPEED_MIN: f64 = 0.5;
pub const SPEED_MAX: f64 = 2.0;
/// 在播状态下位置连着这么久没前进 = 出口失效（缓冲区约 85 毫秒一个，正常时远小于它）
pub const STALL: Duration = Duration::from_secs(3);

/// 控制命令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cmd {
    Pause,
    Resume,
    /// 相对跳转，秒（正往后、负往前）
    Seek(f64),
    /// 倍速
    Speed(f64),
    Stop,
}

/// 为什么拒绝一条命令（播放照常继续）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    /// 倍速不在范围内
    SpeedOutOfRange,
    /// 不是有限数（NaN、无穷）
    NotFinite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Playing,
    Paused,
}

/// 播放怎么结束的
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    Finished,
    Stopped,
    /// 声音出口坏了（见模块说明）
    OutputFailed,
}

/// 一次状态快照（CLI 据它出 `Doc::Playback`）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Status {
    pub state: State,
    pub position_s: f64,
    pub duration_s: f64,
    pub speed: f64,
}

/// 声音出口。`start` 从某一帧起放；`paused` 为真则**只准备、不启动**（一个采样都不出声，等 `resume`）。
/// `played` 是自上次 `start` 以来**已经放完**的帧数；`failed` = 出口在播放中自己发现坏了
pub trait Output {
    fn start(
        &mut self,
        pcm: &Arc<Pcm>,
        from: usize,
        speed: f64,
        paused: bool,
    ) -> Result<(), AudioError>;
    fn pause(&mut self);
    fn resume(&mut self) -> Result<(), AudioError>;
    fn set_speed(&mut self, speed: f64) -> Result<(), AudioError>;
    /// 立刻停、释放资源（之后可再 `start`）
    fn stop(&mut self);
    fn played(&self) -> usize;
    fn failed(&self) -> bool;
}

pub struct Player<O: Output> {
    out: O,
    pcm: Arc<Pcm>,
    /// 上次 `start` 的起点帧
    base: usize,
    state: State,
    speed: f64,
    ended: Option<End>,
    /// 上次看到位置前进：（位置，时刻）——在播却连着 `stall` 没动就判出口失效
    progress: (usize, Instant),
    stall: Duration,
}

impl<O: Output> Player<O> {
    /// 从头开始放
    pub fn start(out: O, pcm: Arc<Pcm>) -> Result<Self, AudioError> {
        Self::open(out, pcm, false)
    }

    /// 打开出口但停在开头（`play --mark-listened`：先确认放得出来，标完已听再 `Resume`）
    pub fn start_paused(out: O, pcm: Arc<Pcm>) -> Result<Self, AudioError> {
        Self::open(out, pcm, true)
    }

    fn open(mut out: O, pcm: Arc<Pcm>, paused: bool) -> Result<Self, AudioError> {
        out.start(&pcm, 0, 1.0, paused)?;
        Ok(Player {
            out,
            pcm,
            base: 0,
            state: if paused {
                State::Paused
            } else {
                State::Playing
            },
            speed: 1.0,
            ended: None,
            progress: (0, Instant::now()),
            stall: STALL,
        })
    }

    /// 改「多久没前进算失效」（测试用短的）
    pub fn with_stall(mut self, stall: Duration) -> Self {
        self.stall = stall;
        self
    }

    fn reset_progress(&mut self) {
        self.progress = (self.position(), Instant::now());
    }

    fn total(&self) -> usize {
        self.pcm.samples.len()
    }

    fn position(&self) -> usize {
        (self.base + self.out.played()).min(self.total())
    }

    pub fn status(&self) -> Status {
        let rate = f64::from(SAMPLE_RATE);
        Status {
            state: self.state,
            position_s: self.position() as f64 / rate,
            duration_s: self.total() as f64 / rate,
            speed: self.speed,
        }
    }

    /// 已经结束了就是 `Some`
    pub fn ended(&self) -> Option<End> {
        self.ended
    }

    fn end(&mut self, how: End) -> Option<End> {
        self.out.stop();
        self.ended = Some(how);
        self.ended
    }

    /// 定时调：放到头了就结束；出口自报失效、或在播却连着 `stall` 没前进，就结束为 `OutputFailed`
    pub fn tick(&mut self) -> Option<End> {
        if self.ended.is_some() {
            return self.ended;
        }
        let pos = self.position();
        if pos >= self.total() {
            return self.end(End::Finished);
        }
        if self.out.failed() {
            return self.end(End::OutputFailed);
        }
        if self.state == State::Playing {
            if pos != self.progress.0 {
                self.progress = (pos, Instant::now());
            } else if self.progress.1.elapsed() >= self.stall {
                return self.end(End::OutputFailed);
            }
        }
        None
    }

    /// 执行一条命令。`Err` = 拒绝（状态不变，照常播）；`Ok(Some(_))` = 因此结束了
    pub fn apply(&mut self, cmd: Cmd) -> Result<Option<End>, Reject> {
        if let Some(e) = self.ended {
            return Ok(Some(e));
        }
        match cmd {
            Cmd::Pause => {
                if self.state == State::Playing {
                    self.out.pause();
                    self.state = State::Paused;
                }
            }
            Cmd::Resume => {
                if self.state == State::Paused {
                    if self.out.resume().is_err() {
                        return Ok(self.end(End::OutputFailed));
                    }
                    self.state = State::Playing;
                    self.reset_progress();
                }
            }
            Cmd::Speed(x) => {
                if !x.is_finite() {
                    return Err(Reject::NotFinite);
                }
                if !(SPEED_MIN..=SPEED_MAX).contains(&x) {
                    return Err(Reject::SpeedOutOfRange);
                }
                self.speed = x;
                if self.out.set_speed(x).is_err() {
                    return Ok(self.end(End::OutputFailed));
                }
            }
            Cmd::Seek(d) => {
                if !d.is_finite() {
                    return Err(Reject::NotFinite);
                }
                let delta = (d * f64::from(SAMPLE_RATE)).round();
                let target =
                    (self.position() as f64 + delta).clamp(0.0, self.total() as f64) as usize;
                if target >= self.total() {
                    return Ok(self.end(End::Finished));
                }
                self.out.stop();
                self.base = target;
                if self
                    .out
                    .start(&self.pcm, target, self.speed, self.state == State::Paused)
                    .is_err()
                {
                    // 出口重开失败：放不下去了，不当作正常的停
                    return Ok(self.end(End::OutputFailed));
                }
                self.reset_progress();
            }
            Cmd::Stop => return Ok(self.end(End::Stopped)),
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 假出口：记下调用，`played` 由测试手动推进
    #[derive(Default)]
    struct Fake {
        calls: Vec<String>,
        played: usize,
        fail_start: bool,
        fail_resume: bool,
        broken: bool,
    }

    impl Output for &mut Fake {
        fn start(
            &mut self,
            _: &Arc<Pcm>,
            from: usize,
            speed: f64,
            paused: bool,
        ) -> Result<(), AudioError> {
            self.calls.push(format!("start {from} {speed} {paused}"));
            self.played = 0;
            if self.fail_start {
                Err(AudioError::Output)
            } else {
                Ok(())
            }
        }
        fn pause(&mut self) {
            self.calls.push("pause".into());
        }
        fn resume(&mut self) -> Result<(), AudioError> {
            self.calls.push("resume".into());
            if self.fail_resume {
                Err(AudioError::Output)
            } else {
                Ok(())
            }
        }
        fn set_speed(&mut self, s: f64) -> Result<(), AudioError> {
            self.calls.push(format!("speed {s}"));
            Ok(())
        }
        fn stop(&mut self) {
            self.calls.push("stop".into());
        }
        fn played(&self) -> usize {
            self.played
        }
        fn failed(&self) -> bool {
            self.broken
        }
    }

    /// 10 秒的静音
    fn pcm() -> Arc<Pcm> {
        Arc::new(Pcm {
            samples: vec![0.0; 10 * SAMPLE_RATE as usize],
        })
    }

    const S: usize = SAMPLE_RATE as usize;

    #[test]
    fn pause_resume_and_status() {
        let mut f = Fake::default();
        let mut p = Player::start(&mut f, pcm()).unwrap();
        assert_eq!(p.status().state, State::Playing);
        assert_eq!(p.apply(Cmd::Pause), Ok(None));
        assert_eq!(p.apply(Cmd::Pause), Ok(None)); // 重复暂停无害
        assert_eq!(p.status().state, State::Paused);
        assert_eq!(p.apply(Cmd::Resume), Ok(None));
        assert_eq!(p.status().duration_s, 10.0);
        drop(p);
        assert_eq!(f.calls, vec!["start 0 1 false", "pause", "resume"]);
    }

    #[test]
    fn speed_range_and_rejects() {
        let mut f = Fake::default();
        let mut p = Player::start(&mut f, pcm()).unwrap();
        assert_eq!(p.apply(Cmd::Speed(1.5)), Ok(None));
        assert_eq!(p.apply(Cmd::Speed(0.5)), Ok(None));
        assert_eq!(p.apply(Cmd::Speed(2.0)), Ok(None));
        assert_eq!(p.apply(Cmd::Speed(2.01)), Err(Reject::SpeedOutOfRange));
        assert_eq!(p.apply(Cmd::Speed(0.0)), Err(Reject::SpeedOutOfRange));
        assert_eq!(p.apply(Cmd::Speed(f64::NAN)), Err(Reject::NotFinite));
        assert_eq!(p.apply(Cmd::Speed(f64::INFINITY)), Err(Reject::NotFinite));
        assert_eq!(p.status().speed, 2.0, "拒绝不改倍速");
        assert_eq!(p.status().state, State::Playing);
    }

    #[test]
    fn seek_clamps_restarts_and_keeps_pause() {
        let mut f = Fake::default();
        let mut p = Player::start(&mut f, pcm()).unwrap();
        p.out.played = 3 * S;
        assert_eq!(p.apply(Cmd::Seek(2.0)), Ok(None));
        assert_eq!(p.status().position_s, 5.0);
        assert_eq!(p.apply(Cmd::Seek(-100.0)), Ok(None)); // 截到 0
        assert_eq!(p.status().position_s, 0.0);
        p.apply(Cmd::Pause).unwrap();
        assert_eq!(p.apply(Cmd::Seek(4.0)), Ok(None));
        assert_eq!(p.status().state, State::Paused, "暂停中跳转仍暂停");
        assert_eq!(p.apply(Cmd::Seek(f64::NAN)), Err(Reject::NotFinite));
        drop(p);
        assert_eq!(
            f.calls,
            vec![
                "start 0 1 false",
                "stop",
                &format!("start {} 1 false", 5 * S),
                "stop",
                "start 0 1 false",
                "pause",
                "stop",
                &format!("start {} 1 true", 4 * S),
            ]
        );
    }

    #[test]
    fn seek_to_end_finishes_even_when_paused() {
        let mut f = Fake::default();
        let mut p = Player::start(&mut f, pcm()).unwrap();
        p.apply(Cmd::Pause).unwrap();
        assert_eq!(p.apply(Cmd::Seek(60.0)), Ok(Some(End::Finished)));
        assert_eq!(p.ended(), Some(End::Finished));
        // 结束之后的命令不再动出口
        assert_eq!(p.apply(Cmd::Resume), Ok(Some(End::Finished)));
        drop(p);
        assert_eq!(f.calls.last().map(String::as_str), Some("stop"));
        assert_eq!(f.calls.iter().filter(|c| *c == "stop").count(), 1);
    }

    #[test]
    fn tick_finishes_at_end_and_stop_stops() {
        let mut f = Fake::default();
        let mut p = Player::start(&mut f, pcm()).unwrap();
        p.out.played = 9 * S;
        assert_eq!(p.tick(), None);
        p.out.played = 10 * S + 5;
        assert_eq!(p.tick(), Some(End::Finished));
        assert_eq!(p.status().position_s, 10.0, "位置不超过时长");

        let mut g = Fake::default();
        let mut q = Player::start(&mut g, pcm()).unwrap();
        assert_eq!(q.apply(Cmd::Stop), Ok(Some(End::Stopped)));
        assert_eq!(q.tick(), Some(End::Stopped));
    }

    #[test]
    fn start_paused_then_resume() {
        let mut f = Fake::default();
        let mut p = Player::start_paused(&mut f, pcm()).unwrap();
        assert_eq!(p.status().state, State::Paused);
        p.apply(Cmd::Resume).unwrap();
        assert_eq!(p.status().state, State::Playing);
        drop(p);
        assert_eq!(f.calls, vec!["start 0 1 true", "resume"]);
    }

    #[test]
    fn output_failures_end_as_output_failed_not_stopped() {
        // 跳转时重开失败
        let mut f = Fake::default();
        let mut p = Player::start(&mut f, pcm()).unwrap();
        p.out.fail_start = true;
        assert_eq!(p.apply(Cmd::Seek(1.0)), Ok(Some(End::OutputFailed)));
        // 停着打开后 resume 失败（标了已听之后出声失败的那条路）
        let mut g = Fake {
            fail_resume: true,
            ..Fake::default()
        };
        let mut q = Player::start_paused(&mut g, pcm()).unwrap();
        assert_eq!(q.apply(Cmd::Resume), Ok(Some(End::OutputFailed)));
        drop(q);
        assert_eq!(g.calls.last().map(String::as_str), Some("stop"), "释放出口");
        // 出口自报失效
        let mut h = Fake::default();
        let mut r = Player::start(&mut h, pcm()).unwrap();
        r.out.broken = true;
        assert_eq!(r.tick(), Some(End::OutputFailed));
    }

    #[test]
    fn stalled_while_playing_is_output_failed_but_paused_is_not() {
        let mut f = Fake::default();
        let mut p = Player::start(&mut f, pcm())
            .unwrap()
            .with_stall(Duration::from_millis(30));
        p.apply(Cmd::Pause).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(p.tick(), None, "暂停时不动是正常的");
        p.apply(Cmd::Resume).unwrap();
        assert_eq!(p.tick(), None);
        p.out.played = 100;
        assert_eq!(p.tick(), None, "前进了");
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(p.tick(), Some(End::OutputFailed));
    }
}
