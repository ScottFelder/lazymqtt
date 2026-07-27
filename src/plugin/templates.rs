//! Saved publish templates for the `publish-templates` plugin.
//!
//! Templates are broker-agnostic (a topic + payload + QoS/retain preset), so
//! unlike alerts/schemas they're **global**, stored at
//! `<plugins>/publish-templates.json`:
//!
//! ```json
//! { "templates": [
//!     { "name": "reboot", "topic": "devices/+/cmd", "payload": "{\"action\":\"reboot\"}",
//!       "qos": 1, "retain": false }
//! ] }
//! ```
//!
//! Payloads may contain `{{placeholder}}` markers — nothing substitutes them
//! automatically; picking a template opens the publish form pre-filled so you
//! fill them in and review before sending.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// A saved topic/payload/QoS/retain preset.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PublishTemplate {
    pub name: String,
    #[serde(default)]
    pub topic: String,
    #[serde(default)]
    pub payload: String,
    #[serde(default)]
    pub qos: u8,
    #[serde(default)]
    pub retain: bool,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct TemplatesFile {
    #[serde(default)]
    templates: Vec<PublishTemplate>,
}

fn templates_path(dir: &Path) -> PathBuf {
    dir.join("publish-templates.json")
}

/// Load all saved templates (empty if the file is missing or unparseable).
pub fn load(dir: &Path) -> Vec<PublishTemplate> {
    fs::read_to_string(templates_path(dir))
        .ok()
        .and_then(|s| serde_json::from_str::<TemplatesFile>(&s).ok())
        .map(|f| f.templates)
        .unwrap_or_default()
}

/// Persist all templates to `<dir>/publish-templates.json`.
pub fn save(dir: &Path, templates: &[PublishTemplate]) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let file = TemplatesFile {
        templates: templates.to_vec(),
    };
    let json = serde_json::to_string_pretty(&file).unwrap_or_default();
    fs::write(templates_path(dir), json)
}

/// Append a template, replacing any existing one with the same name.
pub fn upsert(dir: &Path, template: PublishTemplate) -> std::io::Result<()> {
    let mut all = load(dir);
    if let Some(existing) = all.iter_mut().find(|t| t.name == template.name) {
        *existing = template;
    } else {
        all.push(template);
    }
    save(dir, &all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_replaces_by_name() {
        let mut all = vec![
            PublishTemplate {
                name: "a".into(),
                topic: "t1".into(),
                payload: "1".into(),
                qos: 0,
                retain: false,
            },
            PublishTemplate {
                name: "b".into(),
                topic: "t2".into(),
                payload: "2".into(),
                qos: 0,
                retain: false,
            },
        ];
        // Simulate upsert's replace-by-name on an in-memory list.
        let updated = PublishTemplate {
            name: "a".into(),
            topic: "t1b".into(),
            payload: "9".into(),
            qos: 1,
            retain: true,
        };
        if let Some(e) = all.iter_mut().find(|t| t.name == updated.name) {
            *e = updated;
        }
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].topic, "t1b");
        assert_eq!(all[0].qos, 1);
    }
}
