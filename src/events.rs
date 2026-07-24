use crate::app::{App, Focus, FormBuffer, PublishBuffer, Screen, Status};
use crate::config::{Connection, Subscription};
use crate::mqtt::{self, MqttCommand};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    app.error = None;
    match app.screen {
        Screen::Connections => connections_keys(app, key),
        Screen::ConnectionForm => form_keys(app, key),
        Screen::Broker => broker_keys(app, key),
        Screen::Publish => publish_keys(app, key),
        Screen::Subscribe => subscribe_keys(app, key),
        Screen::ClearRetained => clear_retained_keys(app, key),
        Screen::Help => {
            app.screen = if app.handle.is_some() {
                Screen::Broker
            } else {
                Screen::Connections
            };
        }
    }
}

/// Insert clipboard text (from a bracketed paste) into the focused input.
/// The publish payload keeps newlines; single-line fields strip them, and the
/// port field keeps only digits — matching the per-key typing rules.
pub fn handle_paste(app: &mut App, data: String) {
    app.error = None;
    match app.screen {
        Screen::Publish => match app.publish.field {
            0 => app.publish.topic.push_str(&strip_newlines(&data)),
            1 => app.publish.payload.push_str(&data),
            _ => {}
        },
        Screen::Subscribe => app.sub_input.push_str(&strip_newlines(&data)),
        Screen::ConnectionForm => {
            let is_port = app.form.field == 2;
            if let Some(s) = field_mut(&mut app.form) {
                if is_port {
                    s.extend(data.chars().filter(|c| c.is_ascii_digit()));
                } else {
                    s.push_str(&strip_newlines(&data));
                }
            }
        }
        _ => {}
    }
}

fn strip_newlines(s: &str) -> String {
    s.chars().filter(|c| *c != '\n' && *c != '\r').collect()
}

fn connections_keys(app: &mut App, key: KeyEvent) {
    let len = app.config.connections.len();
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('?') => app.screen = Screen::Help,
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

fn form_keys(app: &mut App, key: KeyEvent) {
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

fn field_mut(f: &mut FormBuffer) -> Option<&mut String> {
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

fn broker_keys(app: &mut App, key: KeyEvent) {
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
            app.should_quit = true;
        }
        KeyCode::Char('?') => app.screen = Screen::Help,
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
        KeyCode::Char('c') => {
            app.tree.clear();
            app.history.clear();
            app.expanded.clear();
            app.tree_selected = 0;
            app.reset_message_view();
        }
        KeyCode::Char('p') => {
            app.publish = PublishBuffer::default();
            if let Some(r) = rows.get(app.tree_selected) {
                app.publish.topic = r.path.clone();
            }
            app.screen = Screen::Publish;
        }
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
        KeyCode::Char('s') => {
            app.sub_input.clear();
            app.screen = Screen::Subscribe;
        }
        KeyCode::Char('u') => {
            if let Some(r) = rows.get(app.tree_selected) {
                let topic = r.path.clone();
                app.send(MqttCommand::Unsubscribe {
                    topic: topic.clone(),
                });
                if let Some(idx) = app.active {
                    if let Some(c) = app.config.connections.get_mut(idx) {
                        c.subscriptions.retain(|s| s.topic != topic);
                        let _ = app.config.save();
                    }
                }
                app.error = Some(format!("unsubscribed from {}", topic));
            }
        }
        KeyCode::Char('r') => {
            if let Some(r) = rows.get(app.tree_selected) {
                app.clear_topic = r.path.clone();
                app.screen = Screen::ClearRetained;
            }
        }
        KeyCode::Char('z') => app.toggle_pane_fold(),
        _ => {}
    }
}

fn clear_retained_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            let topic = app.clear_topic.clone();
            // Clearing a retained message = publish a zero-byte retained payload
            // to the same topic. The broker then drops the stored message.
            app.send(MqttCommand::Publish {
                topic: topic.clone(),
                payload: String::new(),
                qos: 0,
                retain: true,
            });
            app.error = Some(format!("cleared retained message on {}", topic));
            app.screen = Screen::Broker;
        }
        _ => app.screen = Screen::Broker,
    }
}

fn subscribe_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.screen = Screen::Broker,
        KeyCode::Backspace => {
            app.sub_input.pop();
        }
        KeyCode::Char(c) => app.sub_input.push(c),
        KeyCode::Enter => {
            let topic = app.sub_input.trim().to_string();
            if topic.is_empty() {
                app.screen = Screen::Broker;
                return;
            }
            app.send(MqttCommand::Subscribe {
                topic: topic.clone(),
                qos: 0,
            });
            if let Some(idx) = app.active {
                if let Some(c) = app.config.connections.get_mut(idx) {
                    if !c.subscriptions.iter().any(|s| s.topic == topic) {
                        c.subscriptions.push(Subscription::new(topic.clone()));
                        let _ = app.config.save();
                    }
                }
            }
            app.error = Some(format!("subscribed to {}", topic));
            app.screen = Screen::Broker;
        }
        _ => {}
    }
}

fn publish_keys(app: &mut App, key: KeyEvent) {
    let pb = &mut app.publish;
    match key.code {
        KeyCode::Esc => app.screen = Screen::Broker,
        KeyCode::Tab | KeyCode::Down => pb.field = (pb.field + 1) % 4,
        KeyCode::BackTab | KeyCode::Up => pb.field = (pb.field + 3) % 4,
        KeyCode::Char(' ') if pb.field == 2 => pb.qos = (pb.qos + 1) % 3,
        KeyCode::Char(' ') if pb.field == 3 => pb.retain = !pb.retain,
        KeyCode::Char(c) => match pb.field {
            0 => pb.topic.push(c),
            1 => pb.payload.push(c),
            _ => {}
        },
        KeyCode::Backspace => match pb.field {
            0 => {
                pb.topic.pop();
            }
            1 => {
                pb.payload.pop();
            }
            _ => {}
        },
        KeyCode::Enter => {
            if pb.topic.trim().is_empty() {
                app.error = Some("Topic required to publish".into());
                return;
            }
            let cmd = MqttCommand::Publish {
                topic: pb.topic.clone(),
                payload: pb.payload.clone(),
                qos: pb.qos,
                retain: pb.retain,
            };
            app.send(cmd);
            app.screen = Screen::Broker;
        }
        _ => {}
    }
}
