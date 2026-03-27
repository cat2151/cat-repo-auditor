mod app;
mod config;
mod github;
mod github_fetch;
mod github_local;
mod history;
mod main_cli;
mod main_fetch;
mod main_helpers;
mod main_launch;
mod self_update;
mod ui;
mod ui_detail;
mod ui_types;

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
        Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io,
    sync::mpsc,
    time::{Duration, Instant},
};

use crate::{
    app::{App, READY_MSG},
    config::Config,
    github::FetchProgress,
    github_local::{get_cargo_bins, launch_app_with_args, launch_lazygit, open_url},
    history::History,
    main_cli::{parse_subcommand, print_update_notice, Subcommand},
    main_fetch::drain_fetch_channel,
    main_helpers::{
        make_startup_log_line, make_x_log_line, persist_log_line, refresh_log_lines_if_changed,
        start_fetch, STARTUP_LOG_SEPARATOR,
    },
    main_launch::{
        cargo_status_to_launch_args, format_launch_command, x_not_run_feedback_no_cargo_install,
    },
    self_update::{build_commit_hash, check_self_update, run_self_update},
    ui::{draw_ui, Focus, SearchState},
};

#[cfg(test)]
use crate::main_launch::{
    X_NOT_RUN_LOG_NO_CARGO_INSTALLED_APP, X_NOT_RUN_MSG_NO_CARGO_INSTALLED_APP,
};

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;

