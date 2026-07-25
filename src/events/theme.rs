use crate::app::{App, Screen};
use crossterm::event::{KeyCode, KeyEvent};

pub(crate) fn theme_keys(app: &mut App, key: KeyEvent) {
    // Editing a color role's spec in place.
    if app.theme_edit.is_some() {
        match key.code {
            KeyCode::Esc => app.theme_edit = None,
            KeyCode::Enter => app.apply_theme_edit(),
            KeyCode::Backspace => {
                if let Some(buf) = app.theme_edit.as_mut() {
                    buf.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(buf) = app.theme_edit.as_mut() {
                    buf.push(c);
                }
            }
            _ => {}
        }
        return;
    }

    let len = app.theme_row_count();
    let builtins = app.theme_builtins_len();
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.screen = if app.handle.is_some() {
                Screen::Broker
            } else {
                Screen::Connections
            };
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                app.theme_selected = (app.theme_selected + 1).min(len - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.theme_selected = app.theme_selected.saturating_sub(1);
        }
        KeyCode::Char('s') => app.save_theme(),
        // A preset row applies; a color row opens the spec editor.
        KeyCode::Enter | KeyCode::Char('e') => {
            if app.theme_selected < builtins {
                app.apply_theme_builtin(app.theme_selected);
            } else {
                app.begin_theme_edit();
            }
        }
        _ => {}
    }
}
