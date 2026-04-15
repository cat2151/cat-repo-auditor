use super::{c, App, MK_BG, MK_YELLOW};
use ratatui::{
    layout::Constraint,
    style::{Modifier, Style},
    widgets::{Cell, Row},
};

fn header_cell(app: &App, label: &'static str) -> Cell<'static> {
    Cell::from(label).style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(c(app, MK_YELLOW)),
    )
}

pub(super) fn build_header(app: &App) -> Row<'static> {
    let cells = if app.show_columns {
        vec![
            header_cell(app, "Repository"),
            header_cell(app, "Updated"),
            header_cell(app, "PR"),
            header_cell(app, "ISS"),
            header_cell(app, "doc"),
            header_cell(app, "pg"),
            header_cell(app, "ja"),
            header_cell(app, "wki"),
            header_cell(app, "wf"),
            header_cell(app, "Local"),
            header_cell(app, "cgo"),
        ]
    } else {
        vec![
            header_cell(app, "Repository"),
            header_cell(app, "Updated"),
            header_cell(app, "PR"),
            header_cell(app, "ISS"),
        ]
    };
    Row::new(cells).style(Style::default().bg(c(app, MK_BG)))
}

pub(super) fn column_widths(show_columns: bool) -> Vec<Constraint> {
    if show_columns {
        vec![
            Constraint::Min(18),
            Constraint::Length(7),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Length(4),
        ]
    } else {
        vec![
            Constraint::Min(0),
            Constraint::Length(7),
            Constraint::Length(4),
            Constraint::Length(4),
        ]
    }
}
