//! Built-in traffic analytics (`traffic-analytics`).
//!
//! Accumulates rolling stats from the message stream (rate, throughput, QoS mix,
//! retained share, busiest topics) and presents them in a plugin-owned pane
//! (`Plugin::pane`) opened from the `m` menu. Stats reset per connection and
//! keep updating live while the pane is open. Purely observational.

use crate::plugin::api::{
    PaneSpan, PaneStyle, PaneView, PluginAction, PluginCommand, PluginEvent, PluginMetadata,
};
use crate::plugin::Plugin;
use std::collections::{HashMap, VecDeque};

const NAME: &str = "traffic-analytics";
const HISTORY: usize = 60; // seconds of msg-rate history for the sparkline
const TOP_TOPICS: usize = 10;

#[derive(Default)]
pub struct TrafficAnalytics {
    total_msgs: u64,
    total_bytes: u64,
    sec_msgs: u64, // accumulating in the current second
    sec_bytes: u64,
    rate_msgs: u64, // last completed second
    rate_bytes: u64,
    peak_msgs: u64,
    history: VecDeque<u64>, // per-second msg counts (newest at back)
    qos: [u64; 3],
    retained: u64,
    topics: HashMap<String, u64>,
}

impl Plugin for TrafficAnalytics {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: NAME,
            version: "0.1.0",
            description: "live traffic stats (rate, throughput, QoS, top topics) in a pane",
        }
    }

    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("m", "open the analytics pane from the command menu"),
            (
                "",
                "Live stats: message rate, throughput, QoS mix, retained %, top topics.",
            ),
        ]
    }

    fn on_event(&mut self, event: &PluginEvent) -> Vec<PluginAction> {
        match event {
            PluginEvent::Connected { .. } => *self = Self::default(),
            PluginEvent::MessageReceived {
                topic,
                payload,
                qos,
                retained,
                ..
            } => {
                self.total_msgs += 1;
                self.sec_msgs += 1;
                let bytes = payload.len() as u64;
                self.total_bytes += bytes;
                self.sec_bytes += bytes;
                self.qos[(*qos).min(2) as usize] += 1;
                if *retained {
                    self.retained += 1;
                }
                *self.topics.entry(topic.clone()).or_insert(0) += 1;
            }
            PluginEvent::Tick => {
                self.rate_msgs = self.sec_msgs;
                self.rate_bytes = self.sec_bytes;
                self.peak_msgs = self.peak_msgs.max(self.sec_msgs);
                self.history.push_back(self.sec_msgs);
                if self.history.len() > HISTORY {
                    self.history.pop_front();
                }
                self.sec_msgs = 0;
                self.sec_bytes = 0;
            }
            _ => {}
        }
        Vec::new()
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![PluginCommand::action("open", "▤", "Traffic analytics")]
    }

    fn invoke(&mut self, id: &str) -> Vec<PluginAction> {
        match id {
            "open" => vec![PluginAction::OpenPane],
            _ => Vec::new(),
        }
    }

    fn pane(&self) -> Option<PaneView> {
        let mut lines: Vec<Vec<PaneSpan>> = Vec::new();

        lines.push(field("Messages", format!("{}", self.total_msgs)));
        lines.push(field(
            "Rate",
            format!("{}/s  (peak {}/s)", self.rate_msgs, self.peak_msgs),
        ));
        lines.push(vec![
            PaneSpan::new(format!("  {:<11}", "history"), PaneStyle::Label),
            PaneSpan::new(sparkline(&self.history), PaneStyle::Accent),
        ]);
        lines.push(field("Throughput", human_bytes(self.total_bytes)));
        lines.push(field(
            "Bandwidth",
            format!("{}/s", human_bytes(self.rate_bytes)),
        ));
        lines.push(field(
            "QoS",
            format!("0:{}  1:{}  2:{}", self.qos[0], self.qos[1], self.qos[2]),
        ));
        lines.push(field(
            "Retained",
            format!("{} of {}", self.retained, self.total_msgs),
        ));

        lines.push(Vec::new());
        lines.push(vec![PaneSpan::new(
            format!("Top {TOP_TOPICS} topics"),
            PaneStyle::Header,
        )]);

        let mut topics: Vec<(&String, &u64)> = self.topics.iter().collect();
        topics.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let max = topics.first().map(|(_, c)| **c).unwrap_or(1).max(1);
        if topics.is_empty() {
            lines.push(vec![PaneSpan::new("  (no messages yet)", PaneStyle::Muted)]);
        }
        for (topic, count) in topics.into_iter().take(TOP_TOPICS) {
            let width = 24usize;
            let filled = ((*count as f64 / max as f64) * width as f64).round() as usize;
            let bar: String = "█".repeat(filled.min(width));
            lines.push(vec![
                PaneSpan::new(format!("  {topic:<28} "), PaneStyle::Value),
                PaneSpan::new(format!("{count:>6} "), PaneStyle::Label),
                PaneSpan::new(bar, PaneStyle::Accent),
            ]);
        }

        Some(PaneView {
            title: "Traffic Analytics".to_string(),
            lines,
        })
    }
}

