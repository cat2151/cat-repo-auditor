use super::*;
use crate::config::Config;
use crate::github::{FetchProgress, LocalStatus, RateLimit, RepoInfo};
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
fn drain_fetch_channel_updates_cargo_remote_hash_checked_at() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("repo")];
    app.repos[0].local_status = LocalStatus::Pullable;
    app.repos[0].staging_files = vec![String::from(" M src/main.rs")];
    app.repos[0].local_head_hash = String::from("local-live");

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
}

#[test]
fn drain_fetch_channel_starts_auto_update_cargo_hash_polling() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("repo")];

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::StartAutoUpdateCargoHashPolling {
        name: String::from("repo"),
    })
    .unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    assert_eq!(app.cargo_hash_polls.len(), 1);
    assert_eq!(app.cargo_hash_polls[0].repo_name, "repo");
    assert!(app.cargo_hash_polls[0].after_auto_update);
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
