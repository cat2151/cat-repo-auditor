use super::{App, READY_MSG};
use crate::ui::{RepoRow, SearchState};

impl App {
    // ── filter ───────────────────────────────────────────────────────────────

    pub fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_rows.clone_from(&self.rows);
        } else {
            let terms: Vec<String> = self
                .search_query
                .split_whitespace()
                .map(|t| t.to_lowercase())
                .collect();
            self.filtered_rows.clear();
            self.filtered_rows.extend(
                self.rows
                    .iter()
                    .filter(|row| match row {
                        RepoRow::Separator(_) => false, // hide separators in search results
                        RepoRow::Repo(idx) => {
                            let name = self.repos[*idx].name.to_lowercase();
                            terms.iter().all(|t| name.contains(t.as_str()))
                        }
                    })
                    .cloned(),
            );
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
        if len == 0 {
            return;
        }
        let start = self.row_cursor;
        let mut i = start;
        loop {
            if matches!(self.filtered_rows[i], RepoRow::Repo(_)) {
                self.row_cursor = i;
                return;
            }
            if forward {
                i = (i + 1) % len;
            } else {
                if i == 0 {
                    self.snap_cursor_to_repo(true);
                    return;
                }
                i -= 1;
            }
            if i == start {
                break;
            }
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
        let matches: Vec<usize> = self
            .filtered_rows
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r, RepoRow::Repo(_)))
            .map(|(i, _)| i)
            .collect();
        if matches.is_empty() {
            return;
        }
        self.search_match_idx = (self.search_match_idx + 1) % matches.len();
        self.row_cursor = matches[self.search_match_idx];
        self.row_scroll = self.row_cursor.saturating_sub(self.left_visible / 2);
    }

    pub fn search_prev_match(&mut self) {
        let matches: Vec<usize> = self
            .filtered_rows
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r, RepoRow::Repo(_)))
            .map(|(i, _)| i)
            .collect();
        if matches.is_empty() {
            return;
        }
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
        let target_repo_idx = self.filtered_rows.get(self.row_cursor).and_then(|r| {
            if let RepoRow::Repo(i) = r {
                Some(*i)
            } else {
                None
            }
        });
        // Clear filter – filtered_rows becomes full rows again
        self.search_query.clear();
        self.filtered_rows.clone_from(&self.rows);
        // Restore cursor to the same repo in the full list
        if let Some(target) = target_repo_idx {
            if let Some(pos) = self
                .filtered_rows
                .iter()
                .position(|r| matches!(r, RepoRow::Repo(i) if *i == target))
            {
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
        self.row_cursor = self
            .search_saved_cursor
            .min(self.filtered_rows.len().saturating_sub(1));
        self.row_scroll = self.row_cursor.saturating_sub(self.left_visible / 2);
        self.status_msg = String::from(READY_MSG);
    }
}