/// A `label   value` line (dim label, normal value).
fn field(label: &str, value: String) -> Vec<PaneSpan> {
    vec![
        PaneSpan::new(format!("  {label:<11}"), PaneStyle::Label),
        PaneSpan::new(value, PaneStyle::Value),
    ]
}

/// A unicode sparkline of the per-second history, scaled to its own max.
fn sparkline(history: &VecDeque<u64>) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max = history.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return "▁".repeat(history.len().max(1));
    }
    history
        .iter()
        .map(|&v| {
            let idx = ((v as f64 / max as f64) * (BARS.len() - 1) as f64).round() as usize;
            BARS[idx.min(BARS.len() - 1)]
        })
        .collect()
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut v = bytes as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(topic: &str, payload: &str, qos: u8, retained: bool) -> PluginEvent {
        PluginEvent::MessageReceived {
            id: 0,
            topic: topic.into(),
            payload: payload.into(),
            qos,
            retained,
        }
    }

    #[test]
    fn accumulates_and_ticks() {
        let mut a = TrafficAnalytics::default();
        a.on_event(&msg("a/b", "hello", 1, false));
        a.on_event(&msg("a/b", "hi", 0, true));
        a.on_event(&msg("c", "x", 2, false));
        assert_eq!(a.total_msgs, 3);
        assert_eq!(a.total_bytes, 8); // 5 + 2 + 1
        assert_eq!(a.qos, [1, 1, 1]);
        assert_eq!(a.retained, 1);
        assert_eq!(a.topics["a/b"], 2);

        a.on_event(&PluginEvent::Tick);
        assert_eq!(a.rate_msgs, 3);
        assert_eq!(a.peak_msgs, 3);
        assert_eq!(a.sec_msgs, 0); // reset for the next second
        assert_eq!(a.history.back(), Some(&3));
    }

    #[test]
    fn connected_resets_and_pane_lists_top_topic() {
        let mut a = TrafficAnalytics::default();
        a.on_event(&msg("busy", "x", 0, false));
        a.on_event(&PluginEvent::Connected {
            connection: "c".into(),
        });
        assert_eq!(a.total_msgs, 0); // reset

        a.on_event(&msg("busy", "x", 0, false));
        a.on_event(&msg("busy", "x", 0, false));
        a.on_event(&msg("quiet", "x", 0, false));
        let pane = a.pane().unwrap();
        assert_eq!(pane.title, "Traffic Analytics");
        // The rendered text mentions the busiest topic and its count.
        let text: String = pane
            .lines
            .iter()
            .flat_map(|l| l.iter().map(|s| s.text.clone()))
            .collect::<Vec<_>>()
            .join("");
        assert!(text.contains("busy"));
    }

    #[test]
    fn open_command_requests_pane() {
        let mut a = TrafficAnalytics::default();
        assert_eq!(a.invoke("open"), vec![PluginAction::OpenPane]);
        assert_eq!(a.commands().len(), 1);
    }

    #[test]
    fn human_bytes_scales() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
    }
}
