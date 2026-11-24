//! `atg-cli`、`atg-core` 共用的构建脚本（两个包的 `Cargo.toml` 都写 `build = "../tdlib-build/rpath.rs"`，#170）：
//! 开了 `tdlib` 特性时，给本包的程序（二进制、测试、例子）写一条 rpath，指向常驻软链接 `$HOME/<dev-link>/lib`（#42）。
//!
//! 开发版签名开了强化运行时（`scripts/dev-sign.sh`）：`DYLD_LIBRARY_PATH` 被忽略，libtdjson 只能按程序里的 rpath 找
//! （它自称 `@rpath/libtdjson…`）。tdlib-rs 的 `build.rs` 也写 rpath，但只作用于 tdlib-rs 自己的产物，而且指向
//! 构建产物目录，所以跑 TDLib 的包得自己写。软链接由 `src/tdlib-build/build.sh` 维护，不带空格、不随构建目录变。
//! 软链接相对家目录的路径只写在同目录的 `dev-link` 里（唯一正本；`build.sh`、`scripts/check-harden.sh` 也读它）。
//! 写进程序的是构建时拼好的绝对路径，所以开发版二进制里带着构建机的家目录（只限开了 `tdlib` 特性的开发版；
//! 正式包要换载入方式，同 #167 一类，另议）。没开 `tdlib` 特性（默认、CI）时什么都不做。

/// 常驻软链接相对家目录的路径：正本是同目录的 `dev-link`
const DEV_LINK: &str = include_str!("dev-link");

fn main() {
    // 路径按包根目录算：两个包都在 `src/atg-*/` 下，所以这两条对两个包都成立
    println!("cargo:rerun-if-changed=../tdlib-build/rpath.rs");
    println!("cargo:rerun-if-changed=../tdlib-build/dev-link");
    println!("cargo:rerun-if-env-changed=HOME");
    let tdlib = std::env::var_os("CARGO_FEATURE_TDLIB").is_some();
    let macos = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "macos");
    if !(tdlib && macos) {
        return;
    }
    let rel = DEV_LINK.trim();
    let bad = rel.is_empty()
        || rel.starts_with('/')
        || rel.contains("..")
        || rel.contains(char::is_whitespace);
    if bad {
        println!(
            "cargo:warning=src/tdlib-build/dev-link 不对（空、绝对路径、含 .. 或空白），不写 TDLib 的 rpath"
        );
        return;
    }
    let Some(home) = std::env::var_os("HOME") else {
        println!("cargo:warning=没有 HOME，写不了 TDLib 的 rpath；程序会找不到 libtdjson");
        return;
    };
    let lib = std::path::Path::new(&home).join(rel).join("lib");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
}
