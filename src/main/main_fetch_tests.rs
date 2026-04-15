use super::*;
use crate::config::Config;
use crate::github::{AutoUpdateLaunchRequest, FetchProgress, LocalStatus, RateLimit, RepoInfo};
use crate::main_helpers::BACKGROUND_CHECKS_COMPLETED_MSG;
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

static LOG_TEST_MUTEX: Mutex<()> = Mutex::new(());

struct TempLogDir {
    root: PathBuf,
}

impl TempLogDir {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "catrepo-main-fetch-tests-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("should create temp log dir for test");
        Self { root }
    }

    fn log_path(&self) -> PathBuf {
        Config::log_path_from_config_dir(&self.root)
    }
}

impl Drop for TempLogDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn make_repo(name: &str) -> RepoInfo {
    RepoInfo {
        name: name.to_string(),
        full_name: format!("owner/{name}"),
        updated_at: String::from("2024-01-01"),
        updated_at_raw: String::from("2024-01-01T00:00:00Z"),
        open_issues: 0,
        open_prs: 0,
        is_private: false,
        local_status: LocalStatus::Clean,
        has_local_git: true,
        staging_files: vec![],
        local_head_hash: String::new(),
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
        cargo_install: None,
        cargo_checked_at: String::new(),
        cargo_remote_hash: String::new(),
        cargo_remote_hash_checked_at: String::new(),
        cargo_installed_hash: String::new(),
        wf_workflows: None,
        wf_checked_at: String::new(),
    }
}

fn make_config() -> Config {
    Config {
        owner: String::from("owner"),
        local_base_dir: String::from("."),
        app_run_dir: None,
        auto_pull: false,
        auto_update: false,
    }
}

#[test]
fn drain_fetch_channel_applies_done_ok_and_disconnect_cleanup() {
    let mut app = App::new(make_config());
    app.bg_tasks.push(("chk", 1, 1));
    app.checking_repos.insert(String::from("repo"));
    app.pending_local_repos.insert(String::from("repo"));
    app.pending_cargo_repos.insert(String::from("repo"));

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::Done(Ok((
        vec![],
        RateLimit {
            remaining: 9,
            limit: 60,
            reset_at: String::from("2026-01-01T00:00:00Z"),
        },
    ))))
    .unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    assert_eq!(app.rate_limit.as_ref().map(|r| r.remaining), Some(9));
    assert!(!app.loading);
    assert_eq!(app.status_msg, READY_MSG);
    assert!(fetch_rx.is_none());
    assert!(app.bg_tasks.is_empty());
    assert!(app.checking_repos.is_empty());
    assert!(app.pending_local_repos.is_empty());
    assert!(app.pending_cargo_repos.is_empty());
}

#[test]
fn drain_fetch_channel_tracks_multiple_checking_repos_until_each_update_arrives() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("repo-a"), make_repo("repo-b")];

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::CheckingRepo(String::from("repo-a")))
        .unwrap();
    tx.send(FetchProgress::CheckingRepo(String::from("repo-b")))
        .unwrap();
    tx.send(FetchProgress::ExistenceUpdate {
        name: String::from("repo-a"),
        local_status: LocalStatus::Pullable,
        has_local_git: true,
        staging_files: vec![String::from(" M Cargo.toml")],
        local_head_hash: String::from("local-a"),
        readme_ja: None,
        readme_ja_cat: String::new(),
        readme_ja_badge: None,
        readme_ja_badge_cat: String::new(),
        pages: None,
        pages_cat: String::new(),
        deepwiki: None,
        deepwiki_cat: String::new(),
        wf_workflows: None,
        wf_cat: String::new(),
    })
    .unwrap();

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    assert_eq!(app.checking_repos, HashSet::from([String::from("repo-b")]));
}

#[test]
fn drain_fetch_channel_repo_update_refreshes_issue_pr_state_per_repo() {
    let mut app = App::new(make_config());
    let mut repo = make_repo("repo");
    repo.cargo_checked_at = String::from("cargo-live");
    repo.cargo_remote_hash = String::from("remote-live");
    repo.cargo_remote_hash_checked_at = String::from("2024-01-02T00:00:00Z");
    repo.cargo_installed_hash = String::from("installed-live");
    app.repos = vec![repo];
    app.issue_pr_pending_repos.insert(String::from("repo"));

    let mut updated_repo = make_repo("repo");
    updated_repo.open_prs = 7;
    updated_repo.open_issues = 4;
    updated_repo.prs = vec![crate::github::IssueOrPr {
        title: String::from("Update PR"),
        updated_at: String::from("today"),
        updated_at_raw: String::from("2024-01-03T00:00:00Z"),
        number: 12,
        repo_full: String::from("owner/repo"),
        is_pr: true,
        closes_issue: None,
    }];
    updated_repo.issues = vec![crate::github::IssueOrPr {
        title: String::from("Update issue"),
        updated_at: String::from("today"),
        updated_at_raw: String::from("2024-01-03T00:00:00Z"),
        number: 34,
        repo_full: String::from("owner/repo"),
        is_pr: false,
        closes_issue: None,
    }];

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::RepoUpdate(updated_repo)).unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    let repo = &app.repos[0];
    assert_eq!(repo.open_prs, 7);
    assert_eq!(repo.open_issues, 4);
    assert_eq!(repo.prs.len(), 1);
    assert_eq!(repo.issues.len(), 1);
    assert_eq!(repo.cargo_checked_at, "cargo-live");
    assert_eq!(repo.cargo_remote_hash, "remote-live");
    assert_eq!(repo.cargo_remote_hash_checked_at, "2024-01-02T00:00:00Z");
    assert_eq!(repo.cargo_installed_hash, "installed-live");
    assert!(!app.issue_pr_pending_repos.contains("repo"));
}

