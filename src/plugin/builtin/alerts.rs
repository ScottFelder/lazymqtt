//! Built-in topic-alerts plugin.
//!
//! Watches incoming messages against user-defined rules and raises alerts as
//! annotations (Warn/Error) plus a status notification. Rules live in
//! `plugins/alerts.json` (loaded on start). Alerts are visual only — this
//! plugin never runs commands or makes network requests.
//!
//! Rule shapes (`when`):
//! ```json
//! { "rules": [
//!   { "topic": "factory/+/temperature", "when": "above", "value": 80, "severity": "error" },
//!   { "topic": "sensors/#",             "when": "below", "value": 0, "field": "temp" },
//!   { "topic": "system/heartbeat",      "when": "silent", "seconds": 30 },
//!   { "topic": "config/#",              "when": "changed" }
//! ] }
//! ```
//! `field` (optional) extracts a top-level JSON field for `above`/`below`;
//! otherwise the whole payload is parsed as a number.

use crate::plugin::api::{
    Annotation, PluginAction, PluginContext, PluginEvent, PluginMetadata, Severity,
};
use crate::plugin::Plugin;
use serde::Deserialize;
use std::collections::HashMap;

const NAME: &str = "topic-alerts";

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "when", rename_all = "lowercase")]
enum Condition {
    Above { value: f64 },
    Below { value: f64 },
    Changed,
    Silent { seconds: u64 },
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RuleSeverity {
    #[default]
    Warn,
    Error,
}

impl RuleSeverity {
    fn severity(self) -> Severity {
        match self {
            RuleSeverity::Warn => Severity::Warn,
            RuleSeverity::Error => Severity::Error,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Rule {
    topic: String,
    #[serde(default)]
    field: Option<String>,
    #[serde(default)]
    severity: RuleSeverity,
    #[serde(flatten)]
    cond: Condition,
}

#[derive(Debug, Default, Deserialize)]
struct RulesFile {
    #[serde(default)]
    rules: Vec<Rule>,
}

#[derive(Default)]
pub struct TopicAlerts {
    rules: Vec<Rule>,
    last_value: HashMap<String, String>, // for `changed`, per topic
    silence_ticks: Vec<u64>,             // per rule: seconds since a match (Silent only)
    silence_fired: Vec<bool>,            // per rule: already alerted (Silent only)
}

impl Plugin for TopicAlerts {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: NAME,
            version: "0.1.0",
            description: "alerts on topic values, changes, and silence (see plugins/alerts.json)",
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> anyhow::Result<()> {
        // Isolate tests from the real config dir; tests set rules directly.
        if cfg!(test) {
            return Ok(());
        }
        let path = ctx.config_dir.join("alerts.json");
        let rules = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<RulesFile>(&s).ok())
            .map(|f| f.rules)
            .unwrap_or_default();
        self.set_rules(rules);
        Ok(())
    }

    fn on_event(&mut self, event: &PluginEvent) -> Vec<PluginAction> {
        match event {
            PluginEvent::MessageReceived {
                id, topic, payload, ..
            } => self.on_message(*id, topic, payload),
            PluginEvent::Tick => self.on_tick(),
            _ => Vec::new(),
        }
    }
}

impl TopicAlerts {
    fn set_rules(&mut self, rules: Vec<Rule>) {
        self.silence_ticks = vec![0; rules.len()];
        self.silence_fired = vec![false; rules.len()];
        self.rules = rules;
    }

    fn on_message(&mut self, id: u64, topic: &str, payload: &str) -> Vec<PluginAction> {
        let mut actions = Vec::new();
        for i in 0..self.rules.len() {
            // Copy/clone what we need so no borrow of `self.rules` is held while
            // the arms mutate other `self` fields.
            let cond = self.rules[i].cond;
            let sev = self.rules[i].severity;
            let field = self.rules[i].field.clone();
            let filter = self.rules[i].topic.clone();
            if !topic_matches(&filter, topic) {
                continue;
            }
            match cond {
                Condition::Silent { .. } => {
                    self.silence_ticks[i] = 0;
                    self.silence_fired[i] = false;
                }
                Condition::Changed => {
                    if let Some(prev) = self.last_value.get(topic) {
                        if prev != payload {
                            let text = format!("payload changed on {topic}");
                            actions.extend(alert(id, sev.severity(), text));
                        }
                    }
                    self.last_value
                        .insert(topic.to_string(), payload.to_string());
                }
                Condition::Above { value } => {
                    if let Some(v) = extract_number(payload, &field) {
                        if v > value {
                            actions.extend(alert(
                                id,
                                sev.severity(),
                                format!("{topic} {v} > {value}"),
                            ));
                        }
                    }
                }
                Condition::Below { value } => {
                    if let Some(v) = extract_number(payload, &field) {
                        if v < value {
                            actions.extend(alert(
                                id,
                                sev.severity(),
                                format!("{topic} {v} < {value}"),
                            ));
                        }
                    }
                }
            }
        }
        actions
    }

    fn on_tick(&mut self) -> Vec<PluginAction> {
        let mut actions = Vec::new();
        for i in 0..self.rules.len() {
            if let Condition::Silent { seconds } = self.rules[i].cond {
                self.silence_ticks[i] = self.silence_ticks[i].saturating_add(1);
                if !self.silence_fired[i] && self.silence_ticks[i] >= seconds {
                    self.silence_fired[i] = true;
                    let topic = self.rules[i].topic.clone();
                    actions.push(PluginAction::Status(format!(
                        "alert: {topic} silent for {seconds}s"
                    )));
                }
            }
        }
        actions
    }
}

/// An annotation on the triggering message plus a status notification.
fn alert(id: u64, severity: Severity, text: String) -> Vec<PluginAction> {
    vec![
        PluginAction::Annotate {
            id,
            annotation: Annotation {
                plugin: NAME,
                severity,
                text: text.clone(),
            },
        },
        PluginAction::Status(format!("alert: {text}")),
    ]
}

/// A number from the payload: a named top-level JSON field, or the whole
/// payload parsed as a number.
fn extract_number(payload: &str, field: &Option<String>) -> Option<f64> {
    match field {
        Some(f) => serde_json::from_str::<serde_json::Value>(payload)
            .ok()?
            .get(f)?
            .as_f64(),
        None => payload.trim().parse::<f64>().ok(),
    }
}

/// MQTT topic-filter match (`+` one level, `#` the rest).
fn topic_matches(filter: &str, topic: &str) -> bool {
    let f: Vec<&str> = filter.split('/').collect();
    let t: Vec<&str> = topic.split('/').collect();
    let mut fi = 0;
    let mut ti = 0;
    while fi < f.len() {
        match f[fi] {
            "#" => return true,
            "+" => {
                if ti >= t.len() {
                    return false;
                }
                fi += 1;
                ti += 1;
            }
            seg => {
                if ti >= t.len() || t[ti] != seg {
                    return false;
                }
                fi += 1;
                ti += 1;
            }
        }
    }
    ti == t.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(rules: Vec<Rule>) -> TopicAlerts {
        let mut p = TopicAlerts::default();
        p.set_rules(rules);
        p
    }

    fn rule(topic: &str, cond: Condition, severity: RuleSeverity) -> Rule {
        Rule {
            topic: topic.into(),
            field: None,
            severity,
            cond,
        }
    }

    #[test]
    fn parses_rules_file() {
        let json = r#"{ "rules": [
            { "topic": "demo/temp", "when": "above", "value": 80, "severity": "error" },
            { "topic": "hb", "when": "silent", "seconds": 30 },
            { "topic": "cfg/#", "when": "changed" }
        ] }"#;
        let f: RulesFile = serde_json::from_str(json).expect("rules file should parse");
        assert_eq!(f.rules.len(), 3);
        assert!(matches!(f.rules[0].cond, Condition::Above { value } if value == 80.0));
        assert!(matches!(f.rules[0].severity, RuleSeverity::Error));
        assert!(matches!(f.rules[1].cond, Condition::Silent { seconds: 30 }));
        assert!(matches!(f.rules[2].cond, Condition::Changed));
    }

