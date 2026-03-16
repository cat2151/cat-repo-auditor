mod config;
mod github;
mod history;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, sync::mpsc, time::{Duration, Instant}};

use crate::{
    config::Config,
    github::{check_self_update, fetch_repos_with_progress, get_cargo_bins, launch_app, launch_lazygit, open_url, FetchProgress, RateLimit, RepoInfo},
    history::History,
    ui::{build_detail_items, draw_ui, Focus, RepoRow, SearchState},
};

// ── App ──────────────────────────────────────────────────────────────────────

pub struct App {
    pub repos: Vec<RepoInfo>,
    pub rows: Vec<RepoRow>,          // full (unfiltered)
    pub filtered_rows: Vec<RepoRow>, // active list (may equal rows when no filter)
    pub row_cursor: usize,           // index into filtered_rows
    pub row_scroll: usize,
    pub focus: Focus,
    pub detail_selected: usize,
    pub detail_scroll: usize,
    pub rate_limit: Option<RateLimit>,
    pub status_msg: String,
    /// One-shot message shown until next key press; overrides status_msg display
    pub transient_msg: Option<String>,
    pub loading: bool,
    pub config: Config,
    pub num_prefix: u32,
    /// repo currently being checked in phase 3 (empty = none)
    pub checking_repo: String,
    /// Active background tasks: (tag, cur, total)
    pub bg_tasks: Vec<(&'static str, usize, usize)>,
    pub show_help: bool,
    pub show_columns: bool,
    /// Some("owner/repo") when update is available
    pub update_available: Option<String>,
    pub term_height: usize,
    pub left_visible: usize,
    pub right_visible: usize,
    // search
    pub search_state: SearchState,
    pub search_query: String,
    /// cursor saved when entering search, restored on Esc
    pub search_saved_cursor: usize,
    /// index within current filter match list for cycling with Ctrl+G / Ctrl+T
    pub search_match_idx: usize,
}

impl App {
    fn new(config: Config) -> Self {
        Self {
            repos: vec![],
            rows: vec![],
            filtered_rows: vec![],
            row_cursor: 0,
            row_scroll: 0,
            focus: Focus::Repos,
            detail_selected: 0,
            detail_scroll: 0,
            rate_limit: None,
            status_msg: String::from(READY_MSG),
            transient_msg: None,
            loading: true,
            config,
            num_prefix: 0,
            checking_repo: String::new(),
            bg_tasks: vec![],
            show_help: false,
            show_columns: true,
            update_available: None,
            term_height: 40,
            left_visible: 30,
            right_visible: 30,
            search_state: SearchState::Off,
            search_query: String::new(),
            search_saved_cursor: 0,
            search_match_idx: 0,
        }
    }

    pub fn rebuild_rows(&mut self) {
        self.rows = ui::build_rows(&self.repos);
        self.apply_filter();
    }

    // ── filter ───────────────────────────────────────────────────────────────

