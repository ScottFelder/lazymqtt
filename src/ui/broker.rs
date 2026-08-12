//! The main broker screen: topic tree (left) and the Payload/History message
//! panes (right), plus the detail-line rendering they share.
use super::common::*;
use crate::app::{App, DetailKind, DetailLine, Focus, PaneFold};
use crate::plugin::{InspectorStyle, Severity};
use crate::theme::Palette;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

/// Confirmation modal shown over the broker screen before disconnecting, so a
/// stray Esc doesn't drop the live session by accident.
pub(crate) fn draw_confirm_disconnect(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let host = app
        .active_conn()
        .map(|c| c.host.clone())
        .unwrap_or_default();
    let popup = center_rect(area, 60, 30);
    f.render_widget(Clear, popup);
    let lines = vec![
        Line::from("Disconnect from the broker?"),
        Line::from(""),
        Line::from(Span::styled(
            host,
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Ends the live session and clears the topic tree and history.",
            Style::default().fg(pal.dim),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "y/Enter: disconnect · n/Esc: cancel",
            Style::default().fg(pal.dim),
        )),
    ];
    let p = Paragraph::new(lines)
        .block(title_block("Disconnect", pal))
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);
}

pub(crate) fn draw_broker(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
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
                Span::styled(toggle, Style::default().fg(pal.accent)),
                Span::styled(r.label.clone(), Style::default().fg(pal.text)),
            ];
            if r.count > 0 {
                spans.push(Span::styled(
                    format!("  ({})", r.count),
                    Style::default().fg(pal.dim),
                ));
            }
            if let Some(v) = &r.value {
                let preview: String = v.chars().take(24).collect();
                spans.push(Span::styled(
                    format!("  = {}", preview),
                    Style::default().fg(pal.payload),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let tree_border = if app.focus == Focus::Tree {
        pal.accent
    } else {
        pal.dim
    };
    let tree = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(tree_border))
                .title(pane_title("1", "Topics", pal)),
        )
        .highlight_style(Style::default().bg(tree_border).fg(pal.selection_fg))
        .highlight_symbol("");

    let mut state = ListState::default();
    if !rows.is_empty() {
        state.select(Some(app.tree_selected.min(rows.len() - 1)));
    }
    f.render_stateful_widget(tree, cols[0], &mut state);
    render_vscrollbar(f, cols[0], rows.len(), state.offset(), pal);

    // Right: an outer "Messages" container (peer to Topics) holding the Payload
    // and History panes as nested sub-panes. Its border highlights whenever a
    // sub-pane has focus. The container itself carries no [n] jump number.
    let messages_border = if app.focus == Focus::Tree {
        pal.dim
    } else {
        pal.accent
    };
    let messages = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(messages_border))
        .title(Span::styled(
            " Messages ",
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        ));
    let inner = messages.inner(cols[1]);
    f.render_widget(messages, cols[1]);

    // Payload (selected message) on top, History (all messages) below. Both
    // expanded => an even split; one collapsed => it shrinks to a title bar and
    // the other fills the rest. A collapsed pane is COLLAPSED_ROWS tall.
    const COLLAPSED_ROWS: u16 = 3;
    let (constraints, payload_collapsed, history_collapsed) = match app.collapsed {
        PaneFold::None => (
            [Constraint::Percentage(50), Constraint::Percentage(50)],
            false,
            false,
        ),
        PaneFold::Payload => (
            [Constraint::Length(COLLAPSED_ROWS), Constraint::Min(1)],
            true,
            false,
        ),
        PaneFold::History => (
            [Constraint::Min(1), Constraint::Length(COLLAPSED_ROWS)],
            false,
            true,
        ),
    };
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    draw_payload(f, app, right[0], payload_collapsed);
    draw_history(f, app, right[1], history_collapsed);
}

