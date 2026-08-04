//! Demo built-in plugin exercising the plugin API surface.
//!
//! `JsonMarker` observes every received message and annotates whether its
//! payload parses as JSON, and periodically reports how many messages it has
//! seen via a status notification. It touches the two event kinds the lean
//! foundation dispatches (`MessageReceived`, `Tick`) and two action kinds
//! (`Annotate`, `Status`) — a minimal proof that the boundary works end-to-end.

use crate::plugin::api::{Annotation, PluginAction, PluginEvent, PluginMetadata, Severity};
use crate::plugin::Plugin;

const NAME: &str = "json-marker";

#[derive(Default)]
pub struct JsonMarker {
    seen: u64,
}

impl Plugin for JsonMarker {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: NAME,
            version: "0.1.0",
            description: "flags whether each payload is valid JSON",
        }
    }

    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("", "Tags every message as valid or invalid JSON."),
            (
                "",
                "Adds a ✓/✗ annotation plus a running count in the status bar.",
            ),
        ]
    }

    fn on_event(&mut self, event: &PluginEvent) -> Vec<PluginAction> {
        match event {
            PluginEvent::MessageReceived { id, payload, .. } => {
                self.seen += 1;
                if payload.trim().is_empty() {
                    return Vec::new();
                }
                let (severity, text) = if serde_json::from_str::<serde_json::Value>(payload).is_ok()
                {
                    (Severity::Ok, "valid JSON".to_string())
                } else {
                    (Severity::Info, "not JSON".to_string())
                };
                vec![PluginAction::Annotate {
                    id: *id,
                    annotation: Annotation {
                        plugin: NAME,
                        severity,
                        text,
                    },
                }]
            }
            PluginEvent::Tick => vec![PluginAction::Status(format!(
                "json-marker: {} message{} seen",
                self.seen,
                if self.seen == 1 { "" } else { "s" }
            ))],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: u64, payload: &str) -> PluginEvent {
        PluginEvent::MessageReceived {
            id,
            topic: "t".into(),
            payload: payload.into(),
            qos: 0,
            retained: false,
        }
    }

    #[test]
    fn classifies_json_vs_text() {
        let mut p = JsonMarker::default();

        let a = p.on_event(&msg(1, r#"{"a":1}"#));
        assert!(matches!(
            a.as_slice(),
            [PluginAction::Annotate { id: 1, annotation }] if annotation.severity == Severity::Ok
        ));

        let b = p.on_event(&msg(2, "hello"));
        assert!(matches!(
            b.as_slice(),
            [PluginAction::Annotate { id: 2, annotation }] if annotation.severity == Severity::Info
        ));

        // Empty payloads are not annotated.
        assert!(p.on_event(&msg(3, "   ")).is_empty());
    }

    #[test]
    fn tick_reports_count() {
        let mut p = JsonMarker::default();
        p.on_event(&msg(1, "hello"));
        p.on_event(&msg(2, "world"));
        match p.on_event(&PluginEvent::Tick).as_slice() {
            [PluginAction::Status(s)] => assert!(s.contains("2 messages seen")),
            other => panic!("expected a status action, got {other:?}"),
        }
    }
}
