//! 找辅助程序的几种情况（arch 定 2026-11-14 列的：环境变量指错、往上找到 `_build` 却没有认人标记、权限不对）。
//! 权限位、属主只在 Unix 上有
#![cfg(unix)]
use super::*;
use std::os::unix::fs::PermissionsExt;

/// 每条测试一个干净的临时目录
fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("atg-stt-locate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// 放一个辅助程序（`mode` 是权限位）
fn put(p: &Path, mode: u32) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn env_var_is_the_only_place_looked_at_when_set() {
    let d = scratch("env");
    let exe = d.join("bin/atg-cli");
    put(&d.join("bin").join(NAME), 0o755);
    // 指错了：不往下找（旁边明明有一份）
    let missing = Some(d.join("nope").into_os_string());
    assert_eq!(locate_from(missing, Some(exe.clone())), Err(ENV_INVALID));
    let dir = Some(d.clone().into_os_string());
    assert_eq!(locate_from(dir, Some(exe.clone())), Err(ENV_INVALID));
    let good = d.join("mine");
    put(&good, 0o700);
    assert_eq!(
        locate_from(Some(good.clone().into_os_string()), Some(exe)),
        Ok(good)
    );
}

#[test]
fn beside_the_executable() {
    let d = scratch("beside");
    let h = d.join("bin").join(NAME);
    put(&h, 0o755);
    assert_eq!(locate_from(None, Some(d.join("bin/atg-cli"))), Ok(h));
}

#[test]
fn dev_build_needs_the_repo_marker() {
    let d = scratch("dev");
    let exe = d.join("repo/target/debug/atg-cli");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    put(&d.join("repo/_build/20261101/atg-stt"), 0o755);
    put(&d.join("repo/_build/20261114/atg-stt"), 0o755);
    // 有 _build 没有认人标记：不算根，不执行它
    assert_eq!(locate_from(None, Some(exe.clone())), Err(NOT_FOUND));
    std::fs::create_dir_all(d.join("repo").join(MARKER)).unwrap();
    assert_eq!(
        locate_from(None, Some(exe)),
        Ok(d.join("repo/_build/20261114/atg-stt"))
    );
}

#[test]
fn unsafe_or_not_runnable_is_refused() {
    let d = scratch("perm");
    let exe = d.join("bin/atg-cli");
    let h = d.join("bin").join(NAME);
    put(&h, 0o775);
    assert_eq!(locate_from(None, Some(exe.clone())), Err(UNSAFE), "组可写");
    put(&h, 0o757);
    assert_eq!(
        locate_from(None, Some(exe.clone())),
        Err(UNSAFE),
        "其他人可写"
    );
    put(&h, 0o644);
    assert_eq!(locate_from(None, Some(exe.clone())), Err(NOT_RUNNABLE));
    std::fs::remove_file(&h).unwrap();
    std::fs::create_dir_all(&h).unwrap();
    assert_eq!(locate_from(None, Some(exe)), Err(NOT_RUNNABLE), "是目录");
}

#[test]
fn nothing_anywhere_is_not_found() {
    let d = scratch("none");
    assert_eq!(locate_from(None, Some(d.join("atg-cli"))), Err(NOT_FOUND));
    assert_eq!(locate_from(None, None), Err(NOT_FOUND));
}
