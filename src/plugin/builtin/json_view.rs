//! Built-in JSON structured-view plugin.
//!
//! When the selected payload parses as JSON, offers a pretty-printed,
//! syntax-colored view as an alternative to the raw text. The core keeps the
//! raw view available alongside it, so a payload is never hidden.
//!
//! We walk the parsed `serde_json::Value` and emit styled spans ourselves
//! (rather than re-tokenizing `to_string_pretty` output) so each token gets the
//! right kind. Concatenating a line's span texts reproduces standard 2-space
//! pretty JSON, so a yank of the view still yields valid JSON.

use crate::plugin::api::{InspectMessage, InspectorSpan, InspectorStyle, InspectorView};
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
        let mut fmt = Fmt::default();
        write_value(&mut fmt, &value, 0);
        fmt.flush();
        Some(InspectorView {
            label: "JSON".to_string(),
            lines: fmt.lines,
        })
    }

    fn inspector_label(&self) -> Option<&'static str> {
        Some("JSON")
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

/// Accumulates styled spans into lines as the value is walked.
#[derive(Default)]
struct Fmt {
    lines: Vec<Vec<InspectorSpan>>,
    current: Vec<InspectorSpan>,
}

impl Fmt {
    fn push(&mut self, text: impl Into<String>, style: InspectorStyle) {
        self.current.push(InspectorSpan::new(text, style));
    }

    fn newline(&mut self) {
        self.lines.push(std::mem::take(&mut self.current));
    }

    fn flush(&mut self) {
        if !self.current.is_empty() {
            self.newline();
        }
    }

    fn indent(&mut self, level: usize) {
        if level > 0 {
            self.push("  ".repeat(level), InspectorStyle::Punctuation);
        }
    }
}

fn write_value(f: &mut Fmt, value: &Value, level: usize) {
    match value {
        Value::Object(map) if map.is_empty() => f.push("{}", InspectorStyle::Punctuation),
        Value::Object(map) => {
            f.push("{", InspectorStyle::Punctuation);
            f.newline();
            let last = map.len() - 1;
            for (i, (key, val)) in map.iter().enumerate() {
                f.indent(level + 1);
                f.push("\"", InspectorStyle::Punctuation);
                f.push(escape_inner(key), InspectorStyle::Key);
                f.push("\"", InspectorStyle::Punctuation);
                f.push(": ", InspectorStyle::Punctuation);
                write_value(f, val, level + 1);
                if i != last {
                    f.push(",", InspectorStyle::Punctuation);
                }
                f.newline();
            }
            f.indent(level);
            f.push("}", InspectorStyle::Punctuation);
        }
        Value::Array(arr) if arr.is_empty() => f.push("[]", InspectorStyle::Punctuation),
        Value::Array(arr) => {
            f.push("[", InspectorStyle::Punctuation);
            f.newline();
            let last = arr.len() - 1;
            for (i, val) in arr.iter().enumerate() {
                f.indent(level + 1);
                write_value(f, val, level + 1);
                if i != last {
                    f.push(",", InspectorStyle::Punctuation);
                }
                f.newline();
            }
            f.indent(level);
            f.push("]", InspectorStyle::Punctuation);
        }
        Value::String(s) => {
            f.push("\"", InspectorStyle::Punctuation);
            f.push(escape_inner(s), InspectorStyle::Str);
            f.push("\"", InspectorStyle::Punctuation);
        }
        Value::Number(n) => f.push(n.to_string(), InspectorStyle::Number),
        Value::Bool(b) => f.push(b.to_string(), InspectorStyle::Literal),
        Value::Null => f.push("null", InspectorStyle::Literal),
    }
}

/// JSON-escaped inner text of a string (no surrounding quotes), so keys and
/// string values render exactly as JSON would.
fn escape_inner(s: &str) -> String {
    let quoted = serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""));
    quoted[1..quoted.len() - 1].to_string()
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
