use super::*;
use crate::config::Config;
use crate::github::{FetchProgress, LocalStatus, RateLimit, RepoInfo};

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
    }
}

#[test]
fn drain_fetch_channel_applies_done_ok_and_disconnect_cleanup() {
    let mut app = App::new(make_config());
    app.bg_tasks.push(("chk", 1, 1));
    app.checking_repo = String::from("repo");

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
    assert!(app.checking_repo.is_empty());
}

#[test]
fn drain_fetch_channel_updates_cargo_remote_hash_checked_at() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("repo")];

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::ExistenceUpdate {
        name: String::from("repo"),
        readme_ja: None,
        readme_ja_cat: String::new(),
        readme_ja_badge: None,
        readme_ja_badge_cat: String::new(),
        pages: None,
        pages_cat: String::new(),
        deepwiki: None,
        deepwiki_cat: String::new(),
        cargo_install: Some(true),
        cargo_cat: String::from("local123"),
        cargo_remote_hash: String::from("remote456"),
        cargo_remote_hash_cat: String::from("2024-01-02T00:00:00Z"),
        cargo_installed_hash: String::from("installed789"),
        wf_workflows: None,
        wf_cat: String::new(),
    })
    .unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    let repo = &app.repos[0];
    assert_eq!(repo.cargo_checked_at, "local123");
    assert_eq!(repo.cargo_remote_hash, "remote456");
    assert_eq!(repo.cargo_remote_hash_checked_at, "2024-01-02T00:00:00Z");
    assert_eq!(repo.cargo_installed_hash, "installed789");
}
