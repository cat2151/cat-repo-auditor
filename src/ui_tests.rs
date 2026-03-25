use super::*;
use crate::github::{IssueOrPr, LocalStatus, RepoInfo};
use crate::{app::App, config::Config};
use ratatui::{backend::TestBackend, Terminal};

// ── helpers ──────────────────────────────────────────────────────────────────

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

fn make_issue(number: u64, title: &str, repo_full: &str) -> IssueOrPr {
    IssueOrPr {
        title: title.to_string(),
        updated_at: String::from("2024-01-01"),
        updated_at_raw: String::from("2024-01-01T00:00:00Z"),
        number,
        repo_full: repo_full.to_string(),
        is_pr: false,
        closes_issue: None,
    }
}

fn make_pr(number: u64, title: &str, repo_full: &str, closes: Option<u64>) -> IssueOrPr {
    IssueOrPr {
        title: title.to_string(),
        updated_at: String::from("2024-01-01"),
        updated_at_raw: String::from("2024-01-01T00:00:00Z"),
        number,
        repo_full: repo_full.to_string(),
        is_pr: true,
        closes_issue: closes,
    }
}

fn make_config() -> Config {
    Config {
        owner: "owner".to_string(),
        local_base_dir: ".".to_string(),
        app_run_dir: None,
        auto_pull: false,
    }
}

fn make_test_app_with_focus(window_focused: bool) -> App {
    let mut app = App::new(make_config());
    let mut repo = make_repo("focus-test");
    repo.open_prs = 1;
    repo.open_issues = 2;
    app.repos = vec![repo];
    app.window_focused = window_focused;
    app.rebuild_rows();
    app
}

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let pid = std::process::id();
    let dir_name = format!("{prefix}_{pid}_{nanos}");
    let dir = std::env::temp_dir().join(dir_name);
    std::fs::create_dir_all(&dir).expect("failed to create temporary UI test directory");
    dir
}

struct TempDirGuard {
    path: std::path::PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        Self {
            path: unique_temp_dir(prefix),
        }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[path = "ui_tests/bottom_right_tests.rs"]
mod bottom_right_tests;
#[path = "ui_tests/rendering_tests.rs"]
mod rendering_tests;
#[path = "ui_tests/rows_and_detail_tests.rs"]
mod rows_and_detail_tests;
