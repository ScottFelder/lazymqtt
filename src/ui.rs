use crate::app::{App, DetailKind, DetailLine, Focus, PublishBuffer, Screen, Status};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Frame,
};

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    match app.screen {
        Screen::Connections => draw_connections(f, app, chunks[0]),
        Screen::ConnectionForm => draw_form(f, app, chunks[0]),
        Screen::Broker => draw_broker(f, app, chunks[0]),
        Screen::Publish => {
            draw_broker(f, app, chunks[0]);
            draw_publish(f, &app.publish, chunks[0]);
        }
        Screen::Subscribe => {
            draw_broker(f, app, chunks[0]);
            draw_subscribe(f, &app.sub_input, chunks[0]);
        }
        Screen::Help => draw_help(f, chunks[0]),
    }

    draw_statusbar(f, app, chunks[1]);
}

fn title_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            format!(" {} ", title),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
}

/// Draw a vertical scrollbar along the right inner edge of `area` (a
/// bordered pane), but only when `total` items/lines exceed the visible
/// viewport — otherwise the whole pane content already fits and no
/// scrollbar should be shown.
fn render_vscrollbar(f: &mut Frame, area: Rect, total: usize, position: usize) {
    let viewport = area.height.saturating_sub(2) as usize;
    if viewport == 0 || total <= viewport {
        return;
    }
    let max_pos = total.saturating_sub(viewport);
    let mut state = ScrollbarState::new(max_pos).position(position.min(max_pos));
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .style(Style::default().fg(DIM))
        .thumb_style(Style::default().fg(ACCENT));
    f.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut state,
    );
}

/// A lazygit-style pane title: a dim `[n]` number prefix (the key that jumps
/// to this pane) followed by the accent-colored pane name.
fn pane_title(number: &str, name: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" [{}] ", number), Style::default().fg(DIM)),
        Span::styled(
            name.to_string(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ])
}

