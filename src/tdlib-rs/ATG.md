# ATG.md — this fork as used by atg (Accessible Telegram)

Branch `atg/tdlib-1.8.69` of `AllenChen-Xingan/tdlib-rs`, based on upstream
`FedericoBruzzone/tdlib-rs` v1.4.0 (`3bfef94`).

Five changes:

1. Use TDLib **1.8.69** instead of 1.8.61.
2. Generator: follow bare type references (`button:inlineButton`) when looking
   for recursive types. 1.8.69 adds the cycle
   `InlineButton -> RichText -> RichTextButton -> InlineButton` through a bare
   reference; without this the generated code fails with `E0072` (infinite size).
3. When an update can't be parsed, the warning no longer contains the raw JSON
   or serde's error text — only the `@type`. `receive()` also no longer panics
   on invalid JSON or a missing `@client_id`; it logs (without content) and
   returns `None`.
4. New `tdlib_rs::execute(Value) -> Option<Value>`, a binding for
   `td_execute`. Lets callers lower TDLib's log verbosity before the first
   client exists; at the default verbosity TDLib prints whole updates
   (message text included) to stderr.
5. Generated functions no longer `unwrap()` serde errors. A response that
   doesn't match the schema still panics (it's a bug), but the message only
   names the expected Rust type and the response's `@type`.

## Using it from atg-core

Since 2026-10-25 this branch lives inside the atg repo as a `git subtree` at
`src/tdlib-rs` (it keeps its own `[workspace]`; the atg workspace excludes it).
`atg-core` uses it through a path dependency:

```toml
# src/atg-core/Cargo.toml
tdlib-rs = { path = "../tdlib-rs/tdlib-rs", optional = true }

[features]
tdlib-api = ["dep:tdlib-rs"]                    # types only; check / clippy need no TDLib
tdlib = ["tdlib-api", "tdlib-rs/local-tdlib"]  # links the TDLib build below
```

`local-tdlib` reads `LOCAL_TDLIB_PATH` at build time. It must be a folder with
`lib/libtdjson.1.8.69.dylib`, the `lib/libtdjson.dylib` symlink (the link step
uses `-ltdjson`) and `include/`. The build script copies that
folder into `OUT_DIR/tdlib` and sets an rpath to it, so the binary does not
depend on `LOCAL_TDLIB_PATH` after it is built (it does depend on `OUT_DIR`
staying put — fine for development, not for a shipped app bundle).

## Building TDLib 1.8.69 (macOS)

From the atg repo root, one command (about 3 minutes on an M-series Mac):

```sh
bash src/tdlib-build/build.sh
export LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"   # latest finished build
```

The script fetches TDLib at commit `ea3f69ce11aef458bfbb09869b809ede4447486f`,
builds only the `tdjson` target in Release, and lays out the prefix in
`_build/<timestamp>/tdlib`. It needs git, cmake, gperf and Homebrew's
`openssl@3`, and never installs them. It works around two TDLib build quirks:

- TDLib passes `-Wl,-exported_symbols_list,<source dir>/tdclientjson_export_list`
  unquoted, so a source path containing spaces breaks the link step. The script
  builds through a temporary space-free symlink.
- The `tdjson` target alone is not installable with `cmake --install` (it
  expects every target), so the script copies the dylibs and the three headers
  (`td_json_client.h`, `td_log.h`, `tdjson_export.h`) by hand.

The resulting `libtdjson.1.8.69.dylib` (~25 MB) links
`/opt/homebrew/opt/openssl@3/lib/lib{ssl,crypto}.3.dylib` by absolute path;
removing `openssl@3` from Homebrew breaks it.

## Checking it

From `src/tdlib-rs`:

```sh
export LOCAL_TDLIB_PATH="$(bash ../tdlib-build/build.sh path)"
# upstream's get_me example reads API_ID / API_HASH with env!() at compile
# time; placeholders are enough to compile it (it is not run here)
export API_ID=0 API_HASH=compile-only
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --features tdlib-rs/local-tdlib -- -D warnings
cargo test  --workspace --features tdlib-rs/local-tdlib --lib --bins --tests
unset API_ID API_HASH
cargo run   -p tdlib-rs --example version_check --features local-tdlib
```

