//! reducer 与投影的单测：全用手写的假事件，不经 JSON、不需要 TDLib。
//! 重点：乱序到达、重复到达、晚到的旧应答、撤回 vs 清缓存、发送成功换 id（架构方案第一节第 3 小节第 6 条）。

use atg_core::model::*;
use atg_core::project;
use atg_core::store::{Entry, Input, Store, StoreConfig, reduce};
use atg_core::tree::{self, Doc, Sender, Span};

#[path = "../common/mod.rs"]
mod common;
use common::*;

mod cache;
mod chats;
mod fields;
mod history;
mod projection;
mod restricted;
mod send;
mod tombstones;

// ───── cache、restricted 两个主题都用的助手 ─────

fn small(keep: usize) -> Store {
    Store::with_config(StoreConfig {
        keep_per_chat: keep,
    })
}