// ── main ─────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    // ── subcommand dispatch ───────────────────────────────────────────────
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

    // Self-update check (background, non-blocking)
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

    {
        let h = History::load(&Config::history_path().to_string_lossy()).unwrap_or_default();
        if !h.repos.is_empty() {
            app.repos = h.repos;
            app.rate_limit = h.rate_limit;
            app.rebuild_rows();
            app.status_msg = String::from(READY_MSG);
            app.loading = true;
        }
    }

    let mut fetch_rx: Option<mpsc::Receiver<FetchProgress>> =
        Some(start_fetch(config.clone(), history));

    let mut last_key: Option<(KeyCode, Instant)> = None;

    loop {
        // ── drain fetch channel ───────────────────────────────────────────
        drain_fetch_channel(&mut app, &mut fetch_rx);

        // ── poll self-update check ───────────────────────────────────────
        if app.update_available.is_none() {
            if let Ok(result) = update_rx.try_recv() {
                app.update_available = result;
            }
        }

        // ── draw ─────────────────────────────────────────────────────────
        terminal.draw(|f| {
            app.term_height = f.area().height as usize;
            draw_ui(f, &mut app);
        })?;

        // ── input ─────────────────────────────────────────────────────────
        if event::poll(Duration::from_millis(100))? {
            let ev = event::read()?;
            if matches!(ev, Event::FocusLost) {
                app.window_focused = false;
                continue;
            }
            if matches!(ev, Event::FocusGained) {
                app.window_focused = true;
                continue;
            }
            if let Event::Key(key) = ev {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // debounce 50ms
                let now = Instant::now();
                if let Some((lk, lt)) = last_key {
                    if lk == key.code && now.duration_since(lt) < Duration::from_millis(50) {
                        continue;
                    }
                }
                last_key = Some((key.code, now));

                // Clear one-shot transient message on any key press
                app.transient_msg = None;

                // ── help dialog: close on ? or Esc ──────────────────────
                if app.show_help {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Esc => {
                            app.show_help = false;
                        }
                        _ => {}
                    }
                    continue;
                }

                // ── search mode ──────────────────────────────────────────
                if app.search_state == SearchState::Active {
                    match key.code {
                        KeyCode::Esc => {
                            app.search_cancel();
                        }
                        KeyCode::Enter => {
                            app.search_confirm();
                        }
                        KeyCode::Backspace => {
                            app.search_pop();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            // next match (also allow j/k for navigation during search)
                            app.search_next_match();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.search_prev_match();
                        }
                        KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.search_next_match();
                        }
                        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.search_prev_match();
                        }
                        KeyCode::Char(c) => {
                            app.search_push(c);
                        }
                        _ => {}
                    }
                    continue;
                }

                // ── normal mode: digit accumulation ──────────────────────
                if let KeyCode::Char(c) = key.code {
                    if c.is_ascii_digit() && (c != '0' || app.num_prefix > 0) {
                        app.push_digit(c.to_digit(10).unwrap());
                        continue;
                    }
                }
                if matches!(key.code, KeyCode::Char('L'))
                    || (matches!(key.code, KeyCode::Char('l'))
                        && key.modifiers.contains(KeyModifiers::SHIFT))
                {
                    app.num_prefix = 0;
                    app.toggle_log();
                    continue;
                }

                match app.focus {
                    Focus::Repos => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('j') | KeyCode::Down => {
                            let n = app.consume_prefix();
                            app.repo_move_down(n);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            let n = app.consume_prefix();
                            app.repo_move_up(n);
                        }
                        KeyCode::PageDown => {
                            app.num_prefix = 0;
                            app.repo_page_down();
                        }
                        KeyCode::PageUp => {
                            app.num_prefix = 0;
                            app.repo_page_up();
                        }
                        KeyCode::Char('l') | KeyCode::Right => {
                            app.num_prefix = 0;
                            app.focus_detail_first_pr_or_issue();
                        }
                        KeyCode::Char('g') => {
                            app.num_prefix = 0;
                            if let Some(repo) = app.selected_repo() {
                                if repo.has_local_git {
                                    let name = repo.name.clone();
                                    let base = app.config.local_base_dir.clone();
                                    // Drop terminal before handing control to lazygit
                                    if let Err(e) = launch_lazygit(&base, &name) {
                                        app.transient_msg = Some(format!("lazygit: {e}"));
                                    } else {
                                        // Force full redraw after lazygit exits
                                        terminal.clear()?;
                                    }
                                }
                            }
                        }
                        KeyCode::Char('i') => {
                            app.num_prefix = 0;
                            if let Some(repo) = app.selected_repo() {
                                let url = match repo.pages {
                                    Some(true) => format!(
                                        "https://{}.github.io/{}",
                                        app.config.owner, repo.name
                                    ),
                                    _ => format!("https://github.com/{}", repo.full_name),
                                };
                                if let Err(e) = open_url(&url) {
                                    app.transient_msg = Some(format!("open failed: {e}"));
                                }
                            }
                        }
                        KeyCode::Char('w') => {
                            app.num_prefix = 0;
                            if let Some(repo) = app.selected_repo() {
                                let url = format!(
                                    "https://deepwiki.com/{}/{}",
                                    app.config.owner, repo.name
                                );
                                if let Err(e) = open_url(&url) {
                                    app.transient_msg = Some(format!("open failed: {e}"));
                                }
                            }
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
                        }
                        KeyCode::Char('c') => {
                            app.num_prefix = 0;
                            if let Some(repo) = app.selected_repo() {
                                let base = app.config.local_base_dir.trim_end_matches(['/', '\\']);
                                // Always end with backslash (Windows path)
                                let path = format!("{}\\{}", base, repo.name);
                                let clip_path = format!("{}\\", path);
                                let result = std::process::Command::new("cmd")
                                    .args(["/C", &format!("echo {}| clip", clip_path.trim())])
                                    .status();
                                match result {
                                    Ok(_) => {
                                        app.transient_msg = Some(format!("copied: {clip_path}"));
                                    }
                                    Err(e) => {
                                        app.transient_msg = Some(format!("clip failed: {e}"));
                                    }
                                }
                            }
                        }
                        KeyCode::Char('d') => {
                            app.num_prefix = 0;
                            app.show_columns = !app.show_columns;
                        }
                        KeyCode::Char('?') => {
                            app.num_prefix = 0;
                            app.show_help = !app.show_help;
                        }
                        KeyCode::Char('x') => {
                            app.num_prefix = 0;
                            if let Some((repo_full_name, repo_name, cargo_install)) =
                                app.selected_repo().map(|repo| {
                                    (
                                        repo.full_name.clone(),
                                        repo.name.clone(),
                                        repo.cargo_install,
                                    )
                                })
                            {
                                if let Some(args) = cargo_status_to_launch_args(cargo_install) {
                                    let owner = app.config.owner.clone();
                                    let run_dir = app.config.resolved_app_run_dir();
                                    if let Some(bins) = get_cargo_bins(&owner, &repo_name) {
                                        if let Some(bin) = bins.first() {
                                            // Keep .exe suffix – avoids Windows explorer folder collision
                                            let bin = bin.clone();
                                            let cmd = format_launch_command(&bin, args);
                                            let cmd_desc = format!("run: `{cmd}` cwd=`{run_dir}`");
                                            match launch_app_with_args(&bin, args, &run_dir) {
                                                Ok(()) => {
                                                    terminal.clear().ok();
                                                    app.transient_msg =
                                                        Some(format!("launched: {cmd}"));
                                                    let line =
                                                        make_x_log_line(&repo_full_name, &cmd_desc);
                                                    persist_log_line(&mut app, line);
                                                }
                                                Err(e) => {
                                                    app.transient_msg =
                                                        Some(format!("run failed: {e}"));
                                                    let line = make_x_log_line(
                                                        &repo_full_name,
                                                        &format!("{cmd_desc} => failed: {e}"),
                                                    );
                                                    persist_log_line(&mut app, line);
                                                }
                                            }
                                        } else {
                                            let line = make_x_log_line(
                                                &repo_full_name,
                                                "not run: no installed cargo bin found",
                                            );
                                            app.transient_msg = Some(String::from(
                                                "x: no installed cargo bin found",
                                            ));
                                            persist_log_line(&mut app, line);
                                        }
                                    } else {
                                        let line = make_x_log_line(
                                            &repo_full_name,
                                            "not run: .crates2.json has no matching install entry",
                                        );
                                        app.transient_msg = Some(String::from(
                                            "x: no matching cargo install entry",
                                        ));
                                        persist_log_line(&mut app, line);
                                    }
                                } else {
                                    let (log_line, transient_msg) =
                                        x_not_run_feedback_no_cargo_install(&repo_full_name);
                                    app.transient_msg = Some(transient_msg);
                                    persist_log_line(&mut app, log_line);
                                }
                            } else {
                                let line = make_x_log_line("-", "not run: no repository selected");
                                app.transient_msg = Some(String::from("x: no repository selected"));
                                persist_log_line(&mut app, line);
                            }
                        }
                        KeyCode::Char('/') => {
                            app.num_prefix = 0;
                            app.search_enter();
                        }
                        KeyCode::F(5) => {
                            app.num_prefix = 0;
                            if fetch_rx.is_none() {
                                // clear filter before refresh
                                app.search_query.clear();
                                app.apply_filter();
                                app.bg_tasks.clear();
                                app.loading = true;
                                app.status_msg = String::from(READY_MSG);
                                let h = History::load(&Config::history_path().to_string_lossy())
                                    .unwrap_or_default();
                                fetch_rx = Some(start_fetch(config.clone(), h));
                            }
                        }
                        _ => {
                            app.num_prefix = 0;
                        }
                    },
                    Focus::Detail => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('j') | KeyCode::Down => {
                            let n = app.consume_prefix();
                            app.detail_move_down(n);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            let n = app.consume_prefix();
                            app.detail_move_up(n);
                        }
                        KeyCode::PageDown => {
                            app.num_prefix = 0;
                            app.detail_page_down();
                        }
                        KeyCode::PageUp => {
                            app.num_prefix = 0;
                            app.detail_page_up();
                        }
                        KeyCode::Char('h') | KeyCode::Left => {
                            app.num_prefix = 0;
                            app.focus = Focus::Repos;
                        }
                        KeyCode::Enter => {
                            app.num_prefix = 0;
                            if let Some(url) = app.selected_detail_url() {
                                if let Err(e) = open_url(&url) {
                                    app.transient_msg = Some(format!("open failed: {e}"));
                                }
                            }
                        }
                        KeyCode::F(5) => {
                            app.num_prefix = 0;
                            if fetch_rx.is_none() {
                                app.focus = Focus::Repos;
                                app.search_query.clear();
                                app.apply_filter();
                                app.loading = true;
                                app.status_msg = String::from(READY_MSG);
                                let h = History::load(&Config::history_path().to_string_lossy())
                                    .unwrap_or_default();
                                fetch_rx = Some(start_fetch(config.clone(), h));
                            }
                        }
                        _ => {
                            app.num_prefix = 0;
                        }
                    },
                }
            }
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

    // Print update notice after terminal restore (visible in shell)
    print_update_notice(app.update_available.as_deref())
}
