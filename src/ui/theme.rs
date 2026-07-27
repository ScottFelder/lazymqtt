use super::common::*;
use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
    Frame,
};

pub(crate) fn draw_theme(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let builtins = crate::theme::builtins();
    let mut items: Vec<ListItem> = Vec::new();

    // Preset rows (apply a whole palette at once).
    for (name, _) in &builtins {
        items.push(ListItem::new(Line::from(vec![
            Span::styled("◆ ", Style::default().fg(pal.accent)),
            Span::styled(format!("{name} preset"), Style::default().fg(pal.text)),
        ])));
    }
    // One row per color role: label, a swatch in the role's color, and its spec.
    for (i, (_key, label)) in crate::theme::ROLES.iter().enumerate() {
        let editing = app.theme_selected_role() == Some(i) && app.theme_edit.is_some();
        let spec = if editing {
            app.theme_edit.clone().unwrap_or_default()
        } else {
            app.theme.spec(i).to_string()
        };
        let swatch = crate::theme::parse_color(&spec);
        let value = if editing {
            (
                format!("{spec}_"),
                Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
            )
        } else {
            (spec, Style::default().fg(pal.dim))
        };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{label:<26}"), Style::default().fg(pal.text)),
            Span::styled("███  ", Style::default().fg(swatch)),
            Span::styled(value.0, value.1),
        ])));
    }

    let list = List::new(items)
        .block(title_block(
            "Theme — Enter: apply preset / edit color · changes save automatically",
            pal,
        ))
        .highlight_style(
            Style::default()
                .bg(pal.accent)
                .fg(pal.selection_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let total = app.theme_row_count();
    let mut state = ListState::default();
    if total > 0 {
        state.select(Some(app.theme_selected.min(total - 1)));
    }
    f.render_stateful_widget(list, area, &mut state);
    render_vscrollbar(f, area, total, state.offset(), pal);
}
