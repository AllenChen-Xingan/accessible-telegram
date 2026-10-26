//! 错误映射与三线状态合成（`specs/contracts/errors.md` 第一节第 2、3 小节；`testing.md` 第一节第 4 小节）

use atg_core::error::{CoreError, KeySource, TdInfo, map_td_error};
use atg_core::status::{AuthStep, CoreState, NetState, user_status};
use atg_core::tree::UserStatus;

#[test]
fn every_mapping_rule_has_a_case() {
    let cases: &[(i32, &str, &str)] = &[
        (401, "Wrong database encryption key", "db_key_invalid"),
        (400, "Database file is already in use", "profile_busy"),
        (406, "SOMETHING_SECRET_ISH", "internal"),
        (400, "PEER_FLOOD", "account_limited"),
        (400, "PHONE_NUMBER_FLOOD", "account_limited"),
        (429, "Too Many Requests: retry after 12", "rate_limited"),
        (420, "FLOOD_WAIT_30", "rate_limited"),
        (400, "SLOWMODE_WAIT_7", "rate_limited"),
        (420, "SOMETHING_ELSE", "account_limited"),
        (401, "Unauthorized", "auth_required"),
        (403, "CHAT_WRITE_FORBIDDEN", "forbidden"),
        (404, "Not Found", "not_found"),
        (400, "CHAT_NOT_FOUND", "not_found"),
        (400, "USER_ID_INVALID", "not_found"),
        (400, "PHONE_NUMBER_INVALID", "auth_phone_invalid"),
        (400, "PHONE_CODE_INVALID", "auth_code_invalid"),
        (400, "PHONE_CODE_EXPIRED", "auth_code_expired"),
        (400, "PASSWORD_HASH_INVALID", "auth_password_invalid"),
        (400, "MESSAGE_TOO_LONG", "invalid_input"),
        (500, "Request timeout", "network"),
        (502, "Bad gateway", "unknown"),
    ];
    for (code, msg, want) in cases {
        assert_eq!(map_td_error(*code, msg).code(), *want, "{code} {msg}");
    }
}

#[test]
fn code_406_builds_no_td_attachment_at_all() {
    let e = map_td_error(406, "PHONE_CODE_INVALID");
    assert_eq!(e, CoreError::Internal { what: "td_406" });
    assert!(e.td().is_none());
}

#[test]
fn td_message_kept_only_for_registered_codes() {
    // 登记过的：留原文
    let e = map_td_error(400, "PHONE_CODE_INVALID");
    assert_eq!(
        e.td(),
        Some(&TdInfo {
            code: 400,
            message: Some("PHONE_CODE_INVALID")
        })
    );
    // 「全是大写和下划线」不等于脱敏：没登记的只留数字码
    for msg in ["USERNAME_ALICE_SECRET", "MESSAGE_TOO_LONG", "hello world"] {
        let e = map_td_error(400, msg);
        assert_eq!(e.td().and_then(|t| t.message), None, "{msg}");
        assert_eq!(e.td().map(|t| t.code), Some(400));
    }
    let e = map_td_error(599, "SOME_UNREGISTERED");
    assert_eq!(
        e,
        CoreError::Unknown {
            td: TdInfo {
                code: 599,
                message: None
            }
        }
    );
}

#[test]
fn rate_limit_parses_seconds_and_drops_text() {
    let secs = |c, m| match map_td_error(c, m) {
        CoreError::RateLimited { retry_after_s, td } => {
            assert_eq!(td.and_then(|t| t.message), None);
            retry_after_s
        }
        other => panic!("not rate_limited: {}", other.code()),
    };
    assert_eq!(secs(420, "FLOOD_WAIT_30"), 30);
    assert_eq!(secs(400, "SLOWMODE_WAIT_7"), 7);
    assert_eq!(secs(429, "Too Many Requests: retry after 12"), 12);
    assert_eq!(secs(429, "Too Many Requests"), 60);
}

#[test]
fn wrong_db_key_is_not_auth_required() {
    assert_eq!(
        map_td_error(401, "Wrong database encryption key"),
        CoreError::DbKeyInvalid {
            source: KeySource::WrongKey
        }
    );
}

