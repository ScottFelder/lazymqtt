use super::common::*;
use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState},
    Frame,
};

pub(crate) fn draw_command_menu(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let popup = center_rect(area, 66, 72);
    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = app
        .menu_items
        .iter()
        .map(|it| {
            let mut spans = vec![
                Span::styled(
                    format!(" {:<4}", it.key),
                    Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(it.label.clone(), Style::default().fg(pal.text)),
            ];
            if !it.note.is_empty() {
                spans.push(Span::styled(
                    format!("  ({})", it.note),
                    Style::default().fg(pal.dim),
                ));
            }
            // Hint that this row cycles through options with ←/→ (or h/l).
            if it.adjustable {
                spans.push(Span::styled("  ‹ ›", Style::default().fg(pal.dim)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(title_block("Commands", pal))
        .highlight_style(
            Style::default()
                .bg(pal.accent)
                .fg(pal.selection_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    if !app.menu_items.is_empty() {
        state.select(Some(app.menu_selected.min(app.menu_items.len() - 1)));
    }
    f.render_stateful_widget(list, popup, &mut state);
}
