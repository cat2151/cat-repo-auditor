use crate::{
    app::App,
    ui::{MK_BG_DIM, MK_CYAN, MK_GREEN, MK_ORANGE, MK_PURPLE, MK_YELLOW},
};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::{c, centered_rect};

/// Draws the help dialog overlay with keybindings and column legend.
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
            Span::raw("ok=installed hash matches remote HEAD"),
        ]),
        Line::from(vec![
            Span::styled("      ", Style::default().fg(c(app, MK_ORANGE))),
            Span::raw("old=installed differs, ?=check failed"),
        ]),
    ];

    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(c(app, MK_BG_DIM))),
        inner,
    );
}
