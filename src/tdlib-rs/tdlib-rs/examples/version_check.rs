// Copyright 2026 - developers of the `tdlib-rs` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Check which TDLib the crate is linked against, without logging in.
//!
//! 1. Reads `getOption("version")` and checks it equals `EXPECTED_VERSION`.
//! 2. Waits for the first `updateAuthorizationState`.
//! 3. Only if `API_ID` and `API_HASH` are set at run time: sends
//!    `setTdlibParameters` with `use_test_dc = true`, then waits for
//!    `updateConnectionState(Ready)` and `authorizationStateWaitPhoneNumber`
//!    from the Telegram test servers. It never sends a phone number, never
//!    logs in and never sends a message. The database lives in a fresh temp
//!    directory that is removed at the end.
//!
//! Exits with 0 when every step passes, 1 otherwise.
//!
//! Run: `cargo run -p tdlib-rs --example version_check --features local-tdlib`

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tdlib_rs::{
    enums::{AuthorizationState, ConnectionState, OptionValue, Update},
    functions,
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::time::{Instant, timeout_at};

const EXPECTED_VERSION: &str = "1.8.69";
/// Time limit for each step as a whole (not per update).
const STEP: Duration = Duration::from_secs(30);

type Updates = UnboundedReceiver<Update>;

fn auth_name(state: &AuthorizationState) -> &'static str {
    match state {
        AuthorizationState::WaitTdlibParameters => "waitTdlibParameters",
        AuthorizationState::WaitPhoneNumber => "waitPhoneNumber",
        AuthorizationState::Ready => "ready",
        AuthorizationState::Closing => "closing",
        AuthorizationState::Closed => "closed",
        _ => "other",
    }
}

/// Next update before `deadline`, or an error naming `step`.
async fn next(rx: &mut Updates, deadline: Instant, step: &str) -> Result<Update, String> {
    match timeout_at(deadline, rx.recv()).await {
        Ok(Some(update)) => Ok(update),
        Ok(None) => Err(format!("{step}: receive thread stopped")),
        Err(_) => Err(format!("{step}: timed out after {STEP:?}")),
    }
}

async fn check(client_id: i32, rx: &mut Updates, db_dir: &Path) -> Result<(), String> {
    // 1. Version of the linked library.
    let version = match functions::get_option("version".into(), client_id).await {
        Ok(OptionValue::String(v)) => v.value,
        Ok(_) => return Err("getOption(\"version\"): not a string".into()),
        Err(e) => return Err(format!("getOption(\"version\"): {}", e.message)),
    };
    println!("tdlib version: {version}");
    if version != EXPECTED_VERSION {
        return Err(format!(
            "linked TDLib is {version}, expected {EXPECTED_VERSION}"
        ));
    }

    // 2. First authorization state.
    let deadline = Instant::now() + STEP;
    let first = loop {
        if let Update::AuthorizationState(u) = next(rx, deadline, "first auth state").await? {
            break u.authorization_state;
        }
    };
    println!("first authorization state: {}", auth_name(&first));

    // 3. Optional: reach the test servers.
    let (api_id, api_hash) = match (std::env::var("API_ID"), std::env::var("API_HASH")) {
        (Ok(id), Ok(hash)) if !id.is_empty() && !hash.is_empty() => {
            let id = id
                .parse::<i32>()
                .map_err(|_| "API_ID is set but is not a number".to_string())?;
            (id, hash)
        }
        _ => {
            println!("test dc: skipped (API_ID / API_HASH not set)");
            return Ok(());
        }
    };

    functions::set_tdlib_parameters(
        true, // use_test_dc
        db_dir.to_string_lossy().into_owned(),
        String::new(),
        String::new(),
        false,
        false,
        false,
        false,
        api_id,
        api_hash,
        "en".into(),
        "Desktop".into(),
        String::new(),
        env!("CARGO_PKG_VERSION").into(),
        client_id,
    )
    .await
    .map_err(|e| format!("setTdlibParameters: {}", e.message))?;

    let deadline = Instant::now() + STEP;
    let (mut connected, mut wait_phone) = (false, false);
    while !(connected && wait_phone) {
        match next(rx, deadline, "test dc").await? {
            Update::ConnectionState(u) => {
                connected |= matches!(u.state, ConnectionState::Ready);
            }
            Update::AuthorizationState(u) => {
                println!("authorization state: {}", auth_name(&u.authorization_state));
                wait_phone |= matches!(u.authorization_state, AuthorizationState::WaitPhoneNumber);
            }
            _ => {}
        }
    }
    println!("test dc: connection ready, waiting for phone number (not logging in)");
    Ok(())
}

/// Close the client and wait for `authorizationStateClosed`.
async fn close(client_id: i32, rx: &mut Updates) -> Result<(), String> {
    functions::close(client_id)
        .await
        .map_err(|e| format!("close: {}", e.message))?;
    let deadline = Instant::now() + STEP;
    loop {
        if let Update::AuthorizationState(u) = next(rx, deadline, "close").await?
            && matches!(u.authorization_state, AuthorizationState::Closed)
        {
            return Ok(());
        }
    }
}

#[tokio::main]
async fn main() {
    // Quiet TDLib's own log (errors only) before any client exists, so its
    // default verbosity never prints updates to stderr.
    let quiet = tdlib_rs::execute(serde_json::json!({
        "@type": "setLogVerbosityLevel",
        "new_verbosity_level": 1,
    }));
    if quiet.as_ref().map(|r| r["@type"] != "ok").unwrap_or(true) {
        eprintln!("version_check failed: setLogVerbosityLevel via td_execute");
        std::process::exit(1);
    }

    let client_id = tdlib_rs::create_client();
    let (tx, mut rx) = mpsc::unbounded_channel::<Update>();

    // Poll TDLib on a plain thread and forward only the updates we use.
    // The thread is stopped and joined before the process exits: exiting
    // while it is inside TDLib's receive crashes the process.
    let running = Arc::new(AtomicBool::new(true));
    let running_in_thread = running.clone();
    let receiver = std::thread::spawn(move || {
        while running_in_thread.load(Ordering::Acquire) {
            if let Some((update, _)) = tdlib_rs::receive() {
                let keep = matches!(
                    update,
                    Update::AuthorizationState(_) | Update::ConnectionState(_)
                );
                if keep && tx.send(update).is_err() {
                    break;
                }
            }
        }
    });

    let db_dir =
        std::env::temp_dir().join(format!("tdlib-rs-version-check-{}", std::process::id()));

    let checked = check(client_id, &mut rx, &db_dir).await;
    let closed = close(client_id, &mut rx).await;

    running.store(false, Ordering::Release);
    let _ = receiver.join();
    let _ = std::fs::remove_dir_all(&db_dir);

    match checked.and(closed) {
        Ok(()) => println!("closed"),
        Err(e) => {
            eprintln!("version_check failed: {e}");
            std::process::exit(1);
        }
    }
}
