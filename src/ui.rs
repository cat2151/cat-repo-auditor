use crate::{github::{LocalStatus, RepoInfo}, App};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

// ── Monokai palette ──────────────────────────────────────────────────────────
const MK_BG:      Color = Color::Rgb(39,  40,  34);
const MK_BG_SEL:  Color = Color::Rgb(73,  72,  62);
const MK_BG_DIM:  Color = Color::Rgb(55,  56,  48);
const MK_FG:      Color = Color::Rgb(248, 248, 242);
const MK_COMMENT: Color = Color::Rgb(153, 153, 119);
const MK_YELLOW:  Color = Color::Rgb(230, 219, 116);
const MK_GREEN:   Color = Color::Rgb(166, 226, 46);
const MK_ORANGE:  Color = Color::Rgb(253, 151, 31);
const MK_RED:     Color = Color::Rgb(249, 38,  114);
const MK_PURPLE:  Color = Color::Rgb(174, 129, 255);
const MK_CYAN:    Color = Color::Rgb(102, 217, 239);
const MK_BLUE:    Color = Color::Rgb(102, 153, 204);

// ── RepoRow ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum RepoRow {
    Separator(String),
    Repo(usize),
}

fn group_key(r: &RepoInfo) -> u8 {
    if r.is_private                              { return 3; }
    if r.local_status == LocalStatus::NotFound   { return 2; }
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
    let mut attached: HashMap<u64, Vec<&crate::github::IssueOrPr>> = HashMap::new();
    let mut standalone: Vec<&crate::github::IssueOrPr> = vec![];
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

// ── draw_ui ──────────────────────────────────────────────────────────────────

pub fn draw_ui(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // rate limit
            Constraint::Min(0),    // main
            Constraint::Length(1), // search bar or status
        ])
        .split(area);

    // ── rate limit bar ───────────────────────────────────────────────────────
    // Build background task indicator: "gh↓1 scan3/76 chk5/76"
    let tasks_str: String = app.bg_tasks.iter()
        .map(|(tag, cur, total)| {
            if *total == 0 {
                format!("{}{}  ", tag, cur)  // gh↓ page num (total unknown)
            } else {
                format!("{}{}/{}  ", tag, cur, total)
            }
        })
        .collect::<Vec<_>>()
        .join("");
    let tasks_display = if tasks_str.is_empty() {
        String::new()
    } else {
        format!("  {}", tasks_str.trim_end())
    };

    let rl_text = if let Some(rl) = &app.rate_limit {
        format!(
            " API: {}/{} │ resets {}{}",
            rl.remaining, rl.limit,
            format_reset(&rl.reset_at),
            tasks_display,
        )
    } else {
        format!(" API: --{}", tasks_display)
    };
    f.render_widget(
        Paragraph::new(rl_text).style(Style::default().fg(MK_COMMENT).bg(MK_BG)),
        outer[0],
    );

    // ── bottom bar: search input or status ───────────────────────────────────
    if app.search_state == SearchState::Active {
        let query_display = format!("/ {}_", app.search_query);
        let match_count = app.filtered_rows.iter()
            .filter(|r| matches!(r, RepoRow::Repo(_))).count();
        let hint = format!("{:<40} [{} matches]", query_display, match_count);
        f.render_widget(
            Paragraph::new(hint)
                .style(Style::default().fg(MK_YELLOW).bg(MK_BG_DIM)),
            outer[2],
        );
    } else {
        let prefix_str = if app.num_prefix > 0 { format!("[{}]  ", app.num_prefix) } else { String::new() };
        let (display_msg, msg_style) = if let Some(ref t) = app.transient_msg {
            // Transient: highlighted differently so user notices it
            (format!(" {}", t), Style::default().fg(MK_YELLOW).bg(MK_BG_SEL))
        } else {
            (format!(" {}{}", prefix_str, app.status_msg), Style::default().fg(MK_FG).bg(MK_BG_SEL))
        };
        f.render_widget(
            Paragraph::new(display_msg).style(msg_style),
            outer[2],
        );
    }

    // ── panes ────────────────────────────────────────────────────────────────
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(outer[1]);

    draw_left(f, app, panes[0]);
    draw_right(f, app, panes[1]);

    // ── cargo old comparison box ──────────────────────────────────────────────
    if let Some(idx) = app.selected_repo_idx() {
        if app.repos[idx].cargo_install == Some(false) {
            draw_cargo_old_box(f, app, idx, area);
        }
    }

    if app.show_help {
        draw_help_dialog(f, app, area);
    }
}

