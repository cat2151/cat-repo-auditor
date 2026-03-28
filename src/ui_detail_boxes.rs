use crate::{
    app::App,
    ui::{
        truncate, window_color, MK_BG, MK_BG_DIM, MK_BG_SEL, MK_BLUE, MK_COMMENT, MK_CYAN, MK_FG,
        MK_GREEN, MK_ORANGE, MK_PURPLE, MK_YELLOW,
    },
};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub(crate) const CARGO_OLD_BOX_H: u16 = 5;
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

// ── cargo old comparison box ──────────────────────────────────────────────────

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
    let local = if repo.cargo_checked_at.is_empty() {
        "?"
    } else {
        &repo.cargo_checked_at
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
            Span::styled("     local: ", Style::default().fg(c(app, MK_COMMENT))),
            Span::styled(
                truncate(local, max_w),
                Style::default().fg(c(app, MK_GREEN)),
            ),
        ]),
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

// ── help dialog ──────────────────────────────────────────────────────────────

pub(crate) fn draw_help_dialog(f: &mut Frame, app: &App, area: Rect) {
    let dw: u16 = 62;
    let dh: u16 = 30;
    let dialog = centered_rect(area, dw, dh);

    f.render_widget(Clear, dialog);
    f.render_widget(
        Paragraph::new("").style(Style::default().bg(c(app, MK_BG_DIM))),
        dialog,
    );

    let block = Block::default()
        .title(" Help  (? or Esc to close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(c(app, MK_CYAN)))
        .style(Style::default().bg(c(app, MK_BG_DIM)));

    let inner = block.inner(dialog);
    f.render_widget(block, dialog);

    let lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " Keybinds",
            Style::default()
                .fg(c(app, MK_YELLOW))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  q        ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Quit"),
        ]),
        Line::from(vec![
            Span::styled("  F5       ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Refresh from GitHub"),
        ]),
        Line::from(vec![
            Span::styled("  j / k    ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Move cursor down / up  (Nj = N lines)"),
        ]),
        Line::from(vec![
            Span::styled("  PgDn/PgUp", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Page down / up"),
        ]),
        Line::from(vec![
            Span::styled("  h / l    ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Focus left pane / right pane"),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Open README.ja.md (or repo root)"),
        ]),
        Line::from(vec![
            Span::styled("  i        ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Open GitHub Pages (or repo root)"),
        ]),
        Line::from(vec![
            Span::styled("  w        ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Open DeepWiki page"),
        ]),
        Line::from(vec![
            Span::styled("  Shift+W  ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Open workflow repo exist check"),
        ]),
        Line::from(vec![
            Span::styled("  g        ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Open lazygit for this repo"),
        ]),
        Line::from(vec![
            Span::styled("  x        ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Run installed app (cgo=ok only)"),
        ]),
        Line::from(vec![
            Span::styled("  Shift+L  ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Toggle log pane (bottom half)"),
        ]),
        Line::from(vec![
            Span::styled("  c        ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Copy local repo path to clipboard"),
        ]),
        Line::from(vec![
            Span::styled("  d        ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Toggle doc/pg/ja/wki/wf/cgo columns"),
        ]),
        Line::from(vec![
            Span::styled("  /        ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Incremental search  (Space=AND)"),
        ]),
        Line::from(vec![
            Span::styled("  ^G / ^T  ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("Next / prev match  (during search)"),
        ]),
        Line::from(vec![
            Span::styled(
                "  (right pane) Enter",
                Style::default().fg(c(app, MK_ORANGE)),
            ),
            Span::raw("  Open issue / PR in browser"),
        ]),
        Line::from(vec![
            Span::styled(
                "  (right pane) h   ",
                Style::default().fg(c(app, MK_ORANGE)),
            ),
            Span::raw("  Back to repo list"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " Column legend",
            Style::default()
                .fg(c(app, MK_YELLOW))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  doc ", Style::default().fg(c(app, MK_GREEN))),
            Span::raw("README.ja.md exists in repo"),
        ]),
        Line::from(vec![
            Span::styled("  pg  ", Style::default().fg(c(app, MK_CYAN))),
            Span::raw("GitHub Pages is enabled"),
        ]),
        Line::from(vec![
            Span::styled("  ja  ", Style::default().fg(c(app, MK_YELLOW))),
            Span::raw("README.ja.md has a self-link badge"),
        ]),
        Line::from(vec![
            Span::styled("  wki ", Style::default().fg(c(app, MK_PURPLE))),
            Span::raw("README contains a deepwiki.com link"),
        ]),
        Line::from(vec![
            Span::styled("  wf  ", Style::default().fg(c(app, MK_GREEN))),
            Span::raw(".github/workflows has 3 required ymls"),
        ]),
        Line::from(vec![
            Span::styled("  cgo ", Style::default().fg(c(app, MK_GREEN))),
            Span::raw("ok=cargo install HEAD matches local"),
        ]),
        Line::from(vec![
            Span::styled("      ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("old=installed hash differs from HEAD"),
        ]),
    ];

    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(c(app, MK_BG_DIM))),
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
            .map(|repo| repo.updated_at.len())
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
                    truncate(&repo.updated_at, date_width)
                };
                let name_width = width.saturating_sub(1 + date_width);
                let name = truncate(&repo.name, name_width);
                let pad_width = width.saturating_sub(1 + name.len() + updated_at.len());
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
