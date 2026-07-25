# AGENTS.md

Guidance for AI assistants (and humans) working in the LazyMQTT codebase.

## What this is

LazyMQTT is a terminal-UI MQTT client written in Rust, inspired by MQTT Explorer.
It lets a user save broker connections, store per-connection topic subscriptions,
watch incoming messages in a live collapsible topic tree, inspect and publish
messages, clear retained messages, and copy payload text to the clipboard. A
small in-process plugin system can observe the message stream and annotate or
re-render it. The design goal is speed: an async MQTT task feeds a non-blocking
render loop.

## Build & run

```bash
cargo run                  # debug
cargo build --release      # optimized (LTO, stripped)
cargo test                 # unit tests
cargo clippy               # lint before committing
cargo fmt                  # format (rustfmt defaults)
```

There is a small `#[cfg(test)]` suite (selection/yank math, plugin dispatch,
annotations, JSON view). Add tests in the relevant module when you add logic
worth testing (tree building, topic parsing, config round-trips, plugin
behavior). Two long-standing clippy warnings are pre-existing (a derivable
`PublishBuffer::Default` and a needless borrow in `ui.rs`); don't count them as
new. Keep the tree warning-free otherwise.

## Architecture

The app is a single-threaded UI loop plus a set of async MQTT tasks that
communicate over channels. Keep that boundary clean.

```
main.rs      Terminal setup + the render/input loop. Owns the tokio runtime.
app.rs       All UI state (App struct), Screen/Focus/PaneFold/Status enums,
             form buffers, the DetailLine model, plugin host + annotations.
config.rs    Connection + Subscription structs; JSON persistence to disk.
mqtt.rs      Async client task. Message, MqttEvent, MqttCommand, MqttHandle.
tree.rs      TopicTree: aggregates messages into a hierarchy split on '/'.
ui.rs        All ratatui rendering. One draw_* fn per screen. No state mutation.
events.rs    All keyboard handling. One *_keys fn per screen. Mutates App.
plugin/      In-process plugin API + host + built-in plugins.
  mod.rs       Plugin trait, PluginHost (dispatch, enable/disable, inspect).
  api.rs       PluginEvent / PluginAction / Annotation / Inspector* types.
  config.rs    per-plugin enable/disable, persisted under plugins/.
  builtin/     bundled plugins (json-marker, json-view, topic-alerts).
```

### Data flow

- **UI → MQTT**: the UI sends `MqttCommand` values (Subscribe, Unsubscribe,
  Publish, Disconnect) through `MqttHandle.commands`. Use `App::send(cmd)`.
- **MQTT → UI**: the client task emits `MqttEvent` values (Connected,
  Disconnected, Message, Error) through `MqttHandle.events`. The main loop
  drains these each frame with `try_recv` and applies them to `App`.
- **App → plugins**: hook points build a `PluginEvent` and call
  `App::dispatch_plugin` (or it happens inside `App::push_message`); the host
  returns `PluginAction`s which `App::apply_plugin_actions` turns back into
  `MqttCommand`s, annotations, or a status message.

The MQTT channels are the only link between the async world and the UI. Do not
call MQTT client methods directly from `ui.rs` or `events.rs`; go through a
command. Plugins never touch `App` directly — only events in, actions out.

### The render loop (main.rs)

Each iteration: drain MQTT events (dispatching Connected/Disconnected to
plugins) → dispatch a plugin `Tick` about once a second → `terminal.draw` →
poll input for 50ms → handle key. On quit, dispatch `Shutdown`. The 50ms poll
keeps live messages flowing while staying responsive. Do not block anywhere in
this loop — plugin dispatch is synchronous here, so plugins must be fast.

## Conventions and gotchas

- **Borrow checker in the event drain**: MQTT events are collected into a `Vec`
  first so the `&mut app.handle` borrow ends before `app` is mutated in the
  match arms. Keep that pattern (E0499 otherwise).
- **Screens are a state machine**: adding a screen means touching the `Screen`
  enum (app.rs), the render dispatch in `ui::draw`, a `draw_*` function, and a
  `*_keys` handler routed from `events::handle_key` — plus the status-bar hint
  string and the help text.
