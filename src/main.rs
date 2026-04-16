mod app;
mod config;
#[path = "github/github.rs"]
mod github;
#[path = "github/github_fetch.rs"]
mod github_fetch;
#[path = "github/github_local.rs"]
mod github_local;
mod history;
#[path = "main/main_cli.rs"]
mod main_cli;
#[path = "main/main_fetch.rs"]
mod main_fetch;
#[path = "main/main_helpers.rs"]
mod main_helpers;
#[path = "main/main_input.rs"]
mod main_input;
#[path = "main/main_launch.rs"]
mod main_launch;
mod self_update;
#[path = "ui/ui.rs"]
mod ui;
#[path = "ui/ui_detail.rs"]
mod ui_detail;
#[path = "ui/ui_types.rs"]
mod ui_types;

use anyhow::Result;
use crossterm::{
    event::{DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{io, sync::mpsc, time::SystemTime};

use crate::{
    app::{App, READY_MSG},
    config::Config,
    github::{AutoUpdateLaunchRequest, FetchProgress, RepoInfo},
    github_local::{append_cargo_check_after_auto_update_log, check_cargo_git_install},
    history::History,
    main_cli::{parse_subcommand, Subcommand},
    main_fetch::drain_fetch_channel,
    main_helpers::{
        make_log_line, make_startup_log_line, make_x_log_line, persist_log_line,
        refresh_log_lines_if_changed, rerender_terminal, start_fetch, STARTUP_LOG_SEPARATOR,
    },
    main_input::{handle_terminal_input, InputState},
    main_launch::{launch_cargo_app_for_repo, LaunchFeedback},
    self_update::{build_commit_hash, run_self_check, run_self_update},
    ui::draw_ui,
};

#[cfg(test)]
use crate::main_launch::{
    X_NOT_RUN_LOG_NO_CARGO_INSTALLED_APP, X_NOT_RUN_MSG_NO_CARGO_INSTALLED_APP,
};

#[cfg(test)]
#[path = "main/main_tests.rs"]
mod tests;

enum CargoHashPollEvent {
    Checked {
        name: String,
        result: Option<(bool, String, String, String)>,
    },
}

fn apply_cargo_hash_poll_result(
    repo: &mut RepoInfo,
    result: Option<(bool, String, String, String)>,
) -> bool {
    match result {
        Some((ok, installed_hash, local_hash, remote_hash)) => {
            let matches_remote = installed_hash == remote_hash;
            repo.cargo_install = Some(ok);
            repo.cargo_checked_at = local_hash;
            repo.cargo_remote_hash = remote_hash;
            repo.cargo_remote_hash_checked_at = repo.updated_at_raw.clone();
            repo.cargo_installed_hash = installed_hash;
            matches_remote
        }
        None => false,
    }
}

fn persist_repo_cargo_state(repo: &RepoInfo) {
    let path = Config::history_path();
    let path_str = path.to_string_lossy();
    History::update(&path_str, |history| {
        if let Some(history_repo) = history.repos.iter_mut().find(|r| r.name == repo.name) {
            history_repo.cargo_install = repo.cargo_install;
            history_repo.cargo_checked_at = repo.cargo_checked_at.clone();
            history_repo.cargo_remote_hash = repo.cargo_remote_hash.clone();
            history_repo.cargo_remote_hash_checked_at = repo.cargo_remote_hash_checked_at.clone();
            history_repo.cargo_installed_hash = repo.cargo_installed_hash.clone();
        }
    })
    .ok();
}

fn append_auto_update_cargo_poll_log(
    repo_full_name: &str,
    messages: impl IntoIterator<Item = impl AsRef<str>>,
) {
    append_cargo_check_after_auto_update_log(repo_full_name, messages);
}

fn format_installed_hash_check_log(installed_hash: &str, remote_hash: &str) -> String {
    format!("installed hash 確認結果: installed_hash={installed_hash} remote_hash={remote_hash}")
}

fn append_auto_update_cargo_poll_timeout_log(
    repo_full_name: &str,
    installed_hash: &str,
    remote_hash: &str,
) {
    append_auto_update_cargo_poll_log(
        repo_full_name,
        [
            format_installed_hash_check_log(installed_hash, remote_hash),
            String::from(
                "30分経過しても remote hash と一致しなかったため、この repo の polling を終了します。",
            ),
        ],
    );
}

fn apply_cached_history_to_startup(app: &mut App, cached_history: History) {
    if cached_history.repos.is_empty() {
        return;
    }

    let repo_names: Vec<String> = cached_history
        .repos
        .iter()
        .map(|repo| repo.name.clone())
        .collect();
    app.repos = cached_history.repos;
    app.rate_limit = cached_history.rate_limit;
    app.rebuild_rows();
    app.status_msg = String::from(READY_MSG);
    app.loading = true;
    app.set_issue_pr_pending_repos(repo_names.clone());
    app.clear_pending_local_repos();
    app.add_pending_local_repos(repo_names.clone());
    app.set_bg_task_progress("lcl", 0, repo_names.len());
    app.clear_pending_cargo_repos();
    app.add_pending_cargo_repos(repo_names);
    app.set_bg_task_progress("cgo", 0, app.pending_cargo_repos.len());
}

fn drain_cargo_hash_poll_channel(app: &mut App, rx: &mpsc::Receiver<CargoHashPollEvent>) {
    while let Ok(event) = rx.try_recv() {
        match event {
            CargoHashPollEvent::Checked { name, result } => {
                let now = SystemTime::now();
                let auto_update_poll = app.cargo_hash_poll_after_auto_update(&name);
                let check_succeeded = result.is_some();
                let mut repo_full_name = None;
                let mut latest_hashes = None;
                let matched_remote =
                    if let Some(repo) = app.repos.iter_mut().find(|repo| repo.name == name) {
                        repo_full_name = Some(repo.full_name.clone());
                        let matched_remote = apply_cargo_hash_poll_result(repo, result);
                        if auto_update_poll && check_succeeded {
                            latest_hashes = Some((
                                repo.cargo_installed_hash.clone(),
                                repo.cargo_remote_hash.clone(),
                            ));
                        }
                        persist_repo_cargo_state(repo);
                        matched_remote
                    } else {
                        false
                    };

                if matched_remote {
                    app.stop_cargo_hash_polling(&name);
                    if let Some(repo_full_name) = repo_full_name {
                        persist_log_line(
                            app,
                            make_log_line(&format!(
                                "cargo hash polling completed: {repo_full_name} installed==remote"
                            )),
                        );
                        if auto_update_poll {
                            let (installed_hash, remote_hash) = latest_hashes.unwrap_or_default();
                            append_auto_update_cargo_poll_log(
                                &repo_full_name,
                                [
                                    format_installed_hash_check_log(&installed_hash, &remote_hash),
                                    String::from(
                                        "installed hash が remote hash と一致したので、この repo の polling を終了します。",
                                    ),
                                ],
                            );
                        }
                    }
                } else if app.finish_cargo_hash_poll_attempt_at(&name, now) {
                    if let Some(repo_full_name) = repo_full_name {
                        persist_log_line(
                            app,
                            make_log_line(&format!(
                                "cargo hash polling timed out after 30m: {repo_full_name}"
                            )),
                        );
                        if auto_update_poll {
                            let (installed_hash, remote_hash) = latest_hashes.unwrap_or_default();
                            append_auto_update_cargo_poll_timeout_log(
                                &repo_full_name,
                                &installed_hash,
                                &remote_hash,
                            );
                        }
                    }
                } else if auto_update_poll {
                    if let Some(repo_full_name) = repo_full_name {
                        if check_succeeded {
                            let (installed_hash, remote_hash) = latest_hashes.unwrap_or_default();
                            append_auto_update_cargo_poll_log(
                                &repo_full_name,
                                [
                                    format_installed_hash_check_log(
                                        &installed_hash,
                                        &remote_hash,
                                    ),
                                    String::from(
                                        "まだ remote hash と相違していますので、1分後にまた installed hash を確認します。",
                                    ),
                                ],
                            );
                        } else {
                            append_auto_update_cargo_poll_log(
                                &repo_full_name,
                                [String::from(
                                    "installed hash を確認しましたが結果を取得できませんでした。1分後にまた installed hash を確認します。",
                                )],
                            );
                        }
                    }
                }
            }
        }
    }
}

fn start_due_cargo_hash_polls(app: &mut App, tx: &mpsc::Sender<CargoHashPollEvent>) {
    let now = SystemTime::now();
    for expired_poll in app.take_expired_cargo_hash_polls_at(now) {
        let repo_name = expired_poll.repo_name;
        if let Some(repo) = app.repos.iter().find(|repo| repo.name == repo_name) {
            let repo_full_name = repo.full_name.clone();
            let installed_hash = repo.cargo_installed_hash.clone();
            let remote_hash = repo.cargo_remote_hash.clone();
            persist_log_line(
                app,
                make_log_line(&format!(
                    "cargo hash polling timed out after 30m: {repo_full_name}"
                )),
            );
            if expired_poll.after_auto_update {
                append_auto_update_cargo_poll_timeout_log(
                    &repo_full_name,
                    &installed_hash,
                    &remote_hash,
                );
            }
        }
    }
    for repo_name in app.due_cargo_hash_polls_at(now) {
        app.mark_cargo_hash_poll_in_flight(&repo_name);
        let owner = app.config.owner.clone();
        let base_dir = app.config.local_base_dir.clone();
        let tx = tx.clone();
        std::thread::spawn(move || {
            let result = check_cargo_git_install(&owner, &repo_name, &base_dir);
            let _ = tx.send(CargoHashPollEvent::Checked {
                name: repo_name,
                result,
            });
        });
    }
}

fn run_auto_update_launch_request_with<B, Launch, Append, Persist>(
    app: &mut App,
    terminal: &mut Terminal<B>,
    request: AutoUpdateLaunchRequest,
    launch_repo: Launch,
    append_auto_update_log: Append,
    persist_log: Persist,
) -> Result<()>
where
    B: Backend,
    Launch: FnOnce(&str, &str, Option<bool>, &str) -> LaunchFeedback,
    Append: FnOnce(&str, Vec<String>),
    Persist: Fn(&mut App, String),
{
    let run_dir = app.config.resolved_app_run_dir();
    let feedback = launch_repo(
        &app.config.owner,
        &request.name,
        request.cargo_install,
        &run_dir,
    );
    let mut messages = vec![
        String::from(
            "この repo は cargo check で old でしたので、recheck でも old のままか確認しました。",
        ),
        format_installed_hash_check_log(&request.installed_hash, &request.remote_hash),
        format!("update 実行: {}", feedback.log_msg),
    ];
    if feedback.launched {
        messages.push(String::from(
            "1分後から、1分間隔で installed hash を確認し、remote hash と一致したかをこのログに追記します。",
        ));
        app.start_auto_update_cargo_hash_polling(&request.name);
    } else {
        messages.push(String::from(
            "update の起動に失敗したため、1分後の installed hash 確認は開始しません。",
        ));
    }
    append_auto_update_log(&request.full_name, messages);
    persist_log(app, make_x_log_line(&request.full_name, &feedback.log_msg));
    terminal.clear().ok();
    rerender_terminal(app, terminal)?;
    Ok(())
}

fn run_next_pending_auto_update_launch(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<bool> {
    let Some(request) = app.pop_pending_auto_update_launch() else {
        return Ok(false);
    };
    run_auto_update_launch_request_with(
        app,
        terminal,
        request,
        launch_cargo_app_for_repo,
        append_auto_update_cargo_poll_log,
        persist_log_line,
    )?;
    Ok(true)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match parse_subcommand(&args) {
        Ok(Some(Subcommand::Hash)) => {
            println!("{}", build_commit_hash());
            return Ok(());
        }
        Ok(Some(Subcommand::Update)) => {
            let should_exit = run_self_update()?;
            if should_exit {
                std::process::exit(0);
            }
            return Ok(());
        }
        Ok(Some(Subcommand::Check)) => {
            run_self_check()?;
            return Ok(());
        }
        Ok(None) => {}
        Err(err) => err.exit(),
    }

    let config = Config::load()?;
    let history = History::load(&Config::history_path().to_string_lossy()).unwrap_or_default();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableFocusChange
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config.clone());
    refresh_log_lines_if_changed(&mut app);
    persist_log_line(&mut app, String::from(STARTUP_LOG_SEPARATOR));
    persist_log_line(&mut app, make_startup_log_line());

    let cached_history =
        History::load(&Config::history_path().to_string_lossy()).unwrap_or_default();
    apply_cached_history_to_startup(&mut app, cached_history);

    let mut fetch_rx: Option<mpsc::Receiver<FetchProgress>> = Some(start_fetch(config, history));
    let (cargo_hash_poll_tx, cargo_hash_poll_rx) = mpsc::channel();
    let mut input_state = InputState::default();

    loop {
        drain_fetch_channel(&mut app, &mut fetch_rx);
        drain_cargo_hash_poll_channel(&mut app, &cargo_hash_poll_rx);
        start_due_cargo_hash_polls(&mut app, &cargo_hash_poll_tx);

        terminal.draw(|f| {
            app.term_height = f.area().height as usize;
            draw_ui(f, &mut app);
        })?;

        if run_next_pending_auto_update_launch(&mut app, &mut terminal)? {
            continue;
        }
        if !handle_terminal_input(&mut app, &mut terminal, &mut fetch_rx, &mut input_state)? {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableFocusChange
    )?;
    terminal.show_cursor()?;
    Ok(())
}
