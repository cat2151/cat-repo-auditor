use crate::{
    app::App,
    ui::{
        build_detail_items, truncate, window_color, Focus, MK_BG, MK_BG_SEL, MK_BLUE, MK_COMMENT,
        MK_CYAN, MK_FG, MK_GREEN, MK_ORANGE, MK_PURPLE, MK_RED,
    },
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

#[path = "ui_detail_boxes.rs"]
mod boxes;
pub(crate) use boxes::{
    draw_cargo_old_box, draw_help_dialog, draw_local_staging_box, CARGO_OLD_BOX_H,
    LOCAL_CHANGES_BOX_H,
};

fn c(app: &App, color: ratatui::style::Color) -> ratatui::style::Color {
    window_color(app.window_focused, color)
}

// ── right pane ───────────────────────────────────────────────────────────────

pub(crate) fn draw_right(f: &mut Frame, app: &mut App, area: Rect) {
    let active = app.window_focused && app.focus == Focus::Detail;
    let border_col = if active { MK_GREEN } else { MK_COMMENT };

    if app.repos.is_empty() {
        f.render_widget(
            Block::default()
                .title(" Detail ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(c(app, MK_COMMENT)))
                .style(Style::default().bg(c(app, MK_BG))),
            area,
        );
        return;
    }

    let repo_idx = match app.selected_repo_idx() {
        Some(i) => i,
        None => {
            f.render_widget(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().bg(c(app, MK_BG))),
                area,
            );
            return;
        }
    };
    // Clone data needed after mutable borrows to avoid borrow conflict
    let staging_files: Vec<String> = app.repos[repo_idx].staging_files.clone();
    let repo = &app.repos[repo_idx];

    let title = format!(
        " {} │ PR:{} ISS:{} │ {} ",
        repo.name, repo.open_prs, repo.open_issues, repo.local_status
    );
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(c(app, border_col)))
        .style(Style::default().bg(c(app, MK_BG)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // ── layout within right pane ─────────────────────────────────────────────
    // summary(1) + hint(1) + issue/PR section + staging section
    let has_staging = !staging_files.is_empty();
    let staging_height = if has_staging {
        (staging_files.len() + 2).min(inner.height as usize / 3) as u16
    } else {
        0
    };

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),              // summary
            Constraint::Length(1),              // hint
            Constraint::Min(4),                 // issue/PR list
            Constraint::Length(staging_height), // staging (0 if empty)
        ])
        .split(inner);

    // summary
    let priv_tag = if repo.is_private { "  🔒" } else { "" };
    f.render_widget(
        Paragraph::new(format!(" Updated: {}{}", repo.updated_at, priv_tag))
            .style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))),
        vert[0],
    );
    // hint
    let hint = if active {
        " h:back  j/k:move  PgUp/PgDn  Enter:open"
    } else {
        " l:focus  g:lazygit"
    };
    f.render_widget(
        Paragraph::new(hint).style(
            Style::default()
                .fg(c(app, MK_COMMENT))
                .bg(c(app, MK_BG))
                .add_modifier(Modifier::DIM),
        ),
        vert[1],
    );

    // ── issue/PR list ─────────────────────────────────────────────────────────
    let list_area = vert[2];
    let items = build_detail_items(repo);

    if items.is_empty() {
        f.render_widget(
            Paragraph::new("  (no open issues or PRs)")
                .style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))),
            list_area,
        );
    } else {
        let visible = list_area.height as usize;
        app.right_visible = visible;
        app.adjust_detail_scroll(visible);
        let d_scroll = app.detail_scroll;
        let d_sel = app.detail_selected;

        let lines: Vec<Line> = items
            .iter()
            .enumerate()
            .skip(d_scroll)
            .take(visible)
            .map(|(i, item)| {
                let sel = active && i == d_sel;
                let bg = c(app, if sel { MK_BG_SEL } else { MK_BG });
                let indent = if item.is_child { " " } else { "" };
                let connector = if item.is_child { "└─" } else { "  " };
                let (label, label_col) = if item.is_pr {
                    (" PR", MK_PURPLE)
                } else {
                    ("ISS", MK_RED)
                };
                let prefix_len = if item.is_child { 4usize } else { 3usize };
                let max_title = list_area.width.saturating_sub(28 + prefix_len as u16) as usize;
                Line::from(vec![
                    Span::styled(
                        format!("{}{} ", indent, connector),
                        Style::default().fg(c(app, MK_COMMENT)).bg(bg),
                    ),
                    Span::styled(
                        format!("{} ", label),
                        Style::default()
                            .fg(c(app, label_col))
                            .bg(bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("#{:<4} ", item.number),
                        Style::default().fg(c(app, MK_COMMENT)).bg(bg),
                    ),
                    Span::styled(
                        truncate(&item.title, max_title),
                        Style::default().fg(c(app, MK_FG)).bg(bg),
                    ),
                    Span::styled(
                        format!("  {}", item.updated),
                        Style::default()
                            .fg(c(app, if sel { MK_CYAN } else { MK_COMMENT }))
                            .bg(bg),
                    ),
                ])
            })
            .collect();
        f.render_widget(
            Paragraph::new(lines).style(Style::default().bg(c(app, MK_BG))),
            list_area,
        );

        // position indicator
        if items.len() > visible && visible > 0 {
            let txt = format!(" {}/{} ", d_sel + 1, items.len());
            let w = (txt.len() as u16).min(list_area.width);
            f.render_widget(
                Paragraph::new(txt)
                    .style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))),
                Rect {
                    x: list_area.x + list_area.width.saturating_sub(w),
                    y: list_area.y + list_area.height.saturating_sub(1),
                    width: w,
                    height: 1,
                },
            );
        }
    }

    // ── local changes section ────────────────────────────────────────────────
    if has_staging && staging_height > 0 {
        let staging_area = vert[3];
        let stag_block = Block::default()
            .title(" Local Changes ")
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(c(app, MK_BLUE)))
            .style(Style::default().bg(c(app, MK_BG)));
        let stag_inner = stag_block.inner(staging_area);
        f.render_widget(stag_block, staging_area);

        let stag_visible = stag_inner.height as usize;
        let stag_lines: Vec<Line> = staging_files
            .iter()
            .take(stag_visible)
            .map(|line| {
                // porcelain format: "XY filename"
                let (status, rest) = if line.len() > 3 {
                    (&line[..2], &line[3..])
                } else {
                    ("??", line.as_str())
                };
                let status_col = match status.trim() {
                    "M" | "MM" => MK_ORANGE,
                    "A" | "AM" => MK_GREEN,
                    "D" | "DD" => MK_RED,
                    "R" | "RM" => MK_PURPLE,
                    "??" => MK_COMMENT,
                    _ => MK_FG,
                };
                Line::from(vec![
                    Span::styled(
                        format!(" {} ", status),
                        Style::default()
                            .fg(c(app, status_col))
                            .bg(c(app, MK_BG))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        rest.to_string(),
                        Style::default().fg(c(app, MK_FG)).bg(c(app, MK_BG)),
                    ),
                ])
            })
            .collect();
        f.render_widget(
            Paragraph::new(stag_lines).style(Style::default().bg(c(app, MK_BG))),
            stag_inner,
        );
    }
}