    #[test]
    fn topic_wildcards() {
        assert!(topic_matches("a/b", "a/b"));
        assert!(!topic_matches("a/b", "a/c"));
        assert!(topic_matches("a/+/c", "a/x/c"));
        assert!(!topic_matches("a/+/c", "a/x/y"));
        assert!(topic_matches("a/#", "a/b/c"));
        assert!(topic_matches("a/#", "a"));
        assert!(!topic_matches("a/#", "b"));
    }

    #[test]
    fn extract_number_plain_and_field() {
        assert_eq!(extract_number("85", &None), Some(85.0));
        assert_eq!(extract_number("nope", &None), None);
        assert_eq!(
            extract_number(r#"{"temp":21.4}"#, &Some("temp".into())),
            Some(21.4)
        );
        assert_eq!(extract_number(r#"{"other":1}"#, &Some("temp".into())), None);
    }

    #[test]
    fn above_threshold_annotates() {
        let mut p = plugin(vec![rule(
            "f/temp",
            Condition::Above { value: 80.0 },
            RuleSeverity::Error,
        )]);

        let hot = p.on_message(1, "f/temp", "85");
        assert!(hot.iter().any(|a| matches!(
            a,
            PluginAction::Annotate { id: 1, annotation } if annotation.severity == Severity::Error
        )));

        assert!(p.on_message(2, "f/temp", "50").is_empty());
        assert!(p.on_message(3, "other/temp", "999").is_empty()); // topic doesn't match
    }

    #[test]
    fn changed_alerts_only_after_first() {
        let mut p = plugin(vec![rule("cfg", Condition::Changed, RuleSeverity::Warn)]);
        assert!(p.on_message(1, "cfg", "a").is_empty()); // first value: no baseline
        assert!(p.on_message(2, "cfg", "a").is_empty()); // unchanged
        assert!(!p.on_message(3, "cfg", "b").is_empty()); // changed
    }

    #[test]
    fn silence_fires_once() {
        let mut p = plugin(vec![rule(
            "hb",
            Condition::Silent { seconds: 3 },
            RuleSeverity::Warn,
        )]);
        p.on_message(1, "hb", "beat"); // resets the timer

        assert!(p.on_tick().is_empty()); // 1s
        assert!(p.on_tick().is_empty()); // 2s
        assert!(!p.on_tick().is_empty()); // 3s -> fires
        assert!(p.on_tick().is_empty()); // stays fired (no repeat)

        p.on_message(2, "hb", "beat"); // heartbeat returns -> rearm
        assert!(p.on_tick().is_empty());
    }
}
