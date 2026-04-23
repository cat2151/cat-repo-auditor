use super::*;
use crate::{
    app::App,
    config::Config,
    github::{LocalStatus, RepoInfo},
    ui::RepoRow,
};
use ratatui::{backend::TestBackend, Terminal};

fn make_config() -> Config {
    Config {
        owner: String::from("owner"),
        local_base_dir: String::from("/base"),
        app_run_dir: Some(String::from("/run")),
        auto_pull: false,
        auto_update: false,
    }
}

fn make_repo(name: &str, cargo_install: Option<bool>) -> RepoInfo {
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
        cargo_install,
        cargo_checked_at: String::new(),
        cargo_remote_hash: String::new(),
        cargo_remote_hash_checked_at: String::new(),
        cargo_installed_hash: String::new(),
        cargo_check_failed: false,
        wf_workflows: None,
        wf_checked_at: String::new(),
    }
}

#[test]
fn test_launch_with_rerender_and_polling() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("repo", Some(false))];
    app.rebuild_rows();
    app.term_height = 0;
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    launch_selected_repo_with(
        &mut app,
        &mut terminal,
        |owner, repo_name, cargo_install, run_dir| {
            assert_eq!(owner, "owner");
            assert_eq!(repo_name, "repo");
            assert_eq!(cargo_install, Some(false));
            assert_eq!(run_dir, "/run");
            LaunchFeedback {
                transient_msg: String::from("launched: repo-bin update"),
                log_msg: String::from("run: `repo-bin update` cwd=`/run`"),
                launched: true,
            }
        },
        |app, line| app.append_log_line(line),
    )
    .unwrap();

    assert_eq!(app.term_height, 20);
    assert_eq!(
        app.transient_msg.as_deref(),
        Some("launched: repo-bin update")
    );
    assert_eq!(app.cargo_hash_polls.len(), 1);
    assert_eq!(app.cargo_hash_polls[0].repo_name, "repo");
    let log_line = app.log_lines.last().expect("expected launch log line");
    assert!(log_line.contains("x owner/repo"));
    assert!(log_line.ends_with("run: `repo-bin update` cwd=`/run`"));
}

#[test]
fn test_launch_rerenders_on_failure_without_starting_polling() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("repo", Some(false))];
    app.rebuild_rows();
    app.term_height = 0;
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    launch_selected_repo_with(
        &mut app,
        &mut terminal,
        |_owner, _repo_name, _cargo_install, _run_dir| LaunchFeedback {
            transient_msg: String::from("run failed: boom"),
            log_msg: String::from("run: `repo-bin update` cwd=`/run` => failed: boom"),
            launched: false,
        },
        |app, line| app.append_log_line(line),
    )
    .unwrap();

    assert_eq!(app.term_height, 20);
    assert!(app.cargo_hash_polls.is_empty());
    assert_eq!(app.transient_msg.as_deref(), Some("run failed: boom"));
}

#[test]
fn test_refresh_selected_repo_local_status_updates_only_selected_repo() {
    let mut app = App::new(make_config());
    let mut alpha = make_repo("alpha", Some(false));
    alpha.local_status = LocalStatus::Modified;
    alpha.staging_files = vec![String::from("src/lib.rs")];
    let mut beta = make_repo("beta", Some(false));
    beta.local_status = LocalStatus::Modified;
    beta.staging_files = vec![String::from("README.md")];
    app.repos = vec![alpha, beta];
    app.rebuild_rows();
    let repo_rows_before: Vec<usize> = app
        .filtered_rows
        .iter()
        .filter_map(|row| match row {
            RepoRow::Repo(idx) => Some(*idx),
            RepoRow::Separator(_) => None,
        })
        .collect();
    assert_eq!(repo_rows_before, vec![0, 1]);
    app.row_cursor = app
        .filtered_rows
        .iter()
        .position(|row| matches!(row, RepoRow::Repo(idx) if *idx == 1))
        .unwrap();

    refresh_selected_repo_local_status_with(&mut app, |base_dir, repo_name| {
        assert_eq!(base_dir, "/base");
        assert_eq!(repo_name, "beta");
        (
            LocalStatus::Clean,
            true,
            vec![String::from("Cargo.toml"), String::from("src/main.rs")],
        )
    });

    assert_eq!(app.repos[0].local_status, LocalStatus::Modified);
    assert_eq!(app.repos[0].staging_files, vec![String::from("src/lib.rs")]);
    assert_eq!(app.repos[1].local_status, LocalStatus::Clean);
    assert_eq!(
        app.repos[1].staging_files,
        vec![String::from("Cargo.toml"), String::from("src/main.rs")]
    );
    let repo_rows_after: Vec<usize> = app
        .filtered_rows
        .iter()
        .filter_map(|row| match row {
            RepoRow::Repo(idx) => Some(*idx),
            RepoRow::Separator(_) => None,
        })
        .collect();
    assert_eq!(repo_rows_after, vec![0, 1]);
    assert!(matches!(
        app.filtered_rows.get(app.row_cursor),
        Some(RepoRow::Repo(1))
    ));
}

#[test]
fn test_refresh_preserves_selection_after_separator_change() {
    let mut app = App::new(make_config());
    let mut alpha = make_repo("alpha", Some(false));
    alpha.open_prs = 1;
    let mut beta = make_repo("beta", Some(false));
    beta.open_prs = 1;
    app.repos = vec![alpha, beta];
    app.rebuild_rows();
    assert!(matches!(app.filtered_rows.get(1), Some(RepoRow::Repo(1))));
    app.row_cursor = 1;

    refresh_selected_repo_local_status_with(&mut app, |_base_dir, repo_name| {
        assert_eq!(repo_name, "beta");
        (LocalStatus::NotFound, false, vec![])
    });

    assert_eq!(app.repos[1].local_status, LocalStatus::NotFound);
    assert_eq!(app.row_cursor, 2);
    assert!(matches!(
        app.filtered_rows.get(1),
        Some(RepoRow::Separator(_))
    ));
    assert!(matches!(
        app.filtered_rows.get(app.row_cursor),
        Some(RepoRow::Repo(1))
    ));
}
