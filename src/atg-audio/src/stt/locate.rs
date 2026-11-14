//! 找辅助程序 `atg-stt`（arch 定 2026-11-14；design 第三节第 7 条）。三处，按先后：
//! 1. 环境变量 `ATG_STT_HELPER`：设了就**只认它**，指的不是能执行的普通文件 → `helper_env_invalid`，不往下找
//! 2. 可执行文件同目录（以后 app 包里放 Resources）
//! 3. 开发时：从可执行文件往上找仓库根——**同时**有 `src/atg-audio/stt-helper/` 才算根（找到就停），取它 `_build/*/atg-stt`
//!    里最新一份。不编译期写死路径（会把本机路径编进二进制）；认人标记免得跑到家目录里不相干的 `_build/` 里的程序
//!
//! 找到的文件还要过一道：普通文件、属主可执行、属于当前用户、组和其他人都不可写；不满足报 `helper_unsafe` /
//! `helper_not_runnable`，**不执行**。错误是写死的码（进 `stt_unavailable{what}`），不带路径

use std::ffi::OsString;
use std::path::{Path, PathBuf};

const NAME: &str = "atg-stt";
/// 仓库根的认人标记（相对仓库根）
const MARKER: &str = "src/atg-audio/stt-helper";

/// 三处都没找到
pub const NOT_FOUND: &str = "helper_not_found";
/// 环境变量指的不是能执行的普通文件
pub const ENV_INVALID: &str = "helper_env_invalid";
/// 找到了，但不是普通文件或属主不能执行
pub const NOT_RUNNABLE: &str = "helper_not_runnable";
/// 不属于当前用户，或组、其他人可写
pub const UNSAFE: &str = "helper_unsafe";

/// 找辅助程序；找不到或不安全回写死的码
pub fn locate() -> Result<PathBuf, &'static str> {
    locate_from(
        std::env::var_os("ATG_STT_HELPER"),
        std::env::current_exe().ok(),
    )
}

/// 同 [`locate`]，环境变量与可执行文件路径由调用方给（测试用）
pub(crate) fn locate_from(
    env: Option<OsString>,
    exe: Option<PathBuf>,
) -> Result<PathBuf, &'static str> {
    if let Some(p) = env {
        let p = PathBuf::from(p);
        return match vet(&p) {
            Err(NOT_RUNNABLE) => Err(ENV_INVALID),
            r => r.map(|()| p),
        };
    }
    let dir = exe.as_deref().and_then(Path::parent).ok_or(NOT_FOUND)?;
    let beside = dir.join(NAME);
    if beside.exists() {
        return vet(&beside).map(|()| beside);
    }
    let root = dir
        .ancestors()
        .find(|d| d.join(MARKER).is_dir())
        .ok_or(NOT_FOUND)?;
    let p = latest(&root.join("_build")).ok_or(NOT_FOUND)?;
    vet(&p).map(|()| p)
}

/// `_build/<时间戳>/atg-stt` 里名字排最后（最新）的那份
fn latest(build: &Path) -> Option<PathBuf> {
    let mut runs: Vec<PathBuf> = std::fs::read_dir(build)
        .ok()?
        .flatten()
        .map(|e| e.path().join(NAME))
        .filter(|p| p.exists())
        .collect();
    runs.sort();
    runs.pop()
}

/// 能不能放心执行（跟随符号链接，看最终那个文件）
#[cfg(unix)]
fn vet(p: &Path) -> Result<(), &'static str> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::metadata(p).map_err(|_| NOT_RUNNABLE)?;
    if !m.is_file() || m.mode() & 0o100 == 0 {
        return Err(NOT_RUNNABLE);
    }
    if m.uid() != current_uid() || m.mode() & 0o022 != 0 {
        return Err(UNSAFE);
    }
    Ok(())
}

#[cfg(not(unix))]
fn vet(_: &Path) -> Result<(), &'static str> {
    Err(NOT_RUNNABLE)
}

#[cfg(unix)]
fn current_uid() -> u32 {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    // SAFETY: getuid 没有参数、不会失败、不碰内存（POSIX）
    unsafe { getuid() }
}

#[cfg(test)]
mod tests;
