//! #38 迁移：只读真条目 atg.tdlib-db-key 的 test / main。只打印状态码与长度，绝不打印值；从不新建、改、删。
//! 用法：atg-migrate-read quiet|ui test|main
//!   quiet 关交互：过不了关直接失败（不弹框）；ui 开交互：过不了关就弹框（迁移时人点「始终允许」）
use security_framework::os::macos::keychain::SecKeychain;
use security_framework::passwords::get_generic_password;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (mode, acct) = match (args.get(1).map(String::as_str), args.get(2).map(String::as_str)) {
        (Some(m @ ("quiet" | "ui")), Some(a @ ("test" | "main"))) if args.len() == 3 => (m, a),
        _ => {
            eprintln!("usage: atg-migrate-read quiet|ui test|main");
            std::process::exit(2);
        }
    };
    let _lock = if mode == "quiet" {
        Some(SecKeychain::disable_user_interaction().unwrap_or_else(|_| std::process::exit(3)))
    } else {
        None
    };
    let nonce = option_env!("NONCE").unwrap_or("0");
    match get_generic_password("atg.tdlib-db-key", acct) {
        Ok(mut v) => {
            println!("nonce={nonce} mode={mode} acct={acct} status=0 len={}", v.len());
            v.iter_mut().for_each(|b| *b = 0);
        }
        Err(e) => {
            println!("nonce={nonce} mode={mode} acct={acct} status={} len=-1", e.code());
            std::process::exit(1);
        }
    }
}
