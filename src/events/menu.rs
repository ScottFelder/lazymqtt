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
        KeyCode::Enter => app.activate_selected_menu_item(),
        // `?` opens the help screen — a plugin submenu jumps straight to that
        // plugin's help page; the top level opens the core help.
        KeyCode::Char('?') => match app.menu_plugin {
            Some(plugin) => app.open_plugin_help(plugin),
            None => app.open_help(),
        },
        // Accelerator: a row's own key jumps to and activates it, just like
        // selecting it and pressing Enter (e.g. `T` opens the Theme screen).
        // Navigation keys above take precedence, so a plugin glyph that collides
        // with them stays reachable only by scrolling.
        KeyCode::Char(c) => {
            app.activate_menu_key(c);
        }
        _ => {}
    }
}
