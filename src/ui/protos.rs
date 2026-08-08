use super::common::*;
use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
    Frame,
};

pub(crate) fn draw_protos(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let items: Vec<ListItem> = app
        .proto_mappings
        .iter()
        .map(|m| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<26}", m.topic), Style::default().fg(pal.text)),
                Span::styled(m.summary(), Style::default().fg(pal.dim)),
            ]))
        })
        .collect();

    let title = format!("Protobuf Schemas — {}", app.proto_edit_name);
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
    if !app.proto_mappings.is_empty() {
        state.select(Some(app.protos_selected.min(app.proto_mappings.len() - 1)));
    }
    f.render_stateful_widget(list, area, &mut state);

    if app.proto_mappings.is_empty() {
        let hint = Paragraph::new("No protobuf schemas for this connection. Press 'a' to add one.")
            .style(Style::default().fg(pal.dim))
            .alignment(Alignment::Center);
        f.render_widget(hint, center_rect(area, 70, 1));
    }
}

pub(crate) fn draw_proto_form(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let form = &app.proto_form;
    let title = if form.editing_index.is_some() {
        "Edit Protobuf Schema"
    } else {
        "New Protobuf Schema"
    };

    let block = title_block(title, pal);
    let inner = block.inner(area);
    f.render_widget(block, area);

    // topic · message-type · blank · ".proto" label · body · footer/error
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    text_field(f, rows[0], "Topic   ", &form.topic, form.focus == 0, pal);
    text_field(
        f,
        rows[1],
        "Message ",
        &form.message_type,
        form.focus == 1,
        pal,
    );

    // ".proto" label, accented when the editor has focus.
    let body_focused = form.focus == 2;
    f.render_widget(
        Paragraph::new(Span::styled(
            if body_focused {
                "▶ .proto"
            } else {
                "  .proto"
            },
            if body_focused {
                Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(pal.text)
            },
        )),
        rows[3],
    );
    draw_textarea(f, rows[4], &form.body, body_focused, pal);

    let footer = match &form.error {
        Some(e) => Span::styled(format!("⚠ {e}"), Style::default().fg(pal.error)),
        None => Span::styled(
            "Tab: switch field · ^S: save · Esc: cancel",
            Style::default().fg(pal.dim),
        ),
    };
    f.render_widget(Paragraph::new(Line::from(footer)), rows[5]);
}

/// A single-line labeled text field with a focus caret.
fn text_field(
    f: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    focused: bool,
    pal: &crate::theme::Palette,
) {
    let marker = if focused { "▶ " } else { "  " };
    let caret = if focused { "_" } else { "" };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(marker.to_string(), Style::default().fg(pal.accent)),
            Span::styled(
                label.to_string(),
                if focused {
                    Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(pal.text)
                },
            ),
            Span::styled(value.to_string(), Style::default().fg(pal.text)),
            Span::styled(caret.to_string(), Style::default().fg(pal.accent)),
        ])),
        area,
    );
}
