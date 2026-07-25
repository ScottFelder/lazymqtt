use crate::app::{App, Screen};
use crossterm::event::{KeyCode, KeyEvent};

pub(crate) fn plugins_keys(app: &mut App, key: KeyEvent) {
    let len = app.plugins.count();
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('P') => {
            app.screen = if app.handle.is_some() {
                Screen::Broker
            } else {
                Screen::Connections
            };
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                app.plugins_selected = (app.plugins_selected + 1).min(len - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.plugins_selected = app.plugins_selected.saturating_sub(1);
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            if len > 0 {
                app.plugins.toggle(app.plugins_selected);
            }
        }
        _ => {}
    }
}
