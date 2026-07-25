use crate::app::{App, Screen};
use crate::plugin::PluginEvent;

impl App {
    /// Open the alert-rules editor for the active connection (if connected) or
    /// the selected one otherwise. Alerts are per connection.
    pub fn open_alerts_editor(&mut self) {
        let target = if self.handle.is_some() {
            self.active_conn().map(|c| (c.id.clone(), c.name.clone()))
        } else {
            self.config
                .connections
                .get(self.conn_selected)
                .map(|c| (c.id.clone(), c.name.clone()))
        };
        let Some((id, name)) = target else {
            self.error = Some("no connection to edit alerts for".into());
            return;
        };
        self.alert_rules = crate::plugin::load_alert_rules(&id);
        self.alert_edit_conn = id;
        self.alert_edit_name = name;
        self.alerts_selected = 0;
        self.screen = Screen::AlertRules;
    }

    /// Persist the working alert rules for the edited connection, and — if that
    /// connection is the live one — reload them into the plugin immediately.
    pub fn persist_alert_rules(&mut self) {
        if let Err(e) = crate::plugin::save_alert_rules(&self.alert_edit_conn, &self.alert_rules) {
            self.error = Some(format!("save failed: {}", e));
            return;
        }
        let is_active =
            self.active_conn().map(|c| c.id.as_str()) == Some(self.alert_edit_conn.as_str());
        if is_active {
            let id = self.alert_edit_conn.clone();
            self.dispatch_plugin(PluginEvent::Connected { connection: id });
        }
    }
}
