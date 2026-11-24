//! `atg-cli`、`atg-core` 共用的构建脚本（两个包的 `Cargo.toml` 都写 `build = "../tdlib-build/rpath.rs"`，#170）：
//! 开了 `tdlib` 特性时，给本包的程序（二进制、测试、例子）写一条 rpath，指向常驻软链接 `$HOME/<dev-link>/lib`（#42）。
//!
//! 开发版签名开了强化运行时（`scripts/dev-sign.sh`）：`DYLD_LIBRARY_PATH` 被忽略，libtdjson 只能按程序里的 rpath 找
//! （它自称 `@rpath/libtdjson…`）。tdlib-rs 的 `build.rs` 也写 rpath，但只作用于 tdlib-rs 自己的产物，而且指向
//! 构建产物目录，所以跑 TDLib 的包得自己写。软链接由 `src/tdlib-build/build.sh` 维护，不带空格、不随构建目录变。
//! 软链接相对家目录的路径只写在同目录的 `dev-link` 里（唯一正本；`build.sh`、`scripts/check-harden.sh` 也读它）。
//! 写进程序的是构建时拼好的绝对路径，所以开发版二进制里带着构建机的家目录（只限开了 `tdlib` 特性的开发版）。
//! 这件已记在 issue #167：正式包要换一种载入方式（如 `@executable_path` 加打进 `.app` 的库），到时一起解决。没开 `tdlib` 特性（默认、CI）时什么都不做。

/// 常驻软链接相对家目录的路径：正本是同目录的 `dev-link`。按字节读进来（`include_bytes!`），到开了 `tdlib`
/// 特性之后才解码、校验：用 `include_str!` 的话，文件不是 UTF-8（如被编辑器存成 UTF-16）时，默认构建也会编不过（codex 072 第 3 轮）
const DEV_LINK: &[u8] = include_bytes!("dev-link");

fn main() {
    // 这两个文件一改，cargo 就重跑本脚本（改了 dev-link，程序里的 rpath 才会跟着变）；`HOME` 变了同理。
    // 路径按包根目录算：两个包都在 `src/atg-*/` 下，所以这两条对两个包都成立
    println!("cargo:rerun-if-changed=../tdlib-build/rpath.rs");
    println!("cargo:rerun-if-changed=../tdlib-build/dev-link");
    println!("cargo:rerun-if-env-changed=HOME");
    let tdlib = std::env::var_os("CARGO_FEATURE_TDLIB").is_some();
    let macos = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "macos");
    if !(tdlib && macos) {
        return;
    }
    // 规则（三处同一条，见 src/tdlib-build/README.md）：整个文件 = 一行路径 + 可有可无的一个换行；不 trim。
    // 不合规则就让构建失败，不悄悄产出一个找不到 libtdjson 的程序（同两份 shell 的「报错退出」）
    let raw = DEV_LINK.strip_suffix(b"\n").unwrap_or(DEV_LINK);
    // 字符只许 ASCII 字母、数字和 . _ / -（白名单）：空白、非 ASCII 与 shell 那边的判法一致；逗号也拒，
    // 因为下面 `-Wl,-rpath,…` 会把逗号当成参数分隔符（#170 codex 072）
    let allowed = |b: u8| b.is_ascii_alphanumeric() || b"._/-".contains(&b);
    // 前后各补一个 / 再查（同两份 shell）：`//` = 开头的 /、末尾的 / 或连着的 /；`/./` = 有 . 这一段。
    // 末尾带 / 的路径会让 `ln` 和 check-harden 的拼法对不上（codex 072 第 2 轮）
    // 白名单全是 ASCII：先按字节查白名单，过了才当字符串用（非 UTF-8 的内容在这一步就被拒）
    let rel = std::str::from_utf8(raw)
        .ok()
        .filter(|s| !s.is_empty() && s.bytes().all(allowed))
        .unwrap_or("");
    let framed = format!("/{rel}/");
    let bad =
        rel.is_empty() || rel.contains("..") || framed.contains("//") || framed.contains("/./");
    assert!(
        !bad,
        "src/tdlib-build/dev-link 的内容不对：要恰好一行相对家目录的路径，只许字母、数字和 . _ / -，每一段非空、不是 . 也不含 ..（不以 / 开头或结尾、没有 //；见同目录 README.md）"
    );
    let Some(home) = std::env::var_os("HOME") else {
        panic!("没有 HOME，写不了 TDLib 的 rpath（不写的话程序找不到 libtdjson）");
    };
    let lib = std::path::Path::new(&home).join(rel).join("lib");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
}
