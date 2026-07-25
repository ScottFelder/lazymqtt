//! Alert rule model + persistence, shared by the `topic-alerts` plugin and the
//! in-app rules editor so both agree on one schema.
//!
//! Rules are **per connection**: each connection's rules live in
//! `<plugins>/alerts/<connection-id>.json`. A connection with no file simply
//! has no alerts.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(tag = "when", rename_all = "lowercase")]
pub enum AlertCondition {
    Above { value: f64 },
    Below { value: f64 },
    Changed,
    Silent { seconds: u64 },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    #[default]
    Warn,
    Error,
}

impl AlertSeverity {
    pub fn severity(self) -> crate::plugin::Severity {
        match self {
            AlertSeverity::Warn => crate::plugin::Severity::Warn,
            AlertSeverity::Error => crate::plugin::Severity::Error,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AlertSeverity::Warn => "WARN",
            AlertSeverity::Error => "ERR",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AlertRule {
    pub topic: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default)]
    pub severity: AlertSeverity,
    #[serde(flatten)]
    pub cond: AlertCondition,
}

impl AlertRule {
    /// One-line description of the condition (for the editor list).
    pub fn summary(&self) -> String {
        let base = match self.cond {
            AlertCondition::Above { value } => format!("above {value}"),
            AlertCondition::Below { value } => format!("below {value}"),
            AlertCondition::Changed => "changed".to_string(),
            AlertCondition::Silent { seconds } => format!("silent {seconds}s"),
        };
        match &self.field {
            Some(f) => format!("{base} (field: {f})"),
            None => base,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RulesFile {
    #[serde(default)]
    rules: Vec<AlertRule>,
}

#[derive(Serialize)]
struct RulesFileRef<'a> {
    rules: &'a [AlertRule],
}

fn rules_path(dir: &Path, connection_id: &str) -> PathBuf {
    dir.join("alerts").join(format!("{connection_id}.json"))
}

/// Load a connection's rules (empty if the file is missing or unparseable).
pub fn load(dir: &Path, connection_id: &str) -> Vec<AlertRule> {
    fs::read_to_string(rules_path(dir, connection_id))
        .ok()
        .and_then(|s| serde_json::from_str::<RulesFile>(&s).ok())
        .map(|f| f.rules)
        .unwrap_or_default()
}

/// Persist a connection's rules to `<dir>/alerts/<id>.json`.
pub fn save(dir: &Path, connection_id: &str, rules: &[AlertRule]) -> Result<()> {
    fs::create_dir_all(dir.join("alerts")).ok();
    let json = serde_json::to_string_pretty(&RulesFileRef { rules })?;
    fs::write(rules_path(dir, connection_id), json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rules_file() {
        let json = r#"{ "rules": [
            { "topic": "demo/temp", "when": "above", "value": 80, "severity": "error" },
            { "topic": "hb", "when": "silent", "seconds": 30 },
            { "topic": "cfg/#", "when": "changed" }
        ] }"#;
        let f: RulesFile = serde_json::from_str(json).expect("rules file should parse");
        assert_eq!(f.rules.len(), 3);
        assert!(matches!(f.rules[0].cond, AlertCondition::Above { value } if value == 80.0));
        assert_eq!(f.rules[0].severity, AlertSeverity::Error);
        assert!(matches!(
            f.rules[1].cond,
            AlertCondition::Silent { seconds: 30 }
        ));
        assert!(matches!(f.rules[2].cond, AlertCondition::Changed));
    }

    #[test]
    fn round_trips_through_json() {
        let rules = vec![
            AlertRule {
                topic: "a/+/c".into(),
                field: Some("temp".into()),
                severity: AlertSeverity::Error,
                cond: AlertCondition::Above { value: 80.0 },
            },
            AlertRule {
                topic: "hb".into(),
                field: None,
                severity: AlertSeverity::Warn,
                cond: AlertCondition::Silent { seconds: 30 },
            },
        ];
        let json = serde_json::to_string(&RulesFileRef { rules: &rules }).unwrap();
        let back: RulesFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.rules, rules);
    }

    #[test]
    fn summary_strings() {
        let r = |cond, field: Option<&str>| AlertRule {
            topic: "t".into(),
            field: field.map(String::from),
            severity: AlertSeverity::Warn,
            cond,
        };
        assert_eq!(
            r(AlertCondition::Above { value: 80.0 }, None).summary(),
            "above 80"
        );
        assert_eq!(
            r(AlertCondition::Silent { seconds: 30 }, None).summary(),
            "silent 30s"
        );
        assert_eq!(r(AlertCondition::Changed, None).summary(), "changed");
        assert_eq!(
            r(AlertCondition::Below { value: 0.0 }, Some("temp")).summary(),
            "below 0 (field: temp)"
        );
    }
}
