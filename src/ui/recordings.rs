use super::common::*;
use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

pub(crate) fn draw_recordings(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let items: Vec<ListItem> = app
        .recordings
        .iter()
        .enumerate()
        .map(|(i, rec)| {
            // While renaming the selected row, show the live edit buffer instead
            // of the stored label.
            let editing = app
                .recording_rename
                .as_ref()
                .filter(|_| i == app.recordings_selected);
            let (label, label_style) = match editing {
                Some(buf) => (
                    format!("{buf}_"),
                    Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
                ),
                None => (rec.label.clone(), Style::default().fg(pal.text)),
            };
            let count = if rec.messages == 1 {
                "1 msg".to_string()
            } else {
                format!("{} msgs", rec.messages)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{label:<40}"), label_style),
                Span::styled(count, Style::default().fg(pal.dim)),
            ]))
        })
        .collect();

    let title = format!("Recordings — {}", app.recordings_conn_name);
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
    if !app.recordings.is_empty() {
        state.select(Some(app.recordings_selected.min(app.recordings.len() - 1)));
    }
    f.render_stateful_widget(list, area, &mut state);

    if app.recordings.is_empty() {
        let hint = Paragraph::new("No recordings for this connection. Record one from the m menu.")
            .style(Style::default().fg(pal.dim))
            .alignment(Alignment::Center);
        f.render_widget(hint, center_rect(area, 70, 1));
    }
}
pub(crate) fn draw_recording_edit(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let popup = center_rect(area, 88, 84);
    f.render_widget(Clear, popup);

    let title = format!(
        "Edit Recording — {}  (one JSON message per line)",
        app.rec_edit_label
    );
    f.render_widget(title_block(&title, pal), popup);

    // Inner area, split into the text body, a footer hint, and an error line.
    let inner = Rect {
        x: popup.x + 1,
        y: popup.y + 1,
        width: popup.width.saturating_sub(2),
        height: popup.height.saturating_sub(2),
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let (body, footer, errline) = (rows[0], rows[1], rows[2]);

    // Scroll so the cursor stays visible (both axes derived from the cursor).
    let text_h = body.height as usize;
    let text_w = body.width.max(1) as usize;
    let row_off = app.rec_edit_row.saturating_sub(text_h.saturating_sub(1));
    let col_off = app.rec_edit_col.saturating_sub(text_w.saturating_sub(1));

    let mut lines: Vec<Line> = Vec::new();
    for r in row_off..(row_off + text_h).min(app.rec_edit_lines.len()) {
        let chars: Vec<char> = app.rec_edit_lines[r].chars().collect();
        if r == app.rec_edit_row {
            // Draw the line with a block cursor at the current column.
            let cur = app.rec_edit_col;
            let mut spans = Vec::new();
            let before: String = chars[col_off.min(chars.len())..cur.min(chars.len())]
                .iter()
                .collect();
            spans.push(Span::raw(before));
            let cursor_ch = chars.get(cur).copied().unwrap_or(' ');
            spans.push(Span::styled(
                cursor_ch.to_string(),
                Style::default().fg(pal.selection_fg).bg(pal.accent),
            ));
            if cur < chars.len() {
                let after: String = chars[(cur + 1)..].iter().take(text_w).collect();
                spans.push(Span::raw(after));
            }
            lines.push(Line::from(spans));
        } else {
            let visible: String = chars.iter().skip(col_off).take(text_w).collect();
            lines.push(Line::from(visible));
        }
    }
    f.render_widget(Paragraph::new(lines), body);

    let hint = "arrows: move · Enter: split line · ^S: save · ^N: save as · Esc: cancel";
    f.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(pal.dim))),
        footer,
    );
    if let Some(err) = &app.rec_edit_error {
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("⚠ {err}"),
                Style::default().fg(pal.error),
            )),
            errline,
        );
    }

    // "Save as" prompt overlays the editor.
    if let Some(buf) = &app.rec_edit_saveas {
        let prompt = center_rect(area, 60, 20);
        f.render_widget(Clear, prompt);
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    "Save as: ",
                    Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
                ),
                Span::raw(buf.clone()),
                Span::styled("_", Style::default().fg(pal.accent)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "a new recording for this connection · Enter: save · Esc: back",
                Style::default().fg(pal.dim),
            )),
        ];
        f.render_widget(
            Paragraph::new(lines).block(title_block("Save Recording As", pal)),
            prompt,
        );
    }
}
