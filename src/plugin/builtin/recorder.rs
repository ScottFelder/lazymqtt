//! Built-in record/replay plugin (`topic-recorder`).
//!
//! Records the active connection's incoming messages to a JSON Lines file
//! (`plugins/recordings/<connection-id>-<timestamp>.jsonl`, one message per
//! line with a relative `offset_ms`) and replays a recording back to the
//! broker, preserving the recorded timing. Driven entirely through commands in
//! the `m` menu.
//!
//! Replay republishes under a `replay/` topic prefix by default so it never
//! clobbers live topics, and recording pauses during replay so the plugin
//! doesn't capture its own echoes.

use crate::plugin::api::{PluginAction, PluginCommand, PluginContext, PluginEvent, PluginMetadata};
use crate::plugin::Plugin;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

const NAME: &str = "topic-recorder";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct RecordedMessage {
    offset_ms: u64,
    topic: String,
    payload: String,
    qos: u8,
    retain: bool,
}

struct Replay {
    msgs: Vec<RecordedMessage>,
    next: usize,
    started: Instant,
}

pub struct Recorder {
    config_dir: PathBuf,
    active: Option<String>, // active connection id
    // recording
    writer: Option<BufWriter<File>>,
    record_started: Option<Instant>,
    count: u64,
    record_name: String,
    // replay
    replay: Option<Replay>,
    loop_replay: bool,
    prefix_rewrite: bool,
    speed: f64,
}

impl Default for Recorder {
    fn default() -> Self {
        Self {
            config_dir: PathBuf::new(),
            active: None,
            writer: None,
            record_started: None,
            count: 0,
            record_name: String::new(),
            replay: None,
            loop_replay: false,
            prefix_rewrite: true,
            speed: 1.0,
        }
    }
}

impl Plugin for Recorder {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: NAME,
            version: "0.1.0",
            description: "record a connection's traffic and replay it (m menu)",
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> anyhow::Result<()> {
        self.config_dir = ctx.config_dir.clone();
        Ok(())
    }

    fn commands(&self) -> Vec<PluginCommand> {
        let recording = self.writer.is_some();
        let replaying = self.replay.is_some();
        vec![
            PluginCommand::action(
                "record",
                if recording { "■" } else { "●" },
                if recording {
                    format!("Stop recording ({} msgs)", self.count)
                } else {
                    "Start recording".to_string()
                },
            ),
            PluginCommand::action(
                "replay",
                if replaying { "■" } else { "▶" },
                if replaying {
                    "Stop replay".to_string()
                } else {
                    "Replay newest recording".to_string()
                },
            ),
            PluginCommand::option(
                "loop",
                "↻",
                format!("Replay loop: {}", on_off(self.loop_replay)),
            ),
            PluginCommand::option(
                "prefix",
                "#",
                format!(
                    "Replay prefix: {}",
                    if self.prefix_rewrite {
                        "replay/"
                    } else {
                        "off"
                    }
                ),
            ),
            PluginCommand::option(
                "speed",
                "»",
                format!("Replay speed: {}x", self.speed as u64),
            ),
        ]
    }

    fn invoke(&mut self, id: &str) -> Vec<PluginAction> {
        // Enter steps options forward; the one-shot commands run their action.
        self.adjust(id, true)
    }

    fn adjust(&mut self, id: &str, forward: bool) -> Vec<PluginAction> {
        let status = match id {
            "record" => self.toggle_record(),
            "replay" => self.toggle_replay(),
            "loop" => {
                self.loop_replay = !self.loop_replay;
                format!("replay loop {}", on_off(self.loop_replay))
            }
            "prefix" => {
                self.prefix_rewrite = !self.prefix_rewrite;
                if self.prefix_rewrite {
                    "replay prefix: replay/".to_string()
                } else {
                    "replay prefix off".to_string()
                }
            }
            "speed" => {
                self.speed = cycle(&SPEEDS, self.speed, forward);
                format!("replay speed {}x", self.speed as u64)
            }
            _ => return Vec::new(),
        };
        vec![PluginAction::Status(status)]
    }

