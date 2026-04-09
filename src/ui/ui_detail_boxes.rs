use crate::{
    app::App,
    ui::{
        truncate, window_color, MK_BG, MK_BG_DIM, MK_BG_SEL, MK_BLUE, MK_COMMENT, MK_CYAN, MK_FG,
        MK_GREEN, MK_ORANGE, MK_PURPLE, MK_YELLOW,
    },
};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[path = "ui_detail_boxes_help.rs"]
mod help;

pub(crate) use help::draw_help_dialog;

pub(crate) const LOCAL_HASH_BOX_H: u16 = 3;
pub(crate) const CARGO_OLD_BOX_H: u16 = 4;
pub(crate) const LOCAL_CHANGES_BOX_H: u16 = 3;

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let dialog_w = width.min(area.width);
    let dialog_h = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(dialog_w) / 2,
        y: area.y + area.height.saturating_sub(dialog_h) / 2,
        width: dialog_w,
        height: dialog_h,
    }
}

pub(super) fn c(app: &App, color: ratatui::style::Color) -> ratatui::style::Color {
    window_color(app.window_focused, color)
}

// ── commit hash boxes ─────────────────────────────────────────────────────────

pub(crate) fn draw_local_hash_box(
    f: &mut Frame,
    app: &App,
    repo_idx: usize,
    area: Rect,
    bottom_offset: u16,
) {
    let repo = &app.repos[repo_idx];
    let local = if repo.cargo_checked_at.is_empty() {
        "?"
    } else {
        &repo.cargo_checked_at
    };

    let content_w: u16 = 53;
    let box_w = content_w + 2;
    let box_h: u16 = LOCAL_HASH_BOX_H;

    let x = area.x + area.width.saturating_sub(box_w + 1);
    let y = area.y + area.height.saturating_sub(box_h + 1 + bottom_offset);
    let rect = Rect {
        x,
        y,
        width: box_w.min(area.width),
        height: box_h.min(area.height),
    };

    f.render_widget(Clear, rect);
    let block = Block::default()
        .title(" local: commit hash ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(c(app, MK_GREEN)))
        .style(Style::default().bg(c(app, MK_BG)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    f.render_widget(
        Paragraph::new(truncate(local, inner.width as usize))
            .style(Style::default().fg(c(app, MK_GREEN)).bg(c(app, MK_BG))),
        inner,
    );
}

pub(crate) fn draw_cargo_old_box(
    f: &mut Frame,
    app: &App,
    repo_idx: usize,
    area: Rect,
    bottom_offset: u16,
) {
    let repo = &app.repos[repo_idx];
    let remote = if repo.cargo_remote_hash.is_empty() {
        "?"
    } else {
        &repo.cargo_remote_hash
    };
    let inst = if repo.cargo_installed_hash.is_empty() {
        "?"
    } else {
        &repo.cargo_installed_hash
    };

    let content_w: u16 = 53;
    let box_w = content_w + 2;
    let box_h: u16 = CARGO_OLD_BOX_H;

    let x = area.x + area.width.saturating_sub(box_w + 1);
    let y = area.y + area.height.saturating_sub(box_h + 1 + bottom_offset);
    let rect = Rect {
        x,
        y,
        width: box_w.min(area.width),
        height: box_h.min(area.height),
    };

    f.render_widget(Clear, rect);
    let block = Block::default()
        .title(" cgo: commit hash ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(c(app, MK_ORANGE)))
        .style(Style::default().bg(c(app, MK_BG)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let label_w: u16 = 12;
    let max_w = inner.width.saturating_sub(label_w) as usize;
    let lines = vec![
        Line::from(vec![
            Span::styled("    remote: ", Style::default().fg(c(app, MK_COMMENT))),
            Span::styled(
                truncate(remote, max_w),
                Style::default().fg(c(app, MK_CYAN)),
            ),
        ]),
        Line::from(vec![
            Span::styled(" installed: ", Style::default().fg(c(app, MK_COMMENT))),
            Span::styled(
                truncate(inst, max_w),
                Style::default().fg(c(app, MK_ORANGE)),
            ),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(c(app, MK_BG))),
        inner,
    );
}

pub(crate) fn draw_local_staging_box(
    f: &mut Frame,
    app: &App,
    repo_idx: usize,
    area: Rect,
    bottom_offset: u16,
) {
    let repo = &app.repos[repo_idx];
    let local_changes_count = repo.staging_files.len();

    let content_w: u16 = 38;
    let box_w = content_w + 2;
    let box_h: u16 = LOCAL_CHANGES_BOX_H;

    let x = area.x + area.width.saturating_sub(box_w + 1);
    let y = area.y + area.height.saturating_sub(box_h + 1 + bottom_offset);
    let rect = Rect {
        x,
        y,
        width: box_w.min(area.width),
        height: box_h.min(area.height),
    };

    f.render_widget(Clear, rect);
    let block = Block::default()
        .title(" local changes ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(c(app, MK_BLUE)))
        .style(Style::default().bg(c(app, MK_BG)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let msg = format!(" {} file(s) with local changes", local_changes_count);
    f.render_widget(
        Paragraph::new(msg).style(Style::default().fg(c(app, MK_BLUE)).bg(c(app, MK_BG))),
        inner,
    );
}

fn draw_workflow_repo_list_box(
    f: &mut Frame,
    app: &App,
    area: Rect,
    title: String,
    repos: &[crate::github_local::WorkflowRepoExistRepo],
    border_color: ratatui::style::Color,
) {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(c(app, border_color)))
        .style(Style::default().bg(c(app, MK_BG_DIM)));
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let lines: Vec<Line> = if repos.is_empty() {
        vec![Line::from(Span::styled(
            " (none)",
            Style::default().fg(c(app, MK_COMMENT)),
        ))]
    } else {
        let width = inner.width as usize;
        let date_width = repos
            .iter()
            .map(|repo| UnicodeWidthStr::width(repo.updated_at.as_str()))
            .max()
            .unwrap_or(0)
            .min(width.saturating_sub(2));
        repos
            .iter()
            .take(inner.height as usize)
            .map(|repo| {
                let updated_at = if date_width == 0 {
                    String::new()
                } else {
                    truncate_display_width(&repo.updated_at, date_width)
                };
                let name_width = width.saturating_sub(1 + date_width);
                let name = truncate_display_width(&repo.name, name_width);
                let pad_width = width.saturating_sub(
                    1 + UnicodeWidthStr::width(name.as_str())
                        + UnicodeWidthStr::width(updated_at.as_str()),
                );
                Line::from(vec![
                    Span::styled(
                        format!(" {name}"),
                        Style::default().fg(c(app, MK_FG)).bg(c(app, MK_BG_DIM)),
                    ),
                    Span::styled(
                        " ".repeat(pad_width),
                        Style::default().bg(c(app, MK_BG_DIM)),
                    ),
                    Span::styled(
                        updated_at,
                        Style::default()
                            .fg(c(app, MK_COMMENT))
                            .bg(c(app, MK_BG_DIM)),
                    ),
                ])
            })
            .collect()
    };

    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(c(app, MK_BG_DIM))),
        inner,
    );
}

fn truncate_display_width(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    if UnicodeWidthStr::width(s) <= max_width {
        return s.to_string();
    }

    let ellipsis = '…';
    let ellipsis_width = UnicodeWidthChar::width(ellipsis).unwrap_or(1);
    if max_width <= ellipsis_width {
        return ellipsis.to_string();
    }

    let mut result = String::new();
    let mut width = 0;
    for ch in s.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width + ellipsis_width > max_width {
            break;
        }
        result.push(ch);
        width += ch_width;
    }
    result.push(ellipsis);
    result
}

pub(crate) fn draw_workflow_repo_exist_overlay(f: &mut Frame, app: &mut App, area: Rect) {
    let dialog = centered_rect(
        area,
        area.width.saturating_sub(4).min(110),
        area.height.saturating_sub(2).min(28),
    );
    f.render_widget(Clear, dialog);
    f.render_widget(
        Paragraph::new("").style(Style::default().bg(c(app, MK_BG_DIM))),
        dialog,
    );

    let block = Block::default()
        .title(" workflow repo exist check  (Shift+W / Esc to close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(c(app, MK_PURPLE)))
        .style(Style::default().bg(c(app, MK_BG_DIM)));
    let inner = block.inner(dialog);
    f.render_widget(block, dialog);

    let panes = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage(44),
            ratatui::layout::Constraint::Percentage(56),
        ])
        .split(inner);
    let right = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Percentage(50),
            ratatui::layout::Constraint::Percentage(50),
        ])
        .split(panes[1]);

    let list_block = Block::default()
        .title(" github-actions/.github/workflows/call-* ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(c(app, MK_CYAN)))
        .style(Style::default().bg(c(app, MK_BG_DIM)));
    let list_inner = list_block.inner(panes[0]);
    f.render_widget(Clear, panes[0]);
    f.render_widget(list_block, panes[0]);

    let list_lines: Vec<Line> = if app.workflow_repo_exist_items.is_empty() {
        vec![Line::from(Span::styled(
            " (github-actions: no call-* workflows)",
            Style::default().fg(c(app, MK_COMMENT)),
        ))]
    } else {
        let visible = list_inner.height as usize;
        app.adjust_workflow_repo_exist_scroll(visible);
        app.workflow_repo_exist_items
            .iter()
            .enumerate()
            .skip(app.workflow_repo_exist_scroll)
            .take(visible)
            .map(|(idx, item)| {
                let selected = idx == app.workflow_repo_exist_selected;
                let bg = c(app, if selected { MK_BG_SEL } else { MK_BG_DIM });
                let count = item.installed_repos.len();
                let count_width = list_inner.width.saturating_sub(4) as usize;
                let name_width = count_width.saturating_sub(6);
                Line::from(vec![
                    Span::styled(
                        format!(" {}", truncate(&item.workflow_file, name_width)),
                        Style::default().fg(c(app, MK_FG)).bg(bg),
                    ),
                    Span::styled(
                        format!(" {:>3}", count),
                        Style::default()
                            .fg(c(app, if selected { MK_YELLOW } else { MK_GREEN }))
                            .bg(bg),
                    ),
                ])
            })
            .collect()
    };
    f.render_widget(
        Paragraph::new(list_lines).style(Style::default().bg(c(app, MK_BG_DIM))),
        list_inner,
    );

    if let Some(selected) = app.selected_workflow_repo_exist() {
        draw_workflow_repo_list_box(
            f,
            app,
            right[0],
            format!(" 導入済み repo ({}) ", selected.installed_repos.len()),
            &selected.installed_repos,
            MK_GREEN,
        );
        draw_workflow_repo_list_box(
            f,
            app,
            right[1],
            format!(" 未導入 repo ({}) ", selected.missing_repos.len()),
            &selected.missing_repos,
            MK_ORANGE,
        );
    } else {
        let empty_repo_list: Vec<crate::github_local::WorkflowRepoExistRepo> = vec![];
        draw_workflow_repo_list_box(
            f,
            app,
            right[0],
            String::from(" 導入済み repo (0) "),
            &empty_repo_list,
            MK_GREEN,
        );
        draw_workflow_repo_list_box(
            f,
            app,
            right[1],
            String::from(" 未導入 repo (0) "),
            &empty_repo_list,
            MK_ORANGE,
        );
    }
}
