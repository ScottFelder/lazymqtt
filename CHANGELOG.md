# Changelog

All notable changes to lazymqtt are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.4] - 2026-08-12

### Added

- **xml-view** plugin — pretty-prints and syntax-colors XML payloads as an
  alternate Payload view, the XML counterpart to json-view.
- **protobuf-view** plugin — decodes binary protobuf payloads into named fields.
  Map topics to a `.proto` + message type per connection (edited in-app with
  `B`); the `.proto` is compiled at runtime (pure-Rust via protox + prost-reflect,
  no `protoc`) and matching payloads auto-decode to a syntax-colored field tree.
- Message payloads now preserve their exact bytes (`Message.payload_raw`, also on
  `InspectMessage`/`PluginEvent::MessageReceived`) so binary decoders like
  protobuf-view see the real bytes, not the lossy UTF-8 rendering.

### Changed

- The Payload pane now **auto-selects** the structured view matching each
  payload: a JSON payload shows json-view, an XML payload shows xml-view (when
  the plugin is enabled), with no keypress needed.
- The Payload view choice (`i`) is now **sticky** across messages: switching to
  raw stays raw, and the auto choice keeps picking the matching view as you move
  between messages, instead of reverting each time.
- `i` toggles only between raw and the view(s) the current message can actually
  produce — so on a JSON message it's raw ↔ JSON with no dead step (previously it
  also stopped on inapplicable views like XML).

## [0.1.3] - 2026-08-04

### Added

- The `?` help screen is now tabbed: a core "Keybindings" tab plus one tab per
  enabled plugin, each showing that plugin's keys and notes. Tab / ←→ switch
  tabs, 1-9 jump. Plugins contribute their own help via a new `Plugin::help`
  method, so the help stays in sync with what's enabled.
- Pressing `?` inside a plugin's command submenu opens the help straight to that
  plugin's tab.

### Changed

- Message panes now label the quality-of-service as `QoS 0` (etc.) instead of
  the terse `q0`.
- The Subscribe prompt now has a QoS selector (Tab to the field, space/←→ to
  cycle 0/1/2); the chosen QoS is used for the subscription and saved.
- Connection edit: subscriptions are now a managed sub-list (each with its own
  topic + QoS) that you add / edit / delete, replacing the single
  comma-separated topics field. Open it from the Subscriptions field with
  space/→.

## [0.1.2] - 2026-08-01

### Added

- Command menu (`m`) now supports accelerator keys: pressing a row's own key
  activates it directly (e.g. `T` opens the Theme screen), in addition to
  scrolling to it and pressing Enter.
- Enabled plugins now advertise their keys in the broker status bar: e.g.
  enabling json-view adds the `i` (payload view) hint, and disabling it removes
  the hint — so the keys a plugin provides are discoverable.
- Topics tree: expand (`E`) and collapse (`C`) the selected topic and all of its
  children, also available from the `m` command menu. The cursor stays on the
  selected topic.

### Changed

- Status-bar command hints now render each binding as a highlighted key plus a
  dim label, with a single consistent separator — clearer and more polished than
  the previous flat text.

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

[Unreleased]: https://github.com/ScottFelder/lazymqtt/compare/v0.1.4...HEAD
[0.1.4]: https://github.com/ScottFelder/lazymqtt/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/ScottFelder/lazymqtt/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/ScottFelder/lazymqtt/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/ScottFelder/lazymqtt/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ScottFelder/lazymqtt/releases/tag/v0.1.0
