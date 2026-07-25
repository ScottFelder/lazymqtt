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

    let hints = match app.screen {
        Screen::Connections => "n:new e:edit d:del Enter:connect A:alerts P:plugins ?:help q:quit",
        Screen::ConnectionForm => "Tab:field Enter:save Esc:cancel",
        Screen::Broker if app.focus == Focus::Payload => {
            "hjkl:move v:select y:yank i:view z:fold  m:menu  1:topics 3:history"
        }
        Screen::Broker if app.focus == Focus::History => {
            "hjkl:move v:select y:yank Enter:expand z:fold  m:menu  2:payload"
        }
        Screen::Broker => "j/k:move Enter:expand 1/2/3:panes  m:menu  ?:help Esc:disconnect",
        Screen::Publish => "Tab:field Enter:publish Esc:cancel",
        Screen::Subscribe => "Enter:subscribe Esc:cancel",
        Screen::ClearRetained => "y/Enter:clear retained  n/Esc:cancel",
        Screen::CommandMenu => "j/k:move ←/→:cycle option Enter:run Esc:close",
        Screen::Plugins => "j/k:move space/Enter:toggle Esc:back",
        Screen::AlertRules => "j/k:move a:add e:edit d:delete Esc:back",
        Screen::Recordings if app.recording_rename.is_some() => {
            "type new name  Enter:save  Esc:cancel"
        }
        Screen::Recordings => "j/k:move Enter:replay e:edit r:rename d:delete Esc:back",
        Screen::RecordingEdit if app.rec_edit_saveas.is_some() => {
            "type a name  Enter:save  Esc:back"
        }
        Screen::RecordingEdit => "edit text · ^S:save · ^N:save as · Esc:cancel",
        Screen::Theme if app.theme_edit.is_some() => {
            "type a color (name or #rrggbb)  Enter:apply  Esc:cancel"
        }
        Screen::Theme => "j/k:move Enter:apply/edit s:save Esc:back",
        Screen::AlertRuleForm => "Tab:field space:change Enter:save Esc:cancel",
        Screen::Help => "any key: back",
    };

    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", label),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(pal.dim)),
        Span::styled(hints, Style::default().fg(pal.dim)),
        Span::raw(match &app.error {
            Some(e) => format!("   ⚠ {}", e),
            None => String::new(),
        }),
    ]);

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
