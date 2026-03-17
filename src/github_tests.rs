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
