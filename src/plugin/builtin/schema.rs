//! Built-in JSON Schema validator (`json-schema`).
//!
//! Validates each incoming message whose topic matches a per-connection schema
//! mapping, annotating it valid (Ok) or invalid (Error, with the failing path)
//! and raising a status notification on failure. Mappings are stored per
//! connection (`plugins/schemas/<connection-id>.json`, hand-editable) and
//! loaded on `Connected`, cleared on `Disconnected`. Purely observational — it
//! never alters the message or runs anything.
//!
//! Schema/validator details live in [`crate::plugin::schemas`].

use crate::plugin::api::{Annotation, PluginAction, PluginContext, PluginEvent, PluginMetadata};
use crate::plugin::schemas::{self, SchemaMapping};
use crate::plugin::{topics, Plugin, Severity};
use std::path::PathBuf;

const NAME: &str = "json-schema";

#[derive(Default)]
pub struct SchemaValidator {
    config_dir: PathBuf,
    active: Option<String>,       // active connection id, if connected
    mappings: Vec<SchemaMapping>, // the active connection's topic → schema map
}

impl Plugin for SchemaValidator {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: NAME,
            version: "0.1.0",
            description: "validate messages against per-connection JSON Schemas",
        }
    }

    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("S", "open the JSON Schema editor"),
            (
                "",
                "Validates each payload against a per-connection schema, by topic.",
            ),
            ("", "Messages that fail validation get an error annotation."),
        ]
    }

    fn on_load(&mut self, ctx: &PluginContext) -> anyhow::Result<()> {
        // Mappings load per connection on Connected; just remember where they live.
        self.config_dir = ctx.config_dir.clone();
        Ok(())
    }

    fn on_event(&mut self, event: &PluginEvent) -> Vec<PluginAction> {
        match event {
            PluginEvent::Connected { connection } => {
                self.active = Some(connection.clone());
                self.mappings = schemas::load(&self.config_dir, connection);
                Vec::new()
            }
            PluginEvent::Disconnected(_) => {
                self.active = None;
                self.mappings.clear();
                Vec::new()
            }
            PluginEvent::MessageReceived {
                id, topic, payload, ..
            } if self.active.is_some() => self.on_message(*id, topic, payload),
            _ => Vec::new(),
        }
    }
}

impl SchemaValidator {
    /// Validate a message against the first schema whose topic filter matches;
    /// topics with no mapping are left unannotated.
    fn on_message(&self, id: u64, topic: &str, payload: &str) -> Vec<PluginAction> {
        let Some(mapping) = self
            .mappings
            .iter()
            .find(|m| topics::matches(&m.topic, topic))
        else {
            return Vec::new();
        };
        match serde_json::from_str(payload) {
            Err(_) => invalid(id, "payload is not valid JSON".to_string()),
            Ok(value) => match schemas::validate(&value, &mapping.schema) {
                Ok(()) => vec![annotate(id, Severity::Ok, "valid".to_string())],
                Err(reason) => invalid(id, reason),
            },
        }
    }
}

fn annotate(id: u64, severity: Severity, text: String) -> PluginAction {
    PluginAction::Annotate {
        id,
        annotation: Annotation {
            plugin: NAME,
            severity,
            text,
        },
    }
}

/// An Error annotation on the message plus a status notification. The
/// annotation text is just the reason — the renderer prefixes the plugin name;
/// the status line adds "schema invalid" since it shows no plugin name.
fn invalid(id: u64, reason: String) -> Vec<PluginAction> {
    vec![
        annotate(id, Severity::Error, reason.clone()),
        PluginAction::Status(format!("schema invalid: {reason}")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plugin(mappings: Vec<SchemaMapping>) -> SchemaValidator {
        SchemaValidator {
            active: Some("c".into()),
            mappings,
            ..Default::default()
        }
    }

    fn mapping(topic: &str, schema: serde_json::Value) -> SchemaMapping {
        SchemaMapping {
            topic: topic.into(),
            schema,
        }
    }

    #[test]
    fn valid_message_annotates_ok() {
        let p = plugin(vec![mapping(
            "sensors/#",
            json!({ "type": "object", "required": ["temp"] }),
        )]);
        let actions = p.on_message(1, "sensors/a", r#"{"temp":21}"#);
        assert!(matches!(
            actions.as_slice(),
            [PluginAction::Annotate { id: 1, annotation }] if annotation.severity == Severity::Ok
        ));
    }

    #[test]
    fn invalid_message_annotates_error_and_status() {
        let p = plugin(vec![mapping(
            "sensors/#",
            json!({ "type": "object", "required": ["temp"] }),
        )]);
        let actions = p.on_message(2, "sensors/a", r#"{"humidity":50}"#);
        assert!(actions
            .iter()
            .any(|a| matches!(a, PluginAction::Annotate { id: 2, annotation } if annotation.severity == Severity::Error)));
        assert!(actions.iter().any(|a| matches!(a, PluginAction::Status(_))));
    }

    #[test]
    fn non_json_payload_is_invalid() {
        let p = plugin(vec![mapping("cfg", json!({ "type": "object" }))]);
        let actions = p.on_message(3, "cfg", "not json");
        assert!(actions
            .iter()
            .any(|a| matches!(a, PluginAction::Annotate { annotation, .. } if annotation.severity == Severity::Error)));
    }

    #[test]
    fn unmapped_topic_is_ignored() {
        let p = plugin(vec![mapping("sensors/#", json!({ "type": "object" }))]);
        assert!(p.on_message(4, "other/topic", "anything").is_empty());
    }

    #[test]
    fn disconnected_ignores_messages() {
        let mut p = plugin(vec![mapping("sensors/#", json!({ "type": "object" }))]);
        p.on_event(&PluginEvent::Disconnected("bye".into()));
        assert!(p.active.is_none());
        assert!(p
            .on_event(&PluginEvent::MessageReceived {
                id: 1,
                topic: "sensors/a".into(),
                payload: r#"{"x":1}"#.into(),
                payload_raw: br#"{"x":1}"#.to_vec(),
                qos: 0,
                retained: false,
            })
            .is_empty());
    }
}
