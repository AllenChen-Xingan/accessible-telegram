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
//! 进度（#60 的 R3 分两步：R3a 收临时目录与测试档案，R3b 收造 TDLib JSON 的助手）：R3a 第一刀（2026-11-14）换了 core
//! `tests/config.rs`、`tests/lifecycle.rs`，cli `history/tests.rs`、`session/tests.rs`；第二刀（2026-11-17）换了 core
//! `send/tests.rs`、`send/react/tests.rs`、`send_limits/tests.rs`、`tests/config.rs` 的 `test-phone`，cli `chats`、`login`、
//! `send`、`react`、`pending`、`voice`（含 `voice/tests/transcribe.rs`）、`watch` 的测试。
//! 第三刀（2026-11-17）换了 core `gate/tests.rs`、`delete/tests.rs`、`edit/tests.rs`、`send/forward/tests.rs`，cli
//! `delete/tests.rs`、`edit/tests.rs`、`forwarding/tests.rs`、`tests/cli.rs` 的 speech 一处。
//! #12 合并后（2026-11-17，随 R7 拆 `fake.rs`）换了 cli `tests/cli.rs` 的 `send_usage_paths` 和 `send_file/tests.rs`；R3a 换完。
//! 故意不换：core `ipc.rs` 的测试（要 0o700 的目录并断言它）；cli `serve/tests.rs`（套接字路径离 103 字节上限近）；core
//! `tests/send_limits.rs`（同一个 tag 要再算出同一个路径，模拟下一个进程）；`atg-audio`（不依赖 atg-core）。各处都有注释写明原因。
//! R3b 还没做。

use crate::config::{Profile, ServerKind};
use std::path::PathBuf;

/// 一个新的空临时目录：`系统临时目录/atg-<tag>-<进程号>-<本进程第几个>`，权限 0700。正式代码那一份是
/// [`crate::config::private_temp_dir`]（返回 `Result`），这里只是测试包装：出错就 panic（#60 R3；PR-2 arch 审定把实现挪进 config）。
///
/// 同一进程里每次调用都不同名（计数器），并行跑的测试、同一个 `tag` 用两次都不会撞；不同进程靠进程号分开。
/// 同名残留先删，删不掉就 panic——绝不把一个删了一半、还留着旧东西的目录当成新的交出去（codex 044 第 1 轮 #1）。
/// 测完不删（留着便于查失败；macOS 的系统临时目录会被定期清，CI 每次是新机器）
pub fn temp_dir(tag: &str) -> PathBuf {
    match crate::config::private_temp_dir(tag) {
        Ok(d) => d,
        Err(e) => panic!("testkit：建临时目录（tag {tag}）失败：{}", e.code()),
    }
}

/// 在新的临时目录里建一个档案。档案名固定是 `main`（CLI 的默认档案名），与服务器种类无关：`kind` 传 `Test` 得到的是
/// 一个叫 main 的测试服务器档案。要别的名字就用 [`temp_dir`] 自己 `Profile::create`（如 `tests/lifecycle.rs`）
pub fn profile(tag: &str, kind: ServerKind) -> Profile {
    match Profile::create(&temp_dir(tag), "main", kind) {
        Ok(p) => p,
        Err(e) => panic!("testkit：建档案失败：{}", e.code()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn temp_dir_is_new_and_empty_even_with_same_tag() {
        let a = temp_dir("testkit-same");
        std::fs::write(a.join("x"), b"x").unwrap();
        let b = temp_dir("testkit-same");
        assert_ne!(a, b);
        assert_eq!(std::fs::read_dir(&b).unwrap().count(), 0);
    }

    #[test]
    fn leftover_is_wiped() {
        let d = temp_dir("testkit-left");
        std::fs::create_dir_all(d.join("old/deep")).unwrap();
        crate::config::fresh_private_dir(&d).unwrap();
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0);
    }

    #[test]
    fn leftover_that_cannot_be_removed_is_an_error_not_reused() {
        // root 不受目录权限限制，删得掉，这条测不出来
        // SAFETY: geteuid 不读写内存、总是成功
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let d = temp_dir("testkit-stuck");
        let locked = d.join("locked");
        std::fs::create_dir_all(locked.join("inner")).unwrap();
        // 子目录去掉全部权限：普通用户删不掉里面的东西，remove_dir_all 会失败
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let r = crate::config::fresh_private_dir(&d);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(&d);
        assert!(r.is_err(), "删不掉残留时必须报错，不能把旧目录当新的");
    }
}
