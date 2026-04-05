use crate::config::Config;
use crate::github::{RateLimit, RepoInfo};
use crate::github_local::WorkflowRepoExistCheck;
use crate::ui::{build_detail_items, build_rows, Focus, RepoRow, SearchState};
use std::collections::HashSet;
use std::time::{Duration, SystemTime};

#[path = "app_search.rs"]
mod app_search;

const MAX_LOG_LINES: usize = 2_000;
pub(crate) const CARGO_HASH_POLL_INTERVAL: Duration = Duration::from_secs(60);
pub(crate) const CARGO_HASH_POLL_TIMEOUT: Duration = Duration::from_secs(30 * 60);

pub const READY_MSG: &str =
    "q:quit  ?:help  F5:refresh  Nj/Nk:move  h/l:pane  Enter:README  i:pages  w:wiki  Shift+W:workflow  g:lazygit  Shift+L:log  /:search";

#[derive(Debug, Clone)]
pub(crate) struct CargoHashPoll {
    pub repo_name: String,
    pub started_at: SystemTime,
    pub next_check_at: SystemTime,
    pub in_flight: bool,
}

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
    /// repos currently being checked in phase 3
    pub checking_repos: HashSet<String>,
    /// Active background tasks: (tag, cur, total)
    pub bg_tasks: Vec<(&'static str, usize, usize)>,
    pub cargo_hash_polls: Vec<CargoHashPoll>,
    pub show_help: bool,
    pub show_workflow_repo_exist: bool,
    pub workflow_repo_exist_items: Vec<WorkflowRepoExistCheck>,
    pub workflow_repo_exist_selected: usize,
    pub workflow_repo_exist_scroll: usize,
    pub show_columns: bool,
    pub show_log: bool,
    pub log_lines: Vec<String>,
    pub log_last_modified: Option<SystemTime>,
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
            checking_repos: HashSet::new(),
            bg_tasks: vec![],
            cargo_hash_polls: vec![],
            show_help: false,
            show_workflow_repo_exist: false,
            workflow_repo_exist_items: vec![],
            workflow_repo_exist_selected: 0,
            workflow_repo_exist_scroll: 0,
            show_columns: true,
            show_log: false,
            log_lines: vec![],
            log_last_modified: None,
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

    pub fn selected_repo_idx(&self) -> Option<usize> {
        self.filtered_rows.get(self.row_cursor).and_then(|r| {
            if let RepoRow::Repo(idx) = r {
                Some(*idx)
            } else {
                None
            }
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
            if i >= self.filtered_rows.len() {
                break;
            }
        }
        self.reset_detail();
    }

    pub fn repo_move_up(&mut self, n: usize) {
        for _ in 0..n {
            if self.row_cursor == 0 {
                break;
            }
            let mut i = self.row_cursor;
            loop {
                if i == 0 {
                    break;
                }
                i -= 1;
                if matches!(self.filtered_rows[i], RepoRow::Repo(_)) {
                    self.row_cursor = i;
                    break;
                }
            }
        }
        self.reset_detail();
    }

    pub fn repo_page_down(&mut self) {
        self.repo_move_down(self.left_visible.saturating_sub(1).max(1));
    }
    pub fn repo_page_up(&mut self) {
        self.repo_move_up(self.left_visible.saturating_sub(1).max(1));
    }

    fn reset_detail(&mut self) {
        self.detail_selected = 0;
        self.detail_scroll = 0;
    }

    pub fn adjust_row_scroll(&mut self, visible: usize) {
        if visible == 0 {
            return;
        }
        if self.row_cursor < self.row_scroll {
            self.row_scroll = self.row_cursor;
        } else if self.row_cursor >= self.row_scroll + visible {
            self.row_scroll = self.row_cursor + 1 - visible;
        }
    }

    // ── right pane movement ──────────────────────────────────────────────────

    pub fn detail_len(&self) -> usize {
        if let Some(r) = self.selected_repo() {
            build_detail_items(r).len()
        } else {
            0
        }
    }

    pub fn detail_move_down(&mut self, n: usize) {
        let max = self.detail_len().saturating_sub(1);
        self.detail_selected = (self.detail_selected + n).min(max);
    }

    pub fn detail_move_up(&mut self, n: usize) {
        self.detail_selected = self.detail_selected.saturating_sub(n);
    }

    pub fn detail_page_down(&mut self) {
        self.detail_move_down(self.right_visible.saturating_sub(1).max(1));
    }
    pub fn detail_page_up(&mut self) {
        self.detail_move_up(self.right_visible.saturating_sub(1).max(1));
    }

    pub fn adjust_detail_scroll(&mut self, visible: usize) {
        if visible == 0 {
            return;
        }
        if self.detail_selected < self.detail_scroll {
            self.detail_scroll = self.detail_selected;
        } else if self.detail_selected >= self.detail_scroll + visible {
            self.detail_scroll = self.detail_selected + 1 - visible;
        }
    }

    pub fn open_workflow_repo_exist(&mut self, items: Vec<WorkflowRepoExistCheck>) {
        self.show_workflow_repo_exist = true;
        self.workflow_repo_exist_items = items;
        self.workflow_repo_exist_selected = 0;
        self.workflow_repo_exist_scroll = 0;
    }

    pub fn close_workflow_repo_exist(&mut self) {
        self.show_workflow_repo_exist = false;
    }

    pub fn selected_workflow_repo_exist(&self) -> Option<&WorkflowRepoExistCheck> {
        self.workflow_repo_exist_items
            .get(self.workflow_repo_exist_selected)
    }

    pub fn workflow_repo_exist_move_down(&mut self, n: usize) {
        let max = self.workflow_repo_exist_items.len().saturating_sub(1);
        self.workflow_repo_exist_selected = (self.workflow_repo_exist_selected + n).min(max);
    }

    pub fn workflow_repo_exist_move_up(&mut self, n: usize) {
        self.workflow_repo_exist_selected = self.workflow_repo_exist_selected.saturating_sub(n);
    }

    pub fn adjust_workflow_repo_exist_scroll(&mut self, visible: usize) {
        if visible == 0 {
            return;
        }
        if self.workflow_repo_exist_selected < self.workflow_repo_exist_scroll {
            self.workflow_repo_exist_scroll = self.workflow_repo_exist_selected;
        } else if self.workflow_repo_exist_selected >= self.workflow_repo_exist_scroll + visible {
            self.workflow_repo_exist_scroll = self.workflow_repo_exist_selected + 1 - visible;
        }
    }

    pub fn selected_detail_url(&self) -> Option<String> {
        let repo = self.selected_repo()?;
        build_detail_items(repo)
            .get(self.detail_selected)
            .map(|i| i.url.clone())
    }

    // ── lキー: jump to first PR, else first issue ─────────────────────────

    pub fn focus_detail_first_pr_or_issue(&mut self) {
        if let Some(repo) = self.selected_repo() {
            let items = build_detail_items(repo);
            if items.is_empty() {
                return;
            }
            // find first PR
            let idx = items.iter().position(|it| it.is_pr).unwrap_or(0);
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
        let n = if self.num_prefix == 0 {
            1
        } else {
            self.num_prefix as usize
        };
        self.num_prefix = 0;
        n
    }

    pub fn toggle_log(&mut self) {
        self.show_log = !self.show_log;
    }

    pub(crate) fn start_cargo_hash_polling(&mut self, repo_name: &str) {
        self.start_cargo_hash_polling_at(repo_name, SystemTime::now());
    }

    pub(crate) fn start_cargo_hash_polling_at(&mut self, repo_name: &str, now: SystemTime) {
        let next_check_at = now + CARGO_HASH_POLL_INTERVAL;
        if let Some(poll) = self
            .cargo_hash_polls
            .iter_mut()
            .find(|poll| poll.repo_name == repo_name)
        {
            poll.started_at = now;
            poll.next_check_at = next_check_at;
            poll.in_flight = false;
        } else {
            self.cargo_hash_polls.push(CargoHashPoll {
                repo_name: repo_name.to_string(),
                started_at: now,
                next_check_at,
                in_flight: false,
            });
        }
    }

    pub(crate) fn due_cargo_hash_polls_at(&self, now: SystemTime) -> Vec<String> {
        self.cargo_hash_polls
            .iter()
            .filter(|poll| !poll.in_flight && poll.next_check_at <= now)
            .map(|poll| poll.repo_name.clone())
            .collect()
    }

    pub(crate) fn mark_cargo_hash_poll_in_flight(&mut self, repo_name: &str) {
        if let Some(poll) = self
            .cargo_hash_polls
            .iter_mut()
            .find(|poll| poll.repo_name == repo_name)
        {
            poll.in_flight = true;
        }
    }

    pub(crate) fn stop_cargo_hash_polling(&mut self, repo_name: &str) {
        self.cargo_hash_polls
            .retain(|poll| poll.repo_name != repo_name);
    }

    pub(crate) fn finish_cargo_hash_poll_attempt_at(
        &mut self,
        repo_name: &str,
        now: SystemTime,
    ) -> bool {
        if let Some(idx) = self
            .cargo_hash_polls
            .iter()
            .position(|poll| poll.repo_name == repo_name)
        {
            if now
                .duration_since(self.cargo_hash_polls[idx].started_at)
                .unwrap_or(Duration::ZERO)
                >= CARGO_HASH_POLL_TIMEOUT
            {
                self.cargo_hash_polls.remove(idx);
                true
            } else {
                self.cargo_hash_polls[idx].in_flight = false;
                self.cargo_hash_polls[idx].next_check_at = now + CARGO_HASH_POLL_INTERVAL;
                false
            }
        } else {
            false
        }
    }

    pub(crate) fn expire_cargo_hash_polls_at(&mut self, now: SystemTime) -> Vec<String> {
        let expired: Vec<String> = self
            .cargo_hash_polls
            .iter()
            .filter(|poll| {
                now.duration_since(poll.started_at)
                    .unwrap_or(Duration::ZERO)
                    >= CARGO_HASH_POLL_TIMEOUT
            })
            .map(|poll| poll.repo_name.clone())
            .collect();
        for repo_name in &expired {
            self.stop_cargo_hash_polling(repo_name);
        }
        expired
    }

    pub(crate) fn active_cargo_hash_poll_count(&self) -> usize {
        self.cargo_hash_polls.len()
    }

    fn trim_log_lines(lines: &mut Vec<String>) {
        if lines.len() > MAX_LOG_LINES {
            let excess = lines.len() - MAX_LOG_LINES;
            lines.drain(0..excess);
        }
    }

    pub fn set_log_lines(&mut self, mut lines: Vec<String>) {
        Self::trim_log_lines(&mut lines);
        self.log_lines = lines;
    }

    pub fn append_log_line(&mut self, line: String) {
        self.log_lines.push(line);
        Self::trim_log_lines(&mut self.log_lines);
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
