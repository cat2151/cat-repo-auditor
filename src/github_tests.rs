use super::*;

#[test]
fn issue_url_format() {
    let item = IssueOrPr {
        title: String::from("test"),
        updated_at: String::from("2024-01-01"),
        updated_at_raw: String::from("2024-01-01T00:00:00Z"),
        number: 42,
        repo_full: String::from("owner/repo"),
        is_pr: false,
        closes_issue: None,
    };
    assert_eq!(item.url(), "https://github.com/owner/repo/issues/42");
}

#[test]
fn pr_url_format() {
    let item = IssueOrPr {
        title: String::from("test"),
        updated_at: String::from("2024-01-01"),
        updated_at_raw: String::from("2024-01-01T00:00:00Z"),
        number: 7,
        repo_full: String::from("owner/repo"),
        is_pr: true,
        closes_issue: None,
    };
    assert_eq!(item.url(), "https://github.com/owner/repo/pull/7");
}

#[test]
fn local_status_display() {
    assert_eq!(LocalStatus::Pullable.to_string(), "pullable");
    assert_eq!(LocalStatus::Clean.to_string(), "clean");
    assert_eq!(LocalStatus::Staging.to_string(), "staging");
    assert_eq!(LocalStatus::Other.to_string(), "other");
    assert_eq!(LocalStatus::NotFound.to_string(), "-");
    assert_eq!(LocalStatus::NoGit.to_string(), "no-git");
}

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
