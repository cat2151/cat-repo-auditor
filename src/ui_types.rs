use crate::github::{IssueOrPr, LocalStatus, RepoInfo};
use ratatui::style::Color;

// ── Monokai palette ──────────────────────────────────────────────────────────
pub const MK_BG:      Color = Color::Rgb(39,  40,  34);
pub const MK_BG_SEL:  Color = Color::Rgb(73,  72,  62);
pub const MK_BG_DIM:  Color = Color::Rgb(55,  56,  48);
pub const MK_FG:      Color = Color::Rgb(248, 248, 242);
pub const MK_COMMENT: Color = Color::Rgb(153, 153, 119);
pub const MK_YELLOW:  Color = Color::Rgb(230, 219, 116);
pub const MK_GREEN:   Color = Color::Rgb(166, 226, 46);
pub const MK_ORANGE:  Color = Color::Rgb(253, 151, 31);
pub const MK_RED:     Color = Color::Rgb(249, 38,  114);
pub const MK_PURPLE:  Color = Color::Rgb(174, 129, 255);
pub const MK_CYAN:    Color = Color::Rgb(102, 217, 239);
pub const MK_BLUE:    Color = Color::Rgb(102, 153, 204);

// ── RepoRow ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum RepoRow {
    Separator(String),
    Repo(usize),
}

fn group_key(r: &RepoInfo) -> u8 {
    if r.is_private                              { return 3; }
    if r.local_status == LocalStatus::NotFound { return 2; }
    if r.open_issues == 0 && r.open_prs == 0     { return 1; }
    0
}

fn group_label(g: u8) -> &'static str {
    match g {
        1 => "── no open issues / PRs ──────────────────",
        2 => "── no local clone ────────────────────────",
        3 => "── private ───────────────────────────────",
        _ => "",
    }
}

pub fn build_rows(repos: &[RepoInfo]) -> Vec<RepoRow> {
    let mut rows: Vec<RepoRow> = vec![];
    let mut cur_group: Option<u8> = None;
    for (i, repo) in repos.iter().enumerate() {
        let g = group_key(repo);
        if cur_group != Some(g) {
            if g != 0 { rows.push(RepoRow::Separator(group_label(g).to_string())); }
            cur_group = Some(g);
        }
        rows.push(RepoRow::Repo(i));
    }
    rows
}

// ── Focus ─────────────────────────────────────────────────────────────────────
// ── SearchState ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SearchState { Off, Active }


#[derive(Debug, Clone, PartialEq)]
pub enum Focus { Repos, Detail }

// ── DetailItem ───────────────────────────────────────────────────────────────

pub struct DetailItem {
    pub is_pr:    bool,
    pub is_child: bool,
    pub number:   u64,
    pub url:      String,
    pub title:    String,
    pub updated:  String,
}

pub fn build_detail_items(repo: &RepoInfo) -> Vec<DetailItem> {
    use std::collections::{HashMap, HashSet};
    let open_issue_numbers: HashSet<u64> = repo.issues.iter().map(|i| i.number).collect();
    let mut attached: HashMap<u64, Vec<&IssueOrPr>> = HashMap::new();
    let mut standalone: Vec<&IssueOrPr> = vec![];
    for pr in &repo.prs {
        if let Some(n) = pr.closes_issue {
            if open_issue_numbers.contains(&n) { attached.entry(n).or_default().push(pr); }
            else { standalone.push(pr); }
        } else { standalone.push(pr); }
    }
    let mut items: Vec<DetailItem> = vec![];
    for issue in &repo.issues {
        items.push(DetailItem {
            is_pr: false, is_child: false,
            number: issue.number, url: issue.url(),
            title: issue.title.clone(), updated: issue.updated_at.clone(),
        });
        if let Some(prs) = attached.get(&issue.number) {
            let mut ps = prs.to_vec();
            ps.sort_by_key(|p| p.number);
            for pr in ps {
                items.push(DetailItem {
                    is_pr: true, is_child: true,
                    number: pr.number, url: pr.url(),
                    title: pr.title.clone(), updated: pr.updated_at.clone(),
                });
            }
        }
    }
    for pr in standalone {
        items.push(DetailItem {
            is_pr: true, is_child: false,
            number: pr.number, url: pr.url(),
            title: pr.title.clone(), updated: pr.updated_at.clone(),
        });
    }
    items
}