#[test]
fn only_rate_limit_and_network_are_retryable() {
    assert!(map_td_error(429, "x").retryable());
    assert!(map_td_error(500, "x").retryable());
    for e in [
        map_td_error(400, "x"),
        map_td_error(401, "x"),
        map_td_error(406, "x"),
        CoreError::Disconnected,
    ] {
        assert!(!e.retryable(), "{}", e.code());
    }
}

// ───── 三线合成 ─────

fn nets() -> Vec<NetState> {
    vec![
        NetState::WaitingForNetwork,
        NetState::ConnectingToProxy,
        NetState::Connecting,
        NetState::Updating,
        NetState::Ready,
        NetState::Unknown {
            td_type: "connectionStateFuture".into(),
        },
    ]
}

/// 规约表的独立写法：一行一个条件，从上往下先中先出
fn by_table(core: CoreState, auth: AuthStep, net: &NetState) -> UserStatus {
    use AuthStep as A;
    use CoreState as C;
    type Row = (fn(C, A, &NetState) -> bool, UserStatus);
    let rows: &[Row] = &[
        (|c, _, _| c == C::Faulted, UserStatus::Fault),
        (
            |c, a, _| matches!(c, C::Closing | C::Closed) || matches!(a, A::Closing | A::Closed),
            UserStatus::Stopping,
        ),
        (
            |c, a, _| c == C::Starting || a == A::WaitParameters,
            UserStatus::Connecting,
        ),
        (|_, a, _| a == A::Unsupported, UserStatus::UnsupportedStep),
        (
            |_, a, _| matches!(a, A::WaitPhone | A::LoggingOut),
            UserStatus::SignedOut,
        ),
        (
            |_, a, _| {
                matches!(
                    a,
                    A::WaitCode
                        | A::WaitEmail
                        | A::WaitEmailCode
                        | A::WaitPassword
                        | A::WaitRegistration
                        | A::WaitOtherDevice
                )
            },
            UserStatus::SigningIn,
        ),
        (
            |_, _, n| *n == NetState::WaitingForNetwork,
            UserStatus::Offline,
        ),
        (
            |_, _, n| {
                matches!(
                    n,
                    NetState::Connecting | NetState::ConnectingToProxy | NetState::Unknown { .. }
                )
            },
            UserStatus::Connecting,
        ),
        (|_, _, n| *n == NetState::Updating, UserStatus::Syncing),
        (
            |c, a, n| c == C::Running && a == A::Ready && *n == NetState::Ready,
            UserStatus::Online,
        ),
    ];
    let hits: Vec<_> = rows
        .iter()
        .filter(|(cond, _)| cond(core, auth, net))
        .collect();
    assert!(!hits.is_empty(), "组合落不到任何一行");
    hits[0].1
}

#[test]
fn every_combination_lands_on_exactly_one_row() {
    let mut n = 0;
    for core in CoreState::ALL {
        for auth in AuthStep::ALL {
            for net in nets() {
                assert_eq!(
                    user_status(core, auth, &net),
                    by_table(core, auth, &net),
                    "{core:?} {auth:?} {net:?}"
                );
                n += 1;
            }
        }
    }
    assert_eq!(n, 5 * 13 * 6);
}

#[test]
fn online_only_when_all_three_ready() {
    for core in CoreState::ALL {
        for auth in AuthStep::ALL {
            for net in nets() {
                let online = user_status(core, auth, &net) == UserStatus::Online;
                let all =
                    core == CoreState::Running && auth == AuthStep::Ready && net == NetState::Ready;
                assert_eq!(online, all, "{core:?} {auth:?} {net:?}");
            }
        }
    }
    assert_ne!(
        user_status(CoreState::Closing, AuthStep::Ready, &NetState::Ready),
        UserStatus::Online
    );
}

#[test]
fn td_type_names_map_to_lines() {
    assert_eq!(
        AuthStep::from_td_type("authorizationStateWaitPhoneNumber"),
        AuthStep::WaitPhone
    );
    assert_eq!(
        AuthStep::from_td_type("authorizationStateWaitPremiumPurchase"),
        AuthStep::Unsupported
    );
    assert_eq!(
        NetState::from_td_type("connectionStateReady"),
        NetState::Ready
    );
    assert_eq!(
        NetState::from_td_type("connectionStateNew"),
        NetState::Unknown {
            td_type: "connectionStateNew".into()
        }
    );
}