fn draw_connections(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .config
        .connections
        .iter()
        .map(|c| {
            let tls = if c.tls { " 🔒" } else { "" };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<20}", c.name), Style::default().fg(Color::White)),
                Span::styled(
                    format!("{}:{}{}", c.host, c.port, tls),
                    Style::default().fg(DIM),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(title_block("LazyMQTT — Connections"))
        .highlight_style(
            Style::default()
                .bg(ACCENT)
                .fg(Color::Black)
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
            .style(Style::default().fg(DIM))
            .alignment(Alignment::Center);
        let inner = center_rect(area, 60, 1);
        f.render_widget(hint, inner);
    }
}

fn draw_form(f: &mut Frame, app: &App, area: Rect) {
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
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let shown = if i == 5 && form.password.is_empty() {
            ""
        } else {
            *val
        };
        let cursor = if focused && i != 6 { "_" } else { "" };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(ACCENT)),
            Span::styled(format!("{:<11}", label), label_style),
            Span::raw(shown.to_string()),
            Span::styled(cursor, Style::default().fg(ACCENT)),
        ]));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "Topics: comma-separated (e.g. sensors/#, home/+/temp)",
        Style::default().fg(DIM),
    )));

    let p = Paragraph::new(lines)
        .block(title_block(title))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_broker(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // Left: topic tree
    let rows = app.tree.rows(&app.expanded);
    let items: Vec<ListItem> = rows
        .iter()
        .map(|r| {
            let indent = "  ".repeat(r.depth);
            let toggle = if r.has_children {
                if r.expanded {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "• "
            };
            let mut spans = vec![
                Span::raw(indent),
                Span::styled(toggle, Style::default().fg(ACCENT)),
                Span::styled(r.label.clone(), Style::default().fg(Color::White)),
            ];
            if r.count > 0 {
                spans.push(Span::styled(
                    format!("  ({})", r.count),
                    Style::default().fg(DIM),
                ));
            }
            if let Some(v) = &r.value {
                let preview: String = v.chars().take(24).collect();
                spans.push(Span::styled(
                    format!("  = {}", preview),
                    Style::default().fg(Color::Green),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let tree_border = if app.focus == Focus::Tree {
        ACCENT
    } else {
        DIM
    };
    let tree = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(tree_border))
                .title(pane_title("1", "Topics")),
        )
        .highlight_style(Style::default().bg(tree_border).fg(Color::Black))
        .highlight_symbol("");

    let mut state = ListState::default();
    if !rows.is_empty() {
        state.select(Some(app.tree_selected.min(rows.len() - 1)));
    }
    f.render_stateful_widget(tree, cols[0], &mut state);
    render_vscrollbar(f, cols[0], rows.len(), state.offset());

    // Right: Payload (selected message) on top, History (all messages for the
    // selected topic) below.
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(cols[1]);

    draw_payload(f, app, right[0]);
    draw_history(f, app, right[1]);
}

/// Top-right pane: the payload (and metadata) of whichever message is
/// currently selected, with keyboard text selection/yank.
fn draw_payload(f: &mut Frame, app: &App, area: Rect) {
    let payload_border = if app.focus == Focus::Payload {
        ACCENT
    } else {
        DIM
    };
    let lines = app.payload_lines();

    // Only the focused pane shows a cursor/selection — the cursor state is
    // shared between panes, so an unfocused pane must render none of it.
    let show_cursor = app.focus == Focus::Payload;
    let sel = if show_cursor {
        app.sel_anchor.map(|a| {
            let b = app.sel_cursor;
            if a <= b {
                (a, b)
            } else {
                (b, a)
            }
        })
    } else {
        None
    };

    let rendered: Vec<Line> = lines
        .iter()
        .enumerate()
        .map(|(i, dl)| render_detail_line(i, dl, sel, app.sel_cursor, show_cursor))
        .collect();

    // Keep the cursor line visible (approximate: counts logical, not wrapped, rows).
    let inner_h = area.height.saturating_sub(2) as usize;
    let scroll = if show_cursor && inner_h > 0 && app.sel_cursor.0 >= inner_h {
        (app.sel_cursor.0 - inner_h + 1) as u16
    } else {
        0
    };

    let line_count = rendered.len();
    let payload = Paragraph::new(rendered)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(payload_border))
                .title(pane_title("2", "Payload")),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(payload, area);
    render_vscrollbar(f, area, line_count, scroll as usize);
}

/// Bottom-right pane: every message received for the selected topic, newest
/// first, with the same keyboard text selection/yank as the Payload pane.
/// Moving the cursor also picks which message the Payload pane shows; Enter
/// expands/collapses the entry under the cursor.
fn draw_history(f: &mut Frame, app: &App, area: Rect) {
    let history_border = if app.focus == Focus::History {
        ACCENT
    } else {
        DIM
    };
    let lines = app.history_lines();

    // Only the focused pane shows a cursor/selection — the cursor state is
    // shared between panes, so an unfocused pane must render none of it.
    let show_cursor = app.focus == Focus::History;
    let sel = if show_cursor {
        app.sel_anchor.map(|a| {
            let b = app.sel_cursor;
            if a <= b {
                (a, b)
            } else {
                (b, a)
            }
        })
    } else {
        None
    };

    let rendered: Vec<Line> = lines
        .iter()
        .enumerate()
        .map(|(i, dl)| render_detail_line(i, dl, sel, app.sel_cursor, show_cursor))
        .collect();

    let inner_h = area.height.saturating_sub(2) as usize;
    let scroll = if show_cursor && inner_h > 0 && app.sel_cursor.0 >= inner_h {
        (app.sel_cursor.0 - inner_h + 1) as u16
    } else {
        0
    };

    let line_count = rendered.len();
    let history = Paragraph::new(rendered)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(history_border))
                .title(pane_title("3", "History")),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(history, area);
    render_vscrollbar(f, area, line_count, scroll as usize);
}

/// Color for a segment's semantic kind.
fn style_for(kind: DetailKind) -> Style {
    match kind {
        DetailKind::Header => Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        DetailKind::Toggle => Style::default().fg(ACCENT),
        DetailKind::Payload => Style::default().fg(Color::Green),
        DetailKind::Meta | DetailKind::Blank => Style::default().fg(DIM),
    }
}

/// Render one detail line: its decorative lead, then each styled segment with a
/// selection highlight overlaid across the selected column range (or a block
/// cursor when no visual selection is active).
fn render_detail_line(
    i: usize,
    dl: &DetailLine,
    sel: Option<((usize, usize), (usize, usize))>,
    cursor: (usize, usize),
    show_cursor: bool,
) -> Line<'static> {
    let text_len = dl.char_len();

    // Inclusive [start, end] highlight range over the line's selectable text.
    let range: Option<(usize, usize)> = if let Some((a, b)) = sel {
        if i >= a.0 && i <= b.0 && text_len > 0 {
            let start = if i == a.0 { a.1 } else { 0 };
            let end = if i == b.0 { b.1 } else { text_len - 1 };
            Some((start.min(text_len - 1), end.min(text_len - 1)))
        } else {
            None
        }
    } else if show_cursor && i == cursor.0 && text_len > 0 {
        let c = cursor.1.min(text_len - 1);
        Some((c, c))
    } else {
        None
    };

    let hl = Style::default().bg(ACCENT).fg(Color::Black);
    let mut spans = Vec::new();
    if !dl.lead.is_empty() {
        spans.push(Span::styled(dl.lead.clone(), style_for(dl.lead_kind)));
    }

    // Walk segments, splitting each by its intersection with the highlight range.
    let mut offset = 0usize;
    for (text, kind) in &dl.segs {
        let base = style_for(*kind);
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();
        match range {
            Some((s, e)) if n > 0 && s.max(offset) <= e.min(offset + n - 1) => {
                let ls = s.max(offset) - offset;
                let le = e.min(offset + n - 1) - offset;
                if ls > 0 {
                    spans.push(Span::styled(chars[..ls].iter().collect::<String>(), base));
                }
                spans.push(Span::styled(chars[ls..=le].iter().collect::<String>(), hl));
                if le + 1 < n {
                    spans.push(Span::styled(
                        chars[le + 1..].iter().collect::<String>(),
                        base,
                    ));
                }
            }
            _ => spans.push(Span::styled(text.clone(), base)),
        }
        offset += n;
    }

    // Show a one-cell cursor when parked on an otherwise-empty line.
    if text_len == 0 && show_cursor && sel.is_none() && i == cursor.0 {
        spans.push(Span::styled(" ", hl));
    }

    Line::from(spans)
}

fn draw_publish(f: &mut Frame, pb: &PublishBuffer, area: Rect) {
    let popup = center_rect(area, 70, 40);
    f.render_widget(Clear, popup);

    let fields = [
        ("Topic", pb.topic.as_str()),
        ("Payload", pb.payload.as_str()),
        ("QoS", &["0", "1", "2"][pb.qos.min(2) as usize]),
        ("Retain", if pb.retain { "[x]" } else { "[ ]" }),
    ];
    let mut lines = Vec::new();
    for (i, (label, val)) in fields.iter().enumerate() {
        let focused = i == pb.field;
        let marker = if focused { "▶ " } else { "  " };
        let cursor = if focused && i < 2 { "_" } else { "" };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(ACCENT)),
            Span::styled(
                format!("{:<9}", label),
                if focused {
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
            ),
            Span::raw(val.to_string()),
            Span::styled(cursor, Style::default().fg(ACCENT)),
        ]));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "Tab: next field · space: toggle QoS/Retain · Enter: publish · Esc: cancel",
        Style::default().fg(DIM),
    )));

    let p = Paragraph::new(lines)
        .block(title_block("Publish Message"))
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);
}

