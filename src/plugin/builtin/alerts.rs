//! Built-in topic-alerts plugin.
//!
//! Watches incoming messages against per-connection rules and raises alerts as
//! annotations (Warn/Error) plus a status notification. Rules are stored per
//! connection (`plugins/alerts/<connection-id>.json`) and edited in-app
//! (`A`). The active connection's rules are loaded on `Connected` and cleared
//! on `Disconnected`. Alerts are visual only — this plugin never runs commands
//! or makes network requests.
//!
//! Rule shapes are defined in [`crate::plugin::alerts_rules`].

use crate::plugin::alerts_rules::{self, AlertCondition, AlertRule};
use crate::plugin::api::{Annotation, PluginAction, PluginContext, PluginEvent, PluginMetadata};
use crate::plugin::{topics, Plugin};
use std::collections::HashMap;
use std::path::PathBuf;

const NAME: &str = "topic-alerts";

#[derive(Default)]
pub struct TopicAlerts {
    config_dir: PathBuf,
    active: Option<String>,              // active connection id, if connected
    rules: Vec<AlertRule>,               // the active connection's rules
    last_value: HashMap<String, String>, // for `changed`, per topic
    silence_ticks: Vec<u64>,             // per rule: seconds since a match (Silent only)
    silence_fired: Vec<bool>,            // per rule: already alerted (Silent only)
}

impl Plugin for TopicAlerts {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: NAME,
            version: "0.1.0",
            description: "per-connection alerts on values, changes, and silence (edit with A)",
        }
    }

    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("A", "open the alert-rules editor"),
            (
                "",
                "Rules are stored per connection in plugins/alerts.json.",
            ),
            ("", "Conditions: above / below / changed / silent."),
            (
                "",
                "Severity warn (⚠) or error (✗), shown on the message + status bar.",
            ),
        ]
    }

    fn on_load(&mut self, ctx: &PluginContext) -> anyhow::Result<()> {
        // Rules load per connection on Connected; just remember where they live.
        self.config_dir = ctx.config_dir.clone();
        Ok(())
    }

    fn on_event(&mut self, event: &PluginEvent) -> Vec<PluginAction> {
        match event {
            PluginEvent::Connected { connection } => {
                self.active = Some(connection.clone());
                self.reload();
                Vec::new()
            }
            PluginEvent::Disconnected(_) => {
                self.active = None;
                self.set_rules(Vec::new());
                Vec::new()
            }
            PluginEvent::MessageReceived {
                id, topic, payload, ..
            } if self.active.is_some() => self.on_message(*id, topic, payload),
            PluginEvent::Tick if self.active.is_some() => self.on_tick(),
            _ => Vec::new(),
        }
    }
}

impl TopicAlerts {
    fn reload(&mut self) {
        if let Some(id) = self.active.clone() {
            let rules = alerts_rules::load(&self.config_dir, &id);
            self.set_rules(rules);
        }
    }

