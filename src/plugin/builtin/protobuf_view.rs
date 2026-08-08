//! Built-in protobuf structured-view plugin (`protobuf-view`).
//!
//! Protobuf is a binary wire format that carries neither field names nor exact
//! types, so decoding needs a schema. This plugin maps topics to a `.proto` +
//! message type (per connection, edited in-app — see [`crate::plugin::protos`]),
//! compiles them on `Connected`, and for a matching message decodes the raw
//! payload bytes into named fields, then renders them as JSON via the shared
//! colorizer in [`super::jsonfmt`]. The raw view always stays available.
//!
//! Read-only: it decodes for display and never mutates or re-publishes.

use crate::plugin::api::{InspectMessage, InspectorView, PluginAction, PluginContext, PluginEvent};
use crate::plugin::builtin::jsonfmt::value_to_spans;
use crate::plugin::protos;
use crate::plugin::{topics, Plugin, PluginMetadata};
use prost_reflect::{DynamicMessage, MessageDescriptor, SerializeOptions};
use std::path::PathBuf;

const NAME: &str = "protobuf-view";

#[derive(Default)]
pub struct ProtobufView {
    config_dir: PathBuf,
    active: Option<String>, // active connection id, if connected
    // The active connection's successfully-compiled mappings: (topic filter,
    // message descriptor to decode payloads as).
    compiled: Vec<(String, MessageDescriptor)>,
}

impl Plugin for ProtobufView {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: NAME,
            version: "0.1.0",
            description: "decode protobuf payloads against per-connection .proto schemas",
        }
    }

    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[
            (
                "i",
                "cycle the Payload pane between raw bytes and the decoded view",
            ),
            (
                "B",
                "open the .proto editor (map topics to a .proto + message type)",
            ),
            ("", "Decodes binary protobuf payloads into named fields."),
        ]
    }

    fn key_hints(&self) -> &'static [(&'static str, &'static str)] {
        &[("i", "view")]
    }

    fn on_load(&mut self, ctx: &PluginContext) -> anyhow::Result<()> {
        self.config_dir = ctx.config_dir.clone();
        Ok(())
    }

    fn on_event(&mut self, event: &PluginEvent) -> Vec<PluginAction> {
        match event {
            PluginEvent::Connected { connection } => {
                self.active = Some(connection.clone());
                self.recompile(connection)
            }
            PluginEvent::Disconnected(_) => {
                self.active = None;
                self.compiled.clear();
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn inspect(&self, msg: &InspectMessage) -> Option<InspectorView> {
        let (_, descriptor) = self
            .compiled
            .iter()
            .find(|(filter, _)| topics::matches(filter, &msg.topic))?;
        let dynamic =
            DynamicMessage::decode(descriptor.clone(), msg.payload_raw.as_slice()).ok()?;
        // Proto field names (not camelCase) and enum names (not numbers), and
        // keep default-valued fields so the whole message is visible.
        let opts = SerializeOptions::new()
            .use_proto_field_name(true)
            .skip_default_fields(false);
        let value: serde_json::Value = dynamic
            .serialize_with_options(serde_json::value::Serializer, &opts)
            .ok()?;
        Some(InspectorView {
            label: "Protobuf".to_string(),
            lines: value_to_spans(&value),
        })
    }
}

impl ProtobufView {
    /// (Re)compile the connection's mappings, keeping the ones that compile and
    /// reporting how many failed (they were already validated in the editor, so
    /// a failure here usually means an out-of-band edit).
    fn recompile(&mut self, connection: &str) -> Vec<PluginAction> {
        let mappings = protos::load(&self.config_dir, connection);
        self.compiled.clear();
        let mut failed = 0;
        for (i, mapping) in mappings.iter().enumerate() {
            match protos::compile(&self.config_dir, connection, i, mapping) {
                Ok(desc) => self.compiled.push((mapping.topic.clone(), desc)),
                Err(_) => failed += 1,
            }
        }
        if failed > 0 {
            vec![PluginAction::Status(format!(
                "protobuf: {failed} schema(s) failed to compile"
            ))]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::protos::{compile, ProtoMapping};
    use std::path::Path;

    fn tmp_dir() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let d = std::env::temp_dir().join(format!(
            "lazymqtt-proto-test-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    // A plugin with one compiled mapping for `sensors/#` →
    // `message Reading { int32 temp = 1; string unit = 2; }`.
    fn reading_plugin(dir: &Path) -> ProtobufView {
        let mapping = ProtoMapping {
            topic: "sensors/#".into(),
            message_type: "sensors.Reading".into(),
            proto: "syntax = \"proto3\";\npackage sensors;\n\
                    message Reading { int32 temp = 1; string unit = 2; }"
                .into(),
        };
        let desc = compile(dir, "conn", 0, &mapping).expect("proto compiles");
        ProtobufView {
            config_dir: dir.to_path_buf(),
            active: Some("conn".into()),
            compiled: vec![(mapping.topic.clone(), desc)],
        }
    }

    fn view_for(p: &ProtobufView, topic: &str, bytes: &[u8]) -> Option<InspectorView> {
        p.inspect(&InspectMessage {
            topic: topic.into(),
            payload: String::new(),
            payload_raw: bytes.to_vec(),
            qos: 0,
            retained: false,
        })
    }

    #[test]
    fn decodes_named_fields() {
        let dir = tmp_dir();
        let p = reading_plugin(&dir);
        // Wire bytes: field 1 int32 = 21 (0x08,0x15); field 2 string "C" (0x12,0x01,0x43).
        let view = view_for(&p, "sensors/a", &[0x08, 0x15, 0x12, 0x01, 0x43]).expect("decodes");
        assert_eq!(view.label, "Protobuf");
        let text: String = view
            .lines
            .iter()
            .flatten()
            .map(|s| s.text.as_str())
            .collect();
        assert!(text.contains("temp"), "got: {text}");
        assert!(text.contains("21"), "got: {text}");
        assert!(text.contains("unit"), "got: {text}");
        assert!(text.contains('C'), "got: {text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unmapped_topic_yields_no_view() {
        let dir = tmp_dir();
        let p = reading_plugin(&dir);
        assert!(view_for(&p, "other/topic", &[0x08, 0x15]).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compile_reports_unknown_message_type() {
        let dir = tmp_dir();
        let mapping = ProtoMapping {
            topic: "t".into(),
            message_type: "nope.Missing".into(),
            proto: "syntax = \"proto3\";\nmessage Reading { int32 x = 1; }".into(),
        };
        assert!(compile(&dir, "conn", 0, &mapping).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