    fn on_event(&mut self, event: &PluginEvent) -> Vec<PluginAction> {
        match event {
            PluginEvent::Connected { connection } => {
                self.active = Some(connection.clone());
                self.stop_record();
                self.replay = None;
                Vec::new()
            }
            PluginEvent::Disconnected(_) | PluginEvent::Shutdown => {
                self.active = None;
                self.stop_record();
                self.replay = None;
                Vec::new()
            }
            // Record only while not replaying, to avoid capturing our own echoes.
            PluginEvent::MessageReceived {
                topic,
                payload,
                qos,
                retained,
                ..
            } if self.writer.is_some() && self.replay.is_none() => {
                self.record(topic, payload, *qos, *retained);
                Vec::new()
            }
            PluginEvent::FrameTick if self.replay.is_some() => self.replay_due(),
            _ => Vec::new(),
        }
    }
}

impl Recorder {
    fn toggle_record(&mut self) -> String {
        if self.writer.is_some() {
            let n = self.count;
            let name = self.record_name.clone();
            self.stop_record();
            return format!("recorded {n} msgs → {name}");
        }
        let Some(conn) = self.active.clone() else {
            return "connect before recording".to_string();
        };
        let dir = self.config_dir.join("recordings");
        if fs::create_dir_all(&dir).is_err() {
            return "could not create recordings dir".to_string();
        }
        let name = format!("{conn}-{}.jsonl", timestamp());
        match File::create(dir.join(&name)) {
            Ok(file) => {
                self.writer = Some(BufWriter::new(file));
                self.record_started = Some(Instant::now());
                self.count = 0;
                self.record_name = name.clone();
                format!("recording → {name}")
            }
            Err(e) => format!("could not start recording: {e}"),
        }
    }

    fn record(&mut self, topic: &str, payload: &str, qos: u8, retain: bool) {
        let offset_ms = self
            .record_started
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);
        let msg = RecordedMessage {
            offset_ms,
            topic: topic.into(),
            payload: payload.into(),
            qos,
            retain,
        };
        if let (Some(w), Ok(line)) = (self.writer.as_mut(), serde_json::to_string(&msg)) {
            if writeln!(w, "{line}").is_ok() {
                let _ = w.flush();
                self.count += 1;
            }
        }
    }

    fn stop_record(&mut self) {
        if let Some(mut w) = self.writer.take() {
            let _ = w.flush();
        }
        self.record_started = None;
    }

    fn toggle_replay(&mut self) -> String {
        if self.replay.is_some() {
            self.replay = None;
            return "replay stopped".to_string();
        }
        let Some(conn) = self.active.clone() else {
            return "connect before replaying".to_string();
        };
        let Some(path) = newest_recording(&self.config_dir, &conn) else {
            return "no recording for this connection".to_string();
        };
        let msgs = load_recording(&path);
        if msgs.is_empty() {
            return "recording is empty".to_string();
        }
        let n = msgs.len();
        self.replay = Some(Replay {
            msgs,
            next: 0,
            started: Instant::now(),
        });
        format!("replaying {n} msgs")
    }

    fn replay_due(&mut self) -> Vec<PluginAction> {
        let (speed, prefix) = (self.speed, self.prefix_rewrite);
        let mut actions = Vec::new();
        let mut finished = false;

        if let Some(replay) = self.replay.as_mut() {
            let elapsed_ms = replay.started.elapsed().as_secs_f64() * speed * 1000.0;
            let end = drain_due(&replay.msgs, replay.next, elapsed_ms);
            for m in &replay.msgs[replay.next..end] {
                let topic = if prefix {
                    format!("replay/{}", m.topic)
                } else {
                    m.topic.clone()
                };
                actions.push(PluginAction::Publish {
                    topic,
                    payload: m.payload.clone(),
                    qos: m.qos,
                    retain: m.retain,
                });
            }
            replay.next = end;
            if replay.next >= replay.msgs.len() {
                if self.loop_replay {
                    replay.next = 0;
                    replay.started = Instant::now();
                } else {
                    finished = true;
                }
            }
        }

        if finished {
            self.replay = None;
            actions.push(PluginAction::Status("replay complete".to_string()));
        }
        actions
    }
}

/// Replay speed multipliers, cycled by the speed option.
const SPEEDS: [f64; 3] = [1.0, 2.0, 5.0];

