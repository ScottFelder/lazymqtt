use super::common::*;
use crate::app::PublishBuffer;
use crate::theme::Palette;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
    Frame,
};

pub(crate) fn draw_publish(f: &mut Frame, pb: &PublishBuffer, area: Rect, pal: &Palette) {
    let popup = center_rect(area, 70, 40);
    f.render_widget(Clear, popup);

    let fields = [
        ("Topic", pb.topic.as_str()),
        ("Payload", pb.payload.as_str()),
        ("QoS", ["0", "1", "2"][pb.qos.min(2) as usize]),
        ("Retain", if pb.retain { "[x]" } else { "[ ]" }),
    ];
    let mut lines = Vec::new();
    for (i, (label, val)) in fields.iter().enumerate() {
        let focused = i == pb.field;
        let marker = if focused { "▶ " } else { "  " };
        let cursor = if focused && i < 2 { "_" } else { "" };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(pal.accent)),
            Span::styled(
                format!("{:<9}", label),
                if focused {
                    Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(pal.text)
                },
            ),
            Span::raw(val.to_string()),
            Span::styled(cursor, Style::default().fg(pal.accent)),
        ]));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "Tab: next field · space: toggle QoS/Retain · Enter: publish · Esc: cancel",
        Style::default().fg(pal.dim),
    )));

    let p = Paragraph::new(lines)
        .block(title_block("Publish Message", pal))
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);
}
pub(crate) fn draw_subscribe(f: &mut Frame, input: &str, area: Rect, pal: &Palette) {
    let popup = center_rect(area, 60, 20);
    f.render_widget(Clear, popup);
    let lines = vec![
        Line::from(vec![
            Span::styled(
                "Topic: ",
                Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
            ),
            Span::raw(input.to_string()),
            Span::styled("_", Style::default().fg(pal.accent)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "wildcards ok: sensors/#, home/+/temp · Enter: subscribe · Esc: cancel",
            Style::default().fg(pal.dim),
        )),
    ];
    let p = Paragraph::new(lines).block(title_block("Subscribe to Topic", pal));
    f.render_widget(p, popup);
}

pub(crate) fn draw_clear_retained(f: &mut Frame, topic: &str, area: Rect, pal: &Palette) {
    let popup = center_rect(area, 70, 30);
    f.render_widget(Clear, popup);
    let lines = vec![
        Line::from("Clear the retained message on:"),
        Line::from(""),
        Line::from(Span::styled(
            topic.to_string(),
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Publishes an empty retained message — the broker drops the stored one.",
            Style::default().fg(pal.dim),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "y/Enter: clear · n/Esc: cancel",
            Style::default().fg(pal.dim),
        )),
    ];
    let p = Paragraph::new(lines)
        .block(title_block("Clear Retained Message", pal))
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);
}
