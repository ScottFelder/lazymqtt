//! In-process plugin foundation.
//!
//! A plugin is a Rust type implementing [`Plugin`], registered in-tree and
//! compiled into the binary. The [`PluginHost`] owns the registered plugins,
//! dispatches [`PluginEvent`]s to them, and returns the [`PluginAction`]s they
//! emit for `App` to apply. This keeps plugins from reaching into `App`
//! internals or the render loop directly.
//!
//! Dispatch is synchronous on the UI loop, so built-in plugins must be fast.
//! Bounded queues / isolation belong to a future external-process model; this
//! module is the stable internal API those later models would sit behind.

pub mod api;
mod builtin;

pub use api::{Annotation, PluginAction, PluginContext, PluginEvent, PluginMetadata, Severity};

use directories::ProjectDirs;

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
}

/// Owns the registered plugins and fans events out to them.
pub struct PluginHost {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginHost {
    /// Build the host with the built-in plugins registered and loaded.
    pub fn with_builtins() -> Self {
        let ctx = PluginContext {
            config_dir: plugin_config_dir(),
        };
        let mut plugins: Vec<Box<dyn Plugin>> = builtin::all();
        for p in plugins.iter_mut() {
            // A plugin that fails to load is skipped rather than killing the app;
            // it simply won't receive events (its actions are never produced).
            let _ = p.on_load(&ctx);
        }
        Self { plugins }
    }

    /// Dispatch an event to every plugin, collecting all emitted actions.
    pub fn dispatch(&mut self, event: &PluginEvent) -> Vec<PluginAction> {
        let mut actions = Vec::new();
        for p in self.plugins.iter_mut() {
            actions.extend(p.on_event(event));
        }
        actions
    }

    /// Metadata for every registered plugin (for the help/plugins listing).
    pub fn metadata(&self) -> Vec<PluginMetadata> {
        self.plugins.iter().map(|p| p.metadata()).collect()
    }
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::with_builtins()
    }
}

/// Where plugin-scoped config/state lives — a `plugins/` dir beside
/// `connections.json`, kept separate from connection profiles on purpose.
fn plugin_config_dir() -> std::path::PathBuf {
    ProjectDirs::from("dev", "lazymqtt", "lazymqtt")
        .map(|d| d.config_dir().join("plugins"))
        .unwrap_or_else(|| std::path::PathBuf::from("plugins"))
}
