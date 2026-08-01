//! In-process plugin foundation.
//!
//! A plugin is a Rust type implementing [`Plugin`], registered in-tree and
//! compiled into the binary. The [`PluginHost`] owns the registered plugins,
//! dispatches [`PluginEvent`]s to them, and returns the [`PluginAction`]s they
//! emit for `App` to apply. This keeps plugins from reaching into `App`
//! internals or the render loop directly.
//!
//! Each plugin can be enabled/disabled; the state persists in a `plugins/`
//! config dir (see [`config`]). Plugins are **opt-in**: disabled by default, so
//! a plugin runs only after the user enables it. A disabled plugin stays loaded
//! but receives no events.
//!
//! Dispatch is synchronous on the UI loop, so built-in plugins must be fast.
//! Bounded queues / isolation belong to a future external-process model; this
//! module is the stable internal API those later models would sit behind.

pub mod alerts_rules;
pub mod api;
mod builtin;
mod config;
pub mod generators;
pub mod recordings;
pub mod schemas;
pub mod templates;
mod topics;

pub use alerts_rules::{AlertCondition, AlertRule, AlertSeverity};
pub use api::{
    Annotation, InspectMessage, InspectorStyle, InspectorView, PaneStyle, PaneView, PluginAction,
    PluginCommand, PluginContext, PluginEvent, PluginMetadata, Severity,
};
pub use recordings::Recording;
pub use schemas::SchemaMapping;
pub use templates::PublishTemplate;

/// Plugin name of the built-in record/replay plugin (the recordings picker
/// hands chosen recordings to it via `PluginHost::use_item`).
pub const RECORDER: &str = "topic-recorder";

use config::PluginConfig;
use std::path::PathBuf;

/// Load a connection's alert rules from the plugin config dir.
pub fn load_alert_rules(connection_id: &str) -> Vec<AlertRule> {
    alerts_rules::load(&plugin_config_dir(), connection_id)
}

/// Persist a connection's alert rules to the plugin config dir.
pub fn save_alert_rules(connection_id: &str, rules: &[AlertRule]) -> anyhow::Result<()> {
    alerts_rules::save(&plugin_config_dir(), connection_id, rules)
}

/// Load a connection's JSON Schema mappings from the plugin config dir.
pub fn load_schemas(connection_id: &str) -> Vec<SchemaMapping> {
    schemas::load(&plugin_config_dir(), connection_id)
}

/// Persist a connection's JSON Schema mappings to the plugin config dir.
pub fn save_schemas(connection_id: &str, mappings: &[SchemaMapping]) -> std::io::Result<()> {
    schemas::save(&plugin_config_dir(), connection_id, mappings)
}

/// Save (or replace by name) a global publish template.
pub fn save_publish_template(template: PublishTemplate) -> std::io::Result<()> {
    templates::upsert(&plugin_config_dir(), template)
}

/// A connection's recordings, newest first.
pub fn list_recordings(connection_id: &str) -> Vec<Recording> {
    recordings::list(&plugin_config_dir(), connection_id)
}

/// Rename a connection's recording (identified by its current label).
pub fn rename_recording(
    connection_id: &str,
    old_label: &str,
    new_label: &str,
) -> std::io::Result<()> {
    recordings::rename(&plugin_config_dir(), connection_id, old_label, new_label)
}

/// Delete a recording by path.
pub fn delete_recording(path: &std::path::Path) -> std::io::Result<()> {
    recordings::delete(path)
}

/// A recording's messages as editable text — one canonical JSON line each.
pub fn read_recording(connection_id: &str, label: &str) -> Vec<String> {
    let dir = plugin_config_dir();
    recordings::to_lines(&recordings::read(&recordings::path_for(
        &dir,
        connection_id,
        label,
    )))
}

/// Validate edited recording text (one message per line) and write it to
/// `label`. Returns a human-readable error (pointing at the offending line, or
/// the I/O failure) rather than saving invalid content.
pub fn save_recording_text(
    connection_id: &str,
    label: &str,
    lines: &[String],
) -> Result<(), String> {
    let msgs = recordings::parse_lines(lines).map_err(|(i, e)| format!("line {}: {e}", i + 1))?;
    if msgs.is_empty() {
        return Err("recording is empty".into());
    }
    recordings::write_label(&plugin_config_dir(), connection_id, label, &msgs)
        .map_err(|e| e.to_string())
}

pub trait Plugin {
    fn metadata(&self) -> PluginMetadata;

    /// Called once when the host is built. Default: nothing to do.
    fn on_load(&mut self, _ctx: &PluginContext) -> anyhow::Result<()> {
        Ok(())
    }

