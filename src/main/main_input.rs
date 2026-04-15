use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{
    io,
    sync::mpsc,
    time::{Duration, Instant},
};

#[path = "main_input_clipboard.rs"]
mod clipboard;

use crate::{
    app::{App, READY_MSG},
    config::Config,
    github::FetchProgress,
    github_local::{
        check_local_status_no_fetch, collect_workflow_repo_exist_checks, launch_lazygit, open_url,
    },
    history::History,
    main_helpers::{make_x_log_line, persist_log_line, rerender_terminal, start_fetch},
    main_launch::{launch_cargo_app_for_repo, x_not_run_feedback_no_cargo_install, LaunchFeedback},
    ui::{Focus, RepoRow, SearchState},
};

use clipboard::copy_to_clipboard;

/// Tracks keyboard input state to implement 50ms key debouncing.
#[derive(Default)]
pub(crate) struct InputState {
    last_key: Option<(KeyCode, Instant)>,
}

/// Handles terminal input and returns `Ok(true)` to continue or `Ok(false)` to exit.
pub(crate) fn handle_terminal_input(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    fetch_rx: &mut Option<mpsc::Receiver<FetchProgress>>,
    input_state: &mut InputState,
) -> Result<bool> {
    if !event::poll(Duration::from_millis(100))? {
        return Ok(true);
    }

    let ev = event::read()?;
    if matches!(ev, Event::FocusLost) {
        app.window_focused = false;
        return Ok(true);
    }
    if matches!(ev, Event::FocusGained) {
        app.window_focused = true;
        return Ok(true);
    }

    let Event::Key(key) = ev else {
        return Ok(true);
    };
    if key.kind != KeyEventKind::Press {
        return Ok(true);
    }

    let now = Instant::now();
    if let Some((last_code, last_at)) = input_state.last_key {
        if last_code == key.code && now.duration_since(last_at) < Duration::from_millis(50) {
            return Ok(true);
        }
    }
    input_state.last_key = Some((key.code, now));
    app.transient_msg = None;

    if app.show_help {
        if matches!(key.code, KeyCode::Char('?') | KeyCode::Esc) {
            app.show_help = false;
        }
        return Ok(true);
    }

    if app.show_workflow_repo_exist {
        handle_workflow_repo_exist_overlay(app, key.code, key.modifiers);
        return Ok(true);
    }

    if app.search_state == SearchState::Active {
        handle_search_input(app, key.code, key.modifiers);
        return Ok(true);
    }

    if let KeyCode::Char(c) = key.code {
        if c.is_ascii_digit() && (c != '0' || app.num_prefix > 0) {
            app.push_digit(c.to_digit(10).unwrap());
            return Ok(true);
        }
    }

    if matches!(key.code, KeyCode::Char('L'))
        || (matches!(key.code, KeyCode::Char('l')) && key.modifiers.contains(KeyModifiers::SHIFT))
    {
        app.num_prefix = 0;
        app.toggle_log();
        return Ok(true);
    }

    match app.focus {
        Focus::Repos => handle_repo_focus_input(app, terminal, fetch_rx, key.code, key.modifiers),
        Focus::Detail => handle_detail_focus_input(app, fetch_rx, key.code),
    }
}

fn handle_workflow_repo_exist_overlay(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    let shift_w = matches!(code, KeyCode::Char('W'))
        || (matches!(code, KeyCode::Char('w')) && modifiers.contains(KeyModifiers::SHIFT));
    match code {
        KeyCode::Esc => app.close_workflow_repo_exist(),
        KeyCode::Char('j') | KeyCode::Down => app.workflow_repo_exist_move_down(1),
        KeyCode::Char('k') | KeyCode::Up => app.workflow_repo_exist_move_up(1),
        _ if shift_w => app.close_workflow_repo_exist(),
        _ => {}
    }
}

fn handle_search_input(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    match code {
        KeyCode::Esc => app.search_cancel(),
        KeyCode::Enter => app.search_confirm(),
        KeyCode::Backspace => app.search_pop(),
        KeyCode::Down | KeyCode::Char('j') => app.search_next_match(),
        KeyCode::Up | KeyCode::Char('k') => app.search_prev_match(),
        KeyCode::Char('g') if modifiers.contains(KeyModifiers::CONTROL) => app.search_next_match(),
        KeyCode::Char('t') if modifiers.contains(KeyModifiers::CONTROL) => app.search_prev_match(),
        KeyCode::Char(c) => app.search_push(c),
        _ => {}
    }
}