fn on_off(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

/// Return the option after (`forward`) or before `current` in `options`,
/// wrapping around. Falls back to the first option if `current` isn't found.
fn cycle(options: &[f64], current: f64, forward: bool) -> f64 {
    let n = options.len();
    let i = options.iter().position(|&v| v == current).unwrap_or(0);
    let next = if forward {
        (i + 1) % n
    } else {
        (i + n - 1) % n
    };
    options[next]
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// New `next` index after emitting every message (from `next`) whose offset has
/// arrived. Messages are in recorded (ascending-offset) order.
fn drain_due(msgs: &[RecordedMessage], next: usize, elapsed_ms: f64) -> usize {
    let mut i = next;
    while i < msgs.len() && (msgs[i].offset_ms as f64) <= elapsed_ms {
        i += 1;
    }
    i
}

fn newest_recording(config_dir: &Path, conn: &str) -> Option<PathBuf> {
    let prefix = format!("{conn}-");
    fs::read_dir(config_dir.join("recordings"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(&prefix) && n.ends_with(".jsonl"))
                .unwrap_or(false)
        })
        .max_by(|a, b| a.file_name().cmp(&b.file_name()))
}

fn load_recording(path: &Path) -> Vec<RecordedMessage> {
    fs::read_to_string(path)
        .map(|s| {
            s.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(offset_ms: u64) -> RecordedMessage {
        RecordedMessage {
            offset_ms,
            topic: "t".into(),
            payload: "p".into(),
            qos: 0,
            retain: false,
        }
    }

    #[test]
    fn recorded_message_round_trips() {
        let m = RecordedMessage {
            offset_ms: 1234,
            topic: "a/b".into(),
            payload: r#"{"x":1}"#.into(),
            qos: 1,
            retain: true,
        };
        let line = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<RecordedMessage>(&line).unwrap(), m);
    }

    #[test]
    fn drain_due_advances_only_arrived() {
        let msgs = [msg(0), msg(400), msg(2100)];
        assert_eq!(drain_due(&msgs, 0, 0.0), 1); // only offset 0
        assert_eq!(drain_due(&msgs, 0, 500.0), 2); // 0 and 400
        assert_eq!(drain_due(&msgs, 2, 500.0), 2); // nothing new yet
        assert_eq!(drain_due(&msgs, 0, 9999.0), 3); // all
    }

    #[test]
    fn settings_toggle_and_label() {
        let mut r = Recorder::default();
        assert!(r.prefix_rewrite); // default on
        assert_eq!(
            r.invoke("loop"),
            vec![PluginAction::Status("replay loop on".into())]
        );
        assert!(r.loop_replay);
        r.invoke("speed"); // 1 -> 2
        r.invoke("speed"); // 2 -> 5
        assert_eq!(r.speed, 5.0);
        assert!(r
            .commands()
            .iter()
            .any(|c| c.id == "speed" && c.label.contains("5x")));
    }

    #[test]
    fn speed_cycles_both_directions_and_wraps() {
        let mut r = Recorder::default();
        assert_eq!(r.speed, 1.0);
        r.adjust("speed", true); // 1 -> 2
        r.adjust("speed", true); // 2 -> 5
        r.adjust("speed", true); // 5 -> 1 (wrap)
        assert_eq!(r.speed, 1.0);
        r.adjust("speed", false); // 1 -> 5 (wrap backward)
        assert_eq!(r.speed, 5.0);
        r.adjust("speed", false); // 5 -> 2
        assert_eq!(r.speed, 2.0);
    }

    #[test]
    fn options_are_adjustable_actions_are_not() {
        let r = Recorder::default();
        let cmds = r.commands();
        let by_id = |id: &str| cmds.iter().find(|c| c.id == id).unwrap().adjustable;
        assert!(by_id("loop"));
        assert!(by_id("prefix"));
        assert!(by_id("speed"));
        assert!(!by_id("record"));
        assert!(!by_id("replay"));
    }

    #[test]
    fn record_and_replay_need_a_connection() {
        let mut r = Recorder::default(); // active = None
        assert_eq!(
            r.invoke("record"),
            vec![PluginAction::Status("connect before recording".into())]
        );
        assert!(r.writer.is_none());
        assert_eq!(
            r.invoke("replay"),
            vec![PluginAction::Status("connect before replaying".into())]
        );
        assert!(r.replay.is_none());
    }
}
