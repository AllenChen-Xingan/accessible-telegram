// Copyright 2020 - developers of the `grammers` project.
// Copyright 2021 - developers of the `tdlib-rs` project.
// Copyright 2024 - developers of the `tgt` and `tdlib-rs` projects.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
pub mod build;
mod generated;
mod observer;
mod tdjson;

pub use generated::{enums, functions, types};

use enums::Update;
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::atomic::{AtomicU32, Ordering};

static EXTRA_COUNTER: AtomicU32 = AtomicU32::new(0);
static OBSERVER: Lazy<observer::Observer> = Lazy::new(observer::Observer::new);

/// Create a TdLib client returning its id. Note that to start receiving
/// updates for a client you need to send at least a request with it first.
pub fn create_client() -> i32 {
    tdjson::create_client()
}

/// Execute a TDLib request synchronously, without a client.
///
/// Only requests documented as "Can be called synchronously" work, e.g.
/// `setLogVerbosityLevel`, `getLogVerbosityLevel`, `getTextEntities`,
/// `parseTextEntities`. Returns TDLib's JSON answer (an `error` object for
/// requests that can't be executed this way), or `None` if TDLib returned
/// nothing.
///
/// Useful before the first `create_client()`, for example to lower TDLib's
/// log verbosity so that no update reaches its default stderr log:
///
/// ```no_run
/// tdlib_rs::execute(serde_json::json!({
///     "@type": "setLogVerbosityLevel",
///     "new_verbosity_level": 1,
/// }));
/// ```
pub fn execute(request: Value) -> Option<Value> {
    let response = tdjson::execute(request.to_string())?;
    match serde_json::from_str(&response) {
        Ok(value) => Some(value),
        Err(_) => {
            log::warn!("td_execute returned a response that is not valid JSON");
            None
        }
    }
}

/// Deserialize the response to a request (used by the generated functions).
///
/// A response that doesn't match the schema is a bug (in TDLib or in the
/// bindings), so this panics, but the panic message only names the expected
/// Rust type and the response's `@type`: serde's error text can quote field
/// values (message text, names, phone numbers).
pub(crate) fn from_response<T: DeserializeOwned>(response: Value) -> T {
    let type_name = safe_type_name(&response);
    match serde_json::from_value(response) {
        Ok(value) => value,
        Err(_) => panic!(
            "TDLib response could not be parsed as {}: @type={type_name}",
            std::any::type_name::<T>()
        ),
    }
}

/// Receive a single update or response from TdLib. If it's an update, it
/// returns a tuple with the `Update` and the associated `client_id`.
/// Note that to start receiving updates for a client you need to send
/// at least a request with it first.
pub fn receive() -> Option<(Update, i32)> {
    let response = tdjson::receive(2.0);
    response.and_then(|response_str| handle_response(&response_str))
}

/// Route one raw TDLib response: request results go to the observer,
/// updates are deserialized and returned.
///
/// Updates carry message text, phone numbers, names and so on, and serde's
/// error text can quote the offending value. So nothing here logs the
/// response or a serde error message: a log line holds at most the `@type`.
/// (serde reports no line, column or field path for internally tagged enums,
/// since it buffers the whole object first, so there is no position to add.)
/// Malformed input is logged and skipped instead of panicking the caller's
/// receive loop.
fn handle_response(response_str: &str) -> Option<(Update, i32)> {
    let Ok(response) = serde_json::from_str::<Value>(response_str) else {
        log::warn!("Received a response that is not valid JSON");
        return None;
    };

    if response.get("@extra").is_some() {
        OBSERVER.notify(response);
        return None;
    }

    let type_name = safe_type_name(&response);
    let Some(client_id) = response.get("@client_id").and_then(Value::as_i64) else {
        log::warn!("Received an update without @client_id: @type={type_name}");
        return None;
    };
    match serde_json::from_value::<Update>(response) {
        Ok(update) => Some((update, client_id as i32)),
        Err(_) => {
            log::warn!("Received an update that could not be parsed: @type={type_name}");
            None
        }
    }
}

/// The `@type` of a response, only if it looks like a TL identifier.
/// Anything else is replaced, so a malformed value can't reach the log.
fn safe_type_name(response: &Value) -> String {
    match response.get("@type").and_then(Value::as_str) {
        Some(name)
            if !name.is_empty()
                && name.len() <= 64
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') =>
        {
            name.to_owned()
        }
        Some(_) => "<invalid @type>".to_owned(),
        None => "<missing @type>".to_owned(),
    }
}

