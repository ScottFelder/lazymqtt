//! Editable buffers backing the input screens (connection form, publish,
//! alert-rule form, schema form) plus the connection `Status` enum.

use crate::app::TextArea;
use crate::config::Connection;
use crate::plugin::{AlertCondition, AlertRule, AlertSeverity, SchemaMapping};
use serde_json::Value;

pub enum Status {
    Idle,
    Connecting,
    Connected,
    Disconnected(String),
}

/// Editable buffer backing the connection form.
#[derive(Default)]
pub struct FormBuffer {
    pub editing_index: Option<usize>,
    pub name: String,
    pub host: String,
    pub port: String,
    pub client_id: String,
    pub username: String,
    pub password: String,
    pub tls: bool,
    pub topics: String, // newline/comma separated
    pub field: usize,   // which field is focused
}

impl FormBuffer {
    pub const FIELD_COUNT: usize = 8;

    pub fn from(conn: &Connection, index: usize) -> Self {
        Self {
            editing_index: Some(index),
            name: conn.name.clone(),
            host: conn.host.clone(),
            port: conn.port.to_string(),
            client_id: conn.client_id.clone(),
            username: conn.username.clone(),
            password: conn.password.clone(),
            tls: conn.tls,
            topics: conn
                .subscriptions
                .iter()
                .map(|s| s.topic.clone())
                .collect::<Vec<_>>()
                .join(", "),
            field: 0,
        }
    }
}

#[derive(Default)]
pub struct PublishBuffer {
    pub topic: String,
    pub payload: String,
    pub qos: u8,
    pub retain: bool,
    pub field: usize,
}

/// Editable buffer backing the alert-rule form (one rule at a time).
#[derive(Default)]
pub struct AlertForm {
    pub editing_index: Option<usize>, // None = adding a new rule
    pub topic: String,
    pub when: usize,     // 0 above, 1 below, 2 changed, 3 silent
    pub value: String,   // above/below threshold
    pub seconds: String, // silent duration
    pub field: String,   // optional JSON field for above/below
    pub severity: usize, // 0 warn, 1 error
    pub focus: usize,    // focused field, 0..FIELD_COUNT
}

impl AlertForm {
    pub const FIELD_COUNT: usize = 6; // topic, when, value, seconds, field, severity
    pub const WHEN_LABELS: [&'static str; 4] = ["above", "below", "changed", "silent"];
    pub const SEVERITY_LABELS: [&'static str; 2] = ["warn", "error"];

    pub fn from_rule(index: usize, rule: &AlertRule) -> Self {
        let (when, value, seconds) = match rule.cond {
            AlertCondition::Above { value } => (0, value.to_string(), String::new()),
            AlertCondition::Below { value } => (1, value.to_string(), String::new()),
            AlertCondition::Changed => (2, String::new(), String::new()),
            AlertCondition::Silent { seconds } => (3, String::new(), seconds.to_string()),
        };
        Self {
            editing_index: Some(index),
            topic: rule.topic.clone(),
            when,
            value,
            seconds,
            field: rule.field.clone().unwrap_or_default(),
            severity: match rule.severity {
                AlertSeverity::Warn => 0,
                AlertSeverity::Error => 1,
            },
            focus: 0,
        }
    }

    /// Build a rule from the form, validating the inputs.
    pub fn to_rule(&self) -> Result<AlertRule, String> {
        let topic = self.topic.trim();
        if topic.is_empty() {
            return Err("Topic is required".into());
        }
        let cond = match self.when {
            0 | 1 => {
                let v: f64 = self
                    .value
                    .trim()
                    .parse()
                    .map_err(|_| "Value must be a number".to_string())?;
                if self.when == 0 {
                    AlertCondition::Above { value: v }
                } else {
                    AlertCondition::Below { value: v }
                }
            }
            2 => AlertCondition::Changed,
            _ => {
                let s: u64 = self
                    .seconds
                    .trim()
                    .parse()
                    .map_err(|_| "Seconds must be a whole number".to_string())?;
                AlertCondition::Silent { seconds: s }
            }
        };
        let field = {
            let f = self.field.trim();
            (!f.is_empty()).then(|| f.to_string())
        };
        Ok(AlertRule {
            topic: topic.to_string(),
            field,
            severity: if self.severity == 1 {
                AlertSeverity::Error
            } else {
                AlertSeverity::Warn
            },
            cond,
        })
    }
}

/// Editable buffer backing the schema form: a topic filter plus a multi-line
/// JSON editor for the schema body. `focus` is 0 for the topic field, 1 for the
/// schema editor.
pub struct SchemaForm {
    pub editing_index: Option<usize>, // None = adding a new mapping
    pub topic: String,
    pub body: TextArea,
    pub focus: usize,
    pub error: Option<String>,
}

impl Default for SchemaForm {
    fn default() -> Self {
        Self {
            editing_index: None,
            topic: String::new(),
            body: TextArea::from_text("{\n  \"type\": \"object\"\n}"),
            focus: 0,
            error: None,
        }
    }
}

impl SchemaForm {
    pub const FIELD_COUNT: usize = 2; // topic, body

    /// A form editing the mapping at `index`, seeded with its pretty-printed schema.
    pub fn from_mapping(index: usize, mapping: &SchemaMapping) -> Self {
        let body =
            serde_json::to_string_pretty(&mapping.schema).unwrap_or_else(|_| "{}".to_string());
        Self {
            editing_index: Some(index),
            topic: mapping.topic.clone(),
            body: TextArea::from_text(&body),
            focus: 0,
            error: None,
        }
    }

    /// Build a `SchemaMapping` from the form, validating the topic is present
    /// and the body parses as JSON.
    pub fn to_mapping(&self) -> Result<SchemaMapping, String> {
        if self.topic.trim().is_empty() {
            return Err("topic filter is required".into());
        }
        let schema: Value =
            serde_json::from_str(&self.body.text()).map_err(|e| format!("invalid JSON: {e}"))?;
        Ok(SchemaMapping {
            topic: self.topic.trim().to_string(),
            schema,
        })
    }
}
