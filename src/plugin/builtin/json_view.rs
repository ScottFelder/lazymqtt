//! Built-in JSON structured-view plugin.
//!
//! When the selected payload parses as JSON, offers a pretty-printed,
//! syntax-colored view as an alternative to the raw text. The core keeps the
//! raw view available alongside it, so a payload is never hidden.
//!
//! The parsed `serde_json::Value` is rendered by the shared colorizer in
//! [`super::jsonfmt`]: concatenating a line's span texts reproduces standard
//! 2-space pretty JSON, so a yank of the view still yields valid JSON.

use crate::plugin::api::{InspectMessage, InspectorView};
use crate::plugin::builtin::jsonfmt::value_to_spans;
use crate::plugin::Plugin;
use crate::plugin::PluginMetadata;
use serde_json::Value;

pub struct JsonView;

impl Plugin for JsonView {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "json-view",
            version: "0.1.0",
            description: "pretty-prints JSON payloads with syntax colors",
        }
    }

    fn inspect(&self, msg: &InspectMessage) -> Option<InspectorView> {
        let value: Value = serde_json::from_str(&msg.payload).ok()?;
        Some(InspectorView {
            label: "JSON".to_string(),
            lines: value_to_spans(&value),
        })
    }

    fn key_hints(&self) -> &'static [(&'static str, &'static str)] {
        // Makes `i` (cycle payload view) meaningful — it toggles the raw payload
        // and this structured view.
        &[("i", "view")]
    }

    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[
            (
                "i",
                "cycle the Payload pane between raw text and the JSON view",
            ),
            ("", "Pretty-prints and syntax-colors JSON payloads."),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::api::{InspectorSpan, InspectorStyle};

    fn inspect(payload: &str) -> Option<InspectorView> {
        JsonView.inspect(&InspectMessage {
            topic: "t".into(),
            payload: payload.into(),
            payload_raw: payload.as_bytes().to_vec(),
            qos: 0,
            retained: false,
        })
    }

    fn line_text(line: &[InspectorSpan]) -> String {
        line.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn pretty_prints_valid_json() {
        let view = inspect(r#"{"a":1,"b":[2,3]}"#).expect("valid json yields a view");
        assert_eq!(view.label, "JSON");
        assert!(view.lines.len() > 1);
        // Concatenating spans reproduces standard 2-space pretty JSON.
        let text: Vec<String> = view.lines.iter().map(|l| line_text(l)).collect();
        assert!(text.iter().any(|l| l == "  \"a\": 1,"));
        assert_eq!(text.first().map(String::as_str), Some("{"));
    }

    #[test]
    fn tokens_get_semantic_styles() {
        let view = inspect(r#"{"k":"v","n":42,"ok":true,"z":null}"#).unwrap();
        let spans: Vec<&InspectorSpan> = view.lines.iter().flatten().collect();
        let has = |t: &str, s: InspectorStyle| spans.iter().any(|sp| sp.text == t && sp.style == s);

        assert!(has("k", InspectorStyle::Key));
        assert!(has("v", InspectorStyle::Str));
        assert!(has("42", InspectorStyle::Number));
        assert!(has("true", InspectorStyle::Literal));
        assert!(has("null", InspectorStyle::Literal));
        assert!(has("{", InspectorStyle::Punctuation));
        assert!(has("\"", InspectorStyle::Punctuation));
        assert!(has(": ", InspectorStyle::Punctuation));
        assert!(has(",", InspectorStyle::Punctuation));
    }

    #[test]
    fn ignores_non_json() {
        assert!(inspect("just text").is_none());
        assert!(inspect("").is_none());
    }

    #[test]
    fn advertises_the_i_key_hint() {
        // The status bar surfaces this only while the plugin is enabled.
        assert_eq!(JsonView.key_hints(), &[("i", "view")]);
    }
}
