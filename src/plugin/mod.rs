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

pub use alerts_rules::{AlertCondition, AlertRule, AlertSeverity};
pub use api::{
    Annotation, InspectMessage, InspectorStyle, InspectorView, PluginAction, PluginCommand,
    PluginContext, PluginEvent, PluginMetadata, Severity,
};

use config::PluginConfig;
use directories::ProjectDirs;
use std::path::PathBuf;

/// Load a connection's alert rules from the plugin config dir.
pub fn load_alert_rules(connection_id: &str) -> Vec<AlertRule> {
    alerts_rules::load(&plugin_config_dir(), connection_id)
}

/// Persist a connection's alert rules to the plugin config dir.
pub fn save_alert_rules(connection_id: &str, rules: &[AlertRule]) -> anyhow::Result<()> {
    alerts_rules::save(&plugin_config_dir(), connection_id, rules)
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

    /// Run one of this plugin's commands, by the `id` from `commands()`.
    fn invoke(&mut self, _id: &str) -> Vec<PluginAction> {
        Vec::new()
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
                let enabled =
                    loaded && (cfg!(test) || config.is_enabled(plugin.metadata().name));
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
    ProjectDirs::from("dev", "lazymqtt", "lazymqtt")
        .map(|d| d.config_dir().join("plugins"))
        .unwrap_or_else(|| PathBuf::from("plugins"))
}
