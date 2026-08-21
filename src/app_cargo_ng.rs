//! cgo が NG（installed binary が古い）な repo をまとめて見せる overlay の状態。
//!
//! NG を左ペインの 4 桁の列だけで伝えると見逃せてしまうため、fetch が終わった時点で
//! 一覧を 1 度だけ自動で開く。閉じたあとは次の fetch まで自動では開かない。

use super::App;
use crate::github::RepoInfo;
use crate::ui::RepoRow;

/// overlay の 1 行分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CargoNgItem {
    pub name: String,
    pub full_name: String,
    /// 実行バイナリが自己申告した commit hash。
    pub installed_hash: String,
    pub remote_hash: String,
}

/// cgo が NG の repo を、左ペインと同じ並び順で集める。
pub(crate) fn collect_cargo_ng_items(repos: &[RepoInfo]) -> Vec<CargoNgItem> {
    repos
        .iter()
        .filter(|repo| repo.cargo_bin_check == Some(false))
        .map(|repo| CargoNgItem {
            name: repo.name.clone(),
            full_name: repo.full_name.clone(),
            installed_hash: repo.cargo_installed_hash.clone(),
            remote_hash: repo.cargo_remote_hash.clone(),
        })
        .collect()
}

impl App {
    pub fn open_cargo_ng(&mut self, items: Vec<CargoNgItem>) {
        self.show_cargo_ng = true;
        self.cargo_ng_items = items;
        self.cargo_ng_selected = 0;
        self.cargo_ng_scroll = 0;
    }

    pub fn close_cargo_ng(&mut self) {
        self.show_cargo_ng = false;
    }

    /// 現在の repo 一覧から NG を集め直して overlay を開く。
    pub fn open_cargo_ng_from_repos(&mut self) {
        let items = collect_cargo_ng_items(&self.repos);
        self.open_cargo_ng(items);
    }

    /// fetch が一巡した時点で、NG があれば 1 度だけ自動で開く。
    ///
    /// すでに自動表示済み、またはユーザーが別の overlay を開いている間は何もしない。
    /// 戻り値は実際に開いたかどうか。
    pub fn auto_open_cargo_ng_once(&mut self) -> bool {
        if self.cargo_ng_auto_shown || self.show_workflow_repo_exist || self.show_help {
            return false;
        }
        let items = collect_cargo_ng_items(&self.repos);
        if items.is_empty() {
            return false;
        }
        self.cargo_ng_auto_shown = true;
        self.open_cargo_ng(items);
        true
    }

    /// 次の fetch でまた自動表示できるようにする。
    pub fn reset_cargo_ng_auto_shown(&mut self) {
        self.cargo_ng_auto_shown = false;
    }

    pub fn selected_cargo_ng(&self) -> Option<&CargoNgItem> {
        self.cargo_ng_items.get(self.cargo_ng_selected)
    }

    pub fn cargo_ng_move_down(&mut self, n: usize) {
        let max = self.cargo_ng_items.len().saturating_sub(1);
        self.cargo_ng_selected = (self.cargo_ng_selected + n).min(max);
    }

    pub fn cargo_ng_move_up(&mut self, n: usize) {
        self.cargo_ng_selected = self.cargo_ng_selected.saturating_sub(n);
    }

    pub fn adjust_cargo_ng_scroll(&mut self, visible: usize) {
        if visible == 0 {
            return;
        }
        if self.cargo_ng_selected < self.cargo_ng_scroll {
            self.cargo_ng_scroll = self.cargo_ng_selected;
        } else if self.cargo_ng_selected >= self.cargo_ng_scroll + visible {
            self.cargo_ng_scroll = self.cargo_ng_selected + 1 - visible;
        }
    }

    /// overlay で選んでいる repo へカーソルを移し、overlay を閉じる。
    ///
    /// 検索で絞り込み中などで対象が一覧に出ていない場合はカーソルを動かさない。
    pub fn jump_to_selected_cargo_ng(&mut self) {
        let Some(name) = self.selected_cargo_ng().map(|item| item.name.clone()) else {
            return;
        };
        if let Some(idx) = self.filtered_rows.iter().position(|row| match row {
            RepoRow::Repo(repo_idx) => self
                .repos
                .get(*repo_idx)
                .is_some_and(|repo| repo.name == name),
            RepoRow::Separator(_) => false,
        }) {
            self.row_cursor = idx;
            self.detail_selected = 0;
            self.detail_scroll = 0;
        }
        self.close_cargo_ng();
    }
}
