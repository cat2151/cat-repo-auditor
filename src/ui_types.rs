use crate::github::{IssueOrPr, LocalStatus, RepoInfo};
use ratatui::style::Color;

// ── Monokai palette ──────────────────────────────────────────────────────────
pub(crate) const MK_BG:      Color = Color::Rgb(39,  40,  34);
pub(crate) const MK_BG_SEL:  Color = Color::Rgb(73,  72,  62);
pub(crate) const MK_BG_DIM:  Color = Color::Rgb(55,  56,  48);
pub(crate) const MK_FG:      Color = Color::Rgb(248, 248, 242);
pub(crate) const MK_COMMENT: Color = Color::Rgb(153, 153, 119);
pub(crate) const MK_YELLOW:  Color = Color::Rgb(230, 219, 116);
pub(crate) const MK_GREEN:   Color = Color::Rgb(166, 226, 46);
pub(crate) const MK_ORANGE:  Color = Color::Rgb(253, 151, 31);
pub(crate) const MK_RED:     Color = Color::Rgb(249, 38,  114);
pub(crate) const MK_PURPLE:  Color = Color::Rgb(174, 129, 255);
pub(crate) const MK_CYAN:    Color = Color::Rgb(102, 217, 239);
pub(crate) const MK_BLUE:    Color = Color::Rgb(102, 153, 204);

const UNFOCUSED_DIM_NUM: u16 = 3;
const UNFOCUSED_DIM_DEN: u16 = 5;

fn dim_channel(gray: u8) -> u8 {
    ((u16::from(gray) * UNFOCUSED_DIM_NUM) / UNFOCUSED_DIM_DEN) as u8
}

fn grayscale_dim_rgb(r: u8, g: u8, b: u8) -> Color {
    let gray = ((u32::from(r) * 299) + (u32::from(g) * 587) + (u32::from(b) * 114)) / 1000;
    let gray = dim_channel(gray as u8);
    Color::Rgb(gray, gray, gray)
}

pub(crate) fn window_color(window_focused: bool, color: Color) -> Color {
    if window_focused { return color; }

    match color {
        Color::Reset => Color::Reset,
        Color::Black => grayscale_dim_rgb(0, 0, 0),
        Color::Red => grayscale_dim_rgb(205, 49, 49),
        Color::Green => grayscale_dim_rgb(13, 188, 121),
        Color::Yellow => grayscale_dim_rgb(229, 229, 16),
        Color::Blue => grayscale_dim_rgb(36, 114, 200),
        Color::Magenta => grayscale_dim_rgb(188, 63, 188),
        Color::Cyan => grayscale_dim_rgb(17, 168, 205),
        Color::Gray => grayscale_dim_rgb(229, 229, 229),
        Color::DarkGray => grayscale_dim_rgb(102, 102, 102),
        Color::LightRed => grayscale_dim_rgb(241, 76, 76),
        Color::LightGreen => grayscale_dim_rgb(35, 209, 139),
        Color::LightYellow => grayscale_dim_rgb(245, 245, 67),
        Color::LightBlue => grayscale_dim_rgb(59, 142, 234),
        Color::LightMagenta => grayscale_dim_rgb(214, 112, 214),
        Color::LightCyan => grayscale_dim_rgb(41, 184, 219),
        Color::White => grayscale_dim_rgb(255, 255, 255),
        Color::Rgb(r, g, b) => grayscale_dim_rgb(r, g, b),
        Color::Indexed(index) => match index {
            232..=255 => {
                let gray = dim_channel(8 + (index - 232) * 10);
                Color::Rgb(gray, gray, gray)
            }
            _ => grayscale_dim_rgb(128, 128, 128),
        },
    }
}

// ── RepoRow ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) enum RepoRow {
    Separator(String),
    Repo(usize),
}

fn group_key(r: &RepoInfo) -> u8 {
    if r.is_private                              { return 4; }
    if r.local_status == LocalStatus::NotFound { return 3; }
    if r.open_issues == 0 && r.open_prs == 0     { return 2; }
    if r.open_prs == 0                           { return 1; }
    0
}

fn group_label(g: u8) -> &'static str {
    match g {
        1 => "── no open PRs ───────────────────────────",
        2 => "── no open issues / PRs ──────────────────",
        3 => "── no local clone ────────────────────────",
        4 => "── private ───────────────────────────────",
        _ => "",
    }
}

pub(crate) fn build_rows(repos: &[RepoInfo]) -> Vec<RepoRow> {
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
pub(crate) enum SearchState { Off, Active }


#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Focus { Repos, Detail }

// ── DetailItem ───────────────────────────────────────────────────────────────

pub(crate) struct DetailItem {
    pub(crate) is_pr:    bool,
    pub(crate) is_child: bool,
    pub(crate) number:   u64,
    pub(crate) url:      String,
    pub(crate) title:    String,
    pub(crate) updated:  String,
}

/// Returns the display string and color for a local-file-dependent check column
/// (ja / wki / wf).  When the local clone is absent or has no git, and the
/// value has never been determined (None), the column shows a gray "-" to
/// indicate that no investigation is needed rather than an orange "?".
///
/// * `local_no_git` – true when `local_status` is `NotFound` or `NoGit`
/// * `value`        – the cached check result (`None` = not yet checked)
/// * `true_col`     – accent colour shown when `value` is `Some(true)`
pub(crate) fn local_check_cell(local_no_git: bool, value: Option<bool>, true_col: Color) -> (&'static str, Color) {
    match value {
        Some(true)  => ("✔", true_col),
        Some(false) => ("✘", MK_COMMENT),
        None if local_no_git => ("-", MK_COMMENT),
        None        => ("?", MK_ORANGE),
    }
}

pub(crate) fn build_detail_items(repo: &RepoInfo) -> Vec<DetailItem> {
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
