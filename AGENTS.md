# CLAUDE.md

Guidance for AI assistants (and humans) working in the LazyMQTT codebase.

## What this is

LazyMQTT is a terminal-UI MQTT client written in Rust, inspired by MQTT Explorer.
It lets a user save broker connections, store per-connection topic subscriptions,
watch incoming messages in a live collapsible topic tree, and publish messages.
The design goal is speed: an async MQTT task feeds a non-blocking render loop.

## Build & run

```bash
cargo run                  # debug
cargo build --release      # optimized (LTO, stripped)
cargo clippy               # lint before committing
cargo fmt                  # format (rustfmt defaults)
```

There is no test suite yet. If you add logic worth testing (tree building,
topic parsing, config round-trips), put unit tests in the relevant module with
`#[cfg(test)]`.

## Architecture

The app is a single-threaded UI loop plus a set of async MQTT tasks that
communicate over channels. Keep that boundary clean.

```
main.rs      Terminal setup + the render/input loop. Owns the tokio runtime.
app.rs       All UI state (App struct), Screen/Focus/Status enums, form buffers.
config.rs    Connection + Subscription structs; JSON persistence to disk.
mqtt.rs      Async client task. Message, MqttEvent, MqttCommand, MqttHandle.
tree.rs      TopicTree: aggregates messages into a hierarchy split on '/'.
ui.rs        All ratatui rendering. One draw_* fn per screen. No state mutation.
events.rs    All keyboard handling. One *_keys fn per screen. Mutates App.
```

### Data flow

- **UI → MQTT**: the UI sends `MqttCommand` values (Subscribe, Unsubscribe,
  Publish, Disconnect) through `MqttHandle.commands`. Use `App::send(cmd)`.
- **MQTT → UI**: the client task emits `MqttEvent` values (Connected,
  Disconnected, Message, Error) through `MqttHandle.events`. The main loop
  drains these each frame with `try_recv` and applies them to `App`.

The two channels are the only link between the async world and the UI. Do not
call MQTT client methods directly from `ui.rs` or `events.rs`; go through a
command.

### The render loop (main.rs)

Each iteration: drain MQTT events → `terminal.draw` → poll input for 50ms →
handle key. The 50ms poll timeout is what keeps live messages flowing while
staying responsive. Do not block anywhere in this loop.

## Conventions and gotchas

- **Borrow checker in the event drain**: MQTT events are collected into a `Vec`
  first so the `&mut app.handle` borrow ends before `app` is mutated in the
  match arms. Keep that pattern; matching while holding the handle borrow won't
  compile (E0499).
- **Screens are a state machine**: adding a screen means touching four places —
  the `Screen` enum (app.rs), the render dispatch in `ui::draw`, a `draw_*`
  function, and a `*_keys` handler routed from `events::handle_key`. Also update
  the status-bar hint string and the help text.
- **ui.rs never mutates state**; events.rs never renders. Preserve this split.
- **Persistence is explicit**: after any change to `config.connections` (add,
  edit, delete, subscribe, unsubscribe) call `app.config.save()`. There is no
  autosave.
- **Colors**: use the `ACCENT` and `DIM` constants in ui.rs rather than raw
  `Color` values, so theming stays consistent.
- **Timestamps** render as `MM/dd/yyyy HH:mm:ss.SSS` via chrono format
  `%m/%d/%Y %H:%M:%S%.3f`. Keep 24-hour, millisecond precision.
- **Field-indexed forms**: `FormBuffer` and `PublishBuffer` track the focused
  field by `usize` index. If you add a field, update the index count, the
  `field_mut`/render mapping, and any `% N` wraparound arithmetic together —
  they must stay in sync.
- **QoS** is stored as a plain `u8` (0/1/2) in config and commands; convert to
  `rumqttc::QoS` only at the client boundary via `mqtt::qos_from`.

## Security note

Broker passwords are currently stored in plaintext in `connections.json`.
This is a known tradeoff for local-dev convenience. If asked to harden it, the
intended path is the OS keyring (`keyring` crate), not encrypting the JSON.

## Scope guidance

Keep additions in the spirit of a fast, keyboard-driven single-binary TUI.
Favor small, focused modules over frameworks. Avoid adding background threads
beyond the MQTT tasks, and avoid anything that blocks the render loop.
