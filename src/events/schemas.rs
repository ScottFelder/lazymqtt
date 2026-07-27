use crate::app::{App, SchemaForm, Screen};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The schema list: add / edit / delete mappings.
pub(crate) fn schemas_keys(app: &mut App, key: KeyEvent) {
    let len = app.schemas.len();
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
                app.schemas_selected = (app.schemas_selected + 1).min(len - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.schemas_selected = app.schemas_selected.saturating_sub(1);
        }
        KeyCode::Char('a') => app.begin_schema_add(),
        KeyCode::Char('e') | KeyCode::Enter => app.begin_schema_edit(),
        KeyCode::Char('d') => app.delete_selected_schema(),
        _ => {}
    }
}

/// The schema form: a topic field and a multi-line JSON editor. Tab switches
/// between them; the JSON editor gets the usual text-edit keys when focused.
pub(crate) fn schema_form_keys(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('s') if ctrl => app.save_schema_form(),
        KeyCode::Esc => app.screen = Screen::Schemas,
        KeyCode::Tab | KeyCode::BackTab => {
            app.schema_form.focus = (app.schema_form.focus + 1) % SchemaForm::FIELD_COUNT;
        }
        _ if app.schema_form.focus == 0 => topic_key(app, key), // topic field
        _ => body_key(app, key),                                // JSON editor
    }
}

fn topic_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char(c) => app.schema_form.topic.push(c),
        KeyCode::Backspace => {
            app.schema_form.topic.pop();
        }
        _ => {}
    }
}

fn body_key(app: &mut App, key: KeyEvent) {
    let ta = &mut app.schema_form.body;
    match key.code {
        KeyCode::Left => ta.move_h(false),
        KeyCode::Right => ta.move_h(true),
        KeyCode::Up => ta.move_v(false),
        KeyCode::Down => ta.move_v(true),
        KeyCode::Home => ta.home(),
        KeyCode::End => ta.end(),
        KeyCode::Enter => ta.newline(),
        KeyCode::Backspace => ta.backspace(),
        KeyCode::Char(c) => ta.insert(c),
        _ => return,
    }
    app.schema_form.error = None;
}
