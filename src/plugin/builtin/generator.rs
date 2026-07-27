//! Built-in payload generator (`payload-generator`).
//!
//! Publishes synthetic payloads for exercising subscribers, driven from the `m`
//! menu. A one-shot generator publishes once per invoke; a streaming generator
//! (`interval_ms > 0`) toggles on/off and fires at its rate via `FrameTick`.
//! Generators load on `Connected` and stop/clear on `Disconnected`, so nothing
//! publishes while offline.
//!
//! Generator shapes live in [`crate::plugin::generators`].

use crate::plugin::api::{PluginAction, PluginCommand, PluginContext, PluginEvent, PluginMetadata};
use crate::plugin::generators::{self, Generator};
use crate::plugin::Plugin;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const NAME: &str = "payload-generator";
const NONE_ID: &str = "__none__"; // placeholder command id when no generators exist

#[derive(Default)]
pub struct PayloadGenerator {
    config_dir: PathBuf,
    generators: Vec<Generator>,        // loaded for the active connection
    running: HashMap<String, Instant>, // streaming generators -> last fire time
    counters: HashMap<String, i64>,    // per-generator counter state
}

impl Plugin for PayloadGenerator {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: NAME,
            version: "0.1.0",
            description: "publish generated payloads: counters, random, timestamps (m menu)",
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> anyhow::Result<()> {
        self.config_dir = ctx.config_dir.clone();
        Ok(())
    }

    fn on_event(&mut self, event: &PluginEvent) -> Vec<PluginAction> {
        match event {
            PluginEvent::Connected { .. } => {
                self.generators = generators::load(&self.config_dir);
                self.running.clear();
                self.counters.clear();
                Vec::new()
            }
            PluginEvent::Disconnected(_) | PluginEvent::Shutdown => {
                self.generators.clear();
                self.running.clear();
                self.counters.clear();
                Vec::new()
            }
            PluginEvent::FrameTick if !self.running.is_empty() => self.fire_due(),
            _ => Vec::new(),
        }
    }

    /// One command per generator: streams toggle ▶/■; one-shots publish on Enter.
    /// When none are defined, a hint row keeps the enabled plugin visible.
    fn commands(&self) -> Vec<PluginCommand> {
        if self.generators.is_empty() {
            return vec![PluginCommand::action(
                NONE_ID,
                "↑",
                "No generators — define them in plugins/generators.json",
            )];
        }
        self.generators
            .iter()
            .map(|g| {
                if g.interval_ms > 0 {
                    let running = self.running.contains_key(&g.name);
                    PluginCommand::action(
                        g.name.clone(),
                        if running { "■" } else { "▶" },
                        format!(
                            "{} {} → {} ({}ms)",
                            if running { "Stop" } else { "Stream" },
                            g.name,
                            g.topic,
                            g.interval_ms
                        ),
                    )
                } else {
                    PluginCommand::action(
                        g.name.clone(),
                        "↑",
                        format!("Generate {} → {}", g.name, g.topic),
                    )
                }
            })
            .collect()
    }

    fn invoke(&mut self, id: &str) -> Vec<PluginAction> {
        if id == NONE_ID {
            return vec![PluginAction::Status(
                "define generators in plugins/generators.json, then reconnect".into(),
            )];
        }
        let Some(gen) = self.generators.iter().find(|g| g.name == id).cloned() else {
            return Vec::new();
        };
        if gen.interval_ms > 0 {
            // Toggle streaming. Seeding last-fire in the past makes it publish on
            // the next FrameTick rather than waiting a full interval.
            if self.running.remove(&gen.name).is_some() {
                vec![PluginAction::Status(format!("stopped {}", gen.name))]
            } else {
                let seed = Instant::now()
                    .checked_sub(Duration::from_millis(gen.interval_ms))
                    .unwrap_or_else(Instant::now);
                self.running.insert(gen.name.clone(), seed);
                vec![PluginAction::Status(format!(
                    "streaming {} every {}ms",
                    gen.name, gen.interval_ms
                ))]
            }
        } else {
            let payload = gen.render(&mut self.counters);
            vec![
                publish(&gen, payload),
                PluginAction::Status(format!("published to {}", gen.topic)),
            ]
        }
    }
}

impl PayloadGenerator {
    /// Publish every running stream whose interval has elapsed.
    fn fire_due(&mut self) -> Vec<PluginAction> {
        let now = Instant::now();
        let due: Vec<Generator> = self
            .generators
            .iter()
            .filter(|g| g.interval_ms > 0)
            .filter(|g| {
                self.running.get(&g.name).is_some_and(|last| {
                    now.duration_since(*last).as_millis() as u64 >= g.interval_ms
                })
            })
            .cloned()
            .collect();

        let mut actions = Vec::new();
        for gen in due {
            let payload = gen.render(&mut self.counters);
            self.running.insert(gen.name.clone(), now);
            actions.push(publish(&gen, payload));
        }
        actions
    }
}

fn publish(gen: &Generator, payload: String) -> PluginAction {
    PluginAction::Publish {
        topic: gen.topic.clone(),
        payload,
        qos: gen.qos,
        retain: gen.retain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::generators::GeneratorKind;

    fn plugin(generators: Vec<Generator>) -> PayloadGenerator {
        PayloadGenerator {
            generators,
            ..Default::default()
        }
    }

    fn one_shot_counter() -> Generator {
        Generator {
            name: "c".into(),
            topic: "test/c".into(),
            qos: 0,
            retain: false,
            interval_ms: 0,
            template: None,
            kind: GeneratorKind::Counter { start: 0, step: 1 },
        }
    }

    #[test]
    fn one_shot_publishes_and_advances() {
        let mut p = plugin(vec![one_shot_counter()]);
        let a = p.invoke("c");
        assert!(matches!(
            a.first(),
            Some(PluginAction::Publish { topic, payload, .. })
                if topic == "test/c" && payload == "0"
        ));
        // Next invoke advances the counter.
        let b = p.invoke("c");
        assert!(matches!(
            b.first(),
            Some(PluginAction::Publish { payload, .. }) if payload == "1"
        ));
    }

    #[test]
    fn streaming_toggles_and_fires() {
        let g = Generator {
            interval_ms: 100,
            ..one_shot_counter()
        };
        let mut p = plugin(vec![g]);
        // Start streaming: seeded to fire immediately.
        assert!(matches!(
            p.invoke("c").first(),
            Some(PluginAction::Status(_))
        ));
        assert!(p.running.contains_key("c"));
        let fired = p.fire_due();
        assert!(matches!(fired.first(), Some(PluginAction::Publish { .. })));
        // Toggle off.
        p.invoke("c");
        assert!(!p.running.contains_key("c"));
        assert!(p.fire_due().is_empty());
    }

    #[test]
    fn disconnect_stops_everything() {
        let g = Generator {
            interval_ms: 100,
            ..one_shot_counter()
        };
        let mut p = plugin(vec![g]);
        p.invoke("c");
        p.on_event(&PluginEvent::Disconnected("bye".into()));
        assert!(p.running.is_empty() && p.generators.is_empty());
    }
}
