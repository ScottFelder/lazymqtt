//! Terminal rendering. `draw` dispatches to one screen module per `Screen`;
//! `common` holds the shared widgets. UI code never mutates `App` — it only
//! reads state and the resolved color `Palette`.

mod alerts;
mod broker;
mod common;
mod connections;
mod help;
mod menu;
mod pane;
mod plugins;
mod publish;
mod recordings;
mod schemas;
mod statusbar;
mod theme;

use crate::app::{App, Screen};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    let pal = &app.palette;
    match app.screen {
        Screen::Connections => connections::draw_connections(f, app, chunks[0]),
        Screen::ConnectionForm => connections::draw_form(f, app, chunks[0]),
        Screen::Broker => broker::draw_broker(f, app, chunks[0]),
        Screen::Publish => {
            broker::draw_broker(f, app, chunks[0]);
            publish::draw_publish(
                f,
                &app.publish,
                app.publish_save_as.as_deref(),
                chunks[0],
                pal,
            );
        }
        Screen::Subscribe => {
            broker::draw_broker(f, app, chunks[0]);
            publish::draw_subscribe(f, &app.sub_form, chunks[0], pal);
        }
        Screen::SubscriptionList => connections::draw_subscription_list(f, app, chunks[0]),
        Screen::SubscriptionForm => {
            connections::draw_subscription_list(f, app, chunks[0]);
            publish::draw_subscription_form(f, &app.sub_form, chunks[0], pal);
        }
        Screen::ClearRetained => {
            broker::draw_broker(f, app, chunks[0]);
            publish::draw_clear_retained(f, &app.clear_topic, chunks[0], pal);
        }
        Screen::Plugins => plugins::draw_plugins(f, app, chunks[0]),
        Screen::AlertRules => alerts::draw_alert_rules(f, app, chunks[0]),
        Screen::AlertRuleForm => alerts::draw_alert_rule_form(f, app, chunks[0]),
        Screen::Schemas => schemas::draw_schemas(f, app, chunks[0]),
        Screen::SchemaForm => schemas::draw_schema_form(f, app, chunks[0]),
        Screen::Recordings => recordings::draw_recordings(f, app, chunks[0]),
        Screen::RecordingEdit => recordings::draw_recording_edit(f, app, chunks[0]),
        Screen::Theme => theme::draw_theme(f, app, chunks[0]),
        Screen::PluginPane => pane::draw_plugin_pane(f, app, chunks[0]),
        Screen::CommandMenu => {
            broker::draw_broker(f, app, chunks[0]);
            menu::draw_command_menu(f, app, chunks[0]);
        }
        Screen::Help => help::draw_help(f, app, chunks[0]),
    }

    statusbar::draw_statusbar(f, app, chunks[1]);
}
