//! JSON Schema validation for the `json-schema` plugin.
//!
//! Schemas are mapped to topics per connection and stored at
//! `<plugins>/schemas/<connection-id>.json`:
//!
//! ```json
//! { "mappings": [
//!     { "topic": "sensors/#",
//!       "schema": { "type": "object",
//!                   "required": ["temp"],
//!                   "properties": { "temp": { "type": "number", "minimum": -50 } } } }
//! ] }
//! ```
//!
//! The validator implements a practical subset of JSON Schema — enough for the
//! telemetry payloads this tool inspects, without pulling in a heavy dependency
//! (it's plain `serde_json`). Supported keywords: `type` (incl. arrays of
//! types and `integer`), `required`, `properties`, `items`, `enum`, `const`,
//! `minimum`/`maximum`/`exclusiveMinimum`/`exclusiveMaximum`,
//! `minLength`/`maxLength`, and `minItems`/`maxItems`. Unknown keywords are
//! ignored (validation is permissive about what it doesn't understand).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// One topic-filter → schema binding.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SchemaMapping {
    pub topic: String,
    pub schema: Value,
}

#[derive(Debug, Default, Deserialize)]
struct SchemaFile {
    #[serde(default)]
    mappings: Vec<SchemaMapping>,
}

fn schemas_path(dir: &Path, connection_id: &str) -> PathBuf {
    dir.join("schemas").join(format!("{connection_id}.json"))
}

/// Load a connection's schema mappings (empty if the file is missing or
/// unparseable).
pub fn load(dir: &Path, connection_id: &str) -> Vec<SchemaMapping> {
    fs::read_to_string(schemas_path(dir, connection_id))
        .ok()
        .and_then(|s| serde_json::from_str::<SchemaFile>(&s).ok())
        .map(|f| f.mappings)
        .unwrap_or_default()
}

/// Validate `instance` against `schema`, returning the first violation as a
/// `path: message` string, or `Ok(())` when it conforms.
pub fn validate(instance: &Value, schema: &Value) -> Result<(), String> {
    validate_at(instance, schema, "")
}

fn validate_at(inst: &Value, schema: &Value, path: &str) -> Result<(), String> {
    // A non-object schema (e.g. the boolean schema `true`) accepts anything.
    let Some(obj) = schema.as_object() else {
        return Ok(());
    };

    if let Some(t) = obj.get("type") {
        let ok = match t {
            Value::String(s) => type_matches(inst, s),
            Value::Array(types) => types
                .iter()
                .filter_map(Value::as_str)
                .any(|s| type_matches(inst, s)),
            _ => true, // malformed `type`: don't second-guess it
        };
        if !ok {
            return Err(format!(
                "{}: expected {}, got {}",
                loc(path),
                type_desc(t),
                actual_type(inst),
            ));
        }
    }

    if let Some(expected) = obj.get("const") {
        if inst != expected {
            return Err(format!("{}: must equal {}", loc(path), expected));
        }
    }

    if let Some(Value::Array(allowed)) = obj.get("enum") {
        if !allowed.iter().any(|v| v == inst) {
            return Err(format!("{}: not one of the allowed values", loc(path)));
        }
    }

    if let Some(n) = inst.as_f64() {
        if let Some(m) = obj.get("minimum").and_then(Value::as_f64) {
            if n < m {
                return Err(format!("{}: {n} is below minimum {m}", loc(path)));
            }
        }
        if let Some(m) = obj.get("maximum").and_then(Value::as_f64) {
            if n > m {
                return Err(format!("{}: {n} is above maximum {m}", loc(path)));
            }
        }
        if let Some(m) = obj.get("exclusiveMinimum").and_then(Value::as_f64) {
            if n <= m {
                return Err(format!("{}: {n} must be greater than {m}", loc(path)));
            }
        }
        if let Some(m) = obj.get("exclusiveMaximum").and_then(Value::as_f64) {
            if n >= m {
                return Err(format!("{}: {n} must be less than {m}", loc(path)));
            }
        }
    }

    if let Some(s) = inst.as_str() {
        let len = s.chars().count() as u64;
        if let Some(m) = obj.get("minLength").and_then(Value::as_u64) {
            if len < m {
                return Err(format!("{}: shorter than minLength {m}", loc(path)));
            }
        }
        if let Some(m) = obj.get("maxLength").and_then(Value::as_u64) {
            if len > m {
                return Err(format!("{}: longer than maxLength {m}", loc(path)));
            }
        }
    }

    if let Some(arr) = inst.as_array() {
        let len = arr.len() as u64;
        if let Some(m) = obj.get("minItems").and_then(Value::as_u64) {
            if len < m {
                return Err(format!("{}: fewer than minItems {m}", loc(path)));
            }
        }
        if let Some(m) = obj.get("maxItems").and_then(Value::as_u64) {
            if len > m {
                return Err(format!("{}: more than maxItems {m}", loc(path)));
            }
        }
        if let Some(items) = obj.get("items") {
            for (i, el) in arr.iter().enumerate() {
                validate_at(el, items, &format!("{path}/{i}"))?;
            }
        }
    }

    if let Some(map) = inst.as_object() {
        if let Some(Value::Array(required)) = obj.get("required") {
            for r in required.iter().filter_map(Value::as_str) {
                if !map.contains_key(r) {
                    return Err(format!("{}: missing required property \"{r}\"", loc(path)));
                }
            }
        }
        if let Some(Value::Object(props)) = obj.get("properties") {
            for (name, subschema) in props {
                if let Some(v) = map.get(name) {
                    validate_at(v, subschema, &format!("{path}/{name}"))?;
                }
            }
        }
    }

    Ok(())
}

