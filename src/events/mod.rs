//! Keyboard and paste input. `handle_key`/`handle_paste` dispatch to one module
//! per `Screen`. Event handlers mutate `App`; they never render.

mod alerts;
mod broker;
mod connections;
mod menu;
mod plugins;
mod publish;
mod recordings;
mod theme;

use crate::app::{App, Screen};
use connections::field_mut; // shared by the form-paste path
use crossterm::event::KeyEvent;

pub fn handle_key(app: &mut App, key: KeyEvent) {
    app.error = None;
    match app.screen {
        Screen::Connections => connections::connections_keys(app, key),
        Screen::ConnectionForm => connections::form_keys(app, key),
        Screen::Broker => broker::broker_keys(app, key),
        Screen::Publish => publish::publish_keys(app, key),
        Screen::Subscribe => publish::subscribe_keys(app, key),
        Screen::ClearRetained => publish::clear_retained_keys(app, key),
        Screen::Plugins => plugins::plugins_keys(app, key),
        Screen::AlertRules => alerts::alert_rules_keys(app, key),
        Screen::AlertRuleForm => alerts::alert_form_keys(app, key),
        Screen::Recordings => recordings::recordings_keys(app, key),
        Screen::RecordingEdit => recordings::recording_edit_keys(app, key),
        Screen::Theme => theme::theme_keys(app, key),
        Screen::CommandMenu => menu::command_menu_keys(app, key),
        Screen::Help => {
            app.screen = if app.handle.is_some() {
                Screen::Broker
            } else {
                Screen::Connections
            };
        }
    }
}

/// Insert clipboard text (from a bracketed paste) into the focused input.
/// The publish payload keeps newlines; single-line fields strip them, and the
/// port field keeps only digits — matching the per-key typing rules.
pub fn handle_paste(app: &mut App, data: String) {
    app.error = None;
    match app.screen {
        Screen::Publish => match app.publish.field {
            0 => app.publish.topic.push_str(&strip_newlines(&data)),
            1 => app.publish.payload.push_str(&data),
            _ => {}
        },
        Screen::Subscribe => app.sub_input.push_str(&strip_newlines(&data)),
        Screen::Recordings => {
            if let Some(buf) = app.recording_rename.as_mut() {
                buf.push_str(&strip_newlines(&data));
            }
        }
        Screen::RecordingEdit => {
            if let Some(buf) = app.rec_edit_saveas.as_mut() {
                buf.push_str(&strip_newlines(&data));
            } else {
                app.rec_edit_paste(&data);
            }
        }
        Screen::Theme => {
            if let Some(buf) = app.theme_edit.as_mut() {
                buf.push_str(&strip_newlines(&data));
            }
        }
        Screen::ConnectionForm => {
            let is_port = app.form.field == 2;
            if let Some(s) = field_mut(&mut app.form) {
                if is_port {
                    s.extend(data.chars().filter(|c| c.is_ascii_digit()));
                } else {
                    s.push_str(&strip_newlines(&data));
                }
            }
        }
        _ => {}
    }
}

fn strip_newlines(s: &str) -> String {
    s.chars().filter(|c| *c != '\n' && *c != '\r').collect()
}