    pub fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_rows = self.rows.clone();
        } else {
            let terms: Vec<String> = self.search_query
                .split_whitespace()
                .map(|t| t.to_lowercase())
                .collect();
            self.filtered_rows = self.rows.iter().filter(|row| {
                match row {
                    RepoRow::Separator(_) => false, // hide separators in search results
                    RepoRow::Repo(idx) => {
                        let name = self.repos[*idx].name.to_lowercase();
                        terms.iter().all(|t| name.contains(t.as_str()))
                    }
                }
            }).cloned().collect();
        }
        // clamp cursor
        if self.filtered_rows.is_empty() {
            self.row_cursor = 0;
        } else {
            self.row_cursor = self.row_cursor.min(self.filtered_rows.len() - 1);
            self.snap_cursor_to_repo(true);
        }
    }

    fn snap_cursor_to_repo(&mut self, forward: bool) {
        let len = self.filtered_rows.len();
        if len == 0 { return; }
        let start = self.row_cursor;
        let mut i = start;
        loop {
            if matches!(self.filtered_rows[i], RepoRow::Repo(_)) {
                self.row_cursor = i;
                return;
            }
            if forward { i = (i + 1) % len; }
            else {
                if i == 0 { self.snap_cursor_to_repo(true); return; }
                i -= 1;
            }
            if i == start { break; }
        }
    }

    pub fn selected_repo_idx(&self) -> Option<usize> {
        self.filtered_rows.get(self.row_cursor).and_then(|r| {
            if let RepoRow::Repo(idx) = r { Some(*idx) } else { None }
        })
    }

    pub fn selected_repo(&self) -> Option<&RepoInfo> {
        self.selected_repo_idx().and_then(|i| self.repos.get(i))
    }

    // ── left pane movement ───────────────────────────────────────────────────

    pub fn repo_move_down(&mut self, n: usize) {
        for _ in 0..n {
            let mut i = self.row_cursor + 1;
            while i < self.filtered_rows.len() {
                if matches!(self.filtered_rows[i], RepoRow::Repo(_)) {
                    self.row_cursor = i;
                    break;
                }
                i += 1;
            }
            if i >= self.filtered_rows.len() { break; }
        }
        self.reset_detail();
    }

    pub fn repo_move_up(&mut self, n: usize) {
        for _ in 0..n {
            if self.row_cursor == 0 { break; }
            let mut i = self.row_cursor;
            loop {
                if i == 0 { break; }
                i -= 1;
                if matches!(self.filtered_rows[i], RepoRow::Repo(_)) {
                    self.row_cursor = i;
                    break;
                }
            }
        }
        self.reset_detail();
    }

    pub fn repo_page_down(&mut self) { self.repo_move_down(self.left_visible.saturating_sub(1).max(1)); }
    pub fn repo_page_up(&mut self)   { self.repo_move_up(self.left_visible.saturating_sub(1).max(1)); }

    fn reset_detail(&mut self) {
        self.detail_selected = 0;
        self.detail_scroll = 0;
    }

    pub fn adjust_row_scroll(&mut self, visible: usize) {
        if visible == 0 { return; }
        if self.row_cursor < self.row_scroll {
            self.row_scroll = self.row_cursor;
        } else if self.row_cursor >= self.row_scroll + visible {
            self.row_scroll = self.row_cursor + 1 - visible;
        }
    }

    // ── right pane movement ──────────────────────────────────────────────────

    pub fn detail_len(&self) -> usize {
        if let Some(r) = self.selected_repo() { build_detail_items(r).len() }
        else { 0 }
    }

    pub fn detail_move_down(&mut self, n: usize) {
        let max = self.detail_len().saturating_sub(1);
        self.detail_selected = (self.detail_selected + n).min(max);
    }

    pub fn detail_move_up(&mut self, n: usize) {
        self.detail_selected = self.detail_selected.saturating_sub(n);
    }

    pub fn detail_page_down(&mut self) { self.detail_move_down(self.right_visible.saturating_sub(1).max(1)); }
    pub fn detail_page_up(&mut self)   { self.detail_move_up(self.right_visible.saturating_sub(1).max(1)); }

    pub fn adjust_detail_scroll(&mut self, visible: usize) {
        if visible == 0 { return; }
        if self.detail_selected < self.detail_scroll {
            self.detail_scroll = self.detail_selected;
        } else if self.detail_selected >= self.detail_scroll + visible {
            self.detail_scroll = self.detail_selected + 1 - visible;
        }
    }

    pub fn selected_detail_url(&self) -> Option<String> {
        let repo = self.selected_repo()?;
        build_detail_items(repo).get(self.detail_selected).map(|i| i.url.clone())
    }

    // ── lキー: jump to first PR, else first issue ─────────────────────────

    pub fn focus_detail_first_pr_or_issue(&mut self) {
        if let Some(repo) = self.selected_repo() {
            let items = build_detail_items(repo);
            if items.is_empty() { return; }
            // find first PR
            let idx = items.iter().position(|it| it.is_pr)
                .or_else(|| Some(0))
                .unwrap();
            self.detail_selected = idx;
            self.detail_scroll = 0;
            self.focus = Focus::Detail;
        }
    }

    // ── num prefix ───────────────────────────────────────────────────────────

    pub fn push_digit(&mut self, d: u32) {
        self.num_prefix = self.num_prefix.saturating_mul(10).saturating_add(d);
    }

    pub fn consume_prefix(&mut self) -> usize {
        let n = if self.num_prefix == 0 { 1 } else { self.num_prefix as usize };
        self.num_prefix = 0;
        n
    }

    // ── search ───────────────────────────────────────────────────────────────

    pub fn search_enter(&mut self) {
        self.search_state = SearchState::Active;
        self.search_saved_cursor = self.row_cursor;
        self.search_query.clear();
        self.search_match_idx = 0;
        self.status_msg = String::from("/ (Space=AND, Enter=confirm, Esc=cancel, ^G/^T=next/prev)");
    }

    pub fn search_push(&mut self, c: char) {
        self.search_query.push(c);
        self.apply_filter();
        self.search_match_idx = 0;
        // auto-jump to first match
        if !self.filtered_rows.is_empty() {
            self.row_cursor = 0;
            self.snap_cursor_to_repo(true);
            self.row_scroll = 0;
        }
    }

    pub fn search_pop(&mut self) {
        self.search_query.pop();
        self.apply_filter();
        if !self.filtered_rows.is_empty() {
            self.row_cursor = 0;
            self.snap_cursor_to_repo(true);
            self.row_scroll = 0;
        }
    }

    pub fn search_next_match(&mut self) {
        let matches: Vec<usize> = self.filtered_rows.iter().enumerate()
            .filter(|(_, r)| matches!(r, RepoRow::Repo(_)))
            .map(|(i, _)| i)
            .collect();
        if matches.is_empty() { return; }
        self.search_match_idx = (self.search_match_idx + 1) % matches.len();
        self.row_cursor = matches[self.search_match_idx];
        self.row_scroll = self.row_cursor.saturating_sub(self.left_visible / 2);
    }

    pub fn search_prev_match(&mut self) {
        let matches: Vec<usize> = self.filtered_rows.iter().enumerate()
            .filter(|(_, r)| matches!(r, RepoRow::Repo(_)))
            .map(|(i, _)| i)
            .collect();
        if matches.is_empty() { return; }
        self.search_match_idx = if self.search_match_idx == 0 {
            matches.len() - 1
        } else {
            self.search_match_idx - 1
        };
        self.row_cursor = matches[self.search_match_idx];
        self.row_scroll = self.row_cursor.saturating_sub(self.left_visible / 2);
    }

    pub fn search_confirm(&mut self) {
        self.search_state = SearchState::Off;
        // Save the repo index the cursor is currently on (in filtered list)
        let target_repo_idx = self.filtered_rows.get(self.row_cursor)
            .and_then(|r| if let RepoRow::Repo(i) = r { Some(*i) } else { None });
        // Clear filter – filtered_rows becomes full rows again
        self.search_query.clear();
        self.filtered_rows = self.rows.clone();
        // Restore cursor to the same repo in the full list
        if let Some(target) = target_repo_idx {
            if let Some(pos) = self.filtered_rows.iter().position(
                |r| matches!(r, RepoRow::Repo(i) if *i == target)
            ) {
                self.row_cursor = pos;
                self.row_scroll = pos.saturating_sub(self.left_visible / 2);
            }
        }
        self.status_msg = String::from(READY_MSG);
    }

    pub fn search_cancel(&mut self) {
        self.search_state = SearchState::Off;
        self.search_query.clear();
        self.apply_filter();
        self.row_cursor = self.search_saved_cursor.min(
            self.filtered_rows.len().saturating_sub(1)
        );
        self.row_scroll = self.row_cursor.saturating_sub(self.left_visible / 2);
        self.status_msg = String::from(
            "q:quit  ?:help  F5:refresh  Nj/Nk:move  h/l:pane  Enter:README  i:pages  w:wiki  g:lazygit  /:search",
        );
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn start_fetch(config: Config, history: History) -> mpsc::Receiver<FetchProgress> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || { fetch_repos_with_progress(config, history, tx); });
    rx
}

const READY_MSG: &str =
    "q:quit  ?:help  F5:refresh  Nj/Nk:move  h/l:pane  Enter:README  i:pages  w:wiki  g:lazygit  /:search";

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
                        cargo_installed_hash, cargo_local_hash,
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
                            r.cargo_local_hash           = cargo_local_hash;
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
        println!("┌─────────────────────────────────────────────────────┐");
        println!("│  gh-tui update available!                           │");
        println!("│  Run:                                               │");
        println!("│    cargo install --force --git                      │");
        println!("│      https://github.com/{repo:<37}│");
        println!("└─────────────────────────────────────────────────────┘");
        println!();
    }

    Ok(())
}