fn type_matches(inst: &Value, ty: &str) -> bool {
    match ty {
        "object" => inst.is_object(),
        "array" => inst.is_array(),
        "string" => inst.is_string(),
        "number" => inst.is_number(),
        "integer" => {
            inst.is_i64() || inst.is_u64() || inst.as_f64().is_some_and(|f| f.fract() == 0.0)
        }
        "boolean" => inst.is_boolean(),
        "null" => inst.is_null(),
        _ => true, // unknown type keyword: accept
    }
}

fn actual_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn type_desc(t: &Value) -> String {
    match t {
        Value::String(s) => s.clone(),
        Value::Array(types) => types
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" | "),
        other => other.to_string(),
    }
}

/// Human-readable location for an error: a JSON-pointer-ish path, or `root`.
fn loc(path: &str) -> &str {
    if path.is_empty() {
        "root"
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_conforming_object() {
        let schema = json!({
            "type": "object",
            "required": ["temp"],
            "properties": { "temp": { "type": "number", "minimum": -50, "maximum": 150 } }
        });
        assert!(validate(&json!({ "temp": 21.4 }), &schema).is_ok());
    }

    #[test]
    fn reports_missing_required_and_bad_type() {
        let schema = json!({ "type": "object", "required": ["temp"] });
        let err = validate(&json!({ "humidity": 50 }), &schema).unwrap_err();
        assert!(err.contains("required") && err.contains("temp"), "{err}");

        let schema = json!({ "type": "object", "properties": { "temp": { "type": "number" } } });
        let err = validate(&json!({ "temp": "hot" }), &schema).unwrap_err();
        assert!(err.contains("/temp") && err.contains("number"), "{err}");
    }

    #[test]
    fn enforces_numeric_bounds_with_path() {
        let schema = json!({ "properties": { "temp": { "type": "number", "maximum": 100 } } });
        let err = validate(&json!({ "temp": 250 }), &schema).unwrap_err();
        assert!(err.starts_with("/temp") && err.contains("maximum"), "{err}");
    }

    #[test]
    fn integer_type_rejects_fractions_and_enum_matches() {
        assert!(validate(&json!(3), &json!({ "type": "integer" })).is_ok());
        assert!(validate(&json!(3.5), &json!({ "type": "integer" })).is_err());
        assert!(validate(&json!("b"), &json!({ "enum": ["a", "b"] })).is_ok());
        assert!(validate(&json!("c"), &json!({ "enum": ["a", "b"] })).is_err());
    }

    #[test]
    fn validates_array_items() {
        let schema = json!({ "type": "array", "items": { "type": "integer" }, "minItems": 1 });
        assert!(validate(&json!([1, 2, 3]), &schema).is_ok());
        assert!(validate(&json!([]), &schema).is_err()); // minItems
        let err = validate(&json!([1, "two"]), &schema).unwrap_err();
        assert!(err.starts_with("/1"), "{err}");
    }
}
