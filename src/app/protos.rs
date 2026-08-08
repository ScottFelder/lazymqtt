//! App-side logic for the protobuf mapping editor (`Screen::Protos` /
//! `Screen::ProtoForm`). Mirrors the JSON Schema editor: a per-connection list
//! of topic→(`.proto`, message type) mappings, edited in-app and persisted to
//! disk, with the live `protobuf-view` plugin reloaded when the edited
//! connection is the active one.

use crate::app::{App, ProtoForm, Screen};
use crate::plugin::{compile_proto, load_protos, save_protos, PluginEvent};

impl App {
    /// Open the protobuf editor for the active connection (if connected) or the
    /// selected one otherwise. Mappings are per connection.
    pub fn open_protos(&mut self) {
        let target = if self.handle.is_some() {
            self.active_conn().map(|c| (c.id.clone(), c.name.clone()))
        } else {
            self.config
                .connections
                .get(self.conn_selected)
                .map(|c| (c.id.clone(), c.name.clone()))
        };
        let Some((id, name)) = target else {
            self.error = Some("no connection to edit protobuf schemas for".into());
            return;
        };
        self.proto_mappings = load_protos(&id);
        self.proto_edit_conn = id;
        self.proto_edit_name = name;
        self.protos_selected = 0;
        self.screen = Screen::Protos;
    }

    /// Open the form to add a new mapping.
    pub fn begin_proto_add(&mut self) {
        self.proto_form = ProtoForm::default();
        self.screen = Screen::ProtoForm;
    }

    /// Open the form to edit the selected mapping.
    pub fn begin_proto_edit(&mut self) {
        if let Some(mapping) = self.proto_mappings.get(self.protos_selected) {
            self.proto_form = ProtoForm::from_mapping(self.protos_selected, mapping);
            self.screen = Screen::ProtoForm;
        }
    }

    /// Delete the selected mapping and persist.
    pub fn delete_selected_proto(&mut self) {
        if self.protos_selected < self.proto_mappings.len() {
            self.proto_mappings.remove(self.protos_selected);
            self.protos_selected = self
                .protos_selected
                .min(self.proto_mappings.len().saturating_sub(1));
            self.persist_protos();
        }
    }

    /// Validate the form (assemble + compile the `.proto`) and, on success,
    /// add/replace the mapping, persist, and return to the list. On failure keep
    /// the form open showing the compile error.
    pub fn save_proto_form(&mut self) {
        let mapping = match self.proto_form.to_mapping() {
            Ok(m) => m,
            Err(e) => {
                self.proto_form.error = Some(e);
                return;
            }
        };
        // Compile as a validation step so the user sees .proto errors here.
        let slot = self
            .proto_form
            .editing_index
            .unwrap_or(self.proto_mappings.len());
        if let Err(e) = compile_proto(&self.proto_edit_conn, slot, &mapping) {
            self.proto_form.error = Some(e);
            return;
        }
        match self.proto_form.editing_index {
            Some(i) if i < self.proto_mappings.len() => self.proto_mappings[i] = mapping,
            _ => self.proto_mappings.push(mapping),
        }
        self.persist_protos();
        self.screen = Screen::Protos;
    }

    /// Persist the working mappings for the edited connection, reloading the
    /// live plugin when that connection is the active one (so decoding picks up
    /// the change immediately).
    fn persist_protos(&mut self) {
        if let Err(e) = save_protos(&self.proto_edit_conn, &self.proto_mappings) {
            self.error = Some(format!("save failed: {e}"));
            return;
        }
        let is_active =
            self.active_conn().map(|c| c.id.as_str()) == Some(self.proto_edit_conn.as_str());
        if is_active {
            let id = self.proto_edit_conn.clone();
            self.dispatch_plugin(PluginEvent::Connected { connection: id });
        }
    }
}