// ── left pane ────────────────────────────────────────────────────────────────

fn draw_left(f: &mut Frame, app: &mut App, area: Rect) {
    let active = app.focus == Focus::Repos;
    let searching = app.search_state == SearchState::Active;
    let border_col = if active { MK_CYAN } else { MK_COMMENT };

    let title = if searching {
        format!(" {} ({}) – filter: \"{}\" ", app.config.owner, app.repos.len(), app.search_query)
    } else {
        format!(" {} ({}) ", app.config.owner, app.repos.len())
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_col))
        .style(Style::default().bg(MK_BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.filtered_rows.is_empty() {
        let msg = if app.loading { "  Loading…" }
                  else if !app.search_query.is_empty() { "  (no match)" }
                  else { "  No repositories." };
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(MK_COMMENT).bg(MK_BG)),
            inner,
        );
        return;
    }

    let header = if app.show_columns {
        Row::new(vec![
            Cell::from("Repository").style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("Updated"   ).style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("PR" ).style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("ISS").style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("doc").style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("pg" ).style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("ja" ).style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("wki").style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("wf" ).style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("Local").style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("cgo").style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
        ]).style(Style::default().bg(MK_BG))
    } else {
        Row::new(vec![
            Cell::from("Repository").style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("Updated"   ).style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("PR" ).style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
            Cell::from("ISS").style(Style::default().add_modifier(Modifier::BOLD).fg(MK_YELLOW)),
        ]).style(Style::default().bg(MK_BG))
    };

    let visible = inner.height.saturating_sub(1) as usize;
    app.left_visible = visible;
    app.adjust_row_scroll(visible);
    let scroll  = app.row_scroll;
    let cursor  = app.row_cursor;

    let rows: Vec<Row> = app.filtered_rows.iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(row_i, row)| match row {
            RepoRow::Separator(label) => Row::new(vec![
                Cell::from(label.as_str())
                    .style(Style::default().fg(MK_COMMENT).bg(MK_BG).add_modifier(Modifier::DIM)),
                Cell::from(""), Cell::from(""), Cell::from(""),
                Cell::from(""), Cell::from(""), Cell::from(""),
                Cell::from(""), Cell::from(""), Cell::from(""),
                Cell::from(""),
            ]).style(Style::default().bg(MK_BG)),

            RepoRow::Repo(repo_idx) => {
                let repo = &app.repos[*repo_idx];
                let is_cursor = row_i == cursor;
                let sel = is_cursor && active;
                let dim = is_cursor && !active;

                let base_style = if sel {
                    Style::default().bg(MK_BG_SEL).fg(MK_FG).add_modifier(Modifier::BOLD)
                } else if dim {
                    Style::default().bg(MK_BG_DIM).fg(MK_FG)
                } else {
                    Style::default().fg(MK_FG).bg(MK_BG)
                };

                let local_col = match repo.local_status {
                    LocalStatus::Clean    => MK_GREEN,
                    LocalStatus::Pullable => MK_ORANGE,
                    LocalStatus::Staging  => MK_BLUE,
                    LocalStatus::Other    => MK_RED,
                    LocalStatus::NotFound => MK_COMMENT,
                    LocalStatus::NoGit    => MK_COMMENT,
                };
                let pr_col  = if repo.open_prs   > 0 { MK_PURPLE } else { MK_COMMENT };
                let iss_col = if repo.open_issues > 0 { MK_RED    } else { MK_COMMENT };

                let cursor_char = if is_cursor { "▶ " } else { "  " };
                let lock_char   = if repo.is_private { "🔒" } else { "" };
                let name_str    = format!("{}{}{}", cursor_char, lock_char, repo.name);

                // ── existence indicators ─────────────────────────────
                let is_checking = app.checking_repo == repo.name;
                let pending = ("…", MK_ORANGE);

                let (doc_str, doc_col) = if is_checking && repo.readme_ja_checked_at.is_empty() {
                    pending
                } else { match repo.readme_ja {
                    Some(true)  => ("✔", MK_GREEN),
                    Some(false) => ("✘", MK_COMMENT),
                    None        => ("?", MK_ORANGE),
                }};
                let (pg_str, pg_col) = if is_checking && repo.pages_checked_at.is_empty() {
                    pending
                } else { match repo.pages {
                    Some(true)  => ("✔", MK_CYAN),
                    Some(false) => ("✘", MK_COMMENT),
                    None        => ("?", MK_ORANGE),
                }};
                let (ja_str, ja_col) = if is_checking && repo.readme_ja_badge_checked_at.is_empty() {
                    pending
                } else { match repo.readme_ja_badge {
                    Some(true)  => ("✔", MK_YELLOW),
                    Some(false) => ("✘", MK_COMMENT),
                    None        => ("?", MK_ORANGE),
                }};

                let (wki_str, wki_col) = if is_checking && repo.deepwiki_checked_at.is_empty() {
                    pending
                } else { match repo.deepwiki {
                    Some(true)  => ("✔", MK_PURPLE),
                    Some(false) => ("✘", MK_COMMENT),
                    None        => ("?", MK_ORANGE),
                }};
                let (wf_str, wf_col) = if is_checking && repo.wf_checked_at.is_empty() {
                    pending
                } else { match repo.wf_workflows {
                    Some(true)  => ("✔", MK_GREEN),
                    Some(false) => ("✘", MK_COMMENT),
                    None        => ("?", MK_ORANGE),
                }};

                // cargo: None=not in crates2 → empty, Some(true)=ok, Some(false)=old
                let (cgo_str, cgo_col) = if is_checking && repo.cargo_checked_at.is_empty() {
                    pending
                } else { match repo.cargo_install {
                    Some(true)  => ("ok",  MK_GREEN),
                    Some(false) => ("old", MK_ORANGE),
                    None        => ("",    MK_COMMENT),
                }};

                if !app.show_columns {
                    Row::new(vec![
                        Cell::from(name_str).style(base_style),
                        Cell::from(repo.updated_at.clone()).style(
                            if sel || dim { base_style } else { Style::default().fg(MK_COMMENT).bg(MK_BG) }
                        ),
                        Cell::from(format!("{:>3}", repo.open_prs)).style(
                            if sel || dim { base_style } else { Style::default().fg(pr_col).bg(MK_BG) }
                        ),
                        Cell::from(format!("{:>3}", repo.open_issues)).style(
                            if sel || dim { base_style } else { Style::default().fg(iss_col).bg(MK_BG) }
                        ),
                    ]).style(Style::default().bg(
                        if sel { MK_BG_SEL } else if dim { MK_BG_DIM } else { MK_BG }
                    ))
                } else {
                Row::new(vec![
                    Cell::from(name_str).style(base_style),
                    Cell::from(repo.updated_at.clone()).style(
                        if sel || dim { base_style } else { Style::default().fg(MK_COMMENT).bg(MK_BG) }
                    ),
                    Cell::from(format!("{:>3}", repo.open_prs)).style(
                        if sel || dim { base_style } else { Style::default().fg(pr_col).bg(MK_BG) }
                    ),
                    Cell::from(format!("{:>3}", repo.open_issues)).style(
                        if sel || dim { base_style } else { Style::default().fg(iss_col).bg(MK_BG) }
                    ),
                    Cell::from(doc_str).style(
                        if sel || dim { base_style } else { Style::default().fg(doc_col).bg(MK_BG) }
                    ),
                    Cell::from(pg_str).style(
                        if sel || dim { base_style } else { Style::default().fg(pg_col).bg(MK_BG) }
                    ),
                    Cell::from(ja_str).style(
                        if sel || dim { base_style } else { Style::default().fg(ja_col).bg(MK_BG) }
                    ),
                    Cell::from(wki_str).style(
                        if sel || dim { base_style } else { Style::default().fg(wki_col).bg(MK_BG) }
                    ),
                    Cell::from(wf_str).style(
                        if sel || dim { base_style } else { Style::default().fg(wf_col).bg(MK_BG) }
                    ),
                    Cell::from(repo.local_status.to_string()).style(
                        if sel || dim { base_style } else { Style::default().fg(local_col).bg(MK_BG) }
                    ),
                    Cell::from(cgo_str).style(
                        if sel || dim { base_style } else { Style::default().fg(cgo_col).bg(MK_BG) }
                    ),
                ]).style(Style::default().bg(
                    if sel { MK_BG_SEL } else if dim { MK_BG_DIM } else { MK_BG }
                ))
                }
            }
        })
        .collect();

    let widths_full: &[Constraint] = &[
        Constraint::Min(18),
        Constraint::Length(7),  // relative date
        Constraint::Length(4),  // PR
        Constraint::Length(4),  // ISS
        Constraint::Length(3),  // doc
        Constraint::Length(3),  // pg
        Constraint::Length(3),  // ja
        Constraint::Length(3),  // wki
        Constraint::Length(3),  // wf
        Constraint::Length(8),  // Local
        Constraint::Length(4),  // cgo
    ];
    let widths_slim: &[Constraint] = &[
        Constraint::Min(0),     // name (expanded)
        Constraint::Length(7),  // date
        Constraint::Length(4),  // PR
        Constraint::Length(4),  // ISS
    ];
    let widths = if app.show_columns { widths_full } else { widths_slim };

    let mut ts = TableState::default();
    let table = Table::new(rows, widths.to_vec())
        .header(header)
        .row_highlight_style(Style::default())
        .style(Style::default().bg(MK_BG));
    f.render_stateful_widget(table, inner, &mut ts);

    // position indicator
    if app.filtered_rows.len() > visible && visible > 0 {
        let repo_pos = app.filtered_rows[..=cursor].iter()
            .filter(|r| matches!(r, RepoRow::Repo(_))).count();
        let repo_total = app.filtered_rows.iter()
            .filter(|r| matches!(r, RepoRow::Repo(_))).count();
        let txt = format!(" {repo_pos}/{repo_total} ");
        let w = (txt.len() as u16).min(inner.width);
        f.render_widget(
            Paragraph::new(txt).style(Style::default().fg(MK_COMMENT).bg(MK_BG)),
            Rect { x: inner.x + inner.width.saturating_sub(w), y: inner.y + inner.height.saturating_sub(1), width: w, height: 1 },
        );
    }
}

