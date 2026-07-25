use super::common::*;
use crate::app::{AlertForm, App};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

pub(crate) fn draw_alert_rules(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let items: Vec<ListItem> = app
        .alert_rules
        .iter()
        .map(|r| {
            let (label, sev_color) = match r.severity {
                crate::plugin::AlertSeverity::Warn => (r.severity.label(), pal.warn),
                crate::plugin::AlertSeverity::Error => (r.severity.label(), pal.error),
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<24}", r.topic), Style::default().fg(pal.text)),
                Span::styled(format!("{:<22}", r.summary()), Style::default().fg(pal.dim)),
                Span::styled(label.to_string(), Style::default().fg(sev_color)),
            ]))
        })
        .collect();

    let title = format!("Alert Rules — {}", app.alert_edit_name);
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
    if !app.alert_rules.is_empty() {
        state.select(Some(app.alerts_selected.min(app.alert_rules.len() - 1)));
    }
    f.render_stateful_widget(list, area, &mut state);

    if app.alert_rules.is_empty() {
        let hint = Paragraph::new("No alert rules for this connection. Press 'a' to add one.")
            .style(Style::default().fg(pal.dim))
            .alignment(Alignment::Center);
        f.render_widget(hint, center_rect(area, 70, 1));
    }
}
pub(crate) fn draw_alert_rule_form(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let form = &app.alert_form;
    let title = if form.editing_index.is_some() {
        "Edit Alert Rule"
    } else {
        "New Alert Rule"
    };

    // Only the fields relevant to the chosen condition are active.
    let value_active = form.when <= 1; // above / below
    let seconds_active = form.when == 3; // silent
    let field_active = form.when <= 1;

    let when_label = AlertForm::WHEN_LABELS[form.when];
    let sev_label = AlertForm::SEVERITY_LABELS[form.severity];
    let fields: [(&str, String, bool); 6] = [
        ("Topic", form.topic.clone(), true),
        ("When", format!("{when_label}  (space to change)"), true),
        ("Value", form.value.clone(), value_active),
        ("Seconds", form.seconds.clone(), seconds_active),
        ("Field", form.field.clone(), field_active),
        ("Severity", format!("{sev_label}  (space to change)"), true),
    ];

    let mut lines = Vec::new();
    for (i, (label, val, active)) in fields.iter().enumerate() {
        let focused = i == form.focus;
        let marker = if focused { "▶ " } else { "  " };
        let label_style = if focused {
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(pal.text)
        };
        // A text-input caret on the editable text fields.
        let is_text = matches!(i, 0 | 2 | 3 | 4);
        let cursor = if focused && is_text { "_" } else { "" };
        let shown = if *active {
            val.clone()
        } else {
            "(n/a)".to_string()
        };
        let val_style = if *active {
            Style::default().fg(pal.text)
        } else {
            Style::default().fg(pal.dim)
        };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(pal.accent)),
            Span::styled(format!("{:<10}", label), label_style),
            Span::styled(shown, val_style),
            Span::styled(cursor, Style::default().fg(pal.accent)),
        ]));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "Tab: next field · space: change When/Severity · Enter: save · Esc: cancel",
        Style::default().fg(pal.dim),
    )));

    let p = Paragraph::new(lines)
        .block(title_block(title, pal))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
