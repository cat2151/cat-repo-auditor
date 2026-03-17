use super::*;
use chrono::{Duration, Utc};

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

#[test]
fn relative_date_today() {
    let ts = (Utc::now() - Duration::hours(1))
        .format("%Y-%m-%dT%H:%M:%SZ").to_string();
    assert_eq!(relative_date(&ts), "today");
}

#[test]
fn relative_date_days() {
    let ts = (Utc::now() - Duration::days(3))
        .format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let result = relative_date(&ts);
    assert!(result.ends_with('d'), "expected Nd format, got: {result}");
}

#[test]
fn relative_date_weeks() {
    let ts = (Utc::now() - Duration::weeks(2))
        .format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let result = relative_date(&ts);
    assert!(result.ends_with('w'), "expected Nw format, got: {result}");
}

#[test]
fn relative_date_months() {
    let ts = (Utc::now() - Duration::days(45))
        .format("%Y-%m-%dT%H:%M:%SZ").to_string();
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

#[test]
fn check_deepwiki_exists_finds_link_in_readme_ja() {
    let tmp = std::env::temp_dir()
        .join(format!("deepwiki_test_a_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.ja.md"), "See https://deepwiki.com/owner/repo\n").unwrap();
    let result = check_deepwiki_exists(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}

#[test]
fn check_deepwiki_exists_false_when_no_link() {
    let tmp = std::env::temp_dir()
        .join(format!("deepwiki_test_b_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.ja.md"), "No links here\n").unwrap();
    let result = check_deepwiki_exists(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_deepwiki_exists_falls_back_to_readme_md() {
    let tmp = std::env::temp_dir()
        .join(format!("deepwiki_test_c_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.md"), "See https://deepwiki.com/owner/repo\n").unwrap();
    let result = check_deepwiki_exists(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}

#[test]
fn check_deepwiki_exists_false_when_no_files() {
    let tmp = std::env::temp_dir()
        .join(format!("deepwiki_test_d_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    let result = check_deepwiki_exists(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_workflows_all_present_returns_true() {
    let tmp = std::env::temp_dir()
        .join(format!("wf_test_a_{}", std::process::id()));
    let wf_dir = tmp.join("myrepo").join(".github").join("workflows");
    std::fs::create_dir_all(&wf_dir).unwrap();
    for f in &[
        "call-translate-readme.yml",
        "call-issue-note.yml",
        "call-check-large-files.yml",
    ] {
        std::fs::write(wf_dir.join(f), "").unwrap();
    }
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}

#[test]
fn check_workflows_missing_one_returns_false() {
    let tmp = std::env::temp_dir()
        .join(format!("wf_test_b_{}", std::process::id()));
    let wf_dir = tmp.join("myrepo").join(".github").join("workflows");
    std::fs::create_dir_all(&wf_dir).unwrap();
    for f in &["call-translate-readme.yml", "call-issue-note.yml"] {
        std::fs::write(wf_dir.join(f), "").unwrap();
    }
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_workflows_empty_dir_returns_false() {
    let tmp = std::env::temp_dir()
        .join(format!("wf_test_c_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_readme_ja_badge_finds_self_reference() {
    let tmp = std::env::temp_dir()
        .join(format!("badge_test_a_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("README.ja.md"),
        "[![ja](README.ja.md)](README.ja.md)\n",
    )
    .unwrap();
    let result = check_readme_ja_badge(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}

#[test]
fn check_readme_ja_badge_false_when_no_self_reference() {
    let tmp = std::env::temp_dir()
        .join(format!("badge_test_b_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.ja.md"), "No badge here\n").unwrap();
    let result = check_readme_ja_badge(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_readme_ja_badge_falls_back_to_readme_md() {
    let tmp = std::env::temp_dir()
        .join(format!("badge_test_c_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.md"), "See README.ja.md\n").unwrap();
    let result = check_readme_ja_badge(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}