    /// React to an event, optionally emitting actions. Default: ignore.
    fn on_event(&mut self, _event: &PluginEvent) -> Vec<PluginAction> {
        Vec::new()
    }

    /// Optionally supply an alternative rendering of the selected payload (a
    /// structured view). Read-only; return `None` when the plugin has nothing
    /// to offer for this message. Default: no view.
    fn inspect(&self, _msg: &InspectMessage) -> Option<InspectorView> {
        None
    }

    /// Commands this plugin contributes to the `m` command menu. Called fresh
    /// each time the menu opens, so labels may reflect current state.
    fn commands(&self) -> Vec<PluginCommand> {
        Vec::new()
    }

    /// Top-level Broker-screen key hints this plugin contributes to the status
    /// bar, as (key, label) pairs. These advertise keys the plugin makes
    /// meaningful while enabled — e.g. json-view enables the `i` payload-view
    /// toggle — so they appear only when the plugin is on. Default: none.
    fn key_hints(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// Run one of this plugin's commands, by the `id` from `commands()`. Used
    /// for one-shot commands (Enter in the menu) and as the default forward step
    /// for adjustable ones.
    fn invoke(&mut self, _id: &str) -> Vec<PluginAction> {
        Vec::new()
    }

    /// Cycle an `adjustable` command's option in a direction (`forward` = next,
    /// else previous), by the `id` from `commands()`. Default: step forward via
    /// `invoke`, so a plugin whose option only cycles one way needs no override.
    fn adjust(&mut self, id: &str, _forward: bool) -> Vec<PluginAction> {
        self.invoke(id)
    }

    /// Act on a named item chosen from an app-level management screen (e.g. the
    /// recordings picker telling the recorder which recording to replay).
    /// Default: ignore — most plugins have no picker.
    fn use_item(&mut self, _name: &str) -> Vec<PluginAction> {
        Vec::new()
    }

    /// A whole-screen view this plugin owns (e.g. a dashboard). Rendered fresh
    /// each frame while its pane is open. Default: no pane. A plugin exposes its
    /// pane by returning `PluginAction::OpenPane` from a command.
    fn pane(&self) -> Option<PaneView> {
        None
    }
}

struct Slot {
    plugin: Box<dyn Plugin>,
    enabled: bool,
}

/// One row for the plugins management view.
pub struct PluginEntry {
    pub metadata: PluginMetadata,
    pub enabled: bool,
}

/// Owns the registered plugins and fans events out to the enabled ones.
pub struct PluginHost {
    slots: Vec<Slot>,
    config: PluginConfig,
    config_dir: PathBuf,
}

impl PluginHost {
    /// Build the host with the built-in plugins registered and loaded, applying
    /// persisted enable/disable state.
    pub fn with_builtins() -> Self {
        let config_dir = plugin_config_dir();
        // Tests must not read (or be perturbed by) the user's real plugin
        // config; a default (empty) config keeps them isolated. Plugins are
        // opt-in for users, but tests exercise plugin behavior, so every loaded
        // built-in is forced on under cfg!(test).
        let config = if cfg!(test) {
            PluginConfig::default()
        } else {
            PluginConfig::load(&config_dir)
        };
        let ctx = PluginContext {
            config_dir: config_dir.clone(),
        };

        let slots = builtin::all()
            .into_iter()
            .map(|mut plugin| {
                // A plugin that fails to load is disabled rather than killing
                // the app; it stays listed but receives no events. Plugins are
                // disabled until the user enables them (opt-in) — except in
                // tests, where every loaded built-in is on.
                let loaded = plugin.on_load(&ctx).is_ok();
                let enabled = loaded && (cfg!(test) || config.is_enabled(plugin.metadata().name));
                Slot { plugin, enabled }
            })
            .collect();

        Self {
            slots,
            config,
            config_dir,
        }
    }

    /// Dispatch an event to every enabled plugin, collecting all emitted actions.
    pub fn dispatch(&mut self, event: &PluginEvent) -> Vec<PluginAction> {
        let mut actions = Vec::new();
        for slot in self.slots.iter_mut().filter(|s| s.enabled) {
            actions.extend(slot.plugin.on_event(event));
        }
        actions
    }

    /// Metadata for every registered plugin (for the help listing).
    pub fn metadata(&self) -> Vec<PluginMetadata> {
        self.slots.iter().map(|s| s.plugin.metadata()).collect()
    }

    /// Alternative payload views offered by the enabled plugins, in plugin order.
    pub fn inspect(&self, msg: &InspectMessage) -> Vec<InspectorView> {
        self.slots
            .iter()
            .filter(|s| s.enabled)
            .filter_map(|s| s.plugin.inspect(msg))
            .collect()
    }

