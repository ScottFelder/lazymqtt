# LazyMQTT

A fast, terminal-UI MQTT client written in Rust — inspired by [MQTT Explorer](https://mqtt-explorer.com/), but keyboard-driven and living in your terminal.

## Features

- **Saved connections** — create, edit, and delete broker profiles (host, port, client ID, credentials, TLS). Persisted to disk as JSON.
- **Per-connection subscriptions** — store the topics each connection subscribes to on connect (comma-separated, wildcards supported: `sensors/#`, `home/+/temp`).
- **Live topic tree** — incoming messages are aggregated into a collapsible hierarchy split on `/`, with per-node message counts and last-value previews (the signature MQTT Explorer view).
- **Message inspector** — select any topic to view its recent message history (timestamp, QoS, retain flag, payload).
- **Publish** — send messages to any topic with QoS 0/1/2 and an optional retain flag.
- **Fast** — async `rumqttc` event loop feeding a non-blocking `ratatui` render loop; release build is LTO-optimized and stripped.

## Build & Run

Requires a Rust toolchain (`rustup`, stable).

```bash
cargo run            # debug
cargo build --release && ./target/release/lazymqtt   # optimized
```

Config is stored at your platform config dir, e.g. `~/.config/lazymqtt/connections.json` on Linux.

## Keybindings

**Connections**
| Key | Action |
|-----|--------|
| `j`/`k` or ↑/↓ | move |
| `Enter` | connect |
| `n` | new · `e` edit · `d` delete |
| `?` | help · `q` quit |

**Connection form**
| Key | Action |
|-----|--------|
| `Tab`/↑↓ | switch field |
| `space` | toggle TLS |
| `Enter` | save · `Esc` cancel |

**Broker**
| Key | Action |
|-----|--------|
| `j`/`k` or ↑/↓ | move in tree |
| →/`Enter` | expand · ← collapse |
| `Tab` | switch pane |
| `p` | publish · `c` clear tree |
| `Esc` | disconnect · `?` help · `Ctrl-q` quit |

## Quick test

```bash
# public test broker
Name: Test  Host: test.mosquitto.org  Port: 1883  Topics: #
```

## Project layout

```
src/
  main.rs      entry point, terminal + async loop
  app.rs       application state
  config.rs    connection/subscription persistence
  mqtt.rs      async MQTT client task + event/command channels
  tree.rs      hierarchical topic tree
  ui.rs        ratatui rendering
  events.rs    keyboard handling per screen
```

## License

MIT
