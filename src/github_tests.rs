use super::*;

fn make_repo_for_cargo_log() -> RepoInfo {
    RepoInfo {
        name: String::from("repo"),
        full_name: String::from("owner/repo"),
        updated_at: String::from("2024-01-01"),
        updated_at_raw: String::from("2024-01-01T00:00:00Z"),
        open_issues: 0,
        open_prs: 0,
        is_private: false,
        local_status: LocalStatus::Clean,
        has_local_git: true,
        staging_files: vec![],
        issues: vec![],
        prs: vec![],
        readme_ja: None,
        readme_ja_checked_at: String::new(),
        readme_ja_badge: None,
        readme_ja_badge_checked_at: String::new(),
        pages: None,
        pages_checked_at: String::new(),
        deepwiki: None,
        deepwiki_checked_at: String::new(),
        cargo_install: Some(true),
        cargo_checked_at: String::from("local123"),
        cargo_remote_hash: String::from("remote456"),
        cargo_remote_hash_checked_at: String::from("2024-01-01T00:00:00Z"),
        cargo_installed_hash: String::from("installed789"),
        wf_workflows: None,
        wf_checked_at: String::new(),
    }
}

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
    assert_eq!(LocalStatus::Conflict.to_string(), "conflict");
    assert_eq!(LocalStatus::Modified.to_string(), "modified");
    assert_eq!(LocalStatus::Pullable.to_string(), "pullable");
    assert_eq!(LocalStatus::Clean.to_string(), "clean");
    assert_eq!(LocalStatus::Staging.to_string(), "staging");
    assert_eq!(LocalStatus::Other.to_string(), "other");
    assert_eq!(LocalStatus::NotFound.to_string(), "-");
    assert_eq!(LocalStatus::NoGit.to_string(), "no-git");
}

#[test]
fn should_auto_pull_status_matches_issue_rules() {
    assert!(should_auto_pull_status(&LocalStatus::Pullable, false));
    assert!(should_auto_pull_status(&LocalStatus::Modified, false));
    assert!(should_auto_pull_status(&LocalStatus::Staging, false));
    assert!(!should_auto_pull_status(&LocalStatus::Modified, true));
    assert!(!should_auto_pull_status(&LocalStatus::Staging, true));
    assert!(!should_auto_pull_status(&LocalStatus::Clean, false));
    assert!(!should_auto_pull_status(&LocalStatus::Other, false));
}

#[test]
fn cargo_check_decision_log_explains_run_when_cache_is_current() {
    let repo = make_repo_for_cargo_log();

    let log = format_cargo_check_decision_log(
        &repo,
        "local123",
        CargoCheckDecision {
            needs_local: false,
            needs_remote: false,
        },
    );

    assert!(log.contains(
        "cargo check を実行: local HEAD と remote hash cache は最新ですが、installed hash 確認のため毎回実行します"
    ));
    assert!(log.contains("needs_cargo_local=false"));
    assert!(log.contains("needs_cargo_remote=false"));
    assert!(log.contains("local_head=\"local123\""));
    assert!(log.contains("cargo_checked_at=\"local123\""));
    assert!(log.contains("cargo_remote_hash_checked_at=\"2024-01-01T00:00:00Z\""));
    assert!(log.contains("cargo_remote_hash_present=true"));
}

#[test]
fn cargo_check_decision_log_explains_run_when_remote_hash_is_missing() {
    let mut repo = make_repo_for_cargo_log();
    repo.cargo_remote_hash.clear();

    let log = format_cargo_check_decision_log(
        &repo,
        "local123",
        CargoCheckDecision {
            needs_local: false,
            needs_remote: true,
        },
    );

    assert!(log.contains(
        "cargo check を実行: local HEAD cache は最新ですが、remote hash cache が古いか空です"
    ));
    assert!(log.contains("needs_cargo_local=false"));
    assert!(log.contains("needs_cargo_remote=true"));
    assert!(log.contains("cargo_remote_hash_present=false"));
    assert!(log.contains("cargo_install=Some(true)"));
}

#[test]
fn cargo_check_decision_matches_run_state() {
    let repo = make_repo_for_cargo_log();

    let run_with_current_cache = CargoCheckDecision::for_repo(&repo, "local123");
    let run = CargoCheckDecision::for_repo(&repo, "different-local-head");

    assert!(run_with_current_cache.needs_check());
    assert!(run.needs_check());
}

#[test]
fn format_pull_log_includes_repo_and_compacts_success_output() {
    let line = format_pull_log(
        "owner/repo",
        &Ok(String::from("Updating abc..def\nFast-forward\n")),
    );

    assert_eq!(line, "pull owner/repo: Updating abc..def | Fast-forward");
}

#[test]
fn format_pull_log_includes_repo_and_compacts_error_output() {
    let line = format_pull_log(
        "owner/repo",
        &Err(anyhow::anyhow!(
            "git pull failed\nrepository has unresolved conflicts"
        )),
    );

    assert_eq!(
        line,
        "pull owner/repo failed: git pull failed | repository has unresolved conflicts"
    );
}
