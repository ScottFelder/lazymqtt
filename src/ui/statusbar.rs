use crate::app::{App, Focus, Screen, Status};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub(crate) fn draw_statusbar(f: &mut Frame, app: &App, area: Rect) {
    let pal = &app.palette;
    let (label, color) = match &app.status {
        Status::Idle => ("● idle".to_string(), pal.dim),
        Status::Connecting => ("● connecting…".to_string(), pal.warn),
        Status::Connected => {
            let host = app
                .active_conn()
                .map(|c| c.host.clone())
                .unwrap_or_default();
            (format!("● connected: {}", host), pal.ok)
        }
        Status::Disconnected(e) => (format!("● disconnected: {}", e), pal.error),
    };

    // Each hint is a (key, label) pair rendered as a highlighted key + dim
    // label. An empty key renders the label alone as a plain dim note.
    let hints: &[(&str, &str)] = match app.screen {
        Screen::Connections => &[
            ("n", "new"),
            ("e", "edit"),
            ("d", "delete"),
            ("Enter", "connect"),
            ("A", "alerts"),
            ("P", "plugins"),
            ("?", "help"),
            ("q", "quit"),
        ],
        Screen::ConnectionForm => &[("Tab", "field"), ("Enter", "save"), ("Esc", "cancel")],
        Screen::Broker if app.focus == Focus::Payload => &[
            ("hjkl", "move"),
            ("v", "select"),
            ("y", "yank"),
            ("z", "fold"),
            ("m", "menu"),
            ("1", "topics"),
            ("3", "history"),
        ],
        Screen::Broker if app.focus == Focus::History => &[
            ("hjkl", "move"),
            ("v", "select"),
            ("y", "yank"),
            ("Enter", "expand"),
            ("z", "fold"),
            ("m", "menu"),
            ("2", "payload"),
        ],
        Screen::Broker => &[
            ("j/k", "move"),
            ("Enter", "expand"),
            ("E/C", "all"),
            ("1/2/3", "panes"),
            ("m", "menu"),
            ("?", "help"),
            ("Esc", "disconnect"),
        ],
        Screen::Publish => &[("Tab", "field"), ("Enter", "publish"), ("Esc", "cancel")],
        Screen::Subscribe => &[("Enter", "subscribe"), ("Esc", "cancel")],
        Screen::ClearRetained => &[("y/Enter", "clear retained"), ("n/Esc", "cancel")],
        Screen::CommandMenu if app.menu_plugin.is_some() => &[
            ("j/k", "move"),
            ("Enter/→", "run"),
            ("←/Esc", "back"),
            ("←/→", "cycle option"),
        ],
        Screen::CommandMenu => &[("j/k", "move"), ("Enter/→", "open"), ("Esc", "close")],
        Screen::Plugins => &[("j/k", "move"), ("space/Enter", "toggle"), ("Esc", "back")],
        Screen::AlertRules => &[
            ("j/k", "move"),
            ("a", "add"),
            ("e", "edit"),
            ("d", "delete"),
            ("Esc", "back"),
        ],
        Screen::Schemas => &[
            ("j/k", "move"),
            ("a", "add"),
            ("e", "edit"),
            ("d", "delete"),
            ("Esc", "back"),
        ],
        Screen::SchemaForm => &[("Tab", "field"), ("^S", "save"), ("Esc", "cancel")],
        Screen::Recordings if app.recording_rename.is_some() => {
            &[("", "type new name"), ("Enter", "save"), ("Esc", "cancel")]
        }
        Screen::Recordings => &[
            ("j/k", "move"),
            ("Enter", "replay"),
            ("e", "edit"),
            ("r", "rename"),
            ("d", "delete"),
            ("Esc", "back"),
        ],
        Screen::RecordingEdit if app.rec_edit_saveas.is_some() => {
            &[("", "type a name"), ("Enter", "save"), ("Esc", "back")]
        }
        Screen::RecordingEdit => &[("^S", "save"), ("^N", "save as"), ("Esc", "cancel")],
        Screen::Theme if app.theme_edit.is_some() => &[
            ("", "type a color (name or #rrggbb)"),
            ("Enter", "apply"),
            ("Esc", "cancel"),
        ],
        Screen::Theme => &[
            ("j/k", "move"),
            ("Enter", "apply/edit"),
            ("Esc", "back"),
            ("", "auto-saved"),
        ],
        Screen::PluginPane => &[("", "live"), ("Esc", "back")],
        Screen::AlertRuleForm => &[
            ("Tab", "field"),
            ("space", "change"),
            ("Enter", "save"),
            ("Esc", "cancel"),
        ],
        Screen::Help => &[("", "press any key to go back")],
    };

    // On the broker screen, enabled plugins contribute their own key hints
    // (e.g. json-view adds `i`), inserted just before the `m` menu entry so a
    // plugin's keys appear when it is on and disappear when it is off.
    let mut all: Vec<(&str, &str)> = hints.to_vec();
    if matches!(app.screen, Screen::Broker) {
        let plugin_hints: Vec<(&str, &str)> = app
            .plugins
            .key_hints()
            .into_iter()
            .filter(|kh| !all.iter().any(|h| h.0 == kh.0))
            .collect();
        if !plugin_hints.is_empty() {
            let at = all.iter().position(|h| h.0 == "m").unwrap_or(all.len());
            all.splice(at..at, plugin_hints);
        }
    }

    let mut spans: Vec<Span> = vec![
        Span::styled(
            format!(" {} ", label),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(pal.dim)),
    ];
    for (i, (key, desc)) in all.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(pal.dim)));
        }
        if key.is_empty() {
            spans.push(Span::styled(*desc, Style::default().fg(pal.dim)));
        } else {
            spans.push(Span::styled(
                *key,
                Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(" {}", desc),
                Style::default().fg(pal.dim),
            ));
        }
    }
    if let Some(e) = &app.error {
        spans.push(Span::styled(
            format!("   ⚠ {}", e),
            Style::default().fg(pal.error),
        ));
    }
    let line = Line::from(spans);

    // App version, baked in from Cargo.toml at compile time so it tracks the
    // package version automatically. Pinned to the lower-right of the status bar.
    let version = format!(" lazymqtt {} ", env!("CARGO_PKG_VERSION"));
    let bg = Style::default().bg(pal.status_bar_bg);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(version.chars().count() as u16),
        ])
        .split(area);

    f.render_widget(Paragraph::new(line).style(bg), cols[0]);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            version,
            Style::default().fg(pal.dim),
        )))
        .style(bg)
        .alignment(Alignment::Right),
        cols[1],
    );
}
