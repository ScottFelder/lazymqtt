use crate::app::{App, Screen};
use crate::config::Subscription;
use crate::mqtt::MqttCommand;
use crossterm::event::{KeyCode, KeyEvent};

pub(crate) fn clear_retained_keys(app: &mut App, key: KeyEvent) {
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

pub(crate) fn subscribe_keys(app: &mut App, key: KeyEvent) {
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

pub(crate) fn publish_keys(app: &mut App, key: KeyEvent) {
    // "Save as template" mode: a name prompt over the publish form.
    if app.publish_save_as.is_some() {
        match key.code {
            KeyCode::Esc => app.publish_save_as = None,
            KeyCode::Enter => app.commit_publish_template(),
            KeyCode::Backspace => {
                if let Some(buf) = app.publish_save_as.as_mut() {
                    buf.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(buf) = app.publish_save_as.as_mut() {
                    buf.push(c);
                }
            }
            _ => {}
        }
        return;
    }

    let ctrl = key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL);
    if ctrl && matches!(key.code, KeyCode::Char('t')) {
        app.publish_save_as = Some(String::new()); // start the template-name prompt
        return;
    }

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
