//! 媒体相关的节点：带文件的附件共用的小类型（`specs/content-tree.md` 第三节第 3 小节）、播放状态（`Doc::Playback`）。

use serde::Serialize;

/// 附件背后的那个文件。图片、视频、圆视频、音频、文件以后都挂这一个类型（M3.7），现在只有语音用。
///
/// `id` 是 TDLib 的文件 id：只在这次运行里有效的传输句柄，不是给人看的数据，**JSON 里不输出**。
/// 以后「下没下完」这类给人看的状态加在这里、照常输出；现在没有可输出的字段，整体不输出（快照不动，`v` 不加一）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FileRef {
    #[serde(skip)]
    pub id: i32,
}

impl FileRef {
    /// 序列化时整体跳过：还没有给人看的字段
    pub fn nothing_to_show(&self) -> bool {
        true
    }
}

/// 过期的阅后即焚媒体是什么（`messageExpired*`，`td_api.tl:5752-5761`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Expirable {
    Photo,
    Video,
    VideoNote,
    Voice,
}

/// 播放状态（`Doc::Playback`，`atg-cli play` 每次状态变化一行；arch 定 2026-11-13：要读出来的都先成树）。
/// 时间是秒（小数）；句子只由 `speech` 出
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Playback {
    pub state: PlayState,
    pub position_s: f64,
    pub duration_s: f64,
    pub speed: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayState {
    Playing,
    Paused,
}