fn handle_repo_focus_input(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    fetch_rx: &mut Option<mpsc::Receiver<FetchProgress>>,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<bool> {
    let shift_w = matches!(code, KeyCode::Char('W'))
        || (matches!(code, KeyCode::Char('w')) && modifiers.contains(KeyModifiers::SHIFT));

    match code {
        KeyCode::Char('q') => Ok(false),
        KeyCode::Char('j') | KeyCode::Down => {
            let n = app.consume_prefix();
            app.repo_move_down(n);
            Ok(true)
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let n = app.consume_prefix();
            app.repo_move_up(n);
            Ok(true)
        }
        KeyCode::PageDown => {
            app.num_prefix = 0;
            app.repo_page_down();
            Ok(true)
        }
        KeyCode::PageUp => {
            app.num_prefix = 0;
            app.repo_page_up();
            Ok(true)
        }
        KeyCode::Char('l') | KeyCode::Right => {
            app.num_prefix = 0;
            app.focus_detail_first_pr_or_issue();
            Ok(true)
        }
        KeyCode::Char('g') => {
            app.num_prefix = 0;
            if let Some(repo) = app.selected_repo() {
                if repo.has_local_git {
                    let name = repo.name.clone();
                    let base = app.config.local_base_dir.clone();
                    if let Err(e) = launch_lazygit(&base, &name) {
                        app.transient_msg = Some(format!("lazygit: {e}"));
                    } else {
                        refresh_selected_repo_local_status(app);
                        terminal.clear()?;
                        rerender_terminal(app, terminal)?;
                    }
                }
            }
            Ok(true)
        }
        KeyCode::Char('i') => {
            app.num_prefix = 0;
            if let Some(repo) = app.selected_repo() {
                let url = match repo.pages {
                    Some(true) => format!("https://{}.github.io/{}", app.config.owner, repo.name),
                    _ => format!("https://github.com/{}", repo.full_name),
                };
                if let Err(e) = open_url(&url) {
                    app.transient_msg = Some(format!("open failed: {e}"));
                }
            }
            Ok(true)
        }
        _ if shift_w => {
            app.num_prefix = 0;
            match collect_workflow_repo_exist_checks(&app.config.local_base_dir, &app.repos) {
                Ok(items) => app.open_workflow_repo_exist(items),
                Err(e) => app.transient_msg = Some(format!("Shift+W failed: {e}")),
            }
            Ok(true)
        }
        KeyCode::Char('w') => {
            app.num_prefix = 0;
            if let Some(repo) = app.selected_repo() {
                let url = format!("https://deepwiki.com/{}/{}", app.config.owner, repo.name);
                if let Err(e) = open_url(&url) {
                    app.transient_msg = Some(format!("open failed: {e}"));
                }
            }
            Ok(true)
        }
        KeyCode::Enter => {
            app.num_prefix = 0;
            if let Some(repo) = app.selected_repo() {
                let url = match repo.readme_ja {
                    Some(true) => format!(
                        "https://github.com/{}/blob/main/README.ja.md",
                        repo.full_name
                    ),
                    _ => format!("https://github.com/{}", repo.full_name),
                };
                if let Err(e) = open_url(&url) {
                    app.transient_msg = Some(format!("open failed: {e}"));
                }
            }
            Ok(true)
        }
        KeyCode::Char('c') => {
            app.num_prefix = 0;
            if let Some(repo) = app.selected_repo() {
                let base = app.config.local_base_dir.trim_end_matches(['/', '\\']);
                let path = format!("{}\\{}", base, repo.name);
                let clip_path = format!("{}\\", path);
                match copy_to_clipboard(&clip_path) {
                    Ok(()) => app.transient_msg = Some(format!("copied: {clip_path}")),
                    Err(e) => app.transient_msg = Some(format!("clip failed: {e}")),
                }
            }
            Ok(true)
        }
        KeyCode::Char('d') => {
            app.num_prefix = 0;
            app.show_columns = !app.show_columns;
            Ok(true)
        }
        KeyCode::Char('?') => {
            app.num_prefix = 0;
            app.show_help = !app.show_help;
            Ok(true)
        }
        KeyCode::Char('x') => {
            app.num_prefix = 0;
            launch_selected_repo(app, terminal)?;
            Ok(true)
        }
        KeyCode::Char('/') => {
            app.num_prefix = 0;
            app.search_enter();
            Ok(true)
        }
        KeyCode::F(5) => {
            app.num_prefix = 0;
            start_refresh_if_idle(app, fetch_rx);
            Ok(true)
        }
        _ => {
            app.num_prefix = 0;
            Ok(true)
        }
    }
}

fn handle_detail_focus_input(
    app: &mut App,
    fetch_rx: &mut Option<mpsc::Receiver<FetchProgress>>,
    code: KeyCode,
) -> Result<bool> {
    match code {
        KeyCode::Char('q') => Ok(false),
        KeyCode::Char('j') | KeyCode::Down => {
            let n = app.consume_prefix();
            app.detail_move_down(n);
            Ok(true)
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let n = app.consume_prefix();
            app.detail_move_up(n);
            Ok(true)
        }
        KeyCode::PageDown => {
            app.num_prefix = 0;
            app.detail_page_down();
            Ok(true)
        }
        KeyCode::PageUp => {
            app.num_prefix = 0;
            app.detail_page_up();
            Ok(true)
        }
        KeyCode::Char('h') | KeyCode::Left => {
            app.num_prefix = 0;
            app.focus = Focus::Repos;
            Ok(true)
        }
        KeyCode::Enter => {
            app.num_prefix = 0;
            if let Some(url) = app.selected_detail_url() {
                if let Err(e) = open_url(&url) {
                    app.transient_msg = Some(format!("open failed: {e}"));
                }
            }
            Ok(true)
        }
        KeyCode::F(5) => {
            app.num_prefix = 0;
            if fetch_rx.is_none() {
                app.focus = Focus::Repos;
                start_refresh_if_idle(app, fetch_rx);
            }
            Ok(true)
        }
        _ => {
            app.num_prefix = 0;
            Ok(true)
        }
    }
}

fn start_refresh_if_idle(app: &mut App, fetch_rx: &mut Option<mpsc::Receiver<FetchProgress>>) {
    if fetch_rx.is_none() {
        app.search_query.clear();
        app.apply_filter();
        app.bg_tasks.clear();
        app.loading = true;
        app.status_msg = String::from(READY_MSG);
        let history = History::load(&Config::history_path().to_string_lossy()).unwrap_or_default();
        *fetch_rx = Some(start_fetch(app.config.clone(), history));
    }
}

fn launch_selected_repo(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<()> {
    launch_selected_repo_with(app, terminal, launch_cargo_app_for_repo, persist_log_line)
}

fn refresh_selected_repo_local_status(app: &mut App) {
    refresh_selected_repo_local_status_with(app, check_local_status_no_fetch);
}

fn refresh_selected_repo_local_status_with<Check>(app: &mut App, check_local_status: Check)
where
    Check: FnOnce(&str, &str) -> (crate::github::LocalStatus, bool, Vec<String>),
{
    let Some(repo_idx) = app.selected_repo_idx() else {
        return;
    };
    let repo_name = app.repos[repo_idx].name.clone();
    let (local_status, has_local_git, staging_files) =
        check_local_status(&app.config.local_base_dir, &repo_name);
    let repo = &mut app.repos[repo_idx];
    repo.local_status = local_status;
    repo.has_local_git = has_local_git;
    repo.staging_files = staging_files;
    app.rebuild_rows();
    let fallback_row_idx = app.row_cursor;
    app.row_cursor = app
        .filtered_rows
        .iter()
        .position(|row| matches!(row, RepoRow::Repo(idx) if *idx == repo_idx))
        .unwrap_or(fallback_row_idx);
}

/// Launches the selected cargo-installed app and restores the TUI immediately after it exits.
///
/// `launch_repo` must execute the repo app and return launch feedback for the current selection.
/// `persist_log` must record the generated x-key log line in the same way as the production path.
fn launch_selected_repo_with<B, Launch, Persist>(
    app: &mut App,
    terminal: &mut Terminal<B>,
    launch_repo: Launch,
    persist_log: Persist,
) -> Result<()>
where
    B: Backend,
    Launch: FnOnce(&str, &str, Option<bool>, &str) -> LaunchFeedback,
    Persist: Fn(&mut App, String),
{
    if let Some((repo_full_name, repo_name, cargo_install)) = app.selected_repo().map(|repo| {
        (
            repo.full_name.clone(),
            repo.name.clone(),
            repo.cargo_install,
        )
    }) {
        if cargo_install.is_none() {
            let (log_line, transient_msg) = x_not_run_feedback_no_cargo_install(&repo_full_name);
            app.transient_msg = Some(transient_msg);
            persist_log(app, log_line);
        } else {
            let feedback = launch_repo(
                &app.config.owner,
                &repo_name,
                cargo_install,
                &app.config.resolved_app_run_dir(),
            );
            let LaunchFeedback {
                transient_msg,
                log_msg,
                launched,
            } = feedback;
            let needs_cargo_polling = launched && cargo_install == Some(false);
            app.transient_msg = Some(transient_msg);
            persist_log(app, make_x_log_line(&repo_full_name, &log_msg));
            if needs_cargo_polling {
                app.start_cargo_hash_polling(&repo_name);
            }
            terminal.clear().ok();
            rerender_terminal(app, terminal)?;
        }
    } else {
        let line = make_x_log_line("-", "not run: no repository selected");
        app.transient_msg = Some(String::from("x: no repository selected"));
        persist_log(app, line);
    }

    Ok(())
}

#[cfg(test)]
#[path = "main_input_tests.rs"]
mod tests;
