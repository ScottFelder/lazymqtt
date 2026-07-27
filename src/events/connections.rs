use crate::app::{App, Focus, FormBuffer, Screen, Status};
use crate::config::{Connection, Subscription};
use crate::mqtt::{self};
use crossterm::event::{KeyCode, KeyEvent};

pub(crate) fn connections_keys(app: &mut App, key: KeyEvent) {
    let len = app.config.connections.len();
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('?') => app.screen = Screen::Help,
        KeyCode::Char('P') => {
            app.plugins_selected = 0;
            app.screen = Screen::Plugins;
        }
        KeyCode::Char('A') => app.open_alerts_editor(),
        KeyCode::Char('S') => app.open_schemas(),
        KeyCode::Char('T') => app.open_theme(),
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                app.conn_selected = (app.conn_selected + 1) % len;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if len > 0 {
                app.conn_selected = (app.conn_selected + len - 1) % len;
            }
        }
        KeyCode::Char('n') => {
            app.form = FormBuffer {
                port: "1883".into(),
                topics: "#".into(),
                ..Default::default()
            };
            app.screen = Screen::ConnectionForm;
        }
        KeyCode::Char('e') => {
            if let Some(c) = app.config.connections.get(app.conn_selected) {
                app.form = FormBuffer::from(c, app.conn_selected);
                app.screen = Screen::ConnectionForm;
            }
        }
        KeyCode::Char('d') => {
            if len > 0 {
                app.config.connections.remove(app.conn_selected);
                let _ = app.config.save();
                if app.conn_selected >= app.config.connections.len() && app.conn_selected > 0 {
                    app.conn_selected -= 1;
                }
            }
        }
        KeyCode::Enter => start_connection(app),
        _ => {}
    }
}

fn start_connection(app: &mut App) {
    let idx = app.conn_selected;
    let Some(conn) = app.config.connections.get(idx).cloned() else {
        return;
    };
    // Tear down any live connection first so its background task stops instead
    // of lingering and reconnecting — two clients with the same id would
    // otherwise fight ("connection closed by peer").
    if app.handle.is_some() {
        app.disconnect();
    }
    match mqtt::connect(&conn) {
        Ok(handle) => {
            app.handle = Some(handle);
            app.active = Some(idx);
            app.status = Status::Connecting;
            app.tree.clear();
            app.history.clear();
            app.expanded.clear();
            app.tree_selected = 0;
            app.reset_message_view();
            app.focus = Focus::Tree;
            app.screen = Screen::Broker;
        }
        Err(e) => app.error = Some(e.to_string()),
    }
}

pub(crate) fn form_keys(app: &mut App, key: KeyEvent) {
    let f = &mut app.form;
    match key.code {
        KeyCode::Esc => app.screen = Screen::Connections,
        KeyCode::Tab | KeyCode::Down => f.field = (f.field + 1) % FormBuffer::FIELD_COUNT,
        KeyCode::BackTab | KeyCode::Up => {
            f.field = (f.field + FormBuffer::FIELD_COUNT - 1) % FormBuffer::FIELD_COUNT
        }
        KeyCode::Char(' ') if f.field == 6 => f.tls = !f.tls,
        KeyCode::Char(c) => {
            let is_port = f.field == 2;
            if let Some(s) = field_mut(f) {
                if !is_port || c.is_ascii_digit() {
                    s.push(c);
                }
            }
        }
        KeyCode::Backspace => {
            if let Some(s) = field_mut(f) {
                s.pop();
            }
        }
        KeyCode::Enter => save_form(app),
        _ => {}
    }
}

pub(crate) fn field_mut(f: &mut FormBuffer) -> Option<&mut String> {
    match f.field {
        0 => Some(&mut f.name),
        1 => Some(&mut f.host),
        2 => Some(&mut f.port),
        3 => Some(&mut f.client_id),
        4 => Some(&mut f.username),
        5 => Some(&mut f.password),
        7 => Some(&mut f.topics),
        _ => None,
    }
}

fn save_form(app: &mut App) {
    let f = &app.form;
    if f.name.trim().is_empty() || f.host.trim().is_empty() {
        app.error = Some("Name and Host are required".into());
        return;
    }
    let port: u16 = f.port.trim().parse().unwrap_or(1883);
    let subs: Vec<Subscription> = f
        .topics
        .split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(Subscription::new)
        .collect();

    let mut conn = Connection::new(f.name.trim(), f.host.trim(), port);
    conn.client_id = if f.client_id.trim().is_empty() {
        conn.client_id
    } else {
        f.client_id.trim().into()
    };
    conn.username = f.username.trim().into();
    conn.password = f.password.clone();
    conn.tls = f.tls;
    conn.subscriptions = if subs.is_empty() {
        vec![Subscription::new("#")]
    } else {
        subs
    };

    match f.editing_index {
        Some(i) if i < app.config.connections.len() => {
            conn.id = app.config.connections[i].id.clone();
            app.config.connections[i] = conn;
        }
        _ => app.config.connections.push(conn),
    }
    let _ = app.config.save();
    app.screen = Screen::Connections;
}
