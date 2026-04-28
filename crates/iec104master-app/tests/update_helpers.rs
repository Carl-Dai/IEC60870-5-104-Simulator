use chrono::{DateTime, Duration, Utc};
use iec104master_app_lib::update::{is_snoozed, should_check};

fn ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

#[test]
fn should_check_when_no_prior_check() {
    assert!(should_check(None, ts("2026-04-28T10:00:00Z"), Duration::hours(6)));
}

#[test]
fn should_skip_within_throttle_window() {
    let last = ts("2026-04-28T08:00:00Z");
    let now = ts("2026-04-28T10:00:00Z");
    assert!(!should_check(Some(last), now, Duration::hours(6)));
}

#[test]
fn should_check_after_throttle_window() {
    let last = ts("2026-04-28T03:00:00Z");
    let now = ts("2026-04-28T10:00:00Z");
    assert!(should_check(Some(last), now, Duration::hours(6)));
}

#[test]
fn snoozed_when_same_version_within_window() {
    assert!(is_snoozed(
        Some("1.0.9"),
        Some(ts("2026-04-29T00:00:00Z")),
        "1.0.9",
        ts("2026-04-28T10:00:00Z"),
    ));
}

#[test]
fn not_snoozed_after_window_expires() {
    assert!(!is_snoozed(
        Some("1.0.9"),
        Some(ts("2026-04-28T09:00:00Z")),
        "1.0.9",
        ts("2026-04-28T10:00:00Z"),
    ));
}

#[test]
fn not_snoozed_for_different_version() {
    assert!(!is_snoozed(
        Some("1.0.9"),
        Some(ts("2026-04-29T00:00:00Z")),
        "1.0.10",
        ts("2026-04-28T10:00:00Z"),
    ));
}
