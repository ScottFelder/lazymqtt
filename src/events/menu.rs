use crate::app::{App, MenuAction, Screen};
use crossterm::event::{KeyCode, KeyEvent};

pub(crate) fn command_menu_keys(app: &mut App, key: KeyEvent) {
    let len = app.menu_items.len();
    match key.code {
        KeyCode::Esc | KeyCode::Char('m') | KeyCode::Char('q') => app.screen = Screen::Broker,
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                app.menu_selected = (app.menu_selected + 1).min(len - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.menu_selected = app.menu_selected.saturating_sub(1);
        }
        // Left/right (or h/l) cycle an option-style row in place, keeping the
        // menu open with a refreshed label.
        KeyCode::Char('h') | KeyCode::Left => app.adjust_selected_menu_item(false),
        KeyCode::Char('l') | KeyCode::Right => app.adjust_selected_menu_item(true),
        KeyCode::Enter if len > 0 => {
            let item = &app.menu_items[app.menu_selected.min(len - 1)];
            // An adjustable row cycles forward and stays open, like pressing
            // right; a one-shot command closes the menu and runs.
            if item.adjustable {
                app.adjust_selected_menu_item(true);
            } else {
                let action = item.action;
                // Return to the broker first; a command may then set its own screen.
                app.screen = Screen::Broker;
                match action {
                    MenuAction::Core(cmd) => app.run_command(cmd),
                    MenuAction::Plugin { plugin, id } => app.invoke_plugin_command(plugin, id),
                }
            }
        }
        _ => {}
    }
}
