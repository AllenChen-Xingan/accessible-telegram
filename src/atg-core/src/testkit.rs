//! 测试共用的夹具（#60 R3，`docs/plan/010_complexity-budget.md` S11）：临时目录与测试档案。
//!
//! 只在测试里编：本 crate 的单元测试（`cfg(test)`），以及开了 `testkit` 特性的地方——`atg-core` 自己的集成测试
//! （`tests/*.rs`，经自身 dev-dependency 开）和 `atg-cli` 的测试（经它的 dev-dependency 开）。正式构建里没有它。
//! 因为开特性时不算 `cfg(test)`，这里照产品代码的规矩写：不用 `unwrap` / `expect`、不用 `{:?}`（`lib.rs` 头上那行 lint 与
//! `scripts/check-no-debug.sh` 管着）。
//!
//! 造 TDLib JSON 的助手（各测试里的 `chat`、`msg`……）还没收进来，见 010 的 R3b。

use crate::config::{Profile, ServerKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// 一个新的空临时目录：`系统临时目录/atg-<tag>-<进程号>-<本进程第几个>`。先删同名残留再建。
///
/// 同一进程里每次调用都不同名（计数器），并行跑的测试、同一个 `tag` 用两次都不会撞；不同进程靠进程号分开。
/// 测完不删（留着便于查失败），系统临时目录自会清
pub fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let d = std::env::temp_dir().join(format!(
        "atg-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&d);
    if let Err(e) = std::fs::create_dir_all(&d) {
        panic!("testkit：建临时目录 {} 失败：{e}", d.display());
    }
    d
}

/// 在新的临时目录里建一个叫 `main` 的档案
pub fn profile(tag: &str, kind: ServerKind) -> Profile {
    match Profile::create(&temp_dir(tag), "main", kind) {
        Ok(p) => p,
        Err(e) => panic!("testkit：建档案失败：{}", e.code()),
    }
}