/// Top-right pane: the payload (and metadata) of whichever message is
/// currently selected, with keyboard text selection/yank.
fn draw_payload(f: &mut Frame, app: &App, area: Rect, collapsed: bool) {
    let pal = &app.palette;
    let payload_border = if app.focus == Focus::Payload {
        pal.accent
    } else {
        pal.dim
    };
    // Show the active view (e.g. "JSON") when a plugin offers an alternative to
    // the raw text, hinting that `i` cycles views.
    let mut title = folded_title("2", "Payload", collapsed, pal);
    if let Some(label) = app.payload_view_label() {
        title.spans.push(Span::styled(
            format!("{} ", label),
            Style::default().fg(pal.accent),
        ));
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(payload_border))
        .title(title);
    if collapsed {
        f.render_widget(block, area);
        return;
    }
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
        .map(|(i, dl)| render_detail_line(i, dl, sel, app.sel_cursor, show_cursor, pal))
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
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(payload, area);
    render_vscrollbar(f, area, line_count, scroll as usize, pal);
}

/// Bottom-right pane: every message received for the selected topic, newest
/// first, with the same keyboard text selection/yank as the Payload pane.
/// Moving the cursor also picks which message the Payload pane shows; Enter
/// expands/collapses the entry under the cursor.
fn draw_history(f: &mut Frame, app: &App, area: Rect, collapsed: bool) {
    let pal = &app.palette;
    let history_border = if app.focus == Focus::History {
        pal.accent
    } else {
        pal.dim
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(history_border))
        .title(folded_title("3", "History", collapsed, pal));
    if collapsed {
        f.render_widget(block, area);
        return;
    }
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
        .map(|(i, dl)| render_detail_line(i, dl, sel, app.sel_cursor, show_cursor, pal))
        .collect();

    let inner_h = area.height.saturating_sub(2) as usize;
    let scroll = if show_cursor && inner_h > 0 && app.sel_cursor.0 >= inner_h {
        (app.sel_cursor.0 - inner_h + 1) as u16
    } else {
        0
    };

    let line_count = rendered.len();
    let history = Paragraph::new(rendered)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(history, area);
    render_vscrollbar(f, area, line_count, scroll as usize, pal);
}

/// A pane title with a fold chevron: ▾ when expanded, ▸ when collapsed.
fn folded_title(number: &str, name: &str, collapsed: bool, pal: &Palette) -> Line<'static> {
    let mut title = pane_title(number, name, pal);
    title.spans.push(Span::styled(
        if collapsed { "▸ " } else { "▾ " },
        Style::default().fg(pal.dim),
    ));
    title
}

/// Color for a segment's semantic kind, resolved against the active palette.
fn style_for(kind: DetailKind, pal: &Palette) -> Style {
    match kind {
        DetailKind::Header => Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        DetailKind::Toggle => Style::default().fg(pal.accent),
        DetailKind::Payload => Style::default().fg(pal.payload),
        DetailKind::Meta | DetailKind::Blank => Style::default().fg(pal.dim),
        DetailKind::Annotation(severity) => Style::default().fg(match severity {
            Severity::Ok => pal.ok,
            Severity::Info => pal.info,
            Severity::Warn => pal.warn,
            Severity::Error => pal.error,
        }),
        DetailKind::Syntax(style) => Style::default().fg(match style {
            InspectorStyle::Punctuation => pal.json_punctuation,
            InspectorStyle::Key => pal.json_key,
            InspectorStyle::Str => pal.json_string,
            InspectorStyle::Number => pal.json_number,
            InspectorStyle::Literal => pal.json_literal,
            InspectorStyle::Plain => pal.json_plain,
        }),
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
    pal: &Palette,
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

    let hl = Style::default().bg(pal.accent).fg(pal.selection_fg);
    let mut spans = Vec::new();
    if !dl.lead.is_empty() {
        spans.push(Span::styled(dl.lead.clone(), style_for(dl.lead_kind, pal)));
    }

    // Walk segments, splitting each by its intersection with the highlight range.
    let mut offset = 0usize;
    for (text, kind) in &dl.segs {
        let base = style_for(*kind, pal);
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