`--lib --bins --tests` leaves out the doctests in `tdlib-rs/src/build.rs`:
they are build-script snippets that read `CARGO_CFG_TARGET_OS`, which only
exists inside a build script, so they panic under `cargo test` (before this
patch too; upstream's `make test` excludes the `tdlib-rs` package).

Results (macOS arm64): see `docs/plan/004_tdlib-patch.md` in the atg repo
(section 四).

`version_check` prints `getOption("version")` (must be `1.8.69`) and the first
authorization state. With `API_ID` / `API_HASH` set at run time it also
connects to the Telegram **test** servers (`use_test_dc = true`) and waits for
`connectionStateReady` + `authorizationStateWaitPhoneNumber`. It never sends a
phone number, never logs in and never sends a message.

## For an upstream PR (facts only; the description is written by a person)

Files changed:

| File | Change | Why |
|---|---|---|
| `tdlib-rs/tl/api.tl` | replaced by TDLib's `td/generate/scheme/td_api.tl` at commit `ea3f69ce11aef458bfbb09869b809ede4447486f` (version 1.8.69) | bindings are generated from this file (`build.rs:310`) |
| `tdlib-rs/tl/api_1.8.61.tl` | old `api.tl`, kept | same as the 1.8.61 bump kept `api_1.8.29.tl` |
| `tdlib-rs/build.rs:21`, `tdlib-rs/src/build.rs:9` | `TDLIB_VERSION` 1.8.61 → 1.8.69 | library file name `libtdjson.<ver>.dylib` (`build.rs:93`) and download URL use it |
| `tdlib-rs/Cargo.toml:18` | `system-deps` `tdjson = "1.8.69"` | `pkg-config` feature |
| `README.md:46,74,117` | version and commit link | docs |
| `.github/workflows/build-tdlib.yml:8-9` | `TDLIB_VERSION`, `TDLIB_COMMIT` | CI builds the release archives |
| `tdlib-rs-gen/src/metadata.rs` | `def_self_references` follows a bare parameter type to its one constructor; three unit tests | 1.8.69's `richTextButton button:inlineButton = RichText` closes a cycle through a bare type, which the old check never followed → `E0072`. Effect on the generated API: exactly two variants become boxed, `RichText::Button(Box<RichTextButton>)` and `InlineButton::InlineButton(Box<InlineButton>)` (compared the list of `Box<…>` variants before/after) |
| `tdlib-rs/src/lib.rs` | `receive()` split into `handle_response()`; parse-failure warning logs only `@type` (identifier characters only, else `<invalid @type>` / `<missing @type>`); invalid JSON and a missing `@client_id` are logged without content and skipped instead of panicking; three unit tests, one of which first asserts that serde's own error text *does* contain the fake secret, so it fails if the old logging comes back | the old warning logged the whole update (message text, names, phone numbers) plus serde's error text, which can quote field values; a panic in `receive()` kills the caller's receive thread. No position is logged: for internally tagged enums serde reports line 0 column 0 and `serde_path_to_error` reports path `.` (tried both) |
| `tdlib-rs/src/tdjson.rs`, `tdlib-rs/src/lib.rs` | `td_execute` binding; public `execute(Value) -> Option<Value>`; unit test sets and reads back the log verbosity without a client, and checks a non-synchronous request gives `error` | TDLib's default verbosity logs whole updates to stderr before an async `setLogVerbosityLevel` is processed. TDLib keeps the result in a thread-local buffer (`td/telegram/ClientJson.cpp:74`) until that thread's next call, so it is copied out at once |
| `tdlib-rs-gen/src/functions.rs:112,121`, `tdlib-rs/src/lib.rs` | generated code calls `crate::from_response` instead of `serde_json::from_value(response).unwrap()`; it panics with only `std::any::type_name::<T>()` and the response `@type`; unit test (with the same "serde does quote the secret" guard) and a generator test that generated code contains no `.unwrap()` | the old panic message was serde's error, which can quote field values |
| `tdlib-rs/examples/version_check.rs` | new example; quiets TDLib with `execute` before `create_client` (stderr: 0 lines, was 24) | checks the linked TDLib version without logging in |

Not changed, worth noting upstream:

- `download-tdlib` builds its URL from `CARGO_PKG_VERSION` (still 1.4.0,
  `tdlib-rs/build.rs:202`) or the hard-coded `TDLIB_CARGO_PKG_VERSION = "1.4.0"`
  (`tdlib-rs/src/build.rs:11`) plus `TDLIB_VERSION`; release archives for
  1.8.69 don't exist until a release is cut.
- The generator's recursion check now also boxes
  `InlineButton::InlineButton(Box<InlineButton>)`, which isn't strictly needed
  (bare references use the struct directly); harmless.
- Generated types derive `Debug`, so `{:?}` on an `Update` prints its content.
- `CHANGELOG.md` not touched (release-time entry).

Verification: see "Checking it" above; results are recorded in the atg repo,
`docs/plan/004_tdlib-patch.md`.
