use crate::app::{App, PublishBuffer, Screen, Status};
use crate::config::Connection;
use crate::mqtt::{Message, MqttCommand};
use crate::plugin::{Annotation, PluginAction, PluginEvent};

impl App {
    pub fn active_conn(&self) -> Option<&Connection> {
        self.active.and_then(|i| self.config.connections.get(i))
    }

    pub fn send(&self, cmd: MqttCommand) {
        if let Some(h) = &self.handle {
            let _ = h.commands.send(cmd);
        }
    }

    pub fn push_message(&mut self, mut msg: Message) {
        self.next_message_id += 1;
        msg.id = self.next_message_id;

        // Build the plugin event from owned data so dispatch never borrows self.
        let event = PluginEvent::MessageReceived {
            id: msg.id,
            topic: msg.topic.clone(),
            payload: msg.payload.clone(),
            qos: msg.qos,
            retained: msg.retained,
        };

        self.tree.insert(msg.clone());
        self.history.push(msg);
        if self.history.len() > 5000 {
            self.history.drain(0..1000);
            // Drop annotations for messages that just fell out of history.
            if let Some(oldest) = self.history.first().map(|m| m.id) {
                self.annotations.retain(|&id, _| id >= oldest);
            }
        }

        let actions = self.plugins.dispatch(&event);
        self.apply_plugin_actions(actions);
    }

    /// Dispatch a plugin event and apply whatever actions come back. Used for
    /// the lifecycle/tick/shutdown hooks driven from the main loop.
    pub fn dispatch_plugin(&mut self, event: PluginEvent) {
        let actions = self.plugins.dispatch(&event);
        self.apply_plugin_actions(actions);
    }

    pub(crate) fn apply_plugin_actions(&mut self, actions: Vec<PluginAction>) {
        for action in actions {
            match action {
                PluginAction::Annotate { id, annotation } => {
                    self.annotations.entry(id).or_default().push(annotation);
                }
                PluginAction::Publish {
                    topic,
                    payload,
                    qos,
                    retain,
                } => {
                    self.send(MqttCommand::Publish {
                        topic,
                        payload,
                        qos,
                        retain,
                    });
                }
                PluginAction::Subscribe { topic, qos } => {
                    self.send(MqttCommand::Subscribe { topic, qos });
                }
                PluginAction::Unsubscribe { topic } => {
                    self.send(MqttCommand::Unsubscribe { topic });
                }
                PluginAction::OpenPublish {
                    topic,
                    payload,
                    qos,
                    retain,
                } => {
                    self.publish = PublishBuffer {
                        topic,
                        payload,
                        qos,
                        retain,
                        field: 0,
                    };
                    self.publish_save_as = None;
                    self.screen = Screen::Publish;
                }
                PluginAction::Status(text) => self.error = Some(text),
            }
        }
    }

    /// Annotations attached to a given message id, in the order added.
    pub fn annotations_for(&self, id: u64) -> &[Annotation] {
        self.annotations
            .get(&id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn disconnect(&mut self) {
        self.send(MqttCommand::Disconnect);
        self.handle = None;
        self.active = None;
        self.status = Status::Idle;
        self.tree.clear();
        self.history.clear();
        self.expanded.clear();
        self.tree_selected = 0;
        self.annotations.clear();
        self.reset_message_view();
    }
}
