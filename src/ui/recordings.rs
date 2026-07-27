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

    draw_textarea(f, body, &app.rec_editor, true, pal);

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
