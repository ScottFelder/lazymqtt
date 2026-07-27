//! Payload generators for the `payload-generator` plugin.
//!
//! Generators publish synthetic payloads for exercising subscribers. They're
//! broker-agnostic, so like publish templates they're **global**, stored at
//! `<plugins>/generators.json`:
//!
//! ```json
//! { "generators": [
//!     { "name": "temp", "topic": "test/temp", "kind": "random", "min": 15, "max": 30,
//!       "interval_ms": 1000, "template": "{\"temp\": {{value}}}" },
//!     { "name": "count", "topic": "test/count", "kind": "counter", "start": 0, "step": 1 },
//!     { "name": "beat", "topic": "test/heartbeat", "kind": "timestamp", "interval_ms": 2000 }
//! ] }
//! ```
//!
//! `interval_ms > 0` makes a generator a **stream** (start/stop toggle in the
//! `m` menu, firing at that rate); `0`/absent makes it **one-shot** (each invoke
//! publishes once). A `template` with a `{{value}}` marker wraps the generated
//! value (e.g. into JSON); without one the raw value is published.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// What a generator produces on each fire.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum GeneratorKind {
    /// An integer that advances by `step` each fire, starting at `start`.
    Counter {
        #[serde(default)]
        start: i64,
        #[serde(default = "one")]
        step: i64,
    },
    /// A random float in `[min, max]`.
    Random { min: f64, max: f64 },
    /// The current local timestamp (`%Y-%m-%dT%H:%M:%S%.3f`).
    Timestamp,
}

fn one() -> i64 {
    1
}

/// A named generator bound to a topic.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Generator {
    pub name: String,
    pub topic: String,
    #[serde(default)]
    pub qos: u8,
    #[serde(default)]
    pub retain: bool,
    /// Streaming rate in ms; `0` (default) means one-shot.
    #[serde(default)]
    pub interval_ms: u64,
    /// Optional wrapper with a `{{value}}` placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(flatten)]
    pub kind: GeneratorKind,
}

impl Generator {
    /// Produce one payload, advancing counter state in `counters` (keyed by
    /// generator name) as needed.
    pub fn render(&self, counters: &mut HashMap<String, i64>) -> String {
        let value = match &self.kind {
            GeneratorKind::Counter { start, step } => {
                let n = counters.entry(self.name.clone()).or_insert(*start);
                let current = *n;
                *n = n.wrapping_add(*step);
                current.to_string()
            }
            GeneratorKind::Random { min, max } => {
                format!("{:.3}", min + rand_frac() * (max - min))
            }
            GeneratorKind::Timestamp => chrono::Local::now()
                .format("%Y-%m-%dT%H:%M:%S%.3f")
                .to_string(),
        };
        match &self.template {
            Some(t) => t.replace("{{value}}", &value),
            None => value,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct GeneratorsFile {
    #[serde(default)]
    generators: Vec<Generator>,
}

fn generators_path(dir: &Path) -> PathBuf {
    dir.join("generators.json")
}

/// Load all generators (empty if the file is missing or unparseable).
pub fn load(dir: &Path) -> Vec<Generator> {
    fs::read_to_string(generators_path(dir))
        .ok()
        .and_then(|s| serde_json::from_str::<GeneratorsFile>(&s).ok())
        .map(|f| f.generators)
        .unwrap_or_default()
}

/// A pseudo-random fraction in `[0, 1)` from the system clock — good enough for
/// synthetic test data without pulling in an RNG crate.
fn rand_frac() -> f64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let mut x = nanos.wrapping_mul(2_654_435_761).wrapping_add(1) | 1;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    (x % 1_000_000) as f64 / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(kind: GeneratorKind, template: Option<&str>) -> Generator {
        Generator {
            name: "g".into(),
            topic: "t".into(),
            qos: 0,
            retain: false,
            interval_ms: 0,
            template: template.map(String::from),
            kind,
        }
    }

    #[test]
    fn counter_advances_and_wraps_into_template() {
        let g = gen(
            GeneratorKind::Counter { start: 5, step: 2 },
            Some("{\"n\": {{value}}}"),
        );
        let mut c = HashMap::new();
        assert_eq!(g.render(&mut c), "{\"n\": 5}");
        assert_eq!(g.render(&mut c), "{\"n\": 7}");
        assert_eq!(g.render(&mut c), "{\"n\": 9}");
    }

    #[test]
    fn random_stays_in_range() {
        let g = gen(GeneratorKind::Random { min: 1.0, max: 2.0 }, None);
        let mut c = HashMap::new();
        for _ in 0..50 {
            let v: f64 = g.render(&mut c).parse().unwrap();
            assert!((1.0..=2.0).contains(&v), "{v} out of range");
        }
    }

    #[test]
    fn parses_generators_file() {
        let json = r#"{ "generators": [
            { "name": "c", "topic": "test/c", "kind": "counter" },
            { "name": "r", "topic": "test/r", "kind": "random", "min": 0, "max": 10, "interval_ms": 500 },
            { "name": "ts", "topic": "test/ts", "kind": "timestamp" }
        ] }"#;
        let f: GeneratorsFile = serde_json::from_str(json).expect("should parse");
        assert_eq!(f.generators.len(), 3);
        assert!(matches!(
            f.generators[0].kind,
            GeneratorKind::Counter { start: 0, step: 1 }
        ));
        assert_eq!(f.generators[1].interval_ms, 500);
        assert!(matches!(f.generators[2].kind, GeneratorKind::Timestamp));
    }
}
