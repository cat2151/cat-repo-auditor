use crate::config::Config;
use crate::github::{AutoUpdateLaunchRequest, RateLimit, RepoInfo};
use crate::github_local::WorkflowRepoExistCheck;
use crate::ui::{build_detail_items, build_rows, Focus, RepoRow, SearchState};
use std::collections::{HashSet, VecDeque};
use std::time::SystemTime;

#[path = "app_search.rs"]
mod app_search;
#[path = "app_cargo_polls.rs"]
pub(crate) mod cargo_polls;

pub(crate) use cargo_polls::CargoHashPoll;
#[cfg(test)]
pub(crate) use cargo_polls::{CARGO_HASH_POLL_INTERVAL, CARGO_HASH_POLL_TIMEOUT};
#[cfg(test)]
pub(crate) use cargo_polls::ExpiredCargoHashPoll;

const MAX_LOG_LINES: usize = 2_000;
pub const READY_MSG: &str =
    "q:quit  ?:help  F5:refresh  Nj/Nk:move  h/l:pane  Enter:README  i:pages  w:wiki  Shift+W:workflow  g:lazygit  Shift+L:log  /:search";

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
    pub pending_auto_update_launches: VecDeque<AutoUpdateLaunchRequest>,
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
            pending_auto_update_launches: VecDeque::new(),
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

    pub(crate) fn queue_auto_update_launch(&mut self, request: AutoUpdateLaunchRequest) {
        self.pending_auto_update_launches.push_back(request);
    }

    pub(crate) fn pop_pending_auto_update_launch(&mut self) -> Option<AutoUpdateLaunchRequest> {
        self.pending_auto_update_launches.pop_front()
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
