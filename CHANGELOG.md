# Changelog

All notable changes to lazymqtt are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

<!-- Add changes here as they land; they become the next release's notes. -->

## [0.1.1] - 2026-07-30

### Added

- TLS certificate validation toggle, for connecting to brokers with self-signed
  certificates.

## [0.1.0] - 2026-07-28

Initial public release.

### Added

- Keyboard-driven terminal UI MQTT client (ratatui + rumqttc): connections
  manager, live topic tree, message history, and a publish form.
- Payload viewer with text selection/yank, folding, and an inspector.
- In-process plugin system (opt-in, disabled by default), with built-ins:
  - **json-view** — pretty-prints and syntax-colors JSON payloads.
  - **json-marker** — flags topics carrying JSON.
  - **topic-alerts** — rule-based alerts on topics/payloads, edited in-app.
  - **json-schema** — validates payloads against per-topic JSON Schemas, with
    an in-app schema editor.
  - **topic-recorder** — records a connection's traffic to JSONL and replays it,
    with loop, topic-prefix rewrite, and a speed multiplier.
  - **publish-templates** — saved publish presets with `{{placeholder}}` fills;
    save the current publish form with `^T`.
  - **payload-generator** — publishes synthetic payloads (counters, random,
    timestamps), one-shot or streaming.
  - **traffic-analytics** — a live stats pane (rate, throughput, QoS mix,
    retained share, busiest topics).
- Command menu (`m`) with core commands plus a per-plugin submenu.
- Color theming with built-in presets and an in-app editor; auto-persisted.
- Config stored under `~/.config/lazymqtt/` (XDG), with one-time migration from
  the old macOS `Application Support` location.
- `--version` / `--help` command-line flags.

[Unreleased]: https://github.com/ScottFelder/lazymqtt/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/ScottFelder/lazymqtt/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ScottFelder/lazymqtt/releases/tag/v0.1.0
