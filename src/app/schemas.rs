//! App-side logic for the JSON Schema editor (`Screen::Schemas` /
//! `Screen::SchemaForm`). Mirrors the alert-rules editor: a per-connection list
//! of topic→schema mappings, edited in-app and persisted to disk, with the live
//! plugin reloaded when the edited connection is the active one.

use crate::app::{App, SchemaForm, Screen};
use crate::plugin::{PluginEvent, SchemaMapping};

impl App {
    /// Open the schema editor for the active connection (if connected) or the
    /// selected one otherwise. Schemas are per connection.
    pub fn open_schemas(&mut self) {
        let target = if self.handle.is_some() {
            self.active_conn().map(|c| (c.id.clone(), c.name.clone()))
        } else {
            self.config
                .connections
                .get(self.conn_selected)
                .map(|c| (c.id.clone(), c.name.clone()))
        };
        let Some((id, name)) = target else {
            self.error = Some("no connection to edit schemas for".into());
            return;
        };
        self.schemas = crate::plugin::load_schemas(&id);
        self.schema_edit_conn = id;
        self.schema_edit_name = name;
        self.schemas_selected = 0;
        self.screen = Screen::Schemas;
    }

    pub fn selected_schema(&self) -> Option<&SchemaMapping> {
        self.schemas.get(self.schemas_selected)
    }

    /// Open the form to add a new mapping.
    pub fn begin_schema_add(&mut self) {
        self.schema_form = SchemaForm::default();
        self.screen = Screen::SchemaForm;
    }

    /// Open the form to edit the selected mapping.
    pub fn begin_schema_edit(&mut self) {
        if let Some(mapping) = self.selected_schema() {
            self.schema_form = SchemaForm::from_mapping(self.schemas_selected, mapping);
            self.screen = Screen::SchemaForm;
        }
    }

    /// Delete the selected mapping and persist.
    pub fn delete_selected_schema(&mut self) {
        if self.schemas_selected < self.schemas.len() {
            self.schemas.remove(self.schemas_selected);
            self.schemas_selected = self
                .schemas_selected
                .min(self.schemas.len().saturating_sub(1));
            self.persist_schemas();
        }
    }

    /// Validate and save the form; on success add/replace the mapping, persist,
    /// and return to the list. On failure keep the form open with the error.
    pub fn save_schema_form(&mut self) {
        match self.schema_form.to_mapping() {
            Ok(mapping) => {
                match self.schema_form.editing_index {
                    Some(i) if i < self.schemas.len() => self.schemas[i] = mapping,
                    _ => self.schemas.push(mapping),
                }
                self.persist_schemas();
                self.screen = Screen::Schemas;
            }
            Err(e) => self.schema_form.error = Some(e),
        }
    }

    /// Persist the working mappings for the edited connection, reloading the
    /// live plugin when that connection is the active one (so validation picks
    /// up the change immediately).
    fn persist_schemas(&mut self) {
        if let Err(e) = crate::plugin::save_schemas(&self.schema_edit_conn, &self.schemas) {
            self.error = Some(format!("save failed: {e}"));
            return;
        }
        let is_active =
            self.active_conn().map(|c| c.id.as_str()) == Some(self.schema_edit_conn.as_str());
        if is_active {
            let id = self.schema_edit_conn.clone();
            self.dispatch_plugin(PluginEvent::Connected { connection: id });
        }
    }
}
