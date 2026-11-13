//! 播放状态的读法（`Doc::Playback`，`atg-cli play` 的每一行；M2.6）。**句子是 AI 草稿，待人实听**。
//!
//! 「播放中，3 秒，共 12 秒」「已暂停，3 秒，共 12 秒」；倍速不是 1 时末尾加「，1.5 倍速」。
//! 句子的正本是快照 `tests/snapshots/snapshots__playback_brief.snap`（`content-tree.md` 第九节：说法只认快照）。
//! 秒数向下取整（读「3 秒」不读「3.4 秒」）；时长用消息读法同一个说法（`super::dur`）。

use super::dur;
use crate::tree::{PlayState, Playback};

pub(super) fn brief(p: &Playback) -> String {
    let state = match p.state {
        PlayState::Playing => "播放中",
        PlayState::Paused => "已暂停",
    };
    let secs = |s: f64| {
        if s.is_finite() && s > 0.0 {
            s.floor().min(f64::from(u32::MAX)) as u32
        } else {
            0
        }
    };
    let mut out = format!(
        "{state}，{}，共 {}",
        dur(secs(p.position_s)),
        dur(secs(p.duration_s))
    );
    if (p.speed - 1.0).abs() > f64::EPSILON {
        out.push_str(&format!("，{} 倍速", p.speed));
    }
    out
}
