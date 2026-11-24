//! 开了 `tdlib` 特性时，给本包的程序（二进制、测试、例子）写一条 rpath，指向常驻软链接 `~/.atg-dev/tdlib/lib`（#42）。
//!
//! 开发版签名开了强化运行时（`scripts/dev-sign.sh`）：`DYLD_LIBRARY_PATH` 被忽略，libtdjson 只能按程序里的 rpath 找
//! （它自称 `@rpath/libtdjson…`）。tdlib-rs 的 `build.rs` 也写 rpath，但只作用于 tdlib-rs 自己的产物，而且指向
//! 构建产物目录，所以跑 TDLib 的包得自己写。软链接由 `src/tdlib-build/build.sh` 维护，不带空格、不随构建目录变。
//! 没开 `tdlib` 特性（默认、CI）时什么都不做。

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=HOME");
    let tdlib = std::env::var_os("CARGO_FEATURE_TDLIB").is_some();
    let macos = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "macos");
    if !(tdlib && macos) {
        return;
    }
    let Some(home) = std::env::var_os("HOME") else {
        println!("cargo:warning=没有 HOME，写不了 TDLib 的 rpath；程序会找不到 libtdjson");
        return;
    };
    let lib = std::path::Path::new(&home).join(".atg-dev/tdlib/lib");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
}
