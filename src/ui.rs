use crate::{
    app::App,
    github::LocalStatus,
    ui_detail::{
        draw_cargo_old_box, draw_help_dialog, draw_local_staging_box, draw_right,
        CARGO_OLD_BOX_H, LOCAL_CHANGES_BOX_H,
    },
};
// Re-export ui_types items so existing imports from `crate::ui` continue to work
pub(crate) use crate::ui_types::{
    build_detail_items, build_rows, local_check_cell, window_color, Focus,
    MK_BG, MK_BG_DIM, MK_BG_SEL, MK_BLUE, MK_COMMENT, MK_CYAN,
    MK_FG, MK_GREEN, MK_ORANGE, MK_PURPLE, MK_RED, MK_YELLOW,
    RepoRow, SearchState,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Frame,
};

const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SPINNER_FRAME_MS: u64 = 250;

fn c(app: &App, color: ratatui::style::Color) -> ratatui::style::Color {
    window_color(app.window_focused, color)
}

fn spinner_frame(unix_millis: u64) -> &'static str {
    let frame_index = (unix_millis / SPINNER_FRAME_MS) as usize;
    SPINNER_FRAMES[frame_index % SPINNER_FRAMES.len()]
}

fn bottom_right_box_flags(app: &App, repo_idx: usize) -> (bool, bool) {
    let repo = &app.repos[repo_idx];
    (
        matches!(
            repo.local_status,
            LocalStatus::Conflict | LocalStatus::Modified | LocalStatus::Staging
        ) || !repo.staging_files.is_empty(),
        repo.cargo_install == Some(false),
    )
}

fn bottom_right_stack_offsets(box_heights: &[u16]) -> Vec<u16> {
    let mut running = 0u16;
    box_heights.iter().map(|h| {
        let offset = running;
        running = running.saturating_add(*h);
        offset
    }).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BottomRightBox {
    CargoOld,
    LocalChanges,
}

fn bottom_right_boxes(show_staging: bool, show_cargo_old: bool) -> Vec<BottomRightBox> {
    let mut boxes = Vec::new();
    if show_cargo_old {
        boxes.push(BottomRightBox::CargoOld);
    }
    if show_staging {
        boxes.push(BottomRightBox::LocalChanges);
    }
    boxes
}

fn bottom_right_box_height(b: BottomRightBox) -> u16 {
    match b {
        BottomRightBox::CargoOld => CARGO_OLD_BOX_H,
        BottomRightBox::LocalChanges => LOCAL_CHANGES_BOX_H,
    }
}

/// Build the text displayed in the top status bar for background tasks.
///
/// `bg_tasks` items are `(tag, current, total)`, where `total == 0` means
/// unknown total progress. `unix_millis` selects the spinner frame so tests
/// can assert deterministic output.
fn build_tasks_display<'a, I>(bg_tasks: I, unix_millis: u64) -> String
where
    I: IntoIterator<Item = &'a (&'static str, usize, usize)>,
{
    let tasks_str: String = bg_tasks.into_iter()
        .map(|(tag, cur, total)| {
            if *total == 0 {
                format!("{}{}  ", tag, cur)  // gh↓ page num (total unknown)
            } else {
                format!("{}{}/{}  ", tag, cur, total)
            }
        })
        .collect::<Vec<_>>()
        .join("");

    if tasks_str.is_empty() {
        String::new()
    } else {
        format!("  {} {}", spinner_frame(unix_millis), tasks_str.trim_end())
    }
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
    // Build background task indicator: "⠋ gh↓1 scan3/76 chk5/76"
    let unix_millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    let tasks_display = build_tasks_display(app.bg_tasks.iter(), unix_millis);

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
        Paragraph::new(rl_text).style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))),
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
                .style(Style::default().fg(c(app, MK_YELLOW)).bg(c(app, MK_BG_DIM))),
            outer[2],
        );
    } else {
        let prefix_str = if app.num_prefix > 0 { format!("[{}]  ", app.num_prefix) } else { String::new() };
        let (display_msg, msg_style) = if let Some(ref t) = app.transient_msg {
            // Transient: highlighted differently so user notices it
            (format!(" {}", t), Style::default().fg(c(app, MK_YELLOW)).bg(c(app, MK_BG_SEL)))
        } else {
            (format!(" {}{}", prefix_str, app.status_msg), Style::default().fg(c(app, MK_FG)).bg(c(app, MK_BG_SEL)))
        };
        f.render_widget(
            Paragraph::new(display_msg).style(msg_style),
            outer[2],
        );
    }

    let main_chunks = if app.show_log {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(outer[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0)])
            .split(outer[1])
    };

    // ── panes ────────────────────────────────────────────────────────────────
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(main_chunks[0]);

    draw_left(f, app, panes[0]);
    draw_right(f, app, panes[1]);
    if app.show_log {
        draw_log(f, app, main_chunks[1]);
    }

    // ── bottom-right status boxes ────────────────────────────────────────────
    if let Some(idx) = app.selected_repo_idx() {
        let (show_staging, show_cargo_old) = bottom_right_box_flags(app, idx);
        let boxes = bottom_right_boxes(show_staging, show_cargo_old);
        let heights: Vec<u16> = boxes.iter().map(|b| bottom_right_box_height(*b)).collect();
        let offsets = bottom_right_stack_offsets(&heights);

        for (b, offset) in boxes.into_iter().zip(offsets.into_iter()) {
            match b {
                BottomRightBox::CargoOld => draw_cargo_old_box(f, app, idx, area, offset),
                BottomRightBox::LocalChanges => draw_local_staging_box(f, app, idx, area, offset),
            }
        }
    }

    if app.show_help {
        draw_help_dialog(f, app, area);
    }
}

