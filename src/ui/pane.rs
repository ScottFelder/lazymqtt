use super::common::*;
use crate::app::App;
use crate::plugin::PaneStyle;
use crate::theme::Palette;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub(crate) fn draw_plugin_pane(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let Some(view) = app.plugins.pane(app.pane_plugin) else {
        let p = Paragraph::new("This plugin has no pane (or was disabled).")
            .style(Style::default().fg(pal.dim))
            .block(title_block("Plugin", pal));
        f.render_widget(p, area);
        return;
    };

    let lines: Vec<Line> = view
        .lines
        .iter()
        .map(|spans| {
            Line::from(
                spans
                    .iter()
                    .map(|s| Span::styled(s.text.clone(), style_for(s.style, pal)))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    let p = Paragraph::new(lines).block(title_block(&view.title, pal));
    f.render_widget(p, area);
}

/// Map a pane span's semantic style to a theme color.
fn style_for(style: PaneStyle, pal: &Palette) -> Style {
    match style {
        PaneStyle::Header => Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        PaneStyle::Label => Style::default().fg(pal.dim),
        PaneStyle::Value => Style::default().fg(pal.text),
        PaneStyle::Accent => Style::default().fg(pal.accent),
        PaneStyle::Good => Style::default().fg(pal.ok),
        PaneStyle::Warn => Style::default().fg(pal.warn),
        PaneStyle::Bad => Style::default().fg(pal.error),
        PaneStyle::Muted => Style::default().fg(pal.dim),
    }
}
