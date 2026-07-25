# LazyMQTT

A fast, terminal-UI MQTT client written in Rust — inspired by [MQTT Explorer](https://mqtt-explorer.com/), but keyboard-driven and living in your terminal.

![LazyMQTT demo](assets/demo.gif)

> The demo is scripted with [VHS](https://github.com/charmbracelet/vhs); regenerate it with `vhs assets/demo.tape` (see the tape header for prerequisites).

## Features

- **Saved connections** — create, edit, and delete broker profiles (host, port, client ID, credentials, TLS). Persisted to disk as JSON.
- **Per-connection subscriptions** — store the topics each connection subscribes to on connect (comma-separated, wildcards supported: `sensors/#`, `home/+/temp`).
- **Live topic tree** — incoming messages are aggregated into a collapsible hierarchy split on `/`, with per-node message counts and last-value previews (the signature MQTT Explorer view).
- **Message inspector** — a **Payload** pane for the selected message and a **History** pane of every message on the topic (timestamp, QoS, retain flag, expandable inline). Either sub-pane can be collapsed so the other fills the space.
- **Publish** — send messages to any topic with QoS 0/1/2 and an optional retain flag.
- **Clear retained messages** — drop a broker's retained message on the selected topic (with confirmation).
- **Copy & paste** — keyboard text selection in the Payload/History panes yanks to the system clipboard; paste into any input field.
- **Plugins** — an in-process plugin system observes the message stream and annotates or re-renders it. Built-ins: a JSON-validity marker and a JSON pretty-print view. Enable/disable per plugin (persisted).
- **Fast** — async `rumqttc` event loop feeding a non-blocking `ratatui` render loop; release build is LTO-optimized and stripped.

## Build & Run

Requires a Rust toolchain (`rustup`, stable).

```bash
cargo run            # debug
cargo build --release && ./target/release/lazymqtt   # optimized
cargo test           # unit tests
```

Config lives in your platform config dir — `connections.json` for broker
profiles and `plugins/config.json` for plugin enable/disable state. For example
on macOS: `~/Library/Application Support/dev.lazymqtt.lazymqtt/`, and on Linux
`~/.config/lazymqtt/`.

Each connection uses a unique client id at runtime (`<your-id>-<random>`) so two
instances (or a leftover) never collide on the broker.

## Keybindings

**Connections**
| Key | Action |
|-----|--------|
| `j`/`k` or ↑/↓ | move |
| `Enter` | connect |
| `n` · `e` · `d` | new · edit · delete |
| `P` | plugins · `?` help · `q` quit |

**Connection form**
| Key | Action |
|-----|--------|
| `Tab` / ↑↓ | switch field |
| `space` | toggle TLS |
| `Enter` · `Esc` | save · cancel |

**Broker — Topics pane** (`1`)
| Key | Action |
|-----|--------|
| `j`/`k` or ↑/↓ | move in tree |
| `Enter` | expand/collapse · `→` expand · `←` collapse |
| `Tab` | cycle panes · `1`/`2`/`3` focus Topics/Payload/History |
| `s` · `u` | subscribe · unsubscribe (selected) |
| `p` · `r` · `c` | publish · clear retained · clear tree |
| `P` · `?` | plugins · help |
| `Esc` · `Ctrl-q` | disconnect · quit |

**Broker — Payload (`2`) / History (`3`) panes**
| Key | Action |
|-----|--------|
| `hjkl` or ↑/↓/←/→ | move the text cursor |
| `v` · `y` | start/extend selection · yank to clipboard (whole line if no selection) |
| `Esc` | clear selection |
| `z` | collapse/expand the focused pane (the other fills the space) |
| `i` | cycle Payload view (raw ↔ plugin views, e.g. JSON) |
| `Enter` (History) | expand/collapse the entry under the cursor |

**Plugins screen** (`P`)
| Key | Action |
|-----|--------|
| `j`/`k` | move |
| `space`/`Enter` | toggle enabled |
| `Esc` | close |

## Plugins

Plugins are compiled into the binary and run in-process. They observe events
(message received, connect/disconnect, tick, …) and respond with actions
(annotate a message, publish, subscribe, show a status) or supply an alternative
Payload rendering — without touching connection state directly. Toggle them on
the Plugins screen (`P`); the choice persists in `plugins/config.json`.

Built-in plugins:

- **json-marker** — flags whether each payload is valid JSON (annotation).
- **json-view** — pretty-prints JSON payloads (syntax-colored) as an alternate Payload view (`i`).
- **topic-alerts** — raises alerts (annotations + status) from **per-connection** rules.

Alert rules are **per connection** and edited in-app: press `A` (from
Connections or Broker) to open the rules editor — `a` add, `e` edit, `d` delete.
They're stored at `plugins/alerts/<connection-id>.json` and loaded when that
connection connects.

A rule's condition (`when`) is `above` / `below` (with `value`), `changed`, or
`silent` (with `seconds`). An optional JSON `field` extracts a top-level field
for numeric comparisons; otherwise the whole payload is parsed as a number.
Severity is `warn` (default) or `error`. Alerts are visual only — no commands
are run.

See `FEATURES.md` for the plugin roadmap (schema validation, record/replay,
analytics, external-process/WASM loading, …).

## Quick test

```bash
# public test broker
Name: Test  Host: test.mosquitto.org  Port: 1883  Topics: #
```

## Project layout

```
src/
  main.rs      entry point, terminal + async render/input loop
  app.rs       application state (App, Screen/Focus/PaneFold, DetailLine)
  config.rs    connection/subscription persistence
  mqtt.rs      async MQTT client task + event/command channels
  tree.rs      hierarchical topic tree
  ui.rs        ratatui rendering (one draw_* fn per screen)
  events.rs    keyboard handling per screen
  plugin/      in-process plugin API, host, and built-in plugins
    mod.rs       Plugin trait + PluginHost
    api.rs       event/action/annotation/inspector types
    config.rs    per-plugin enable/disable persistence
    builtin/     bundled plugins (json-marker, json-view)
```

## License

MIT
