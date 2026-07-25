use crate::app::{App, Command, Focus, Screen};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(crate) fn broker_keys(app: &mut App, key: KeyEvent) {
    let rows = app.tree.rows(&app.expanded);
    let len = rows.len();

    // Number keys jump straight to a pane, lazygit/lazydocker-style, regardless
    // of which pane currently has focus. Switching panes clears any in-progress
    // selection, since the cursor indexes the focused pane's own lines.
    match key.code {
        KeyCode::Char('1') => {
            app.focus = Focus::Tree;
            app.reset_selection();
            return;
        }
        KeyCode::Char('2') => {
            app.focus = Focus::Payload;
            app.reset_selection();
            return;
        }
        KeyCode::Char('3') => return app.focus_history(),
        _ => {}
    }

    // When the Payload pane is focused, movement keys drive the text-selection
    // cursor instead of the topic tree. Global actions below still apply.
    if app.focus == Focus::Payload {
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => return app.sel_move_col(-1),
            KeyCode::Char('l') | KeyCode::Right => return app.sel_move_col(1),
            KeyCode::Char('j') | KeyCode::Down => return app.sel_move_line(1),
            KeyCode::Char('k') | KeyCode::Up => return app.sel_move_line(-1),
            KeyCode::Char('v') => return app.sel_toggle_anchor(),
            KeyCode::Char('y') => return app.sel_yank(),
            _ => {}
        }
    }

    // When the History pane is focused, movement keys drive the same text
    // selection cursor as the Payload pane (over the History lines); moving
    // between messages keeps the Payload pane in sync, and Enter expands or
    // collapses the entry under the cursor.
    if app.focus == Focus::History {
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => return app.sel_move_col(-1),
            KeyCode::Char('l') | KeyCode::Right => return app.sel_move_col(1),
            KeyCode::Char('j') | KeyCode::Down => {
                app.sel_move_line(1);
                app.sync_history_selected();
                return;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.sel_move_line(-1);
                app.sync_history_selected();
                return;
            }
            KeyCode::Char('v') => return app.sel_toggle_anchor(),
            KeyCode::Char('y') => return app.sel_yank(),
            KeyCode::Enter => return app.toggle_history_at_cursor(),
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc => {
            // A pending selection cancels first, so Esc doesn't disconnect mid-select.
            if app.sel_anchor.is_some() {
                app.sel_anchor = None;
            } else {
                app.disconnect();
                app.screen = Screen::Connections;
            }
        }
        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.run_command(Command::Quit)
        }
        KeyCode::Char('?') => app.run_command(Command::Help),
        KeyCode::Char('m') => app.open_command_menu(),
        KeyCode::Tab => match app.focus {
            Focus::Tree => {
                app.focus = Focus::Payload;
                app.reset_selection();
            }
            Focus::Payload => app.focus_history(),
            Focus::History => {
                app.focus = Focus::Tree;
                app.reset_selection();
            }
        },
        KeyCode::Char('c') => app.run_command(Command::ClearTree),
        KeyCode::Char('x') => app.run_command(Command::ClearTopic),
        KeyCode::Char('p') => app.run_command(Command::Publish),
        KeyCode::Char('j') | KeyCode::Down if app.focus == Focus::Tree => {
            if len > 0 {
                app.tree_selected = (app.tree_selected + 1).min(len - 1);
                app.reset_message_view();
            }
        }
        KeyCode::Char('k') | KeyCode::Up if app.focus == Focus::Tree => {
            app.tree_selected = app.tree_selected.saturating_sub(1);
            app.reset_message_view();
        }
        KeyCode::Right if app.focus == Focus::Tree => {
            if let Some(r) = rows.get(app.tree_selected) {
                if r.has_children {
                    app.expanded.insert(r.path.clone());
                }
            }
        }
        KeyCode::Enter if app.focus == Focus::Tree => {
            if let Some(r) = rows.get(app.tree_selected) {
                if r.has_children {
                    // Toggle: HashSet::remove returns false when it was absent,
                    // so collapse an expanded node or expand a collapsed one.
                    if !app.expanded.remove(&r.path) {
                        app.expanded.insert(r.path.clone());
                    }
                }
            }
        }
        KeyCode::Left if app.focus == Focus::Tree => {
            if let Some(r) = rows.get(app.tree_selected) {
                app.expanded.remove(&r.path);
            }
        }
        KeyCode::Char('s') => app.run_command(Command::Subscribe),
        KeyCode::Char('u') => app.run_command(Command::Unsubscribe),
        KeyCode::Char('r') => app.run_command(Command::ClearRetained),
        KeyCode::Char('z') => app.toggle_pane_fold(),
        KeyCode::Char('i') => app.cycle_payload_view(),
        KeyCode::Char('P') => app.run_command(Command::Plugins),
        KeyCode::Char('A') => app.run_command(Command::AlertRules),
        KeyCode::Char('R') => app.run_command(Command::Recordings),
        KeyCode::Char('T') => app.run_command(Command::Theme),
        _ => {}
    }
}
