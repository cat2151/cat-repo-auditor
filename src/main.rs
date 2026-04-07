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
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, sync::mpsc, time::SystemTime};

use crate::{
    app::{App, READY_MSG},
    config::Config,
    github::{FetchProgress, RepoInfo},
    github_local::{append_cargo_check_after_auto_update_log, check_cargo_git_install},
    history::History,
    main_cli::{parse_subcommand, Subcommand},
    main_fetch::drain_fetch_channel,
    main_helpers::{
        make_log_line, make_startup_log_line, persist_log_line, refresh_log_lines_if_changed,
        start_fetch, STARTUP_LOG_SEPARATOR,
    },
    main_input::{handle_terminal_input, InputState},
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

fn drain_cargo_hash_poll_channel(app: &mut App, rx: &mpsc::Receiver<CargoHashPollEvent>) {
    while let Ok(event) = rx.try_recv() {
        match event {
            CargoHashPollEvent::Checked { name, result } => {
                let now = SystemTime::now();
                let auto_update_poll = app.cargo_hash_poll_after_auto_update(&name);
                let attempt_result = if auto_update_poll { result.clone() } else { None };
                let mut repo_full_name = None;
                let mut latest_hashes = None;
                let matched_remote =
                    if let Some(repo) = app.repos.iter_mut().find(|repo| repo.name == name) {
                        repo_full_name = Some(repo.full_name.clone());
                        let matched_remote = apply_cargo_hash_poll_result(repo, result);
                        if auto_update_poll {
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
                        match attempt_result {
                            Some((_ok, installed_hash, _local_hash, remote_hash)) => {
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
                            }
                            None => {
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
}

fn start_due_cargo_hash_polls(app: &mut App, tx: &mpsc::Sender<CargoHashPollEvent>) {
    let now = SystemTime::now();
    let expired_auto_update_repos: Vec<String> = app
        .cargo_hash_polls
        .iter()
        .filter(|poll| {
            poll.after_auto_update
                && now
                    .duration_since(poll.started_at)
                    .unwrap_or(std::time::Duration::ZERO)
                    >= crate::app::CARGO_HASH_POLL_TIMEOUT
        })
        .map(|poll| poll.repo_name.clone())
        .collect();
    for repo_name in app.expire_cargo_hash_polls_at(now) {
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
            if expired_auto_update_repos.contains(&repo_name) {
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
    if !cached_history.repos.is_empty() {
        app.repos = cached_history.repos;
        app.rate_limit = cached_history.rate_limit;
        app.rebuild_rows();
        app.status_msg = String::from(READY_MSG);
        app.loading = true;
    }

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
