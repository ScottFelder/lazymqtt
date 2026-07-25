use super::common::*;
use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
    Frame,
};

pub(crate) fn draw_plugins(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let entries = app.plugins.entries();
    let items: Vec<ListItem> = entries
        .iter()
        .map(|e| {
            let (check, check_color) = if e.enabled {
                ("[x]", pal.ok)
            } else {
                ("[ ]", pal.dim)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", check), Style::default().fg(check_color)),
                Span::styled(
                    format!("{:<16}", e.metadata.name),
                    Style::default().fg(pal.text),
                ),
                Span::styled(
                    format!("v{}  ", e.metadata.version),
                    Style::default().fg(pal.dim),
                ),
                Span::styled(
                    e.metadata.description.to_string(),
                    Style::default().fg(pal.dim),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(title_block("Plugins — space to toggle · Esc to close", pal))
        .highlight_style(
            Style::default()
                .bg(pal.accent)
                .fg(pal.selection_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    if !entries.is_empty() {
        state.select(Some(app.plugins_selected.min(entries.len() - 1)));
    }
    f.render_stateful_widget(list, area, &mut state);
}
