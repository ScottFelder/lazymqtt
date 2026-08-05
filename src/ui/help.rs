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
    let sections = app.plugins.help_sections();
    let count = 1 + sections.len();
    let active = app.help_section.min(count - 1);

    // A key/description row (highlighted key + text); an empty key is a note.
    let kv = |key: &str, text: &str| -> Line<'static> {
        if key.is_empty() {
            Line::from(Span::styled(
                format!("  {text}"),
                Style::default().fg(pal.dim),
            ))
        } else {
            Line::from(vec![
                Span::styled(
                    format!("  {key:<8}"),
                    Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(text.to_string(), Style::default().fg(pal.text)),
            ])
        }
    };

    // Tab bar: "Keybindings" plus one tab per enabled plugin, numbered for the
    // 1-9 jump keys, with the active tab highlighted.
    let mut titles: Vec<String> = vec!["Keybindings".to_string()];
    titles.extend(sections.iter().map(|s| s.metadata.name.to_string()));
    let mut tab_spans: Vec<Span> = Vec::new();
    for (i, t) in titles.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::styled("   ", Style::default().fg(pal.dim)));
        }
        let style = if i == active {
            Style::default()
                .fg(pal.accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(pal.dim)
        };
        tab_spans.push(Span::styled(format!("{} {}", i + 1, t), style));
    }

    let mut text: Vec<Line> = vec![Line::from(tab_spans), Line::from("")];

    if active == 0 {
        text.extend(core_help_lines());
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            "Plugins (P to enable/disable — each enabled one has a Help tab above):",
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        )));
        for e in app.plugins.entries() {
            let mark = if e.enabled { "[x]" } else { "[ ]" };
            let name_color = if e.enabled { pal.text } else { pal.dim };
            text.push(Line::from(vec![
                Span::styled(
                    format!("  {} {:<18}", mark, e.metadata.name),
                    Style::default().fg(name_color),
                ),
                Span::styled(
                    e.metadata.description.to_string(),
                    Style::default().fg(pal.dim),
                ),
            ]));
        }
    } else {
        let sec = &sections[active - 1];
        text.push(Line::from(vec![
            Span::styled(
                sec.metadata.name.to_string(),
                Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  v{}", sec.metadata.version),
                Style::default().fg(pal.dim),
            ),
        ]));
        text.push(Line::from(Span::styled(
            sec.metadata.description.to_string(),
            Style::default().fg(pal.dim),
        )));
        text.push(Line::from(""));
        if sec.lines.is_empty() {
            text.push(kv(
                "",
                "No extra keys — open this plugin from the command menu (m).",
            ));
        } else {
            for &(k, t) in sec.lines {
                text.push(kv(k, t));
            }
        }
    }

    text.push(Line::from(""));
    let footer = if count > 1 {
        "Tab / ←→  switch section    ·    1-9  jump    ·    any other key  close"
    } else {
        "Press any key to return."
    };
    text.push(Line::from(Span::styled(
        footer,
        Style::default().fg(pal.dim),
    )));

    let p = Paragraph::new(text).block(title_block("Help", pal));
    f.render_widget(p, area);
}

/// The core (non-plugin) keybindings, shown on the first help tab.
fn core_help_lines() -> Vec<Line<'static>> {
    vec![
        Line::from("Connections screen:"),
        Line::from("  ↑/↓ or j/k   move   ·   Enter   connect"),
        Line::from("  n            new    ·   e       edit"),
        Line::from("  d            delete ·   q       quit"),
        Line::from(""),
        Line::from("Connection form:"),
        Line::from("  Tab/↑↓       switch field   ·   space  toggle TLS"),
        Line::from("  space/→      on Subscriptions: open the sub-list"),
        Line::from("  Enter        save            ·   Esc    cancel"),
        Line::from("  Subscriptions list: a add · e edit · d delete (each has topic + QoS)"),
        Line::from(""),
        Line::from("Broker screen:"),
        Line::from("  m            command menu (scroll + Enter; ←/→ or h/l cycle options)"),
        Line::from("  ↑/↓ or j/k   move in tree   ·   Enter    expand/collapse"),
        Line::from("  →            expand         ·   ←        collapse"),
        Line::from("  E            expand subtree ·   C        collapse subtree"),
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
    ]
}
