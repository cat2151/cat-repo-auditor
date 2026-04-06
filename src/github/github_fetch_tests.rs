use super::*;
use chrono::{Duration, Utc};

#[test]
fn relative_date_invalid_input_returns_as_is() {
    let result = relative_date("not-a-date");
    assert_eq!(result, "not-a-date");
}

#[test]
fn relative_date_old_timestamp_returns_years() {
    // 2000-01-01 is always many years in the past
    let result = relative_date("2000-01-01T00:00:00Z");
    assert!(result.ends_with('y'), "expected year format, got: {result}");
}

#[test]
fn fnv1a_is_deterministic() {
    assert_eq!(fnv1a("hello"), fnv1a("hello"));
    assert_ne!(fnv1a("hello"), fnv1a("world"));
}

#[test]
fn relative_date_today() {
    let ts = (Utc::now() - Duration::hours(1))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    assert_eq!(relative_date(&ts), "today");
}

#[test]
fn relative_date_days() {
    let ts = (Utc::now() - Duration::days(3))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let result = relative_date(&ts);
    assert!(result.ends_with('d'), "expected Nd format, got: {result}");
}

#[test]
fn relative_date_weeks() {
    let ts = (Utc::now() - Duration::weeks(2))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let result = relative_date(&ts);
    assert!(result.ends_with('w'), "expected Nw format, got: {result}");
}

#[test]
fn relative_date_months() {
    let ts = (Utc::now() - Duration::days(45))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let result = relative_date(&ts);
    assert!(result.ends_with("mo"), "expected Nmo format, got: {result}");
}

#[test]
fn format_date_iso_roundtrips_valid_date() {
    let iso = "2024-03-15T10:30:00Z";
    assert_eq!(format_date_iso(iso), "2024-03-15T10:30:00Z");
}

#[test]
fn format_date_iso_returns_input_on_invalid() {
    let bad = "not-a-date";
    assert_eq!(format_date_iso(bad), bad);
}
