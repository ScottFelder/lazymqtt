//! Shared JSON → styled-span pretty-printer used by the view plugins.
//!
//! Walks a `serde_json::Value` and emits [`InspectorSpan`] lines: concatenating
//! a line's span texts reproduces standard 2-space pretty JSON, so a yank of the
//! rendered view still yields valid JSON. Both json-view (parsing JSON text) and
//! protobuf-view (serializing a decoded message to JSON) render through this.

use crate::plugin::api::{InspectorSpan, InspectorStyle};
use serde_json::Value;

/// Render a JSON value into styled, 2-space-indented lines.
pub fn value_to_spans(value: &Value) -> Vec<Vec<InspectorSpan>> {
    let mut fmt = Fmt::default();
    write_value(&mut fmt, value, 0);
    fmt.flush();
    fmt.lines
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
