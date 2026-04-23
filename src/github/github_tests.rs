use super::cargo_worker::{
    cargo_check_order, format_cargo_check_status_log, resolve_cargo_check_fields, CargoCheckStatus,
};
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
        local_head_hash: String::from("local123"),
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
        cargo_check_failed: false,
        wf_workflows: None,
        wf_checked_at: String::new(),
    }
}

fn make_repo_with_cargo_state(name: &str, cargo_install: Option<bool>) -> RepoInfo {
    let mut repo = make_repo_for_cargo_log();
    repo.name = String::from(name);
    repo.full_name = format!("owner/{name}");
    repo.cargo_install = cargo_install;
    repo
}

fn make_repo_with_cargo_state_and_updated(
    name: &str,
    cargo_install: Option<bool>,
    updated_at_raw: &str,
) -> RepoInfo {
    let mut repo = make_repo_with_cargo_state(name, cargo_install);
    repo.updated_at_raw = String::from(updated_at_raw);
    repo
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
fn cargo_check_status_log_explains_run_when_cache_is_current() {
    let repo = make_repo_for_cargo_log();

    let log = format_cargo_check_status_log(&repo, CargoCheckStatus::new(false));

    assert!(log.contains(
        "cargo check を実行: local check には依存せず、installed hash 確認のため毎回実行します"
    ));
    assert!(log.contains("needs_cargo_remote=false"));
    assert!(log.contains("cargo_remote_hash_checked_at=\"2024-01-01T00:00:00Z\""));
    assert!(log.contains("cargo_remote_hash_present=true"));
    assert!(log.contains("cargo_check_failed=false"));
}

#[test]
fn cargo_check_status_log_explains_run_when_remote_hash_is_missing() {
    let mut repo = make_repo_for_cargo_log();
    repo.cargo_remote_hash.clear();

    let log = format_cargo_check_status_log(&repo, CargoCheckStatus::new(true));

    assert!(log.contains(
        "cargo check を実行: local check には依存せず、remote hash cache が古いか空です"
    ));
    assert!(log.contains("needs_cargo_remote=true"));
    assert!(log.contains("cargo_remote_hash_present=false"));
    assert!(log.contains("cargo_install=Some(true)"));
}

#[test]
fn cargo_check_status_matches_run_state() {
    let repo = make_repo_for_cargo_log();

    let run_with_current_cache = CargoCheckStatus::for_repo(&repo);
    let mut missing_remote_hash = repo.clone();
    missing_remote_hash.cargo_remote_hash.clear();
    let run = CargoCheckStatus::for_repo(&missing_remote_hash);

    assert!(!run_with_current_cache.needs_remote());
    assert!(run.needs_remote());
}

#[test]
fn resolve_cargo_check_fields_clears_hashes_and_marks_failure_on_failure() {
    let repo = make_repo_for_cargo_log();

    let resolved = resolve_cargo_check_fields(
        &repo.updated_at_raw,
        crate::github_local::CargoGitInstallCheck::Failed,
    );

    assert_eq!(
        resolved,
        (
            None,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            true,
        )
    );
}

#[test]
fn resolve_cargo_check_fields_clears_hashes_without_failure_when_not_installed() {
    let repo = make_repo_for_cargo_log();

    let resolved = resolve_cargo_check_fields(
        &repo.updated_at_raw,
        crate::github_local::CargoGitInstallCheck::NotInstalled,
    );

    assert_eq!(
        resolved,
        (
            None,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            false,
        )
    );
}

#[test]
fn cargo_check_order_follows_updated_descending() {
    let repos = vec![
        make_repo_with_cargo_state_and_updated("repo-ok", Some(true), "2024-01-02T00:00:00Z"),
        make_repo_with_cargo_state_and_updated("repo-old-1", Some(false), "2024-01-03T00:00:00Z"),
        make_repo_with_cargo_state_and_updated("repo-none", None, "2024-01-04T00:00:00Z"),
        make_repo_with_cargo_state_and_updated("repo-old-2", Some(false), "2024-01-01T00:00:00Z"),
    ];

    assert_eq!(
        cargo_check_order(&repos),
        vec![
            String::from("repo-none"),
            String::from("repo-old-1"),
            String::from("repo-ok"),
            String::from("repo-old-2"),
        ]
    );
}

#[test]
fn split_startup_and_post_fetch_cargo_repos_runs_cached_repos_first() {
    let cached_repos = vec![
        make_repo_with_cargo_state("repo-a", Some(true)),
        make_repo_with_cargo_state("repo-b", Some(false)),
    ];
    let fetched_repos = vec![
        make_repo_with_cargo_state("repo-a", Some(true)),
        make_repo_with_cargo_state("repo-b", Some(false)),
        make_repo_with_cargo_state("repo-c", None),
    ];

    let (startup_repos, post_fetch_repos) =
        split_startup_and_post_fetch_cargo_repos(&cached_repos, &fetched_repos);

    assert_eq!(
        startup_repos
            .iter()
            .map(|repo| repo.name.as_str())
            .collect::<Vec<_>>(),
        vec!["repo-a", "repo-b"]
    );
    assert_eq!(
        post_fetch_repos
            .iter()
            .map(|repo| repo.name.as_str())
            .collect::<Vec<_>>(),
        vec!["repo-c"]
    );
}

#[test]
fn split_startup_and_post_fetch_cargo_repos_checks_all_fetched_repos_without_cache() {
    let fetched_repos = vec![
        make_repo_with_cargo_state("repo-a", Some(true)),
        make_repo_with_cargo_state("repo-b", Some(false)),
    ];

    let (startup_repos, post_fetch_repos) =
        split_startup_and_post_fetch_cargo_repos(&[], &fetched_repos);

    assert!(startup_repos.is_empty());
    assert_eq!(
        post_fetch_repos
            .iter()
            .map(|repo| repo.name.as_str())
            .collect::<Vec<_>>(),
        vec!["repo-a", "repo-b"]
    );
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

#[test]
fn refresh_repos_after_auto_pull_updates_only_targeted_local_state() {
    let mut repo_a = make_repo_for_cargo_log();
    repo_a.name = String::from("repo-a");
    repo_a.full_name = String::from("owner/repo-a");
    repo_a.local_status = LocalStatus::Pullable;
    repo_a.local_head_hash = String::from("old-head-a");

    let mut repo_b = make_repo_for_cargo_log();
    repo_b.name = String::from("repo-b");
    repo_b.full_name = String::from("owner/repo-b");
    repo_b.local_status = LocalStatus::Clean;
    repo_b.local_head_hash = String::from("old-head-b");

    let mut repos = vec![repo_a, repo_b];
    let refreshed_repo_names = vec![String::from("repo-a")];

    refresh_repos_after_auto_pull_with(
        &mut repos,
        "C:\\repos",
        &refreshed_repo_names,
        |_base_dir, repo_name| match repo_name {
            "repo-a" => (
                LocalStatus::Modified,
                true,
                vec![String::from(" M Cargo.toml")],
            ),
            other => panic!("unexpected repo status refresh: {other}"),
        },
        |_base_dir, repo_name| match repo_name {
            "repo-a" => String::from("new-head-a"),
            other => panic!("unexpected repo head refresh: {other}"),
        },
    );

    assert_eq!(repos[0].local_status, LocalStatus::Modified);
    assert_eq!(repos[0].staging_files, vec![String::from(" M Cargo.toml")]);
    assert_eq!(repos[0].local_head_hash, "new-head-a");
    assert_eq!(repos[1].local_status, LocalStatus::Clean);
    assert_eq!(repos[1].local_head_hash, "old-head-b");
}

#[test]
fn should_spawn_auto_update_after_recheck_requires_repo_to_still_be_old() {
    assert!(should_spawn_auto_update_after_recheck(
        "owner",
        "repo",
        "/base",
        Some(false),
        |_owner, _repo_name, _base_dir| Some((false, String::new(), String::new(), String::new())),
    ));
    assert!(!should_spawn_auto_update_after_recheck(
        "owner",
        "repo",
        "/base",
        Some(false),
        |_owner, _repo_name, _base_dir| Some((true, String::new(), String::new(), String::new())),
    ));
    assert!(!should_spawn_auto_update_after_recheck(
        "owner",
        "repo",
        "/base",
        Some(false),
        |_owner, _repo_name, _base_dir| None,
    ));
    assert!(!should_spawn_auto_update_after_recheck(
        "owner",
        "repo",
        "/base",
        Some(true),
        |_owner, _repo_name, _base_dir| panic!("recheck should not run for cargo ok"),
    ));
}

#[test]
fn should_skip_auto_update_for_repo_when_target_is_cat_repo_auditor_itself() {
    assert!(should_skip_auto_update_for_repo(
        crate::self_update::REPO_OWNER,
        crate::self_update::REPO_NAME,
    ));
    assert!(should_skip_auto_update_for_repo(
        "Cat2151",
        "Cat-Repo-Auditor"
    ));
    assert!(!should_skip_auto_update_for_repo(
        crate::self_update::REPO_OWNER,
        "another-repo",
    ));
}

#[test]
fn should_spawn_auto_update_after_recheck_skips_cat_repo_auditor_itself() {
    assert!(!should_spawn_auto_update_after_recheck(
        crate::self_update::REPO_OWNER,
        crate::self_update::REPO_NAME,
        "/base",
        Some(false),
        |_owner, _repo_name, _base_dir| {
            panic!("recheck should not run for cat-repo-auditor itself")
        },
    ));
}

#[test]
fn inspect_auto_update_after_recheck_reports_updated_and_failed_cases() {
    assert_eq!(
        inspect_auto_update_after_recheck("owner", "repo", "/base", Some(false), |_o, _r, _b| {
            Some((
                true,
                String::from("installed123"),
                String::from("local123"),
                String::from("remote123"),
            ))
        }),
        AutoUpdateAfterRecheck::UpdatedDuringRecheck {
            installed_hash: String::from("installed123"),
            remote_hash: String::from("remote123"),
        }
    );
    assert_eq!(
        inspect_auto_update_after_recheck("owner", "repo", "/base", Some(false), |_o, _r, _b| {
            None
        }),
        AutoUpdateAfterRecheck::RecheckFailed
    );
}
