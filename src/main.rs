mod app;
mod config;
mod github;
mod github_fetch;
mod github_local;
mod history;
mod ui;
mod ui_detail;
mod ui_types;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, sync::mpsc, time::{Duration, Instant}};

use crate::{
    app::{App, READY_MSG},
    config::Config,
    github::{fetch_repos_with_progress, FetchProgress},
    github_local::{check_self_update, get_cargo_bins, launch_app, launch_lazygit, open_url},
    history::History,
    ui::{draw_ui, Focus, SearchState},
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn start_fetch(config: Config, history: History) -> mpsc::Receiver<FetchProgress> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || { fetch_repos_with_progress(config, history, tx); });
    rx
}

// ── main ─────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let config = Config::load()?;
    let history = History::load(&Config::history_path().to_string_lossy()).unwrap_or_default();

    // Self-update check (background, non-blocking)
    let update_rx = {
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
        std::thread::spawn(move || { let _ = tx.send(check_self_update()); });
        rx
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config.clone());

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
        if let Some(ref rx) = fetch_rx {
            loop {
                match rx.try_recv() {
                    Ok(FetchProgress::Status(_msg)) => {
                        // Background status messages are shown in the rate-limit bar via PhaseProgress.
                        // status_msg stays as operation help.
                    }
                    Ok(FetchProgress::PhaseProgress { tag, cur, total }) => {
                        if cur == 0 && total == 0 {
                            // Clear signal
                            app.bg_tasks.retain(|t: &(&str, usize, usize)| t.0 != tag);
                        } else {
                            // Upsert
                            if let Some(entry) = app.bg_tasks.iter_mut().find(|t| t.0 == tag) {
                                entry.1 = cur;
                                entry.2 = total;
                            } else {
                                app.bg_tasks.push((tag, cur, total));
                            }
                        }
                    }
                    Ok(FetchProgress::CheckingRepo(name)) => {
                        app.checking_repo = name;
                    }
                    Ok(FetchProgress::ExistenceUpdate {
                        name,
                        readme_ja, readme_ja_cat,
                        readme_ja_badge, readme_ja_badge_cat,
                        pages, pages_cat,
                        deepwiki, deepwiki_cat,
                        cargo_install, cargo_cat,
                        cargo_installed_hash,
                        wf_workflows, wf_cat,
                    }) => {
                        if let Some(r) = app.repos.iter_mut().find(|r| r.name == name) {
                            r.readme_ja                  = readme_ja;
                            r.readme_ja_checked_at       = readme_ja_cat;
                            r.readme_ja_badge            = readme_ja_badge;
                            r.readme_ja_badge_checked_at = readme_ja_badge_cat;
                            r.pages                      = pages;
                            r.pages_checked_at           = pages_cat;
                            r.deepwiki                   = deepwiki;
                            r.deepwiki_checked_at        = deepwiki_cat;
                            r.cargo_install              = cargo_install;
                            r.cargo_checked_at           = cargo_cat;
                            r.cargo_installed_hash       = cargo_installed_hash;
                            r.wf_workflows               = wf_workflows;
                            r.wf_checked_at              = wf_cat;
                        }
                        app.checking_repo.clear();
                    }
                    Ok(FetchProgress::Done(Ok((repos, rl)))) => {
                        app.repos = repos;
                        app.rate_limit = Some(rl);
                        app.rebuild_rows();
                        app.loading = false;
                        app.status_msg = String::from(READY_MSG);
                        // Do NOT set fetch_rx = None here:
                        // phase 3 (CheckingRepo / ExistenceUpdate) messages come after Done.
                        // Keep draining until Disconnected.
                    }
                    Ok(FetchProgress::Done(Err(e))) => {
                        app.loading = false;
                        app.status_msg = format!("Error: {e}");
                        fetch_rx = None;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        fetch_rx = None;
                        app.bg_tasks.clear();
                        app.checking_repo.clear();
                        break;
                    }
                }
            }
        }

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
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press { continue; }

                // debounce 50ms
                let now = Instant::now();
                if let Some((lk, lt)) = last_key {
                    if lk == key.code && now.duration_since(lt) < Duration::from_millis(50) { continue; }
                }
                last_key = Some((key.code, now));

                // Clear one-shot transient message on any key press
                app.transient_msg = None;

                // ── help dialog: close on ? or Esc ──────────────────────
                if app.show_help {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Esc => { app.show_help = false; }
                        _ => {}
                    }
                    continue;
                }

                // ── search mode ──────────────────────────────────────────
                if app.search_state == SearchState::Active {
                    match key.code {
                        KeyCode::Esc => { app.search_cancel(); }
                        KeyCode::Enter => { app.search_confirm(); }
                        KeyCode::Backspace => { app.search_pop(); }
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
                        KeyCode::Char(c) => { app.search_push(c); }
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

                match app.focus {
                    Focus::Repos => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('j') | KeyCode::Down => {
                            let n = app.consume_prefix(); app.repo_move_down(n);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            let n = app.consume_prefix(); app.repo_move_up(n);
                        }
                        KeyCode::PageDown => { app.num_prefix = 0; app.repo_page_down(); }
                        KeyCode::PageUp   => { app.num_prefix = 0; app.repo_page_up(); }
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
                                let base = app.config.local_base_dir
                                    .trim_end_matches(['/', '\\']);
                                // Always end with backslash (Windows path)
                                let path = format!("{}\\{}", base, repo.name);
                                let clip_path = format!("{}\\", path);
                                let result = std::process::Command::new("cmd")
                                    .args(["/C", &format!("echo {}| clip", clip_path.trim())])
                                    .status();
                                match result {
                                    Ok(_) => { app.transient_msg = Some(format!("copied: {clip_path}")); }
                                    Err(e) => { app.transient_msg = Some(format!("clip failed: {e}")); }
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
                            if let Some(repo) = app.selected_repo() {
                                if repo.cargo_install == Some(true) {
                                    let owner   = app.config.owner.clone();
                                    let name    = repo.name.clone();
                                    let run_dir = app.config.resolved_app_run_dir();
                                    if let Some(bins) = get_cargo_bins(&owner, &name) {
                                        if let Some(bin) = bins.first() {
                                            // Keep .exe suffix – avoids Windows explorer folder collision
                                            let bin = bin.clone();
                                            match launch_app(&bin, &run_dir) {
                                                Ok(()) => { terminal.clear().ok(); app.transient_msg = Some(format!("launched: {bin}")); }
                                                Err(e) => { app.transient_msg = Some(format!("run failed: {e}")); }
                                            }
                                        }
                                    }
                                }
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
                                let h = History::load(&Config::history_path().to_string_lossy()).unwrap_or_default();
                                fetch_rx = Some(start_fetch(config.clone(), h));
                            }
                        }
                        _ => { app.num_prefix = 0; }
                    },
                    Focus::Detail => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('j') | KeyCode::Down => {
                            let n = app.consume_prefix(); app.detail_move_down(n);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            let n = app.consume_prefix(); app.detail_move_up(n);
                        }
                        KeyCode::PageDown => { app.num_prefix = 0; app.detail_page_down(); }
                        KeyCode::PageUp   => { app.num_prefix = 0; app.detail_page_up(); }
                        KeyCode::Char('h') | KeyCode::Left => {
                            app.num_prefix = 0; app.focus = Focus::Repos;
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
                                let h = History::load(&Config::history_path().to_string_lossy()).unwrap_or_default();
                                fetch_rx = Some(start_fetch(config.clone(), h));
                            }
                        }
                        _ => { app.num_prefix = 0; }
                    },
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    // Print update notice after terminal restore (visible in shell)
    if let Some(ref repo) = app.update_available {
        println!();
        println!("gh-tui update available!");
        println!("Run: cargo install --force --git https://github.com/{repo}");
        println!();
    }

    Ok(())
}