#[test]
fn drain_fetch_channel_begin_repo_refresh_replaces_pending_issue_pr_repos() {
    let mut app = App::new(make_config());
    app.issue_pr_pending_repos
        .insert(String::from("stale-repo"));

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::BeginRepoRefresh(vec![
        String::from("repo-a"),
        String::from("repo-b"),
    ]))
    .unwrap();

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    assert_eq!(
        app.issue_pr_pending_repos,
        HashSet::from([String::from("repo-a"), String::from("repo-b")])
    );
}

#[test]
fn drain_fetch_channel_begin_cargo_refresh_adds_pending_cargo_repos() {
    let mut app = App::new(make_config());
    app.pending_cargo_repos.insert(String::from("stale-repo"));

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::BeginCargoRefresh(vec![
        String::from("repo-a"),
        String::from("repo-b"),
    ]))
    .unwrap();

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    assert_eq!(
        app.pending_cargo_repos,
        HashSet::from([
            String::from("stale-repo"),
            String::from("repo-a"),
            String::from("repo-b"),
        ])
    );
    assert!(app.bg_tasks.contains(&("cgo", 0, 3)));
}

#[test]
fn drain_fetch_channel_begin_local_refresh_adds_pending_local_repos() {
    let mut app = App::new(make_config());
    app.pending_local_repos.insert(String::from("stale-repo"));

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::BeginLocalRefresh(vec![
        String::from("repo-a"),
        String::from("repo-b"),
    ]))
    .unwrap();

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    assert_eq!(
        app.pending_local_repos,
        HashSet::from([
            String::from("stale-repo"),
            String::from("repo-a"),
            String::from("repo-b"),
        ])
    );
    assert!(app.bg_tasks.contains(&("lcl", 0, 3)));
}

#[test]
fn drain_fetch_channel_done_err_clears_background_state() {
    let mut app = App::new(make_config());
    app.bg_tasks.push(("cgo", 0, 2));
    app.checking_repos.insert(String::from("repo"));
    app.issue_pr_pending_repos.insert(String::from("repo"));
    app.pending_local_repos.insert(String::from("repo"));
    app.pending_cargo_repos.insert(String::from("repo"));

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::Done(Err(anyhow::anyhow!("boom"))))
        .unwrap();

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    assert!(app.bg_tasks.is_empty());
    assert!(app.checking_repos.is_empty());
    assert!(app.issue_pr_pending_repos.is_empty());
    assert!(app.pending_local_repos.is_empty());
    assert!(app.pending_cargo_repos.is_empty());
    assert_eq!(app.status_msg, "Error: boom");
    assert!(fetch_rx.is_none());
}

#[test]
fn drain_fetch_channel_updates_cargo_remote_hash_checked_at() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("repo")];
    app.repos[0].local_status = LocalStatus::Pullable;
    app.repos[0].staging_files = vec![String::from(" M src/main.rs")];
    app.repos[0].local_head_hash = String::from("local-live");
    app.pending_cargo_repos.insert(String::from("repo"));

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::CargoUpdate {
        name: String::from("repo"),
        cargo_install: Some(true),
        cargo_cat: String::from("local123"),
        cargo_remote_hash: String::from("remote456"),
        cargo_remote_hash_cat: String::from("2024-01-02T00:00:00Z"),
        cargo_installed_hash: String::from("installed789"),
    })
    .unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    let repo = &app.repos[0];
    assert_eq!(repo.local_status, LocalStatus::Pullable);
    assert_eq!(repo.staging_files, vec![String::from(" M src/main.rs")]);
    assert_eq!(repo.local_head_hash, "local-live");
    assert_eq!(repo.cargo_checked_at, "local123");
    assert_eq!(repo.cargo_remote_hash, "remote456");
    assert_eq!(repo.cargo_remote_hash_checked_at, "2024-01-02T00:00:00Z");
    assert_eq!(repo.cargo_installed_hash, "installed789");
    assert!(!app.pending_cargo_repos.contains("repo"));
}

