use super::common::*;
use crate::app::App;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

pub(crate) fn draw_connections(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let items: Vec<ListItem> = app
        .config
        .connections
        .iter()
        .map(|c| {
            let tls = if c.tls { " 🔒" } else { "" };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<20}", c.name), Style::default().fg(pal.text)),
                Span::styled(
                    format!("{}:{}{}", c.host, c.port, tls),
                    Style::default().fg(pal.dim),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(title_block("LazyMQTT — Connections", pal))
        .highlight_style(
            Style::default()
                .bg(pal.accent)
                .fg(pal.selection_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    if !app.config.connections.is_empty() {
        state.select(Some(app.conn_selected));
    }
    f.render_stateful_widget(list, area, &mut state);

    if app.config.connections.is_empty() {
        let hint = Paragraph::new("No connections yet. Press 'n' to create one.")
            .style(Style::default().fg(pal.dim))
            .alignment(Alignment::Center);
        let inner = center_rect(area, 60, 1);
        f.render_widget(hint, inner);
    }
}
pub(crate) fn draw_form(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let form = &app.form;
    let title = if form.editing_index.is_some() {
        "Edit Connection"
    } else {
        "New Connection"
    };
    let fields = [
        ("Name", form.name.as_str()),
        ("Host", form.host.as_str()),
        ("Port", form.port.as_str()),
        ("Client ID", form.client_id.as_str()),
        ("Username", form.username.as_str()),
        ("Password", "••••••"),
        (
            "TLS",
            if form.tls {
                "[x] enabled  (space to toggle)"
            } else {
                "[ ] disabled (space to toggle)"
            },
        ),
        ("Topics", form.topics.as_str()),
    ];

    let mut lines = Vec::new();
    for (i, (label, val)) in fields.iter().enumerate() {
        let focused = i == form.field;
        let marker = if focused { "▶ " } else { "  " };
        let label_style = if focused {
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(pal.text)
        };
        let shown = if i == 5 && form.password.is_empty() {
            ""
        } else {
            *val
        };
        let cursor = if focused && i != 6 { "_" } else { "" };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(pal.accent)),
            Span::styled(format!("{:<11}", label), label_style),
            Span::raw(shown.to_string()),
            Span::styled(cursor, Style::default().fg(pal.accent)),
        ]));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "Topics: comma-separated (e.g. sensors/#, home/+/temp)",
        Style::default().fg(pal.dim),
    )));

    let p = Paragraph::new(lines)
        .block(title_block(title, pal))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
