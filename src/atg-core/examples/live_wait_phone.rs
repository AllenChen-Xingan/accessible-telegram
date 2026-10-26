//! 本机冒烟（`testing.md` 第一节第 1 小节「测试服务器」层，不在 CI）：对 Telegram **测试服务器**起真实后端，
//! 看授权状态推进到 `wait_phone`，然后正常关闭。不登录、不发消息、不打印任何凭据。
//!
//! 写死的前置（任一不满足就退出，一个请求都不发）：`ATG_LIVE=1`；用 `test` 档案；档案标记是测试服务器。
//!
//! 第一次跑之前要准备两样（都不入库）：
//! 1. 仓库根 `.env.local` 写 `ATG_API_ID=…`、`ATG_API_HASH=…`（自己在 my.telegram.org 申请；
//!    `specs/contracts/config-privacy.md` 第一节第 1 小节）
//! 2. 开发签名身份：本人的 Apple Development 证书（在 Xcode 领，免费；`bash scripts/setup-dev-codesign.sh`
//!    检查并给出步骤）
//!
//! 钥匙在钥匙串（服务名 `atg.tdlib-db-key`，账户 `test`）。条目记的是建它的程序的签名（标识 + 团队号），所以每个
//! 开发版都用这个身份、固定标识 `atg.dev` 签（`scripts/dev-sign.sh`；`cargo run` 经 runner 自动签），重编后读钥匙
//! 才不弹框（issue #38）。出错时只打机读码
//! （如 `env_local_unreadable`），含义见 `src/atg-core/src/error.rs` 与 `config.rs`：
//!
//! ```sh
//! export LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"
//! ATG_LIVE=1 cargo run -p atg-core --features tdlib --example live_wait_phone   # runner 签名并带上 TDLib 库目录
//! ```
//!
//! 只读出状态名、序号、耗时；不打印 `api_id` / `api_hash` / 钥匙。

use atg_core::backend::{Backend, SHUTDOWN_TIMEOUT};
use atg_core::config::{self, Profile, ServerKind};
use atg_core::error::Holder;
use atg_core::keystore::Keychain;
use atg_core::link::block_on;
use atg_core::status::AuthStep;
use atg_core::td::{TdBackend, TdConfig};
use std::process::ExitCode;
use std::time::{Duration, Instant};

fn name<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn main() -> ExitCode {
    if std::env::var("ATG_LIVE").as_deref() != Ok("1") {
        eprintln!("要设 ATG_LIVE=1 才跑（会联网到 Telegram 测试服务器）");
        return ExitCode::from(2);
    }
    let Ok(root) = config::default_root() else {
        return ExitCode::from(2);
    };
    let profile = match Profile::create(&root, "test", ServerKind::Test) {
        Ok(p) if p.kind() == ServerKind::Test => p,
        Ok(_) | Err(_) => {
            eprintln!("test 档案打不开或不是测试服务器档案，不跑");
            return ExitCode::from(2);
        }
    };
    let Ok(log_dir) = config::default_log_dir() else {
        return ExitCode::from(2);
    };
    let backend = match TdBackend::new(TdConfig {
        env_file: config::default_env_file(),
        keystore: Box::new(Keychain),
        log_dir,
        holder: Holder::Cli,
    }) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error={}", e.code());
            return ExitCode::from(1);
        }
    };
    let Ok(mut stream) = backend.subscribe() else {
        return ExitCode::from(1);
    };
    let t0 = Instant::now();
    if let Err(e) = block_on(backend.start(&profile)) {
        eprintln!("start error={}", e.code());
        let _ = backend.shutdown(SHUTDOWN_TIMEOUT);
        return ExitCode::from(1);
    }
    println!(
        "started use_test_dc={} previous_unclean={}",
        backend.use_test_dc().unwrap_or(false),
        backend.status().previous_unclean
    );
    let reached = backend.wait_auth(Duration::from_secs(30), &|a| {
        matches!(a, AuthStep::WaitPhone | AuthStep::Ready)
    });
    println!(
        "auth={} after_ms={} status={}",
        reached
            .map(|a| name(&a))
            .unwrap_or_else(|| "timeout".into()),
        t0.elapsed().as_millis(),
        name(&backend.status().status)
    );
    let events = stream.try_batch(10_000);
    let max_seq = events.last().map_or(0, |e| e.seq);
    println!("events={} last_seq={max_seq}", events.len());
    let closed = backend.shutdown(SHUTDOWN_TIMEOUT);
    println!(
        "shutdown={} core={}",
        closed.as_ref().map_or_else(|e| e.code(), |()| "ok"),
        name(&backend.status().lines.core)
    );
    match (reached, closed) {
        (Some(AuthStep::WaitPhone | AuthStep::Ready), Ok(())) => ExitCode::SUCCESS,
        _ => ExitCode::from(1),
    }
}
