//! 测试共用的夹具（#60 R3，`docs/plan/010_complexity-budget.md` S11）：临时目录与测试档案。
//!
//! 只在测试里编：本 crate 的单元测试（`cfg(test)`），以及开了 `testkit` 特性的地方——`atg-core` 自己的集成测试
//! （`tests/*.rs`，经自身 dev-dependency 开）和 `atg-cli` 的测试（经它的 dev-dependency 开）。正式构建里没有它。
//! 因为开特性时不算 `cfg(test)`，这里照产品代码的规矩写：不用 `unwrap` / `expect`、不用 `{:?}`（`lib.rs` 头上那行 lint 与
//! `scripts/check-no-debug.sh` 管着）。
//!
//! 怎么核「正式构建里没有它」：`cargo tree -p atg-cli -e normal -f '{p} {f}' | grep atg-core`，atg-core 后面不该出现
//! `testkit`。规矩：这个特性只许写在 `[dev-dependencies]` 里。
//!
//! **atg-core 自己的单元测试里写 `crate::testkit`，别写 `atg_core::testkit`**：自身 dev-dependency 让单元测试的二进制里
//! 同时有两份 atg-core（`crate` 和 extern 的 `atg_core`），两份的类型不通用（`atg_core::config::Profile` ≠
//! `crate::config::Profile`），计数器也各有一个——混着用，同一个 `tag` 会算出同一个路径。集成测试（`tests/*.rs`）
//! 和 atg-cli 只有一份，写 `atg_core::testkit`。
//!
//! `tag` 写成「测试套-用途」（如 `cfg-garbled`），留下的目录才认得出是谁的。
//!
//! 进度（R3a 第一刀，2026-11-14）：已换 core `tests/config.rs`、`tests/lifecycle.rs`，cli `history/tests.rs`、
//! `session/tests.rs`。还没换的同类助手（各在别的线手上，那条线合并后各补一刀）：core `gate/tests.rs`、`send_limits.rs`、
//! `send/tests.rs`、`ipc.rs` 的测试、`tests/send_limits.rs`；cli `chats.rs`（R2a 后在 `chats/tests.rs`）、`login/tests.rs`、
//! `send/tests.rs`、`serve/tests.rs`、`voice/tests.rs`、`watch/tests.rs`。`tests/config.rs` 里 `atg-test-phone` 那处
//! 是一个文件路径，不是目录助手，不换。造 TDLib JSON 的助手（各测试里的 `chat`、`msg`……）是 R3b。

use crate::config::{Profile, ServerKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// 一个新的空临时目录：`系统临时目录/atg-<tag>-<进程号>-<本进程第几个>`。先删同名残留再建。
///
/// 同一进程里每次调用都不同名（计数器），并行跑的测试、同一个 `tag` 用两次都不会撞；不同进程靠进程号分开。
/// 测完不删（留着便于查失败；macOS 的系统临时目录会被定期清，CI 每次是新机器）。目录在 `std::env::temp_dir()` 下
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

/// 在新的临时目录里建一个档案。档案名固定是 `main`（CLI 的默认档案名），与服务器种类无关：`kind` 传 `Test` 得到的是
/// 一个叫 main 的测试服务器档案。要别的名字就用 [`temp_dir`] 自己 `Profile::create`（如 `tests/lifecycle.rs`）
pub fn profile(tag: &str, kind: ServerKind) -> Profile {
    match Profile::create(&temp_dir(tag), "main", kind) {
        Ok(p) => p,
        Err(e) => panic!("testkit：建档案失败：{}", e.code()),
    }
}
