use crate::config::Config;
use crate::github::{RateLimit, RepoInfo};
use crate::ui::{build_detail_items, build_rows, Focus, RepoRow, SearchState};

const MAX_LOG_LINES: usize = 2_000;

pub const READY_MSG: &str =
    "q:quit  ?:help  F5:refresh  Nj/Nk:move  h/l:pane  Enter:README  i:pages  w:wiki  g:lazygit  Shift+L:log  /:search";

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
    pub window_focused: bool,
    pub config: Config,
    pub num_prefix: u32,
    /// repo currently being checked in phase 3 (empty = none)
    pub checking_repo: String,
    /// Active background tasks: (tag, cur, total)
    pub bg_tasks: Vec<(&'static str, usize, usize)>,
    pub show_help: bool,
    pub show_columns: bool,
    pub show_log: bool,
    pub log_lines: Vec<String>,
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
    pub(crate) fn new(config: Config) -> Self {
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
            window_focused: true,
            config,
            num_prefix: 0,
            checking_repo: String::new(),
            bg_tasks: vec![],
            show_help: false,
            show_columns: true,
            show_log: false,
            log_lines: vec![],
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
        self.rows = build_rows(&self.repos);
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

    pub fn toggle_log(&mut self) {
        self.show_log = !self.show_log;
    }

    pub fn append_log_line(&mut self, line: String) {
        self.log_lines.push(line);
        if self.log_lines.len() > MAX_LOG_LINES {
            let excess = self.log_lines.len() - MAX_LOG_LINES;
            self.log_lines.drain(0..excess);
        }
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
            READY_MSG,
        );
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
