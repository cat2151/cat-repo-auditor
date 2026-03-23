use crate::{
    app::App,
    ui::{build_detail_items, Focus, MK_BG, MK_BG_DIM, MK_BG_SEL, MK_BLUE, MK_COMMENT, MK_CYAN,
         MK_FG, MK_GREEN, MK_ORANGE, MK_PURPLE, MK_RED, MK_YELLOW, truncate, window_color},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub(crate) const CARGO_OLD_BOX_H: u16 = 4;
pub(crate) const LOCAL_CHANGES_BOX_H: u16 = 3;

fn c(app: &App, color: ratatui::style::Color) -> ratatui::style::Color {
    window_color(app.window_focused, color)
}

// ── right pane ───────────────────────────────────────────────────────────────

pub(crate) fn draw_right(f: &mut Frame, app: &mut App, area: Rect) {
    let active = app.window_focused && app.focus == Focus::Detail;
    let border_col = if active { MK_GREEN } else { MK_COMMENT };

    if app.repos.is_empty() {
        f.render_widget(
            Block::default().title(" Detail ").borders(Borders::ALL)
                .border_style(Style::default().fg(c(app, MK_COMMENT)))
                .style(Style::default().bg(c(app, MK_BG))),
            area,
        );
        return;
    }

    let repo_idx = match app.selected_repo_idx() {
        Some(i) => i,
        None => {
            f.render_widget(Block::default().borders(Borders::ALL).style(Style::default().bg(c(app, MK_BG))), area);
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
        .border_style(Style::default().fg(c(app, border_col)))
        .style(Style::default().bg(c(app, MK_BG)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // ── layout within right pane ─────────────────────────────────────────────
    // summary(1) + hint(1) + issue/PR section + staging section
    let has_staging = !staging_files.is_empty();
    let staging_height = if has_staging {
        (staging_files.len() + 2).min(inner.height as usize / 3) as u16
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
            .style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))),
        vert[0],
    );
    // hint
    let hint = if active { " h:back  j/k:move  PgUp/PgDn  Enter:open" } else { " l:focus  g:lazygit" };
    f.render_widget(
        Paragraph::new(hint).style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG)).add_modifier(Modifier::DIM)),
        vert[1],
    );

    // ── issue/PR list ─────────────────────────────────────────────────────────
    let list_area = vert[2];
    let items = build_detail_items(repo);

    if items.is_empty() {
        f.render_widget(
            Paragraph::new("  (no open issues or PRs)").style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))),
            list_area,
        );
    } else {
        let visible = list_area.height as usize;
        app.right_visible = visible;
        app.adjust_detail_scroll(visible);
        let d_scroll = app.detail_scroll;
        let d_sel    = app.detail_selected;

        let lines: Vec<Line> = items.iter()
            .enumerate()
            .skip(d_scroll)
            .take(visible)
            .map(|(i, item)| {
                let sel = active && i == d_sel;
                let bg  = c(app, if sel { MK_BG_SEL } else { MK_BG });
                let indent    = if item.is_child { " " } else { "" };
                let connector = if item.is_child { "└─" } else { "  " };
                let (label, label_col) = if item.is_pr { (" PR", MK_PURPLE) } else { ("ISS", MK_RED) };
                let prefix_len = if item.is_child { 4usize } else { 3usize };
                let max_title = list_area.width.saturating_sub(28 + prefix_len as u16) as usize;
                Line::from(vec![
                    Span::styled(format!("{}{} ", indent, connector), Style::default().fg(c(app, MK_COMMENT)).bg(bg)),
                    Span::styled(format!("{} ", label), Style::default().fg(c(app, label_col)).bg(bg).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("#{:<4} ", item.number), Style::default().fg(c(app, MK_COMMENT)).bg(bg)),
                    Span::styled(truncate(&item.title, max_title), Style::default().fg(c(app, MK_FG)).bg(bg)),
                    Span::styled(format!("  {}", item.updated), Style::default().fg(c(app, if sel { MK_CYAN } else { MK_COMMENT })).bg(bg)),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(lines).style(Style::default().bg(c(app, MK_BG))), list_area);

        // position indicator
        if items.len() > visible && visible > 0 {
            let txt = format!(" {}/{} ", d_sel + 1, items.len());
            let w = (txt.len() as u16).min(list_area.width);
            f.render_widget(
                Paragraph::new(txt).style(Style::default().fg(c(app, MK_COMMENT)).bg(c(app, MK_BG))),
                Rect { x: list_area.x + list_area.width.saturating_sub(w), y: list_area.y + list_area.height.saturating_sub(1), width: w, height: 1 },
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
                    Span::styled(format!(" {} ", status), Style::default().fg(c(app, status_col)).bg(c(app, MK_BG)).add_modifier(Modifier::BOLD)),
                    Span::styled(rest.to_string(), Style::default().fg(c(app, MK_FG)).bg(c(app, MK_BG))),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(stag_lines).style(Style::default().bg(c(app, MK_BG))), stag_inner);
    }
}

// ── cargo old comparison box ──────────────────────────────────────────────────

pub(crate) fn draw_cargo_old_box(
    f: &mut Frame, app: &App, repo_idx: usize, area: Rect, bottom_offset: u16,
) {
    let repo = &app.repos[repo_idx];
    let inst  = if repo.cargo_installed_hash.is_empty() { "?" } else { &repo.cargo_installed_hash };
    // cargo_checked_at stores the local HEAD hash used in the last comparison
    let local = if repo.cargo_checked_at.is_empty()     { "?" } else { &repo.cargo_checked_at };

    // Inner content width: " installed: " (12) + hash up to 40 chars + 1 padding = 53
    // Box width (including borders): 53 + 2 = 55
    let content_w: u16 = 53;
    let box_w = content_w + 2; // +2 for left/right borders
    let box_h: u16 = CARGO_OLD_BOX_H; // top border + 2 lines + bottom border

    // Place in bottom-right, above the bottom status bar (outer[2] is 1 line tall)
    let x = area.x + area.width.saturating_sub(box_w + 1);
    let y = area.y + area.height.saturating_sub(box_h + 1 + bottom_offset);
    let rect = Rect { x, y, width: box_w.min(area.width), height: box_h.min(area.height) };

    f.render_widget(Clear, rect);
    let block = Block::default()
        .title(" cgo old: commit hash ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(c(app, MK_ORANGE)))
        .style(Style::default().bg(c(app, MK_BG)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let label_w: u16 = 12; // length of " installed: " / "     local: " labels
    let max_w = inner.width.saturating_sub(label_w) as usize;
    let lines = vec![
        Line::from(vec![
            Span::styled(" installed: ", Style::default().fg(c(app, MK_COMMENT))),
            Span::styled(truncate(inst,  max_w), Style::default().fg(c(app, MK_ORANGE))),
        ]),
        Line::from(vec![
            Span::styled("     local: ", Style::default().fg(c(app, MK_COMMENT))),
            Span::styled(truncate(local, max_w), Style::default().fg(c(app, MK_GREEN))),
        ]),
    ];
    f.render_widget(Paragraph::new(lines).style(Style::default().bg(c(app, MK_BG))), inner);
}

pub(crate) fn draw_local_staging_box(f: &mut Frame, app: &App, repo_idx: usize, area: Rect, bottom_offset: u16) {
    let repo = &app.repos[repo_idx];
    let local_changes_count = repo.staging_files.len();

    let content_w: u16 = 38;
    let box_w = content_w + 2;
    let box_h: u16 = LOCAL_CHANGES_BOX_H;

    let x = area.x + area.width.saturating_sub(box_w + 1);
    let y = area.y + area.height.saturating_sub(box_h + 1 + bottom_offset);
    let rect = Rect { x, y, width: box_w.min(area.width), height: box_h.min(area.height) };

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

pub(crate) fn draw_help_dialog(f: &mut Frame, _app: &App, area: Rect) {
    // Center a dialog box
    let dw: u16 = 62;
    let dh: u16 = 30;
    let x = area.x + area.width.saturating_sub(dw) / 2;
    let y = area.y + area.height.saturating_sub(dh) / 2;
    let dialog = Rect { x, y, width: dw.min(area.width), height: dh.min(area.height) };

    // Clear the area first to prevent bleed-through from background
    f.render_widget(Clear, dialog);
    f.render_widget(
        Paragraph::new("").style(Style::default().bg(c(_app, MK_BG_DIM))),
        dialog,
    );

    let block = Block::default()
        .title(" Help  (? or Esc to close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(c(_app, MK_CYAN)))
        .style(Style::default().bg(c(_app, MK_BG_DIM)));

    let inner = block.inner(dialog);
    f.render_widget(block, dialog);

    let lines: Vec<Line> = vec![
        Line::from(Span::styled(" Keybinds", Style::default().fg(c(_app, MK_YELLOW)).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  q        ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Quit"),
        ]),
        Line::from(vec![
            Span::styled("  F5       ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Refresh from GitHub"),
        ]),
        Line::from(vec![
            Span::styled("  j / k    ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Move cursor down / up  (Nj = N lines)"),
        ]),
        Line::from(vec![
            Span::styled("  PgDn/PgUp", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Page down / up"),
        ]),
        Line::from(vec![
            Span::styled("  h / l    ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Focus left pane / right pane"),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Open README.ja.md (or repo root)"),
        ]),
        Line::from(vec![
            Span::styled("  i        ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Open GitHub Pages (or repo root)"),
        ]),
        Line::from(vec![
            Span::styled("  w        ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Open DeepWiki page"),
        ]),
        Line::from(vec![
            Span::styled("  g        ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Open lazygit for this repo"),
        ]),
        Line::from(vec![
            Span::styled("  x        ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Run installed app (cgo=ok only)"),
        ]),
        Line::from(vec![
            Span::styled("  Shift+L  ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Toggle log pane (bottom half)"),
        ]),
        Line::from(vec![
            Span::styled("  c        ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Copy local repo path to clipboard"),
        ]),
        Line::from(vec![
            Span::styled("  d        ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Toggle doc/pg/ja/wki/wf/cgo columns"),
        ]),
        Line::from(vec![
            Span::styled("  /        ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Incremental search  (Space=AND)"),
        ]),
        Line::from(vec![
            Span::styled("  ^G / ^T  ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("Next / prev match  (during search)"),
        ]),
        Line::from(vec![
            Span::styled("  (right pane) Enter", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("  Open issue / PR in browser"),
        ]),
        Line::from(vec![
            Span::styled("  (right pane) h   ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("  Back to repo list"),
        ]),
        Line::from(""),
        Line::from(Span::styled(" Column legend", Style::default().fg(c(_app, MK_YELLOW)).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  doc ", Style::default().fg(c(_app, MK_GREEN))),
            Span::raw("README.ja.md exists in repo"),
        ]),
        Line::from(vec![
            Span::styled("  pg  ", Style::default().fg(c(_app, MK_CYAN))),
            Span::raw("GitHub Pages is enabled"),
        ]),
        Line::from(vec![
            Span::styled("  ja  ", Style::default().fg(c(_app, MK_YELLOW))),
            Span::raw("README.ja.md has a self-link badge"),
        ]),
        Line::from(vec![
            Span::styled("  wki ", Style::default().fg(c(_app, MK_PURPLE))),
            Span::raw("README contains a deepwiki.com link"),
        ]),
        Line::from(vec![
            Span::styled("  wf  ", Style::default().fg(c(_app, MK_GREEN))),
            Span::raw(".github/workflows has 3 required ymls"),
        ]),
        Line::from(vec![
            Span::styled("  cgo ", Style::default().fg(c(_app, MK_GREEN))),
            Span::raw("ok=cargo install HEAD matches local"),
        ]),
        Line::from(vec![
            Span::styled("      ", Style::default().fg(c(_app, MK_ORANGE))),
            Span::raw("old=installed hash differs from HEAD"),
        ]),
    ];

    f.render_widget(Paragraph::new(lines).style(Style::default().bg(c(_app, MK_BG_DIM))), inner);
}
