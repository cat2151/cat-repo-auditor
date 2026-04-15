use super::{
    c, local_check_cell, spinner_frame, Focus, RepoRow, SearchState, MK_BG, MK_BG_DIM, MK_BG_SEL,
    MK_BLUE, MK_COMMENT, MK_CYAN, MK_FG, MK_GREEN, MK_ORANGE, MK_PURPLE, MK_RED, MK_YELLOW,
};
#[path = "ui_draw_left_columns.rs"]
mod columns;
use crate::{
    app::App,
    github::{LocalStatus, RepoInfo},
};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};
use std::collections::HashSet;

/// Returns whether the `cgo` cell should stay in pending state for this repo.
///
/// `has_active_cargo_tasks` covers the initial background cargo check phase, where
/// freshness is determined from the stored local/remote check markers.
/// `has_active_cargo_poll` covers post-update cargo hash polling, which should
/// keep the cell spinning until the poll completes or times out.
fn repo_has_pending_cargo_check(
    has_active_cargo_tasks: bool,
    has_active_cargo_poll: bool,
    repo: &RepoInfo,
) -> bool {
    has_active_cargo_poll
        || (has_active_cargo_tasks
            && (repo.cargo_checked_at != repo.local_head_hash
                || repo.cargo_remote_hash_checked_at != repo.updated_at_raw
                || repo.cargo_remote_hash.is_empty()))
}

/// Returns the rendered `cgo` status from the installed/remote hash comparison.
///
/// When either hash is unavailable, the left pane leaves the cell blank.
/// Otherwise, matching hashes render `ok` and mismatched hashes render `old`.
fn cargo_check_status_cell(repo: &RepoInfo) -> Option<(&'static str, ratatui::style::Color)> {
    if repo.cargo_installed_hash.is_empty() || repo.cargo_remote_hash.is_empty() {
        None
    } else if repo.cargo_installed_hash == repo.cargo_remote_hash {
        Some(("ok", MK_GREEN))
    } else {
        Some(("old", MK_ORANGE))
    }
}