    fn set_rules(&mut self, rules: Vec<AlertRule>) {
        self.silence_ticks = vec![0; rules.len()];
        self.silence_fired = vec![false; rules.len()];
        self.last_value.clear();
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
            if !topics::matches(&filter, topic) {
                continue;
            }
            match cond {
                AlertCondition::Silent { .. } => {
                    self.silence_ticks[i] = 0;
                    self.silence_fired[i] = false;
                }
                AlertCondition::Changed => {
                    if let Some(prev) = self.last_value.get(topic) {
                        if prev != payload {
                            actions.extend(alert(
                                id,
                                sev.severity(),
                                format!("payload changed on {topic}"),
                            ));
                        }
                    }
                    self.last_value
                        .insert(topic.to_string(), payload.to_string());
                }
                AlertCondition::Above { value } => {
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
                AlertCondition::Below { value } => {
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
            if let AlertCondition::Silent { seconds } = self.rules[i].cond {
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
fn alert(id: u64, severity: crate::plugin::Severity, text: String) -> Vec<PluginAction> {
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

/// A number from the payload.
///
/// With `field`, the payload is parsed as JSON and the value at that path is
/// read — the path is dot-separated and may nest into objects and index arrays
/// (e.g. `data.sensors.0.temp`). Without `field`, the whole payload is used.
/// In both cases a JSON number is taken as-is and a numeric string (`"85"`) is
/// coerced.
fn extract_number(payload: &str, field: &Option<String>) -> Option<f64> {
    match field {
        Some(f) => {
            let root: serde_json::Value = serde_json::from_str(payload).ok()?;
            let pointer = format!("/{}", f.trim().replace('.', "/"));
            value_as_number(root.pointer(&pointer)?)
        }
        None => {
            if let Ok(v) = payload.trim().parse::<f64>() {
                Some(v)
            } else {
                value_as_number(&serde_json::from_str(payload).ok()?)
            }
        }
    }
}

/// A JSON value as a number: numbers directly, numeric strings coerced.
fn value_as_number(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::alerts_rules::AlertSeverity;

    fn plugin(rules: Vec<AlertRule>) -> TopicAlerts {
        let mut p = TopicAlerts {
            active: Some("c".into()),
            ..Default::default()
        };
        p.set_rules(rules);
        p
    }

    fn rule(topic: &str, cond: AlertCondition, severity: AlertSeverity) -> AlertRule {
        AlertRule {
            topic: topic.into(),
            field: None,
            severity,
            cond,
        }
    }

    #[test]
    fn extract_number_plain_and_field() {
        let f = |s: &str| Some(s.to_string());
        // Plain payloads.
        assert_eq!(extract_number("85", &None), Some(85.0));
        assert_eq!(extract_number("nope", &None), None);
        assert_eq!(extract_number("\"85\"", &None), Some(85.0)); // JSON numeric string
                                                                 // Top-level field.
        assert_eq!(extract_number(r#"{"temp":21.4}"#, &f("temp")), Some(21.4));
        assert_eq!(extract_number(r#"{"other":1}"#, &f("temp")), None);
        // Numeric string coercion.
        assert_eq!(extract_number(r#"{"temp":"85"}"#, &f("temp")), Some(85.0));
        // Nested path.
        assert_eq!(
            extract_number(r#"{"data":{"temp":21.4}}"#, &f("data.temp")),
            Some(21.4)
        );
        // Array indexing.
        assert_eq!(
            extract_number(r#"{"vals":[1,2,3]}"#, &f("vals.1")),
            Some(2.0)
        );
        // Missing path.
        assert_eq!(extract_number(r#"{"data":{}}"#, &f("data.temp")), None);
    }

    #[test]
    fn above_threshold_annotates() {
        let mut p = plugin(vec![rule(
            "f/temp",
            AlertCondition::Above { value: 80.0 },
            AlertSeverity::Error,
        )]);

        let hot = p.on_message(1, "f/temp", "85");
        assert!(hot.iter().any(|a| matches!(
            a,
            PluginAction::Annotate { id: 1, annotation } if annotation.severity == crate::plugin::Severity::Error
        )));

        assert!(p.on_message(2, "f/temp", "50").is_empty());
        assert!(p.on_message(3, "other/temp", "999").is_empty());
    }

    #[test]
    fn changed_alerts_only_after_first() {
        let mut p = plugin(vec![rule(
            "cfg",
            AlertCondition::Changed,
            AlertSeverity::Warn,
        )]);
        assert!(p.on_message(1, "cfg", "a").is_empty());
        assert!(p.on_message(2, "cfg", "a").is_empty());
        assert!(!p.on_message(3, "cfg", "b").is_empty());
    }

    #[test]
    fn silence_fires_once() {
        let mut p = plugin(vec![rule(
            "hb",
            AlertCondition::Silent { seconds: 3 },
            AlertSeverity::Warn,
        )]);
        p.on_message(1, "hb", "beat");
        assert!(p.on_tick().is_empty());
        assert!(p.on_tick().is_empty());
        assert!(!p.on_tick().is_empty()); // 3s -> fires
        assert!(p.on_tick().is_empty()); // stays fired
        p.on_message(2, "hb", "beat"); // rearm
        assert!(p.on_tick().is_empty());
    }

    #[test]
    fn disconnected_ignores_messages() {
        let mut p = plugin(vec![rule(
            "f/temp",
            AlertCondition::Above { value: 80.0 },
            AlertSeverity::Error,
        )]);
        // Simulate a disconnect via the event path.
        p.on_event(&PluginEvent::Disconnected("bye".into()));
        assert!(p.active.is_none());
        // Even a matching message produces nothing while disconnected.
        assert!(p
            .on_event(&PluginEvent::MessageReceived {
                id: 1,
                topic: "f/temp".into(),
                payload: "85".into(),
                payload_raw: b"85".to_vec(),
                qos: 0,
                retained: false,
            })
            .is_empty());
    }
}
