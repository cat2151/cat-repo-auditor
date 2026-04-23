use super::*;
#[cfg(test)]
use crate::main_cli::command as cli_command;
use crate::main_helpers::{make_log_line, make_x_log_line, STARTUP_LOG_MSG, STARTUP_LOG_SEPARATOR};
use crate::main_launch::{
    cargo_status_to_launch_args, format_launch_command, launch_cargo_app_for_repo_with,
    x_not_run_feedback_no_cargo_install, LaunchFeedback,
};
use clap::error::ErrorKind;
use ratatui::{backend::TestBackend, Terminal};
use std::cell::RefCell;
use std::{
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

static AUTO_UPDATE_LOG_TEST_MUTEX: Mutex<()> = Mutex::new(());

fn make_poll_repo(name: &str) -> crate::github::RepoInfo {
    crate::github::RepoInfo {
        name: name.to_string(),
        full_name: format!("owner/{name}"),
        updated_at: String::from("2024-01-01"),
        updated_at_raw: String::from("2024-01-01T00:00:00Z"),
        open_issues: 0,
        open_prs: 0,
        is_private: false,
        local_status: crate::github::LocalStatus::Clean,
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

struct TempConfigDir {
    root: PathBuf,
    previous_xdg_config_home: Option<std::ffi::OsString>,
}

impl TempConfigDir {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "catrepo-main-tests-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("should create temp config dir");
        let previous_xdg_config_home = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        Self {
            root,
            previous_xdg_config_home,
        }
    }

    fn auto_update_log_path(&self) -> PathBuf {
        crate::config::Config::cargo_check_after_auto_update_log_path()
    }
}

impl Drop for TempConfigDir {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous_xdg_config_home {
            std::env::set_var("XDG_CONFIG_HOME", previous);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn parse_subcommand_recognizes_hash() {
    let args = vec!["catrepo".to_string(), "hash".to_string()];
    assert_eq!(parse_subcommand(&args).unwrap(), Some(Subcommand::Hash));
}

#[test]
fn parse_subcommand_recognizes_update() {
    let args = vec!["catrepo".to_string(), "update".to_string()];
    assert_eq!(parse_subcommand(&args).unwrap(), Some(Subcommand::Update));
}

#[test]
fn parse_subcommand_recognizes_check() {
    let args = vec!["catrepo".to_string(), "check".to_string()];
    assert_eq!(parse_subcommand(&args).unwrap(), Some(Subcommand::Check));
}

#[test]
fn parse_subcommand_allows_no_command_for_tui_launch() {
    let args = vec!["catrepo".to_string()];
    assert_eq!(parse_subcommand(&args).unwrap(), None);
}

#[test]
fn parse_subcommand_rejects_unknown_command() {
    let args = vec!["catrepo".to_string(), "unknown".to_string()];
    let err = parse_subcommand(&args).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
}

#[test]
fn parse_subcommand_help_subcommand_displays_help() {
    let args = vec!["catrepo".to_string(), "help".to_string()];
    let err = parse_subcommand(&args).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    let output = err.to_string();
    assert!(output.contains("check"));
    assert!(output.contains("update"));
}

#[test]
fn parse_subcommand_help_option_displays_help() {
    let args = vec!["catrepo".to_string(), "--help".to_string()];
    let err = parse_subcommand(&args).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    let output = err.to_string();
    assert!(output.contains("check"));
    assert!(output.contains("help"));
}

#[test]
fn command_help_lists_check_subcommand() {
    let output = cli_command().render_help().to_string();
    assert!(output.contains("check"));
    assert!(output.contains("update"));
    assert!(output.contains("hash"));
}

#[test]
fn make_x_log_line_contains_repo_and_message() {
    let line = make_x_log_line("owner/repo", "run: `bin` cwd=`.`");
    assert!(line.contains("x owner/repo run: `bin` cwd=`.`"));
}

#[test]
fn make_startup_log_line_contains_message() {
    assert!(make_startup_log_line().contains(STARTUP_LOG_MSG));
}

#[test]
fn make_log_line_contains_message() {
    assert!(make_log_line("background checks completed").contains("background checks completed"));
}

#[test]
fn startup_log_separator_matches_expected() {
    assert_eq!(STARTUP_LOG_SEPARATOR, "---");
}

#[test]
fn apply_cached_history_to_startup_marks_cached_repos_pending_in_all_status_columns() {
    let mut app = App::new(crate::config::Config {
        owner: String::from("owner"),
        local_base_dir: String::from("/base"),
        app_run_dir: None,
        auto_pull: false,
        auto_update: false,
    });
    app.issue_pr_pending_repos
        .insert(String::from("stale-issue"));
    app.pending_local_repos.insert(String::from("stale-local"));
    app.pending_cargo_repos.insert(String::from("stale-cargo"));
    let history = crate::history::History {
        repos: vec![make_poll_repo("alpha"), make_poll_repo("beta")],
        rate_limit: Some(crate::github::RateLimit {
            remaining: 42,
            limit: 60,
            reset_at: String::from("2024-01-01T00:00:00Z"),
        }),
        ..crate::history::History::default()
    };

    apply_cached_history_to_startup(&mut app, history);

    assert_eq!(app.repos.len(), 2);
    assert_eq!(app.repos[0].name, "alpha");
    assert_eq!(app.repos[1].name, "beta");
    assert_eq!(app.rate_limit.as_ref().map(|rl| rl.remaining), Some(42));
    assert_eq!(app.status_msg, READY_MSG);
    assert!(app.loading);
    assert_eq!(app.issue_pr_pending_repos.len(), 2);
    assert!(app.issue_pr_pending_repos.contains("alpha"));
    assert!(app.issue_pr_pending_repos.contains("beta"));
    assert_eq!(app.pending_local_repos.len(), 2);
    assert!(app.pending_local_repos.contains("alpha"));
    assert!(app.pending_local_repos.contains("beta"));
    assert_eq!(app.pending_cargo_repos.len(), 2);
    assert!(app.pending_cargo_repos.contains("alpha"));
    assert!(app.pending_cargo_repos.contains("beta"));
    assert!(app.bg_tasks.contains(&("lcl", 0, 2)));
    assert!(app.bg_tasks.contains(&("cgo", 0, 2)));
}

#[test]
fn test_x_launch_args_cargo_ok_returns_empty_slice() {
    assert_eq!(cargo_status_to_launch_args(Some(true)), Some(&[][..]));
}

#[test]
fn test_x_launch_args_cargo_old_returns_update() {
    assert_eq!(
        cargo_status_to_launch_args(Some(false)),
        Some(&["update"][..])
    );
}

#[test]
fn test_x_launch_args_none_returns_none() {
    assert_eq!(cargo_status_to_launch_args(None), None);
}

#[test]
fn test_x_launch_display_joins_args() {
    assert_eq!(
        format_launch_command("foo.exe", &["update"]),
        "foo.exe update"
    );
}

#[test]
fn test_x_not_run_messages_match_expected_wording() {
    let (line, transient_msg) = x_not_run_feedback_no_cargo_install("owner/repo");
    assert_eq!(
        line,
        make_x_log_line("owner/repo", X_NOT_RUN_LOG_NO_CARGO_INSTALLED_APP)
    );
    assert_eq!(transient_msg, X_NOT_RUN_MSG_NO_CARGO_INSTALLED_APP);
}

#[test]
fn launch_cargo_app_for_repo_with_runs_update_for_cargo_old() {
    let feedback = launch_cargo_app_for_repo_with(
        "owner",
        "repo",
        Some(false),
        "/run",
        |owner, repo_name| {
            assert_eq!(owner, "owner");
            assert_eq!(repo_name, "repo");
            Some(vec![String::from("repo-bin")])
        },
        |bin, args, run_dir| {
            assert_eq!(bin, "repo-bin");
            assert_eq!(args, &["update"]);
            assert_eq!(run_dir, "/run");
            Ok(())
        },
    );

    assert_eq!(feedback.transient_msg, "launched: repo-bin update");
    assert_eq!(feedback.log_msg, "run: `repo-bin update` cwd=`/run`");
    assert!(feedback.launched);
}

#[test]
fn launch_cargo_app_for_repo_with_skips_when_repo_has_no_cargo_install() {
    let feedback = launch_cargo_app_for_repo_with(
        "owner",
        "repo",
        None,
        "/run",
        |_owner, _repo_name| panic!("bins lookup should not be called"),
        |_bin, _args, _run_dir| panic!("launcher should not be called"),
    );

    assert_eq!(feedback.transient_msg, X_NOT_RUN_MSG_NO_CARGO_INSTALLED_APP);
    assert_eq!(feedback.log_msg, X_NOT_RUN_LOG_NO_CARGO_INSTALLED_APP);
    assert!(!feedback.launched);
}

fn make_auto_update_request(name: &str) -> crate::github::AutoUpdateLaunchRequest {
    crate::github::AutoUpdateLaunchRequest {
        name: name.to_string(),
        full_name: format!("owner/{name}"),
        cargo_install: Some(false),
        installed_hash: String::from("installed123"),
        remote_hash: String::from("remote456"),
    }
}

#[test]
fn run_auto_update_launch_request_with_starts_polling_and_logs() {
    let mut app = App::new(crate::config::Config {
        owner: String::from("owner"),
        local_base_dir: String::from("/base"),
        app_run_dir: Some(String::from("/run")),
        auto_pull: false,
        auto_update: true,
    });
    app.repos = vec![make_poll_repo("repo")];
    app.rebuild_rows();
    app.term_height = 0;
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let appended = RefCell::new(Vec::<String>::new());
    let persisted = RefCell::new(Vec::<String>::new());

    run_auto_update_launch_request_with(
        &mut app,
        &mut terminal,
        make_auto_update_request("repo"),
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
        |repo_full_name, messages| {
            assert_eq!(repo_full_name, "owner/repo");
            *appended.borrow_mut() = messages;
        },
        |app, line| {
            persisted.borrow_mut().push(line.clone());
            app.append_log_line(line);
        },
    )
    .unwrap();

    assert_eq!(app.term_height, 20);
    assert_eq!(app.cargo_hash_polls.len(), 1);
    assert_eq!(app.cargo_hash_polls[0].repo_name, "repo");
    assert!(app.cargo_hash_polls[0].after_auto_update);
    assert_eq!(persisted.borrow().len(), 1);
    assert!(persisted.borrow()[0].contains("x owner/repo run: `repo-bin update` cwd=`/run`"));
    assert_eq!(
        appended.borrow().as_slice(),
        &[
            String::from("この repo は cargo check で old でしたので、recheck でも old のままか確認しました。"),
            String::from(
                "installed hash 確認結果: installed_hash=installed123 remote_hash=remote456"
            ),
            String::from("update 実行: run: `repo-bin update` cwd=`/run`"),
            String::from(
                "1分後から、1分間隔で installed hash を確認し、remote hash と一致したかをこのログに追記します。"
            ),
        ]
    );
}

#[test]
fn run_auto_update_launch_request_with_failure_skips_polling() {
    let mut app = App::new(crate::config::Config {
        owner: String::from("owner"),
        local_base_dir: String::from("/base"),
        app_run_dir: Some(String::from("/run")),
        auto_pull: false,
        auto_update: true,
    });
    app.repos = vec![make_poll_repo("repo")];
    app.rebuild_rows();
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let appended = RefCell::new(Vec::<String>::new());

    run_auto_update_launch_request_with(
        &mut app,
        &mut terminal,
        make_auto_update_request("repo"),
        |_owner, _repo_name, _cargo_install, _run_dir| LaunchFeedback {
            transient_msg: String::from("run failed: boom"),
            log_msg: String::from("run: `repo-bin update` cwd=`/run` => failed: boom"),
            launched: false,
        },
        |_repo_full_name, messages| {
            *appended.borrow_mut() = messages;
        },
        |app, line| app.append_log_line(line),
    )
    .unwrap();

    assert!(app.cargo_hash_polls.is_empty());
    assert_eq!(
        appended.borrow().last(),
        Some(&String::from(
            "update の起動に失敗したため、1分後の installed hash 確認は開始しません。"
        ))
    );
}

#[test]
fn apply_cargo_hash_poll_result_updates_repo_and_detects_remote_match() {
    let mut repo = make_poll_repo("repo");

    let matched_remote = apply_cargo_hash_poll_result(
        &mut repo,
        Some((
            true,
            String::from("installed123"),
            String::from("local456"),
            String::from("installed123"),
        )),
    );

    assert!(matched_remote);
    assert_eq!(repo.cargo_install, Some(true));
    assert_eq!(repo.cargo_checked_at, "local456");
    assert_eq!(repo.cargo_remote_hash, "installed123");
    assert_eq!(repo.cargo_remote_hash_checked_at, "2024-01-01T00:00:00Z");
    assert_eq!(repo.cargo_installed_hash, "installed123");
}

#[test]
fn test_auto_update_timeout_logged_to_dedicated_file() {
    let _guard = AUTO_UPDATE_LOG_TEST_MUTEX
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let temp_config_dir = TempConfigDir::new();
    let mut app = App::new(Config {
        owner: String::from("owner"),
        local_base_dir: String::from("/base"),
        app_run_dir: None,
        auto_pull: false,
        auto_update: false,
    });
    let mut repo = make_poll_repo("repo");
    repo.cargo_installed_hash = String::from("installed123");
    repo.cargo_remote_hash = String::from("remote456");
    app.repos = vec![repo];
    app.cargo_hash_polls.push(crate::app::CargoHashPoll {
        repo_name: String::from("repo"),
        started_at: UNIX_EPOCH,
        next_check_at: UNIX_EPOCH + Duration::from_secs(60),
        in_flight: true,
        after_auto_update: true,
    });

    let (tx, _rx) = std::sync::mpsc::channel();
    start_due_cargo_hash_polls(&mut app, &tx);

    assert!(app.cargo_hash_polls.is_empty());
    let persisted =
        fs::read_to_string(temp_config_dir.auto_update_log_path()).expect("should write log");
    assert!(persisted.contains("========== owner/repo =========="));
    assert!(persisted
        .contains("installed hash 確認結果: installed_hash=installed123 remote_hash=remote456"));
    assert!(persisted.contains(
        "30分経過しても remote hash と一致しなかったため、この repo の polling を終了します。"
    ));
}
