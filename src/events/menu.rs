use crate::app::{App, MenuAction, Screen};
use crossterm::event::{KeyCode, KeyEvent};

pub(crate) fn command_menu_keys(app: &mut App, key: KeyEvent) {
    let len = app.menu_items.len();
    let in_submenu = app.menu_plugin.is_some();
    let sel = app.menu_selected.min(len.saturating_sub(1));
    // Inspect the selected row up front (fields are public).
    let (adjustable, submenu) = match app.menu_items.get(sel) {
        Some(it) => (
            it.adjustable,
            match &it.action {
                MenuAction::Submenu(p) => Some(*p),
                _ => None,
            },
        ),
        None => (false, None),
    };

    match key.code {
        // `q`/`m` always close the whole menu.
        KeyCode::Char('q') | KeyCode::Char('m') => app.screen = Screen::Broker,
        // Esc / Left back out one level: a submenu returns to the top, the top
        // closes. An adjustable row's Left instead cycles its option backward.
        KeyCode::Char('h') | KeyCode::Left if adjustable => app.adjust_selected_menu_item(false),
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => {
            if in_submenu {
                app.close_plugin_submenu();
            } else {
                app.screen = Screen::Broker;
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                app.menu_selected = (app.menu_selected + 1).min(len - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.menu_selected = app.menu_selected.saturating_sub(1);
        }
        // Right / l descends into a submenu, else cycles an option forward.
        KeyCode::Char('l') | KeyCode::Right => match submenu {
            Some(plugin) => app.open_plugin_submenu(plugin),
            None => app.adjust_selected_menu_item(true),
        },
        KeyCode::Enter if len > 0 => {
            if let Some(plugin) = submenu {
                app.open_plugin_submenu(plugin);
            } else if adjustable {
                // Cycle forward in place, like pressing Right.
                app.adjust_selected_menu_item(true);
            } else {
                // A concrete command closes the menu first; it may then open its
                // own screen (publish form, pane, …).
                let action = app.menu_items[sel].action.clone();
                app.screen = Screen::Broker;
                match action {
                    MenuAction::Core(cmd) => app.run_command(cmd),
                    MenuAction::Plugin { plugin, id } => app.invoke_plugin_command(plugin, &id),
                    MenuAction::Submenu(_) => {}
                }
            }
        }
        _ => {}
    }
}
