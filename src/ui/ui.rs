use crate::{
    app::App,
    config::Config,
    github::LocalStatus,
    main_helpers::refresh_log_lines_if_changed_for_path,
    ui_detail::{
        draw_cargo_old_box, draw_help_dialog, draw_local_hash_box, draw_local_staging_box,
        draw_right, draw_workflow_repo_exist_overlay, CARGO_OLD_BOX_H, LOCAL_CHANGES_BOX_H,
        LOCAL_HASH_BOX_H,
    },
};
#[path = "ui_draw_left.rs"]
mod draw_left;
// Re-export ui_types items so existing imports from `crate::ui` continue to work
pub(crate) use crate::ui_types::{
    build_detail_items, build_rows, local_check_cell, window_color, Focus, RepoRow, SearchState,
    MK_BG, MK_BG_DIM, MK_BG_SEL, MK_BLUE, MK_COMMENT, MK_CYAN, MK_FG, MK_GREEN, MK_ORANGE,
    MK_PURPLE, MK_RED, MK_YELLOW,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::path::Path;

use draw_left::draw_left;

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
        true,
    )
}

fn bottom_right_stack_offsets(box_heights: &[u16]) -> Vec<u16> {
    let mut running = 0u16;
    box_heights
        .iter()
        .map(|h| {
            let offset = running;
            running = running.saturating_add(*h);
            offset
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BottomRightBox {
    CargoHash,
    LocalHash,
    LocalChanges,
}

fn bottom_right_boxes(show_staging: bool, show_cargo_hash: bool) -> Vec<BottomRightBox> {
    let mut boxes = Vec::new();
    if show_cargo_hash {
        boxes.push(BottomRightBox::CargoHash);
        boxes.push(BottomRightBox::LocalHash);
    }
    if show_staging {
        boxes.push(BottomRightBox::LocalChanges);
    }
    boxes
}

fn bottom_right_box_height(b: BottomRightBox) -> u16 {
    match b {
        BottomRightBox::CargoHash => CARGO_OLD_BOX_H,
        BottomRightBox::LocalHash => LOCAL_HASH_BOX_H,
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
    let tasks_str: String = bg_tasks
        .into_iter()
        .map(|(tag, cur, total)| {
            if *total == 0 {
                format!("{}{}  ", tag, cur) // gh↓ page num (total unknown)
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

fn refresh_visible_log_panel(app: &mut App, log_path: &Path) {
    if app.show_log {
        refresh_log_lines_if_changed_for_path(app, log_path);
    }
}

pub fn draw_ui(f: &mut Frame, app: &mut App) {
    let area = f.area();
    refresh_visible_log_panel(app, &Config::log_path());

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
    let cargo_poll_tasks = if app.active_cargo_hash_poll_count() > 0 {
        vec![("cgo", app.active_cargo_hash_poll_count(), 0)]
    } else {
        vec![]
    };
    let tasks_display = build_tasks_display(
        app.bg_tasks.iter().chain(cargo_poll_tasks.iter()),
        unix_millis,
    );

    let rl_text = if let Some(rl) = &app.rate_limit {
        format!(
            " API: {}/{} │ resets {}{}",
            rl.remaining,
            rl.limit,
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
        let match_count = app
            .filtered_rows
            .iter()
            .filter(|r| matches!(r, RepoRow::Repo(_)))
            .count();
        let hint = format!("{:<40} [{} matches]", query_display, match_count);
        f.render_widget(
            Paragraph::new(hint)
                .style(Style::default().fg(c(app, MK_YELLOW)).bg(c(app, MK_BG_DIM))),
            outer[2],
        );
    } else {
        let prefix_str = if app.num_prefix > 0 {
            format!("[{}]  ", app.num_prefix)
        } else {
            String::new()
        };
        let (display_msg, msg_style) = if let Some(ref t) = app.transient_msg {
            // Transient: highlighted differently so user notices it
            (
                format!(" {}", t),
                Style::default().fg(c(app, MK_YELLOW)).bg(c(app, MK_BG_SEL)),
            )
        } else {
            (
                format!(" {}{}", prefix_str, app.status_msg),
                Style::default().fg(c(app, MK_FG)).bg(c(app, MK_BG_SEL)),
            )
        };
        f.render_widget(Paragraph::new(display_msg).style(msg_style), outer[2]);
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

    draw_left(f, app, panes[0], unix_millis);
    draw_right(f, app, panes[1]);
    if app.show_log {
        draw_log(f, app, main_chunks[1]);
    }

    // ── bottom-right status boxes ────────────────────────────────────────────
    if let Some(idx) = app.selected_repo_idx() {
        let (show_staging, show_cargo_hash) = bottom_right_box_flags(app, idx);
        let boxes = bottom_right_boxes(show_staging, show_cargo_hash);
        let heights: Vec<u16> = boxes.iter().map(|b| bottom_right_box_height(*b)).collect();
        let offsets = bottom_right_stack_offsets(&heights);

        for (b, offset) in boxes.into_iter().zip(offsets) {
            match b {
                BottomRightBox::CargoHash => draw_cargo_old_box(f, app, idx, area, offset),
                BottomRightBox::LocalHash => draw_local_hash_box(f, app, idx, area, offset),
                BottomRightBox::LocalChanges => draw_local_staging_box(f, app, idx, area, offset),
            }
        }
    }

    if app.show_help {
        draw_help_dialog(f, app, area);
    }
    if app.show_workflow_repo_exist {
        draw_workflow_repo_exist_overlay(f, app, area);
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

// ── helpers ──────────────────────────────────────────────────────────────────

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        let mut t: String = chars[..max.saturating_sub(1)].iter().collect();
        t.push('…');
        t
    }
}

fn format_reset(reset_at: &str) -> String {
    use chrono::{DateTime, Utc};
    if let Ok(dt) = reset_at.parse::<DateTime<Utc>>() {
        let now = Utc::now();
        if dt > now {
            let diff = dt - now;
            format!("in {}m{}s", diff.num_minutes(), diff.num_seconds() % 60)
        } else {
            String::from("now")
        }
    } else {
        reset_at.to_string()
    }
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