// ── right pane ───────────────────────────────────────────────────────────────

fn draw_right(f: &mut Frame, app: &mut App, area: Rect) {
    let active = app.focus == Focus::Detail;
    let border_col = if active { MK_GREEN } else { MK_COMMENT };

    if app.repos.is_empty() {
        f.render_widget(
            Block::default().title(" Detail ").borders(Borders::ALL)
                .border_style(Style::default().fg(MK_COMMENT))
                .style(Style::default().bg(MK_BG)),
            area,
        );
        return;
    }

    let repo_idx = match app.selected_repo_idx() {
        Some(i) => i,
        None => {
            f.render_widget(Block::default().borders(Borders::ALL).style(Style::default().bg(MK_BG)), area);
            return;
        }
    };
    // Clone data needed after mutable borrows to avoid borrow conflict
    let staging_files: Vec<String> = app.repos[repo_idx].staging_files.clone();
    let repo = &app.repos[repo_idx];

    let title = format!(" {} │ PR:{} ISS:{} │ {} ", repo.name, repo.open_prs, repo.open_issues, repo.local_status);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_col))
        .style(Style::default().bg(MK_BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // ── layout within right pane ─────────────────────────────────────────────
    // summary(1) + hint(1) + issue/PR section + staging section
    let has_staging = !staging_files.is_empty();
    let staging_height = if has_staging {
        (repo.staging_files.len() + 2).min(inner.height as usize / 3) as u16
    } else { 0 };

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),                               // summary
            Constraint::Length(1),                               // hint
            Constraint::Min(4),                                  // issue/PR list
            Constraint::Length(staging_height),                  // staging (0 if empty)
        ])
        .split(inner);

    // summary
    let priv_tag = if repo.is_private { "  🔒" } else { "" };
    f.render_widget(
        Paragraph::new(format!(" Updated: {}{}", repo.updated_at, priv_tag))
            .style(Style::default().fg(MK_COMMENT).bg(MK_BG)),
        vert[0],
    );
    // hint
    let hint = if active { " h:back  j/k:move  PgUp/PgDn  Enter:open" } else { " l:focus  g:lazygit" };
    f.render_widget(
        Paragraph::new(hint).style(Style::default().fg(MK_COMMENT).bg(MK_BG).add_modifier(Modifier::DIM)),
        vert[1],
    );

    // ── issue/PR list ─────────────────────────────────────────────────────────
    let list_area = vert[2];
    let items = build_detail_items(repo);

    if items.is_empty() {
        f.render_widget(
            Paragraph::new("  (no open issues or PRs)").style(Style::default().fg(MK_COMMENT).bg(MK_BG)),
            list_area,
        );
    } else {
        let visible = list_area.height as usize;
        app.right_visible = visible;
        app.adjust_detail_scroll(visible);
        let d_scroll = app.detail_scroll;
        let d_sel    = app.detail_selected;
        // re-borrow repo after mutable borrow is done
        let _repo = &app.repos[repo_idx];

        let lines: Vec<Line> = items.iter()
            .enumerate()
            .skip(d_scroll)
            .take(visible)
            .map(|(i, item)| {
                let sel = active && i == d_sel;
                let bg  = if sel { MK_BG_SEL } else { MK_BG };
                let indent    = if item.is_child { " " } else { "" };
                let connector = if item.is_child { "└─" } else { "  " };
                let (label, label_col) = if item.is_pr { (" PR", MK_PURPLE) } else { ("ISS", MK_RED) };
                let prefix_len = if item.is_child { 4usize } else { 3usize };
                let max_title = list_area.width.saturating_sub(28 + prefix_len as u16) as usize;
                Line::from(vec![
                    Span::styled(format!("{}{} ", indent, connector), Style::default().fg(MK_COMMENT).bg(bg)),
                    Span::styled(format!("{} ", label), Style::default().fg(label_col).bg(bg).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("#{:<4} ", item.number), Style::default().fg(MK_COMMENT).bg(bg)),
                    Span::styled(truncate(&item.title, max_title), Style::default().fg(MK_FG).bg(bg)),
                    Span::styled(format!("  {}", item.updated), Style::default().fg(if sel { MK_CYAN } else { MK_COMMENT }).bg(bg)),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(lines).style(Style::default().bg(MK_BG)), list_area);

        // position indicator
        if items.len() > visible && visible > 0 {
            let txt = format!(" {}/{} ", d_sel + 1, items.len());
            let w = (txt.len() as u16).min(list_area.width);
            f.render_widget(
                Paragraph::new(txt).style(Style::default().fg(MK_COMMENT).bg(MK_BG)),
                Rect { x: list_area.x + list_area.width.saturating_sub(w), y: list_area.y + list_area.height.saturating_sub(1), width: w, height: 1 },
            );
        }
    }

    // ── staging section ───────────────────────────────────────────────────────
    if has_staging && staging_height > 0 {
        let staging_area = vert[3];
        let stag_block = Block::default()
            .title(" Staging ")
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(MK_BLUE))
            .style(Style::default().bg(MK_BG));
        let stag_inner = stag_block.inner(staging_area);
        f.render_widget(stag_block, staging_area);

        let stag_visible = stag_inner.height as usize;
        let stag_lines: Vec<Line> = staging_files.iter()
            .take(stag_visible)
            .map(|line| {
                // porcelain format: "XY filename"
                let (status, rest) = if line.len() > 3 { (&line[..2], &line[3..]) } else { ("??", line.as_str()) };
                let status_col = match status.trim() {
                    "M" | "MM" => MK_ORANGE,
                    "A" | "AM" => MK_GREEN,
                    "D" | "DD" => MK_RED,
                    "R" | "RM" => MK_PURPLE,
                    "??" => MK_COMMENT,
                    _ => MK_FG,
                };
                Line::from(vec![
                    Span::styled(format!(" {} ", status), Style::default().fg(status_col).bg(MK_BG).add_modifier(Modifier::BOLD)),
                    Span::styled(rest.to_string(), Style::default().fg(MK_FG).bg(MK_BG)),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(stag_lines).style(Style::default().bg(MK_BG)), stag_inner);
    }
}

// ── cargo old comparison box ──────────────────────────────────────────────────

fn draw_cargo_old_box(f: &mut Frame, app: &App, repo_idx: usize, area: Rect) {
    let repo = &app.repos[repo_idx];
    let inst  = if repo.cargo_installed_hash.is_empty() { "?" } else { &repo.cargo_installed_hash };
    let local = if repo.cargo_local_hash.is_empty()     { "?" } else { &repo.cargo_local_hash };

    // Box width: border(2) + " installed: " (12) + hash(40) + padding(1) = 55
    let content_w: u16 = 55;
    let box_w = content_w + 2; // borders
    let box_h: u16 = 4; // top border + 2 lines + bottom border

    // Place in bottom-right, above the bottom status bar (outer[2] is 1 line tall)
    let x = area.x + area.width.saturating_sub(box_w + 1);
    let y = area.y + area.height.saturating_sub(box_h + 1);
    let rect = Rect { x, y, width: box_w.min(area.width), height: box_h.min(area.height) };

    f.render_widget(Clear, rect);
    let block = Block::default()
        .title(" cgo old: commit hash ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MK_ORANGE))
        .style(Style::default().bg(MK_BG));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let label_w: u16 = 12; // length of " installed: " / "     local: " labels
    let max_w = inner.width.saturating_sub(label_w) as usize;
    let lines = vec![
        Line::from(vec![
            Span::styled(" installed: ", Style::default().fg(MK_COMMENT)),
            Span::styled(truncate(inst,  max_w), Style::default().fg(MK_ORANGE)),
        ]),
        Line::from(vec![
            Span::styled("     local: ", Style::default().fg(MK_COMMENT)),
            Span::styled(truncate(local, max_w), Style::default().fg(MK_GREEN)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines).style(Style::default().bg(MK_BG)), inner);
}

// ── help dialog ──────────────────────────────────────────────────────────────

fn draw_help_dialog(f: &mut Frame, _app: &App, area: Rect) {
    // Center a dialog box
    let dw: u16 = 62;
    let dh: u16 = 30;
    let x = area.x + area.width.saturating_sub(dw) / 2;
    let y = area.y + area.height.saturating_sub(dh) / 2;
    let dialog = Rect { x, y, width: dw.min(area.width), height: dh.min(area.height) };

    // Clear the area first to prevent bleed-through from background
    f.render_widget(Clear, dialog);
    f.render_widget(
        Paragraph::new("").style(Style::default().bg(MK_BG_DIM)),
        dialog,
    );

    let block = Block::default()
        .title(" Help  (? or Esc to close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MK_CYAN))
        .style(Style::default().bg(MK_BG_DIM));

    let inner = block.inner(dialog);
    f.render_widget(block, dialog);

    let lines: Vec<Line> = vec![
        Line::from(Span::styled(" Keybinds", Style::default().fg(MK_YELLOW).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  q        ", Style::default().fg(MK_ORANGE)),
            Span::raw("Quit"),
        ]),
        Line::from(vec![
            Span::styled("  F5       ", Style::default().fg(MK_ORANGE)),
            Span::raw("Refresh from GitHub"),
        ]),
        Line::from(vec![
            Span::styled("  j / k    ", Style::default().fg(MK_ORANGE)),
            Span::raw("Move cursor down / up  (Nj = N lines)"),
        ]),
        Line::from(vec![
            Span::styled("  PgDn/PgUp", Style::default().fg(MK_ORANGE)),
            Span::raw("Page down / up"),
        ]),
        Line::from(vec![
            Span::styled("  h / l    ", Style::default().fg(MK_ORANGE)),
            Span::raw("Focus left pane / right pane"),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", Style::default().fg(MK_ORANGE)),
            Span::raw("Open README.ja.md (or repo root)"),
        ]),
        Line::from(vec![
            Span::styled("  i        ", Style::default().fg(MK_ORANGE)),
            Span::raw("Open GitHub Pages (or repo root)"),
        ]),
        Line::from(vec![
            Span::styled("  w        ", Style::default().fg(MK_ORANGE)),
            Span::raw("Open DeepWiki page"),
        ]),
        Line::from(vec![
            Span::styled("  g        ", Style::default().fg(MK_ORANGE)),
            Span::raw("Open lazygit for this repo"),
        ]),
        Line::from(vec![
            Span::styled("  x        ", Style::default().fg(MK_ORANGE)),
            Span::raw("Run installed app (cgo=ok only)"),
        ]),
        Line::from(vec![
            Span::styled("  c        ", Style::default().fg(MK_ORANGE)),
            Span::raw("Copy local repo path to clipboard"),
        ]),
        Line::from(vec![
            Span::styled("  d        ", Style::default().fg(MK_ORANGE)),
            Span::raw("Toggle doc/pg/ja/wki/wf/cgo columns"),
        ]),
        Line::from(vec![
            Span::styled("  /        ", Style::default().fg(MK_ORANGE)),
            Span::raw("Incremental search  (Space=AND)"),
        ]),
        Line::from(vec![
            Span::styled("  ^G / ^T  ", Style::default().fg(MK_ORANGE)),
            Span::raw("Next / prev match  (during search)"),
        ]),
        Line::from(vec![
            Span::styled("  (right pane) Enter", Style::default().fg(MK_ORANGE)),
            Span::raw("  Open issue / PR in browser"),
        ]),
        Line::from(vec![
            Span::styled("  (right pane) h   ", Style::default().fg(MK_ORANGE)),
            Span::raw("  Back to repo list"),
        ]),
        Line::from(""),
        Line::from(Span::styled(" Column legend", Style::default().fg(MK_YELLOW).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  doc ", Style::default().fg(MK_GREEN)),
            Span::raw("README.ja.md exists in repo"),
        ]),
        Line::from(vec![
            Span::styled("  pg  ", Style::default().fg(MK_CYAN)),
            Span::raw("GitHub Pages is enabled"),
        ]),
        Line::from(vec![
            Span::styled("  ja  ", Style::default().fg(MK_YELLOW)),
            Span::raw("README.ja.md has a self-link badge"),
        ]),
        Line::from(vec![
            Span::styled("  wki ", Style::default().fg(MK_PURPLE)),
            Span::raw("README contains a deepwiki.com link"),
        ]),
        Line::from(vec![
            Span::styled("  wf  ", Style::default().fg(MK_GREEN)),
            Span::raw(".github/workflows has 3 required ymls"),
        ]),
        Line::from(vec![
            Span::styled("  cgo ", Style::default().fg(MK_GREEN)),
            Span::raw("ok=cargo install HEAD matches local"),
        ]),
        Line::from(vec![
            Span::styled("      ", Style::default().fg(MK_ORANGE)),
            Span::raw("old=installed hash differs from HEAD"),
        ]),
    ];

    f.render_widget(Paragraph::new(lines).style(Style::default().bg(MK_BG_DIM)), inner);
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if max == 0 { return String::new(); }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max { s.to_string() }
    else { let mut t: String = chars[..max.saturating_sub(1)].iter().collect(); t.push('…'); t }
}

fn format_reset(reset_at: &str) -> String {
    use chrono::{DateTime, Utc};
    if let Ok(dt) = reset_at.parse::<DateTime<Utc>>() {
        let now = Utc::now();
        if dt > now {
            let diff = dt - now;
            format!("in {}m{}s", diff.num_minutes(), diff.num_seconds() % 60)
        } else { String::from("now") }
    } else { reset_at.to_string() }
}
