use crate::app::{App, Screen};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(crate) fn recordings_keys(app: &mut App, key: KeyEvent) {
    // Rename mode: the selected recording's label is being edited in place.
    if app.recording_rename.is_some() {
        match key.code {
            KeyCode::Esc => app.recording_rename = None,
            KeyCode::Enter => app.apply_recording_rename(),
            KeyCode::Backspace => {
                if let Some(buf) = app.recording_rename.as_mut() {
                    buf.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(buf) = app.recording_rename.as_mut() {
                    buf.push(c);
                }
            }
            _ => {}
        }
        return;
    }

    let len = app.recordings.len();
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
                app.recordings_selected = (app.recordings_selected + 1).min(len - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.recordings_selected = app.recordings_selected.saturating_sub(1);
        }
        KeyCode::Enter => app.replay_selected_recording(),
        KeyCode::Char('r') => app.begin_recording_rename(),
        KeyCode::Char('e') => app.open_recording_edit(),
        KeyCode::Char('d') => app.delete_selected_recording(),
        _ => {}
    }
}

pub(crate) fn recording_edit_keys(app: &mut App, key: KeyEvent) {
    // "Save as" mode: a single-line filename prompt over the editor.
    if app.rec_edit_saveas.is_some() {
        match key.code {
            KeyCode::Esc => app.rec_edit_saveas = None,
            KeyCode::Enter => app.commit_recording_saveas(),
            KeyCode::Backspace => {
                if let Some(buf) = app.rec_edit_saveas.as_mut() {
                    buf.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(buf) = app.rec_edit_saveas.as_mut() {
                    buf.push(c);
                }
            }
            _ => {}
        }
        return;
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('s') if ctrl => app.save_recording_edit_current(),
        KeyCode::Char('n') if ctrl => app.begin_recording_saveas(),
        KeyCode::Esc => app.screen = Screen::Recordings, // discard edits
        KeyCode::Left => app.rec_edit_move_h(false),
        KeyCode::Right => app.rec_edit_move_h(true),
        KeyCode::Up => app.rec_edit_move_v(false),
        KeyCode::Down => app.rec_edit_move_v(true),
        KeyCode::Home => app.rec_edit_home(),
        KeyCode::End => app.rec_edit_end(),
        KeyCode::Enter => app.rec_edit_newline(),
        KeyCode::Backspace => app.rec_edit_backspace(),
        KeyCode::Char(c) => app.rec_edit_insert(c),
        _ => {}
    }
}