#[test]
fn drain_fetch_channel_queues_auto_update_launch_request() {
    let mut app = App::new(make_config());

    let request = AutoUpdateLaunchRequest {
        name: String::from("repo"),
        full_name: String::from("owner/repo"),
        cargo_install: Some(false),
        installed_hash: String::from("installed123"),
        remote_hash: String::from("remote456"),
    };
    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::RequestAutoUpdateLaunch(request.clone()))
        .unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    assert_eq!(app.pending_auto_update_launches.len(), 1);
    assert_eq!(app.pending_auto_update_launches.front(), Some(&request));
}

#[test]
fn drain_fetch_channel_done_preserves_live_cargo_state() {
    let mut app = App::new(make_config());
    let mut existing = make_repo("repo");
    existing.cargo_install = Some(false);
    existing.cargo_checked_at = String::from("local-live");
    existing.cargo_remote_hash = String::from("remote-live");
    existing.cargo_remote_hash_checked_at = String::from("2024-01-03T00:00:00Z");
    existing.cargo_installed_hash = String::from("installed-live");
    app.repos = vec![existing];

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::Done(Ok((
        vec![make_repo("repo")],
        RateLimit {
            remaining: 9,
            limit: 60,
            reset_at: String::from("2026-01-01T00:00:00Z"),
        },
    ))))
    .unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    let repo = &app.repos[0];
    assert_eq!(repo.cargo_install, Some(false));
    assert_eq!(repo.cargo_checked_at, "local-live");
    assert_eq!(repo.cargo_remote_hash, "remote-live");
    assert_eq!(repo.cargo_remote_hash_checked_at, "2024-01-03T00:00:00Z");
    assert_eq!(repo.cargo_installed_hash, "installed-live");
}

#[test]
fn drain_fetch_channel_existence_update_refreshes_local_state_without_touching_cargo_cache() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("repo")];
    app.repos[0].cargo_checked_at = String::from("cargo-cache-local");
    app.pending_local_repos.insert(String::from("repo"));

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::ExistenceUpdate {
        name: String::from("repo"),
        local_status: LocalStatus::Pullable,
        has_local_git: true,
        staging_files: vec![String::from(" M src/main.rs")],
        local_head_hash: String::from("local-live"),
        readme_ja: None,
        readme_ja_cat: String::new(),
        readme_ja_badge: None,
        readme_ja_badge_cat: String::new(),
        pages: None,
        pages_cat: String::new(),
        deepwiki: None,
        deepwiki_cat: String::new(),
        wf_workflows: None,
        wf_cat: String::new(),
    })
    .unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    let repo = &app.repos[0];
    assert_eq!(repo.local_status, LocalStatus::Pullable);
    assert_eq!(repo.staging_files, vec![String::from(" M src/main.rs")]);
    assert_eq!(repo.local_head_hash, "local-live");
    assert_eq!(repo.cargo_checked_at, "cargo-cache-local");
    assert!(!app.pending_local_repos.contains("repo"));
}

#[test]
fn drain_fetch_channel_persists_background_checks_completed_log() {
    let _guard = LOG_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut app = App::new(make_config());
    let temp_log_dir = TempLogDir::new();
    let log_path = temp_log_dir.log_path();
    fs::create_dir_all(
        log_path
            .parent()
            .expect("log path should have parent directory"),
    )
    .expect("should create log directory for test");

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::BackgroundChecksCompleted).unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel_for_log_path(&mut app, &mut fetch_rx, log_path.as_path());

    let persisted = fs::read_to_string(&log_path).unwrap();
    assert!(persisted.contains(BACKGROUND_CHECKS_COMPLETED_MSG));
    assert!(app
        .log_lines
        .last()
        .is_some_and(|line| line.contains(BACKGROUND_CHECKS_COMPLETED_MSG)));
}

#[test]
fn drain_fetch_channel_persists_log_messages() {
    let _guard = LOG_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut app = App::new(make_config());
    let temp_log_dir = TempLogDir::new();
    let log_path = temp_log_dir.log_path();
    fs::create_dir_all(
        log_path
            .parent()
            .expect("log path should have parent directory"),
    )
    .expect("should create log directory for test");

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::Log(String::from("pull owner/repo: ok")))
        .unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel_for_log_path(&mut app, &mut fetch_rx, log_path.as_path());

    let persisted = fs::read_to_string(&log_path).unwrap();
    assert!(persisted.contains("pull owner/repo: ok"));
    assert!(app
        .log_lines
        .last()
        .is_some_and(|line| line.contains("pull owner/repo: ok")));
}