pub(crate) async fn send_request(client_id: i32, mut request: Value) -> Value {
    let extra = EXTRA_COUNTER.fetch_add(1, Ordering::Relaxed);
    request["@extra"] = serde_json::to_value(extra).unwrap();

    let receiver = OBSERVER.subscribe(extra);
    tdjson::send(client_id, request.to_string());

    receiver.await.unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Logger that keeps every record, so the test can check what was written.
    struct CaptureLogger(Mutex<Vec<String>>);

    impl log::Log for CaptureLogger {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            self.0.lock().unwrap().push(record.args().to_string());
        }
        fn flush(&self) {}
    }

    static LOGGER: CaptureLogger = CaptureLogger(Mutex::new(Vec::new()));

    const SECRET_TEXT: &str = "FAKE-SECRET-chat-+15550001111";
    const SECRET_TITLE: &str = "FAKE-SECRET-title-Alice";

    /// An update with a known `@type` whose `chat_id` holds a string where a
    /// number is expected, so it can't be parsed. Both fields hold fake secrets.
    fn unparsable_update() -> String {
        serde_json::json!({
            "@type": "updateChatTitle",
            "@client_id": 1,
            "chat_id": SECRET_TEXT,
            "title": SECRET_TITLE,
        })
        .to_string()
    }

    /// Run `f` and return the log lines it produced that came from
    /// `handle_response` (other tests may log at the same time).
    fn captured(f: impl FnOnce()) -> Vec<String> {
        static INIT: std::sync::Once = std::sync::Once::new();
        static SERIAL: Mutex<()> = Mutex::new(());
        INIT.call_once(|| {
            log::set_logger(&LOGGER).unwrap();
            log::set_max_level(log::LevelFilter::Trace);
        });
        let _serial = SERIAL.lock().unwrap();
        LOGGER.0.lock().unwrap().clear();
        f();
        LOGGER
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.starts_with("Received "))
            .cloned()
            .collect()
    }

    fn assert_no_secret(line: &str) {
        for secret in [SECRET_TEXT, SECRET_TITLE, "FAKE-SECRET", "5550001111"] {
            assert!(!line.contains(secret), "leaked {secret:?}: {line}");
        }
    }

    #[test]
    fn unparsed_update_log_has_no_content() {
        let raw = unparsable_update();

        // Guard: serde's own error text does quote the value, so this test
        // would catch a regression back to logging `{e}` or the raw JSON.
        let serde_err = serde_json::from_str::<Update>(&raw)
            .unwrap_err()
            .to_string();
        assert!(serde_err.contains(SECRET_TEXT), "serde error: {serde_err}");

        let logs = captured(|| assert!(handle_response(&raw).is_none()));
        assert_eq!(logs.len(), 1, "logs: {logs:?}");
        assert!(
            logs[0].contains("@type=updateChatTitle"),
            "line: {}",
            logs[0]
        );
        assert_no_secret(&logs[0]);
    }

    #[test]
    fn malformed_responses_are_skipped_not_panicking() {
        let no_client =
            serde_json::json!({ "@type": "updateChatTitle", "title": SECRET_TITLE }).to_string();
        let not_json = format!("{{\"title\": \"{SECRET_TITLE}\"");

        let logs = captured(|| {
            assert!(handle_response(&no_client).is_none());
            assert!(handle_response(&not_json).is_none());
        });
        assert_eq!(logs.len(), 2, "logs: {logs:?}");
        assert!(logs[0].contains("without @client_id"), "line: {}", logs[0]);
        assert!(logs[1].contains("not valid JSON"), "line: {}", logs[1]);
        logs.iter().for_each(|l| assert_no_secret(l));
    }

    #[test]
    fn response_parse_panic_has_no_content() {
        // A `user` result whose `id` holds a string where a number is expected.
        let response = serde_json::json!({
            "@type": "user",
            "id": SECRET_TEXT,
            "first_name": SECRET_TITLE,
        });

        // Guard: what the old `from_value(..).unwrap()` would have panicked with.
        let serde_err = serde_json::from_value::<types::User>(response.clone())
            .unwrap_err()
            .to_string();
        assert!(serde_err.contains(SECRET_TEXT), "serde error: {serde_err}");

        let payload = std::panic::catch_unwind(|| from_response::<types::User>(response))
            .expect_err("must panic");
        let message = payload
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_default();
        assert!(message.contains("@type=user"), "message: {message}");
        assert!(message.contains("types::User"), "message: {message}");
        assert_no_secret(&message);
    }

    #[test]
    fn execute_works_without_a_client() {
        let set = execute(serde_json::json!({
            "@type": "setLogVerbosityLevel",
            "new_verbosity_level": 1,
        }))
        .unwrap();
        assert_eq!(set["@type"], "ok", "{set}");

        let get = execute(serde_json::json!({ "@type": "getLogVerbosityLevel" })).unwrap();
        assert_eq!(get["@type"], "logVerbosityLevel", "{get}");
        assert_eq!(get["verbosity_level"], 1, "{get}");

        // A request that can't run synchronously yields an `error` object.
        let not_sync = execute(serde_json::json!({ "@type": "getMe" })).unwrap();
        assert_eq!(not_sync["@type"], "error", "{not_sync}");
    }

    #[test]
    fn type_name_is_not_trusted() {
        let bad = serde_json::json!({ "@type": "x\" phone=+15550001111" });
        assert_eq!(safe_type_name(&bad), "<invalid @type>");
        let missing = serde_json::json!({ "text": "hi" });
        assert_eq!(safe_type_name(&missing), "<missing @type>");
        let ok = serde_json::json!({ "@type": "updateNewMessage" });
        assert_eq!(safe_type_name(&ok), "updateNewMessage");
    }
}
