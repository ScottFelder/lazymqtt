//! Persistent, per-plugin enable/disable state.
//!
//! Kept in a `plugins/` directory beside `connections.json` (never mixed into
//! connection profiles). Only the enabled flag is stored today; opaque
//! per-plugin settings can join `PluginConfig` later without a format break.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PluginConfig {
    /// plugin name -> enabled. Absent means "enabled" (opt-out model).
    #[serde(default)]
    enabled: HashMap<String, bool>,
}

impl PluginConfig {
    fn file(dir: &Path) -> PathBuf {
        dir.join("config.json")
    }

    pub fn load(dir: &Path) -> Self {
        match fs::read_to_string(Self::file(dir)) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir).ok();
        fs::write(Self::file(dir), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Whether a plugin should receive events. Unknown plugins default to on.
    pub fn is_enabled(&self, name: &str) -> bool {
        *self.enabled.get(name).unwrap_or(&true)
    }

    pub fn set(&mut self, name: &str, enabled: bool) {
        self.enabled.insert(name.to_string(), enabled);
    }
}
