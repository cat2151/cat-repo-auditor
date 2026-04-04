mod app;
mod config;
mod github;
mod github_fetch;
mod github_local;
mod history;
mod main_cli;
mod main_fetch;
mod main_helpers;
mod main_input;
mod main_launch;
mod self_update;
mod ui;
mod ui_detail;
mod ui_types;

use anyhow::Result;
use crossterm::{
    event::{DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io,
    sync::mpsc,
    time::SystemTime,
};

use crate::{
    app::{App, READY_MSG},
    config::Config,
    github::{FetchProgress, RepoInfo},
    github_local::check_cargo_git_install,
    history::History,
    main_cli::{parse_subcommand, print_update_notice, Subcommand},
    main_fetch::drain_fetch_channel,
    main_helpers::{
        make_log_line, make_startup_log_line, persist_log_line, refresh_log_lines_if_changed,
        start_fetch, STARTUP_LOG_SEPARATOR,
    },
    main_input::{handle_terminal_input, InputState},
    self_update::{build_commit_hash, check_self_update, run_self_update},
    ui::draw_ui,
};

#[cfg(test)]
use crate::main_launch::{
    X_NOT_RUN_LOG_NO_CARGO_INSTALLED_APP, X_NOT_RUN_MSG_NO_CARGO_INSTALLED_APP,
};

#[cfg(test)]
#[path = "main_tests.rs"]
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

fn drain_cargo_hash_poll_channel(app: &mut App, rx: &mpsc::Receiver<CargoHashPollEvent>) {
    while let Ok(event) = rx.try_recv() {
        match event {
            CargoHashPollEvent::Checked { name, result } => {
                let now = SystemTime::now();
                let mut repo_full_name = None;
                let matched_remote = if let Some(repo) =
                    app.repos.iter_mut().find(|repo| repo.name == name)
                {
                    repo_full_name = Some(repo.full_name.clone());
                    let matched_remote = apply_cargo_hash_poll_result(repo, result);
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
                    }
                } else if app.finish_cargo_hash_poll_attempt_at(&name, now) {
                    if let Some(repo_full_name) = repo_full_name {
                        persist_log_line(
                            app,
                            make_log_line(&format!(
                                "cargo hash polling timed out after 30m: {repo_full_name}"
                            )),
                        );
                    }
                }
            }
        }
    }
}

fn start_due_cargo_hash_polls(
    app: &mut App,
    tx: &mpsc::Sender<CargoHashPollEvent>,
) {
    let now = SystemTime::now();
    for repo_name in app.expire_cargo_hash_polls_at(now) {
        if let Some(repo_full_name) = app
            .repos
            .iter()
            .find(|repo| repo.name == repo_name)
            .map(|repo| repo.full_name.clone())
        {
            persist_log_line(
                app,
                make_log_line(&format!(
                    "cargo hash polling timed out after 30m: {repo_full_name}"
                )),
            );
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
        Some(Subcommand::Hash) => {
            println!("{}", build_commit_hash());
            return Ok(());
        }
        Some(Subcommand::Update) => {
            let should_exit = run_self_update()?;
            if should_exit {
                std::process::exit(0);
            }
            return Ok(());
        }
        None => {}
    }

    let config = Config::load()?;
    let history = History::load(&Config::history_path().to_string_lossy()).unwrap_or_default();

    let update_rx = {
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
        std::thread::spawn(move || {
            let _ = tx.send(check_self_update());
        });
        rx
    };

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

        if app.update_available.is_none() {
            if let Ok(result) = update_rx.try_recv() {
                app.update_available = result;
            }
        }

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

    print_update_notice(app.update_available.as_deref())
}
