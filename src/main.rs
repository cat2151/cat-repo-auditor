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
use std::{io, sync::mpsc};

use crate::{
    app::{App, READY_MSG},
    config::Config,
    github::FetchProgress,
    history::History,
    main_cli::{parse_subcommand, print_update_notice, Subcommand},
    main_fetch::drain_fetch_channel,
    main_helpers::{
        make_startup_log_line, persist_log_line, refresh_log_lines_if_changed, start_fetch,
        STARTUP_LOG_SEPARATOR,
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
    let mut input_state = InputState::default();

    loop {
        drain_fetch_channel(&mut app, &mut fetch_rx);

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
