//! Built-in JSON structured-view plugin.
//!
//! When the selected payload parses as JSON, offers a pretty-printed view
//! (2-space indented) as an alternative to the raw text. The core keeps the
//! raw view available alongside it, so a payload is never hidden.

use crate::plugin::api::{InspectMessage, InspectorView, PluginMetadata};
use crate::plugin::Plugin;

pub struct JsonView;

impl Plugin for JsonView {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "json-view",
            version: "0.1.0",
            description: "pretty-prints JSON payloads in the Payload pane",
        }
    }

    fn inspect(&self, msg: &InspectMessage) -> Option<InspectorView> {
        let value: serde_json::Value = serde_json::from_str(&msg.payload).ok()?;
        let pretty = serde_json::to_string_pretty(&value).ok()?;
        // Keep each pretty line intact (indentation included) so a yank of the
        // structured view still produces valid, re-pasteable JSON.
        let lines = pretty.lines().map(|l| l.to_string()).collect();
        Some(InspectorView {
            label: "JSON".to_string(),
            lines,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inspect(payload: &str) -> Option<InspectorView> {
        JsonView.inspect(&InspectMessage {
            topic: "t".into(),
            payload: payload.into(),
            qos: 0,
            retained: false,
        })
    }

    #[test]
    fn pretty_prints_valid_json() {
        let view = inspect(r#"{"a":1,"b":[2,3]}"#).expect("valid json yields a view");
        assert_eq!(view.label, "JSON");
        // Pretty output spans multiple indented lines.
        assert!(view.lines.len() > 1);
        assert!(view.lines.iter().any(|l| l.contains("\"a\": 1")));
        assert!(view.lines.iter().any(|l| l.starts_with("  ")));
    }

    #[test]
    fn ignores_non_json() {
        assert!(inspect("just text").is_none());
        assert!(inspect("").is_none());
    }
}
