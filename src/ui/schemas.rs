use super::common::*;
use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
    Frame,
};

pub(crate) fn draw_schemas(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let items: Vec<ListItem> = app
        .schemas
        .iter()
        .map(|m| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<26}", m.topic), Style::default().fg(pal.text)),
                Span::styled(m.summary(), Style::default().fg(pal.dim)),
            ]))
        })
        .collect();

    let title = format!("JSON Schemas — {}", app.schema_edit_name);
    let list = List::new(items)
        .block(title_block(&title, pal))
        .highlight_style(
            Style::default()
                .bg(pal.accent)
                .fg(pal.selection_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    if !app.schemas.is_empty() {
        state.select(Some(app.schemas_selected.min(app.schemas.len() - 1)));
    }
    f.render_stateful_widget(list, area, &mut state);

    if app.schemas.is_empty() {
        let hint = Paragraph::new("No schemas for this connection. Press 'a' to add one.")
            .style(Style::default().fg(pal.dim))
            .alignment(Alignment::Center);
        f.render_widget(hint, center_rect(area, 70, 1));
    }
}

pub(crate) fn draw_schema_form(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let form = &app.schema_form;
    let title = if form.editing_index.is_some() {
        "Edit Schema"
    } else {
        "New Schema"
    };

    let block = title_block(title, pal);
    let inner = block.inner(area);
    f.render_widget(block, area);

    // topic line · blank · "Schema (JSON)" label · body · footer/error
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let topic_focused = form.focus == 0;
    let body_focused = form.focus == 1;

    // Topic field.
    let topic_marker = if topic_focused { "▶ " } else { "  " };
    let caret = if topic_focused { "_" } else { "" };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(topic_marker, Style::default().fg(pal.accent)),
            Span::styled(
                "Topic  ",
                if topic_focused {
                    Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(pal.text)
                },
            ),
            Span::styled(form.topic.clone(), Style::default().fg(pal.text)),
            Span::styled(caret, Style::default().fg(pal.accent)),
        ])),
        rows[0],
    );

    // "Schema (JSON)" label, accented when the editor has focus.
    f.render_widget(
        Paragraph::new(Span::styled(
            if body_focused {
                "▶ Schema (JSON)"
            } else {
                "  Schema (JSON)"
            },
            if body_focused {
                Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(pal.text)
            },
        )),
        rows[2],
    );

    // The JSON editor body.
    draw_textarea(f, rows[3], &form.body, body_focused, pal);

    // Footer: hint or the validation error.
    let footer = match &form.error {
        Some(e) => Span::styled(format!("⚠ {e}"), Style::default().fg(pal.error)),
        None => Span::styled(
            "Tab: switch field · ^S: save · Esc: cancel",
            Style::default().fg(pal.dim),
        ),
    };
    f.render_widget(Paragraph::new(Line::from(footer)), rows[4]);
}