pub(super) fn draw_left(f: &mut Frame, app: &mut App, area: Rect, unix_millis: u64) {
    let active = app.window_focused && app.focus == Focus::Repos;
    let searching = app.search_state == SearchState::Active;
    let border_col = if active { MK_CYAN } else { MK_COMMENT };

    let title = if searching {
        format!(
            " {} ({}) – filter: \"{}\" ",
            app.config.owner,
            app.repos.len(),
            app.search_query
        )
    } else {
        format!(" {} ({}) ", app.config.owner, app.repos.len())
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(c(app, border_col)))
        .style(Style::default().bg(c(app, MK_BG)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.filtered_rows.is_empty() {
        let msg = if app.loading {
            "  Loading…"
        } else if !app.search_query.is_empty() {
            "  (no match)"
        } else {
            "  No repositories."
        };
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))),
            inner,
        );
        return;
    }

    let header = columns::build_header(app);

    let visible = inner.height.saturating_sub(1) as usize;
    app.left_visible = visible;
    app.adjust_row_scroll(visible);
    let scroll = app.row_scroll;
    let cursor = app.row_cursor;
    let cargo_check_active = app
        .bg_tasks
        .iter()
        .any(|(tag, _cur, total)| *tag == "cgo" && *total > 0);
    let active_cargo_poll_repos: HashSet<&str> = app
        .cargo_hash_polls
        .iter()
        .map(|poll| poll.repo_name.as_str())
        .collect();

    let rows: Vec<Row> = app
        .filtered_rows
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(row_i, row)| match row {
            RepoRow::Separator(label) => Row::new(vec![
                Cell::from(label.as_str()).style(
                    Style::default()
                        .fg(c(app, MK_COMMENT))
                        .bg(c(app, MK_BG))
                        .add_modifier(Modifier::DIM),
                ),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(Style::default().bg(c(app, MK_BG))),
            RepoRow::Repo(repo_idx) => {
                let repo = &app.repos[*repo_idx];
                let is_cursor = row_i == cursor;
                let sel = is_cursor && active;
                let dim = is_cursor && !active;

                let base_style = if sel {
                    Style::default()
                        .bg(c(app, MK_BG_SEL))
                        .fg(c(app, MK_FG))
                        .add_modifier(Modifier::BOLD)
                } else if dim {
                    Style::default().bg(c(app, MK_BG_DIM)).fg(c(app, MK_FG))
                } else {
                    Style::default().fg(c(app, MK_FG)).bg(c(app, MK_BG))
                };

                let local_status_col = match repo.local_status {
                    LocalStatus::Conflict => MK_RED,
                    LocalStatus::Modified => MK_ORANGE,
                    LocalStatus::Clean => MK_GREEN,
                    LocalStatus::Pullable => MK_ORANGE,
                    LocalStatus::Staging => MK_BLUE,
                    LocalStatus::Other => MK_RED,
                    LocalStatus::NotFound => MK_COMMENT,
                    LocalStatus::NoGit => MK_COMMENT,
                };

                let pending = (spinner_frame(unix_millis), MK_ORANGE);
                let is_checking = app.checking_repos.contains(&repo.name);
                let has_active_cargo_poll = active_cargo_poll_repos.contains(repo.name.as_str());
                let has_pending_cargo_check =
                    repo_has_pending_cargo_check(cargo_check_active, has_active_cargo_poll, repo);
                let cursor_char = if is_cursor { "▶" } else { " " };
                let lock_char = if repo.is_private { "🔒" } else { "" };
                let name_str = format!("{}{}{}", cursor_char, lock_char, repo.name);
                let pr_pending = app.loading;
                let iss_pending = app.loading;

                let (pr_str, pr_col) = if pr_pending {
                    (pending.0.to_string(), pending.1)
                } else if repo.open_prs > 0 {
                    (format!("{:>3}", repo.open_prs), MK_PURPLE)
                } else {
                    (format!("{:>3}", repo.open_prs), MK_COMMENT)
                };
                let (iss_str, iss_col) = if iss_pending {
                    (pending.0.to_string(), pending.1)
                } else if repo.open_issues > 0 {
                    (format!("{:>3}", repo.open_issues), MK_RED)
                } else {
                    (format!("{:>3}", repo.open_issues), MK_COMMENT)
                };

                let (doc_str, doc_col) =
                    if is_checking && repo.readme_ja_checked_at != repo.updated_at_raw {
                        pending
                    } else {
                        match repo.readme_ja {
                            Some(true) => ("✔", MK_GREEN),
                            Some(false) => ("✘", MK_COMMENT),
                            None => ("?", MK_ORANGE),
                        }
                    };
                let (pg_str, pg_col) =
                    if is_checking && repo.pages_checked_at != repo.updated_at_raw {
                        pending
                    } else {
                        match repo.pages {
                            Some(true) => ("✔", MK_CYAN),
                            Some(false) => ("✘", MK_COMMENT),
                            None => ("?", MK_ORANGE),
                        }
                    };
                let local_no_git = matches!(
                    repo.local_status,
                    LocalStatus::NotFound | LocalStatus::NoGit
                );

                let (ja_str, ja_col) = if is_checking
                    && !local_no_git
                    && repo.readme_ja_badge_checked_at != repo.local_head_hash
                {
                    pending
                } else {
                    local_check_cell(local_no_git, repo.readme_ja_badge, MK_YELLOW)
                };

                let (wki_str, wki_col) = if is_checking
                    && !local_no_git
                    && repo.deepwiki_checked_at != repo.local_head_hash
                {
                    pending
                } else {
                    local_check_cell(local_no_git, repo.deepwiki, MK_PURPLE)
                };
                let (wf_str, wf_col) =
                    if is_checking && !local_no_git && repo.wf_checked_at != repo.local_head_hash {
                        pending
                    } else {
                        local_check_cell(local_no_git, repo.wf_workflows, MK_GREEN)
                    };

                let (local_str, local_col) = if is_checking {
                    (pending.0.to_string(), pending.1)
                } else {
                    (repo.local_status.to_string(), local_status_col)
                };

                let (cgo_str, cgo_col) = if has_pending_cargo_check {
                    pending
                } else if let Some((status, color)) = cargo_check_status_cell(repo) {
                    (status, color)
                } else {
                    ("", MK_COMMENT)
                };

                if !app.show_columns {
                    Row::new(vec![
                        Cell::from(name_str).style(base_style),
                        Cell::from(repo.updated_at.clone()).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))
                        }),
                        Cell::from(pr_str).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, pr_col)).bg(c(app, MK_BG))
                        }),
                        Cell::from(iss_str).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, iss_col)).bg(c(app, MK_BG))
                        }),
                    ])
                    .style(Style::default().bg(c(
                        app,
                        if sel {
                            MK_BG_SEL
                        } else if dim {
                            MK_BG_DIM
                        } else {
                            MK_BG
                        },
                    )))
                } else {
                    Row::new(vec![
                        Cell::from(name_str).style(base_style),
                        Cell::from(repo.updated_at.clone()).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))
                        }),
                        Cell::from(pr_str).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, pr_col)).bg(c(app, MK_BG))
                        }),
                        Cell::from(iss_str).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, iss_col)).bg(c(app, MK_BG))
                        }),
                        Cell::from(doc_str).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, doc_col)).bg(c(app, MK_BG))
                        }),
                        Cell::from(pg_str).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, pg_col)).bg(c(app, MK_BG))
                        }),
                        Cell::from(ja_str).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, ja_col)).bg(c(app, MK_BG))
                        }),
                        Cell::from(wki_str).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, wki_col)).bg(c(app, MK_BG))
                        }),
                        Cell::from(wf_str).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, wf_col)).bg(c(app, MK_BG))
                        }),
                        Cell::from(local_str).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, local_col)).bg(c(app, MK_BG))
                        }),
                        Cell::from(cgo_str).style(if sel || dim {
                            base_style
                        } else {
                            Style::default().fg(c(app, cgo_col)).bg(c(app, MK_BG))
                        }),
                    ])
                    .style(Style::default().bg(c(
                        app,
                        if sel {
                            MK_BG_SEL
                        } else if dim {
                            MK_BG_DIM
                        } else {
                            MK_BG
                        },
                    )))
                }
            }
        })
        .collect();

    let mut ts = TableState::default();
    let table = Table::new(rows, columns::column_widths(app.show_columns))
        .header(header)
        .row_highlight_style(Style::default())
        .style(Style::default().bg(c(app, MK_BG)));
    f.render_stateful_widget(table, inner, &mut ts);

    if app.filtered_rows.len() > visible && visible > 0 {
        let repo_pos = app.filtered_rows[..=cursor]
            .iter()
            .filter(|r| matches!(r, RepoRow::Repo(_)))
            .count();
        let repo_total = app
            .filtered_rows
            .iter()
            .filter(|r| matches!(r, RepoRow::Repo(_)))
            .count();
        let txt = format!(" {repo_pos}/{repo_total} ");
        let w = (txt.len() as u16).min(inner.width);
        f.render_widget(
            Paragraph::new(txt).style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))),
            Rect {
                x: inner.x + inner.width.saturating_sub(w),
                y: inner.y + inner.height.saturating_sub(1),
                width: w,
                height: 1,
            },
        );
    }
}