fn draw_subscribe(f: &mut Frame, input: &str, area: Rect) {
    let popup = center_rect(area, 60, 20);
    f.render_widget(Clear, popup);
    let lines = vec![
        Line::from(vec![
            Span::styled(
                "Topic: ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(input.to_string()),
            Span::styled("_", Style::default().fg(ACCENT)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "wildcards ok: sensors/#, home/+/temp · Enter: subscribe · Esc: cancel",
            Style::default().fg(DIM),
        )),
    ];
    let p = Paragraph::new(lines).block(title_block("Subscribe to Topic"));
    f.render_widget(p, popup);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(Span::styled(
            "LazyMQTT — Keybindings",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
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
        Line::from("  ↑/↓ or j/k   move in tree   ·   Enter    expand/collapse"),
        Line::from("  →            expand         ·   ←        collapse"),
        Line::from("  Tab          cycle panes"),
        Line::from("  1            focus Topics   ·   2        focus Payload"),
        Line::from("  3            focus History"),
        Line::from("  s            subscribe      ·   u        unsubscribe (selected)"),
        Line::from("  p            publish        ·   c        clear tree"),
        Line::from("  Esc          disconnect     ·   ?        this help"),
        Line::from(""),
        Line::from("Payload & History panes (Tab or 2/3 to focus):"),
        Line::from("  ↑/↓/←/→ or hjkl   move cursor   ·   v   start/extend selection"),
        Line::from("  y                 yank to clipboard (whole line if no selection)"),
        Line::from("  Esc               clear selection"),
        Line::from("  Enter (History)   expand/collapse the entry under the cursor"),
        Line::from(""),
        Line::from("Clipboard:"),
        Line::from("  Paste into any input field, incl. the publish payload."),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to return.",
            Style::default().fg(DIM),
        )),
    ];
    let p = Paragraph::new(text).block(title_block("Help"));
    f.render_widget(p, area);
}

fn draw_statusbar(f: &mut Frame, app: &App, area: Rect) {
    let (label, color) = match &app.status {
        Status::Idle => ("● idle".to_string(), DIM),
        Status::Connecting => ("● connecting…".to_string(), Color::Yellow),
        Status::Connected => {
            let name = app
                .active_conn()
                .map(|c| c.name.clone())
                .unwrap_or_default();
            (format!("● connected: {}", name), Color::Green)
        }
        Status::Disconnected(e) => (format!("● disconnected: {}", e), Color::Red),
    };

    let hints = match app.screen {
        Screen::Connections => "n:new e:edit d:del Enter:connect ?:help q:quit",
        Screen::ConnectionForm => "Tab:field Enter:save Esc:cancel",
        Screen::Broker if app.focus == Focus::Payload => {
            "hjkl/arrows:move v:select y:yank 1:topics 3:history p:publish ?:help"
        }
        Screen::Broker if app.focus == Focus::History => {
            "hjkl/arrows:move v:select y:yank Enter:expand/collapse 2:payload p:publish ?:help"
        }
        Screen::Broker => {
            "j/k:move Enter:expand/collapse 2:payload 3:history s:sub u:unsub p:publish Esc:disconnect"
        }
        Screen::Publish => "Tab:field Enter:publish Esc:cancel",
        Screen::Subscribe => "Enter:subscribe Esc:cancel",
        Screen::Help => "any key: back",
    };

    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", label),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(DIM)),
        Span::styled(hints, Style::default().fg(DIM)),
        Span::raw(match &app.error {
            Some(e) => format!("   ⚠ {}", e),
            None => String::new(),
        }),
    ]);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(Color::Rgb(20, 20, 25))),
        area,
    );
}

fn center_rect(area: Rect, pct_x: u16, pct_y: u16) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(v[1])[1]
}
