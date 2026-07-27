use crate::app::{App, Screen, TextArea};
use crate::plugin::Recording;

impl App {
    /// Open the recordings picker for the active connection (if connected) or the
    /// selected one otherwise. Recordings are per connection.
    pub fn open_recordings(&mut self) {
        let target = if self.handle.is_some() {
            self.active_conn().map(|c| (c.id.clone(), c.name.clone()))
        } else {
            self.config
                .connections
                .get(self.conn_selected)
                .map(|c| (c.id.clone(), c.name.clone()))
        };
        let Some((id, name)) = target else {
            self.error = Some("no connection to manage recordings for".into());
            return;
        };
        self.recordings_conn = id;
        self.recordings_conn_name = name;
        self.recording_rename = None;
        self.reload_recordings();
        self.recordings_selected = 0;
        self.screen = Screen::Recordings;
    }

    /// Re-read the recordings list for the picker's connection, keeping the
    /// cursor in range.
    pub fn reload_recordings(&mut self) {
        self.recordings = crate::plugin::list_recordings(&self.recordings_conn);
        self.recordings_selected = self
            .recordings_selected
            .min(self.recordings.len().saturating_sub(1));
    }

    /// The recording the picker cursor is on, if any.
    pub fn selected_recording(&self) -> Option<&Recording> {
        self.recordings.get(self.recordings_selected)
    }

    /// Replay the selected recording now (hands it to the recorder plugin), then
    /// return to the broker. Requires the recorder plugin to be enabled and a
    /// live connection.
    pub fn replay_selected_recording(&mut self) {
        let Some(label) = self.selected_recording().map(|r| r.label.clone()) else {
            return;
        };
        if !self.plugins.is_enabled(crate::plugin::RECORDER) {
            self.error = Some("enable the topic-recorder plugin to replay".into());
            return;
        }
        let actions = self.plugins.use_item(crate::plugin::RECORDER, &label);
        self.apply_plugin_actions(actions);
        self.screen = Screen::Broker;
    }

    /// Begin renaming the selected recording (seeds the buffer with its label).
    pub fn begin_recording_rename(&mut self) {
        if let Some(rec) = self.selected_recording() {
            self.recording_rename = Some(rec.label.clone());
        }
    }

    /// Apply the in-progress rename to the selected recording, then reload.
    pub fn apply_recording_rename(&mut self) {
        let Some(new_label) = self.recording_rename.take() else {
            return;
        };
        let Some(old_label) = self.selected_recording().map(|r| r.label.clone()) else {
            return;
        };
        if new_label.trim().is_empty() || new_label == old_label {
            self.reload_recordings();
            return;
        }
        match crate::plugin::rename_recording(&self.recordings_conn, &old_label, &new_label) {
            Ok(()) => self.error = Some(format!("renamed to {new_label}")),
            Err(e) => self.error = Some(format!("rename failed: {e}")),
        }
        self.reload_recordings();
    }

    /// Delete the selected recording, then reload.
    pub fn delete_selected_recording(&mut self) {
        let Some(rec) = self.selected_recording() else {
            return;
        };
        let (path, label) = (rec.path.clone(), rec.label.clone());
        match crate::plugin::delete_recording(&path) {
            Ok(()) => self.error = Some(format!("deleted {label}")),
            Err(e) => self.error = Some(format!("delete failed: {e}")),
        }
        self.reload_recordings();
    }

    // ---- Recording editor -------------------------------------------------

    /// Open the multi-line editor on the selected recording's contents.
    pub fn open_recording_edit(&mut self) {
        let Some(label) = self.selected_recording().map(|r| r.label.clone()) else {
            return;
        };
        let lines = crate::plugin::read_recording(&self.recordings_conn, &label);
        self.rec_editor = TextArea::from_lines(lines);
        self.rec_edit_label = label;
        self.rec_edit_error = None;
        self.rec_edit_saveas = None;
        self.screen = Screen::RecordingEdit;
    }

    /// Save the editor's contents to `label`. Validates the JSONL first; on
    /// failure keeps the editor open with the error shown, otherwise reloads the
    /// picker and returns to it.
    pub fn save_recording_edit(&mut self, label: &str) {
        match crate::plugin::save_recording_text(
            &self.recordings_conn,
            label,
            &self.rec_editor.lines,
        ) {
            Ok(()) => {
                self.error = Some(format!("saved {label}"));
                self.rec_edit_label = label.to_string();
                self.reload_recordings();
                self.screen = Screen::Recordings;
            }
            Err(e) => self.rec_edit_error = Some(e),
        }
    }

    /// Save to the recording currently being edited (overwrite).
    pub fn save_recording_edit_current(&mut self) {
        self.save_recording_edit(&self.rec_edit_label.clone());
    }

    /// Enter "save as" mode, seeding the filename with an `-edited` suffix.
    pub fn begin_recording_saveas(&mut self) {
        self.rec_edit_saveas = Some(format!("{}-edited", self.rec_edit_label));
    }

    /// Commit the "save as": write the editor contents to the new label.
    pub fn commit_recording_saveas(&mut self) {
        let Some(label) = self.rec_edit_saveas.take() else {
            return;
        };
        if label.trim().is_empty() {
            self.rec_edit_error = Some("name required".into());
            return;
        }
        self.save_recording_edit(&label);
    }
}