    /// Broker-screen key hints contributed by the enabled plugins, deduped by
    /// key (first wins), in plugin order. The status bar appends these so a
    /// plugin's keys appear when it is enabled and vanish when disabled.
    pub fn key_hints(&self) -> Vec<(&'static str, &'static str)> {
        let mut out: Vec<(&'static str, &'static str)> = Vec::new();
        for slot in self.slots.iter().filter(|s| s.enabled) {
            for &hint in slot.plugin.key_hints() {
                if !out.iter().any(|h| h.0 == hint.0) {
                    out.push(hint);
                }
            }
        }
        out
    }

    /// Enabled plugins that contribute at least one command, as (slot index,
    /// metadata) — used to build the command menu's per-plugin submenu entries.
    pub fn command_plugins(&self) -> Vec<(usize, PluginMetadata)> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.enabled && !s.plugin.commands().is_empty())
            .map(|(i, s)| (i, s.plugin.metadata()))
            .collect()
    }

    /// Commands contributed by the enabled plugins, each tagged with its slot
    /// index so `invoke` can target the right plugin.
    pub fn commands(&self) -> Vec<(usize, PluginCommand)> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.enabled)
            .flat_map(|(i, s)| s.plugin.commands().into_iter().map(move |c| (i, c)))
            .collect()
    }

    /// Run the plugin at `index`'s command `id`, returning its actions.
    pub fn invoke(&mut self, index: usize, id: &str) -> Vec<PluginAction> {
        match self.slots.get_mut(index) {
            Some(slot) if slot.enabled => slot.plugin.invoke(id),
            _ => Vec::new(),
        }
    }

    /// Cycle the plugin at `index`'s adjustable command `id` in a direction.
    pub fn adjust(&mut self, index: usize, id: &str, forward: bool) -> Vec<PluginAction> {
        match self.slots.get_mut(index) {
            Some(slot) if slot.enabled => slot.plugin.adjust(id, forward),
            _ => Vec::new(),
        }
    }

    /// Whether the enabled plugin named `name` exists (used to gate features
    /// that need a specific plugin, e.g. replaying from the recordings picker).
    pub fn is_enabled(&self, name: &str) -> bool {
        self.slots
            .iter()
            .any(|s| s.enabled && s.plugin.metadata().name == name)
    }

    /// Hand a named item to the enabled plugin `name` (see `Plugin::use_item`),
    /// returning its actions. No-op if the plugin isn't enabled.
    pub fn use_item(&mut self, name: &str, item: &str) -> Vec<PluginAction> {
        match self
            .slots
            .iter_mut()
            .find(|s| s.enabled && s.plugin.metadata().name == name)
        {
            Some(slot) => slot.plugin.use_item(item),
            None => Vec::new(),
        }
    }

    /// The pane view for the enabled plugin at `index`, if it provides one.
    pub fn pane(&self, index: usize) -> Option<PaneView> {
        self.slots
            .get(index)
            .filter(|s| s.enabled)
            .and_then(|s| s.plugin.pane())
    }

    /// Name + enabled state for every plugin (for the management view).
    pub fn entries(&self) -> Vec<PluginEntry> {
        self.slots
            .iter()
            .map(|s| PluginEntry {
                metadata: s.plugin.metadata(),
                enabled: s.enabled,
            })
            .collect()
    }

    /// Number of registered plugins (enabled or not).
    pub fn count(&self) -> usize {
        self.slots.len()
    }

    /// Flip the enabled state of the plugin at `index` and persist it.
    pub fn toggle(&mut self, index: usize) {
        if let Some(slot) = self.slots.get_mut(index) {
            slot.enabled = !slot.enabled;
            self.config.set(slot.plugin.metadata().name, slot.enabled);
            let _ = self.config.save(&self.config_dir);
        }
    }
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::with_builtins()
    }
}

/// Where plugin-scoped config/state lives — a `plugins/` dir beside
/// `connections.json`, kept separate from connection profiles on purpose.
fn plugin_config_dir() -> PathBuf {
    crate::paths::config_dir().join("plugins")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_hints_surface_enabled_plugin_keys() {
        // Under cfg!(test) every built-in loads enabled, so json-view's `i`
        // hint is present and deduped to a single entry.
        let host = PluginHost::with_builtins();
        let hints = host.key_hints();
        assert_eq!(hints.iter().filter(|h| h.0 == "i").count(), 1);
        assert!(hints.contains(&("i", "view")));
    }
}