- **Broker commands go through a registry**: `Command` + `BROKER_COMMANDS` +
  `App::run_command` (app.rs) are the single source of truth. A shortcut key in
  `broker_keys` and the `m` command menu both call `run_command`, and the menu
  renders from `BROKER_COMMANDS`. Add new broker commands there, not as bespoke
  key arms.
- **ui.rs never mutates state**; events.rs never renders. Preserve this split.
- **Persistence is explicit**: after any change to `config.connections` call
  `app.config.save()`. There is no autosave. Plugin enable/disable persists via
  `PluginHost::toggle`.
- **Colors**: use the `ACCENT` and `DIM` constants in ui.rs rather than raw
  `Color` values.
- **DetailLine model**: the Payload and History panes are built from
  `Vec<DetailLine>` (segments + a decorative non-selectable `lead`). The
  keyboard selection cursor and yank operate on `App::active_lines()` (the
  focused pane's lines), and `payload_lines()` branches on `payload_view` to
  render either the raw text or a plugin inspector view. Keep decorative bits in
  `lead` so yanks stay clean.
- **Stable message identity**: `Message.id` is a monotonic id assigned in
  `App::push_message` (the MQTT task constructs messages with `0`). It keys both
  plugin annotations and History expand/collapse — use it, not the timestamp.
- **Timestamps** render as `MM/dd/yyyy HH:mm:ss.SSS` via chrono format
  `%m/%d/%Y %H:%M:%S%.3f`. Keep 24-hour, millisecond precision.
- **Field-indexed forms**: `FormBuffer` and `PublishBuffer` track the focused
  field by `usize` index. If you add a field, update the index count, the
  `field_mut`/render mapping, and any `% N` wraparound arithmetic together.
- **QoS** is a plain `u8` (0/1/2) in config and commands; convert to
  `rumqttc::QoS` only at the client boundary via `mqtt::qos_from`.
- **Connection lifecycle**: rumqttc's event loop auto-reconnects as long as it
  is polled, so a user disconnect must actually stop it — the `mqtt.rs` task
  uses a shared `stop` flag and breaks on `Outgoing::Disconnect`. Don't
  reintroduce a loop that reconnects after a disconnect, or old connections leak
  and fight the new one. `start_connection` also tears down any live handle
  first.
- **Client id**: `mqtt::connect` appends a short random suffix to the configured
  client id so concurrent instances never collide ("session taken over").

## Plugins

Plugins implement the `Plugin` trait (`plugin/mod.rs`) and are registered in
`plugin/builtin/all()`. The trait methods are defaulted, so a plugin implements
only what it needs:

- `on_event(&PluginEvent) -> Vec<PluginAction>` — observe and react
  (annotate a message, publish, (un)subscribe, show a status).
- `inspect(&InspectMessage) -> Option<InspectorView>` — supply an alternative
  Payload rendering (e.g. pretty JSON); the raw view always stays available.

Keep the boundary UI-agnostic: events carry owned data, actions are the only way
to affect the app, and annotations attach to a message by id. Execution is
in-process/compiled-in by design; external-process and WASM loading, plus a
bytes-payload refactor and command palette, are deferred (see `FEATURES.md`).
Don't pull a heavy scripting/WASM runtime in without discussion — it undercuts
the single-binary, small-dependency goal.

`topic-alerts` config is **per connection**: rules live in
`plugins/alerts/<connection-id>.json` (shared model in `plugin/alerts_rules.rs`,
reused by the in-app editor reached with `A`). `PluginEvent::Connected` carries
the connection id so the plugin loads that connection's rules; editing while
connected re-dispatches `Connected` to reload live.

## Security note

Broker passwords are currently stored in plaintext in `connections.json`.
This is a known tradeoff for local-dev convenience. If asked to harden it, the
intended path is the OS keyring (`keyring` crate), not encrypting the JSON.

## Scope guidance

Keep additions in the spirit of a fast, keyboard-driven single-binary TUI.
Favor small, focused modules over frameworks. Avoid adding background threads
beyond the MQTT tasks, and avoid anything that blocks the render loop.
