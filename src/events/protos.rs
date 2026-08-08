use crate::app::{App, ProtoForm, Screen};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The protobuf mapping list: add / edit / delete mappings.
pub(crate) fn protos_keys(app: &mut App, key: KeyEvent) {
    let len = app.proto_mappings.len();
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
                app.protos_selected = (app.protos_selected + 1).min(len - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.protos_selected = app.protos_selected.saturating_sub(1);
        }
        KeyCode::Char('a') => app.begin_proto_add(),
        KeyCode::Char('e') | KeyCode::Enter => app.begin_proto_edit(),
        KeyCode::Char('d') => app.delete_selected_proto(),
        _ => {}
    }
}

/// The protobuf form: topic + message-type fields and a multi-line `.proto`
/// editor. Tab cycles the three; the `.proto` editor gets text-edit keys.
pub(crate) fn proto_form_keys(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('s') if ctrl => app.save_proto_form(),
        KeyCode::Esc => app.screen = Screen::Protos,
        KeyCode::Tab | KeyCode::BackTab => {
            app.proto_form.focus = (app.proto_form.focus + 1) % ProtoForm::FIELD_COUNT;
        }
        _ if app.proto_form.focus == 0 => text_field_key(app, key, 0),
        _ if app.proto_form.focus == 1 => text_field_key(app, key, 1),
        _ => body_key(app, key),
    }
}

fn text_field_key(app: &mut App, key: KeyEvent, field: usize) {
    let buf = if field == 0 {
        &mut app.proto_form.topic
    } else {
        &mut app.proto_form.message_type
    };
    match key.code {
        KeyCode::Char(c) => buf.push(c),
        KeyCode::Backspace => {
            buf.pop();
        }
        _ => {}
    }
}

fn body_key(app: &mut App, key: KeyEvent) {
    let ta = &mut app.proto_form.body;
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
    app.proto_form.error = None;
}
