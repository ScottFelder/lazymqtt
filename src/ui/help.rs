use super::common::*;
use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub(crate) fn draw_help(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let mut text = vec![
        Line::from(Span::styled(
            "LazyMQTT — Keybindings",
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Connections screen:"),
        Line::from("  ↑/↓ or j/k   move   ·   Enter   connect"),
        Line::from("  n            new    ·   e       edit"),
        Line::from("  d            delete ·   q       quit"),
        Line::from(""),
        Line::from("Connection form:"),
        Line::from("  Tab/↑↓       switch field   ·   space  toggle TLS"),
        Line::from("  Enter        save            ·   Esc    cancel"),
        Line::from(""),
        Line::from("Broker screen:"),
        Line::from("  m            command menu (scroll + Enter; ←/→ or h/l cycle options)"),
        Line::from("  ↑/↓ or j/k   move in tree   ·   Enter    expand/collapse"),
        Line::from("  →            expand         ·   ←        collapse"),
        Line::from("  E            expand all     ·   C        collapse all"),
        Line::from("  Tab          cycle panes"),
        Line::from("  1            focus Topics   ·   2        focus Payload"),
        Line::from("  3            focus History"),
        Line::from("  s            subscribe      ·   u        unsubscribe (selected)"),
        Line::from("  p            publish        ·   c        clear tree"),
        Line::from("  x            clear selected topic (from view) ·  r  clear retained msg"),
        Line::from("  A            edit alert rules (per connection) · P  plugins"),
        Line::from("  S            edit JSON schemas (per connection)"),
        Line::from("  R            recordings — replay, edit, rename, delete"),
        Line::from("  T            theme — pick a preset or edit each color"),
        Line::from("  Esc          disconnect     ·   ?        this help"),
        Line::from(""),
        Line::from("Payload & History panes (Tab or 2/3 to focus):"),
        Line::from("  ↑/↓/←/→ or hjkl   move cursor   ·   v   start/extend selection"),
        Line::from("  y                 yank to clipboard (whole line if no selection)"),
        Line::from("  Esc               clear selection"),
        Line::from("  z                 collapse/expand this pane (the other fills the space)"),
        Line::from("  i                 cycle Payload view (raw ↔ plugin views, e.g. JSON)"),
        Line::from("  Enter (History)   expand/collapse the entry under the cursor"),
        Line::from(""),
        Line::from("Clipboard:"),
        Line::from("  Paste into any input field, incl. the publish payload."),
        Line::from(""),
    ];

    text.push(Line::from("Plugins (press P to enable/disable):"));
    for meta in app.plugins.metadata() {
        text.push(Line::from(vec![
            Span::styled(format!("  {} ", meta.name), Style::default().fg(pal.text)),
            Span::styled(format!("v{}", meta.version), Style::default().fg(pal.dim)),
            Span::styled(
                format!("  — {}", meta.description),
                Style::default().fg(pal.dim),
            ),
        ]));
    }
    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        "Press any key to return.",
        Style::default().fg(pal.dim),
    )));

    let p = Paragraph::new(text).block(title_block("Help", pal));
    f.render_widget(p, area);
}
