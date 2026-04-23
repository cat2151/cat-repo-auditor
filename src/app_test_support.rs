use crate::{
    app::App,
    config::Config,
    github::{IssueOrPr, LocalStatus, RepoInfo},
    ui::RepoRow,
};

pub(super) fn make_config() -> Config {
    Config {
        owner: String::from("owner"),
        local_base_dir: String::from("/base"),
        app_run_dir: None,
        auto_pull: false,
        auto_update: false,
    }
}

pub(super) fn make_repo(name: &str) -> RepoInfo {
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
        cargo_check_failed: false,
        wf_workflows: None,
        wf_checked_at: String::new(),
    }
}

pub(super) fn make_issue(number: u64, title: &str, repo_full: &str) -> IssueOrPr {
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

pub(super) fn make_pr(number: u64, title: &str, repo_full: &str, closes: Option<u64>) -> IssueOrPr {
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

pub(super) fn make_active_repo(name: &str) -> RepoInfo {
    let mut r = make_repo(name);
    r.open_prs = 1;
    r
}

pub(super) fn repo_count(app: &App) -> usize {
    app.filtered_rows
        .iter()
        .filter(|r| matches!(r, RepoRow::Repo(_)))
        .count()
}