fn draw_log(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" logs/log.txt ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(c(app, MK_COMMENT)))
        .style(Style::default().bg(c(app, MK_BG)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let visible = inner.height as usize;
    let start = app.log_lines.len().saturating_sub(visible);
    let lines: Vec<Line> = app.log_lines[start..]
        .iter()
        .map(|s| Line::from(s.as_str()))
        .collect();
    f.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(c(app, MK_FG)).bg(c(app, MK_BG)))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

// ── left pane ────────────────────────────────────────────────────────────────

fn draw_left(f: &mut Frame, app: &mut App, area: Rect) {
    let active = app.window_focused && app.focus == Focus::Repos;
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
        .border_style(Style::default().fg(c(app, border_col)))
        .style(Style::default().bg(c(app, MK_BG)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.filtered_rows.is_empty() {
        let msg = if app.loading { "  Loading…" }
                  else if !app.search_query.is_empty() { "  (no match)" }
                  else { "  No repositories." };
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))),
            inner,
        );
        return;
    }

    let header = if app.show_columns {
        Row::new(vec![
            Cell::from("Repository").style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("Updated"   ).style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("PR" ).style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("ISS").style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("doc").style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("pg" ).style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("ja" ).style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("wki").style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("wf" ).style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("Local").style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("cgo").style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
        ]).style(Style::default().bg(c(app, MK_BG)))
    } else {
        Row::new(vec![
            Cell::from("Repository").style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("Updated"   ).style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("PR" ).style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
            Cell::from("ISS").style(Style::default().add_modifier(Modifier::BOLD).fg(c(app, MK_YELLOW))),
        ]).style(Style::default().bg(c(app, MK_BG)))
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
                    .style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG)).add_modifier(Modifier::DIM)),
                Cell::from(""), Cell::from(""), Cell::from(""),
                Cell::from(""), Cell::from(""), Cell::from(""),
                Cell::from(""), Cell::from(""), Cell::from(""),
                Cell::from(""),
            ]).style(Style::default().bg(c(app, MK_BG))),

            RepoRow::Repo(repo_idx) => {
                let repo = &app.repos[*repo_idx];
                let is_cursor = row_i == cursor;
                let sel = is_cursor && active;
                let dim = is_cursor && !active;

                let base_style = if sel {
                    Style::default().bg(c(app, MK_BG_SEL)).fg(c(app, MK_FG)).add_modifier(Modifier::BOLD)
                } else if dim {
                    Style::default().bg(c(app, MK_BG_DIM)).fg(c(app, MK_FG))
                } else {
                    Style::default().fg(c(app, MK_FG)).bg(c(app, MK_BG))
                };

                let local_col = match repo.local_status {
                    LocalStatus::Conflict => MK_RED,
                    LocalStatus::Modified => MK_ORANGE,
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
                let local_no_git = matches!(repo.local_status, LocalStatus::NotFound | LocalStatus::NoGit);

                let (ja_str, ja_col) = if is_checking && !local_no_git && repo.readme_ja_badge_checked_at.is_empty() {
                    pending
                } else {
                    local_check_cell(local_no_git, repo.readme_ja_badge, MK_YELLOW)
                };

                let (wki_str, wki_col) = if is_checking && !local_no_git && repo.deepwiki_checked_at.is_empty() {
                    pending
                } else {
                    local_check_cell(local_no_git, repo.deepwiki, MK_PURPLE)
                };
                let (wf_str, wf_col) = if is_checking && !local_no_git && repo.wf_checked_at.is_empty() {
                    pending
                } else {
                    local_check_cell(local_no_git, repo.wf_workflows, MK_GREEN)
                };

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
                            if sel || dim { base_style } else { Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG)) }
                        ),
                        Cell::from(format!("{:>3}", repo.open_prs)).style(
                            if sel || dim { base_style } else { Style::default().fg(c(app, pr_col)).bg(c(app, MK_BG)) }
                        ),
                        Cell::from(format!("{:>3}", repo.open_issues)).style(
                            if sel || dim { base_style } else { Style::default().fg(c(app, iss_col)).bg(c(app, MK_BG)) }
                        ),
                    ]).style(Style::default().bg(
                        c(app, if sel { MK_BG_SEL } else if dim { MK_BG_DIM } else { MK_BG })
                    ))
                } else {
                Row::new(vec![
                    Cell::from(name_str).style(base_style),
                    Cell::from(repo.updated_at.clone()).style(
                        if sel || dim { base_style } else { Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG)) }
                    ),
                    Cell::from(format!("{:>3}", repo.open_prs)).style(
                        if sel || dim { base_style } else { Style::default().fg(c(app, pr_col)).bg(c(app, MK_BG)) }
                    ),
                    Cell::from(format!("{:>3}", repo.open_issues)).style(
                        if sel || dim { base_style } else { Style::default().fg(c(app, iss_col)).bg(c(app, MK_BG)) }
                    ),
                    Cell::from(doc_str).style(
                        if sel || dim { base_style } else { Style::default().fg(c(app, doc_col)).bg(c(app, MK_BG)) }
                    ),
                    Cell::from(pg_str).style(
                        if sel || dim { base_style } else { Style::default().fg(c(app, pg_col)).bg(c(app, MK_BG)) }
                    ),
                    Cell::from(ja_str).style(
                        if sel || dim { base_style } else { Style::default().fg(c(app, ja_col)).bg(c(app, MK_BG)) }
                    ),
                    Cell::from(wki_str).style(
                        if sel || dim { base_style } else { Style::default().fg(c(app, wki_col)).bg(c(app, MK_BG)) }
                    ),
                    Cell::from(wf_str).style(
                        if sel || dim { base_style } else { Style::default().fg(c(app, wf_col)).bg(c(app, MK_BG)) }
                    ),
                    Cell::from(repo.local_status.to_string()).style(
                        if sel || dim { base_style } else { Style::default().fg(c(app, local_col)).bg(c(app, MK_BG)) }
                    ),
                    Cell::from(cgo_str).style(
                        if sel || dim { base_style } else { Style::default().fg(c(app, cgo_col)).bg(c(app, MK_BG)) }
                    ),
                ]).style(Style::default().bg(
                    c(app, if sel { MK_BG_SEL } else if dim { MK_BG_DIM } else { MK_BG })
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
        .style(Style::default().bg(c(app, MK_BG)));
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
            Paragraph::new(txt).style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))),
            Rect { x: inner.x + inner.width.saturating_sub(w), y: inner.y + inner.height.saturating_sub(1), width: w, height: 1 },
        );
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

pub(crate) fn truncate(s: &str, max: usize) -> String {
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

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
