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

pub(crate) fn draw_publish(
    f: &mut Frame,
    pb: &PublishBuffer,
    save_as: Option<&str>,
    area: Rect,
    pal: &Palette,
) {
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
        "Tab: field · space: toggle QoS/Retain · Enter: publish · ^T: save template · Esc: cancel",
        Style::default().fg(pal.dim),
    )));

    let p = Paragraph::new(lines)
        .block(title_block("Publish Message", pal))
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);

    // "Save as template" name prompt over the form.
    if let Some(name) = save_as {
        let prompt = center_rect(area, 60, 20);
        f.render_widget(Clear, prompt);
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    "Template name: ",
                    Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
                ),
                Span::raw(name.to_string()),
                Span::styled("_", Style::default().fg(pal.accent)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "saves the current topic/payload/QoS/retain · Enter: save · Esc: back",
                Style::default().fg(pal.dim),
            )),
        ];
        f.render_widget(
            Paragraph::new(lines).block(title_block("Save Publish Template", pal)),
            prompt,
        );
    }
}
pub(crate) fn draw_subscribe(f: &mut Frame, sf: &crate::app::SubForm, area: Rect, pal: &Palette) {
    let popup = center_rect(area, 60, 25);
    f.render_widget(Clear, popup);
    let mut lines = sub_form_lines(sf, pal);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "wildcards ok: sensors/#, home/+/temp · Tab: field · space: QoS · Enter: subscribe · Esc: cancel",
        Style::default().fg(pal.dim),
    )));
    let p = Paragraph::new(lines)
        .block(title_block("Subscribe to Topic", pal))
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);
}

/// A whole-screen editor for one connection subscription (topic + QoS), shown
/// over the connection form. Shares the two-field layout with the live prompt.
pub(crate) fn draw_subscription_form(
    f: &mut Frame,
    sf: &crate::app::SubForm,
    area: Rect,
    pal: &Palette,
) {
    let popup = center_rect(area, 60, 25);
    f.render_widget(Clear, popup);
    let title = if sf.editing_index.is_some() {
        "Edit Subscription"
    } else {
        "Add Subscription"
    };
    let mut lines = sub_form_lines(sf, pal);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Tab: field · space/←→: QoS · Enter: save · Esc: cancel",
        Style::default().fg(pal.dim),
    )));
    let p = Paragraph::new(lines)
        .block(title_block(title, pal))
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);
}

/// The shared two-row (Topic, QoS) body for both subscription editors.
fn sub_form_lines(sf: &crate::app::SubForm, pal: &Palette) -> Vec<Line<'static>> {
    let field = |label: &str, val: String, focused: bool, cursor: bool| {
        let label_style = if focused {
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(pal.text)
        };
        Line::from(vec![
            Span::styled(
                if focused { "▶ " } else { "  " }.to_string(),
                Style::default().fg(pal.accent),
            ),
            Span::styled(format!("{:<7}", label), label_style),
            Span::raw(val),
            Span::styled(
                if focused && cursor { "_" } else { "" }.to_string(),
                Style::default().fg(pal.accent),
            ),
        ])
    };
    vec![
        field("Topic", sf.topic.clone(), sf.field == 0, true),
        Line::from(""),
        field("QoS", sf.qos.min(2).to_string(), sf.field == 1, false),
    ]
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
