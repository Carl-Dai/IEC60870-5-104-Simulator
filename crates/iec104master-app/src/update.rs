use chrono::{DateTime, Duration, Utc};

pub fn should_check(
    last_check: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    throttle: Duration,
) -> bool {
    match last_check {
        None => true,
        Some(last) => now - last >= throttle,
    }
}

pub fn is_snoozed(
    snoozed_version: Option<&str>,
    snoozed_until: Option<DateTime<Utc>>,
    remote_version: &str,
    now: DateTime<Utc>,
) -> bool {
    match (snoozed_version, snoozed_until) {
        (Some(v), Some(until)) => v == remote_version && now < until,
        _ => false,
    }
}
