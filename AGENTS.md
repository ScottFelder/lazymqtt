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

## Working agreement (read first)

Two rules apply to every change, no exceptions:

1. **Branch per feature.** Never commit a new feature directly to `master`.
   Start each feature (or non-trivial change) on its own branch off `master`
   (`git checkout -b <name>`), and merge via pull request. `master` stays
   releasable at all times.
2. **Leave the code cleaner than you found it.** The codebase is growing, so
   pay down technical debt as you go: when a change touches messy, duplicated,
   or awkward code, refactor it in the same branch — extract shared helpers,
   remove duplication, keep modules small and focused. Don't just bolt new
   code on. (Example: the JSON→styled-span colorizer was extracted into
   `plugin/builtin/jsonfmt.rs` so json-view and protobuf-view share it.) Keep
   refactors scoped to what's relevant so the diff stays reviewable, and keep
   `fmt`/`clippy`/`test` green.

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
app/         The App state object + its behavior, one submodule per concern.
  mod.rs       App struct, App::new, shared free helpers, re-exports, tests.
  screen.rs    Screen/Command enums + the command-menu registry.
  view.rs      Focus/PaneFold/DetailKind + the DetailLine render model.
  forms.rs     FormBuffer/PublishBuffer/AlertForm buffers + Status.
  connection.rs  connect/disconnect, send, push_message, plugin dispatch.
  commands.rs  the command registry entry point + `m`-menu building.
  broker.rs    topic selection, payload/history line building, text selection.
  alerts.rs / schemas.rs / recordings.rs / theme.rs  each screen's App-side logic.
  textarea.rs  reusable multi-line text buffer + cursor (recording & schema editors).
config.rs    Connection + Subscription structs; JSON persistence to disk.
paths.rs     Config dir (~/.config/lazymqtt, XDG on every OS) + one-time
             migration from the old macOS Application Support location.
theme.rs     Theme (color specs) + Palette (resolved Colors) + presets +
             theme.json persistence. See the theming note below.
mqtt.rs      Async client task. Message, MqttEvent, MqttCommand, MqttHandle.
tree.rs      TopicTree: aggregates messages into a hierarchy split on '/'.
ui/          ratatui rendering, one module per screen; never mutates state.
  mod.rs       `draw` dispatcher (Screen -> screen module).
  common.rs    shared widgets: title_block, pane_title, scrollbar, center_rect.
  <screen>.rs  broker, connections, publish, alerts, recordings, theme,
               plugins, menu, help, statusbar.
events/      keyboard/paste handling, one module per screen; mutates App.
  mod.rs       handle_key/handle_paste dispatchers + strip_newlines.
  <screen>.rs  broker, connections, publish, alerts, recordings, theme,
               plugins, menu.
plugin/      In-process plugin API + host + built-in plugins.
  mod.rs       Plugin trait, PluginHost (dispatch, enable/disable, inspect).
  api.rs       PluginEvent / PluginAction / Annotation / Inspector* types.
  config.rs    per-plugin enable/disable, persisted under plugins/.
  builtin/     bundled plugins (json-marker, json-view, xml-view,
               topic-alerts, json-schema, publish-templates,
               payload-generator, traffic-analytics, topic-recorder).
  topics.rs    shared MQTT topic-filter matching (`+`/`#`).
  schemas.rs   per-connection topic→schema mappings + subset validator.
  templates.rs global publish presets (topic/payload/QoS/retain).
  generators.rs global payload generators (counter/random/timestamp).
```

`App`'s methods are split across `app/*.rs` as separate `impl App` blocks; the
state types live in `app/{screen,view,forms}.rs` and are re-exported from
`app/mod.rs`, so `crate::app::{App, Screen, DetailLine, …}` still resolve. When
adding a screen, its state/logic, rendering, and input each get a file in
`app/`, `ui/`, and `events/` respectively.

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
plugins) → dispatch a plugin `Tick` about once a second and a `FrameTick` about
every 100ms → `terminal.draw` → poll input for 50ms → handle key. On quit,
dispatch `Shutdown`. `Tick` is for slow work (alerts' silence counter assumes
1 tick ≈ 1s); `FrameTick` is for time-sensitive work like replay pacing. The 50ms poll
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
  `broker_keys` and the `m` command menu both call `run_command`. Add new broker
  commands there, not as bespoke key arms. The `m` menu (`open_command_menu`)
  builds a `Vec<MenuItem>` fresh each open, two levels deep tracked by
  `menu_plugin`: the top level is the core `BROKER_COMMANDS` plus one
  `MenuAction::Submenu(plugin)` opener per plugin that has commands
  (`PluginHost::command_plugins`); descending (`open_plugin_submenu`) shows that
  plugin's own `commands()`. `build_menu_items` is level-aware and re-runs after
  each adjust, so labels reflect live state. Enter routes by `MenuAction` —
  `Core` → `run_command`, `Submenu` → descend, `Plugin { plugin, id }` →
  `invoke_plugin_command`. Plugins add entries via `commands`/`invoke`, not by
  editing the menu.
- **Adjustable menu items cycle in place**: a `PluginCommand` with
  `adjustable: true` (built with `PluginCommand::option`) is an option cycler,
  not a one-shot action. Left/right (or `h`/`l`) call `App::adjust_selected_menu_item`
  → `PluginHost::adjust(index, id, forward)` → `Plugin::adjust(id, forward)`,
  then rebuild the rows (`build_menu_items`) so the label reflects the new option
  while the menu stays open. Enter on an adjustable row steps forward too (stays
  open); Enter on a one-shot row (`PluginCommand::action`) closes the menu and
  runs. `Plugin::adjust` defaults to `invoke` (forward-only), so single-direction
  cyclers need no override.
- **ui.rs never mutates state**; events.rs never renders. Preserve this split.
- **Persistence is explicit**: after any change to `config.connections` call
  `app.config.save()`. There is no autosave. Plugin enable/disable persists via
  `PluginHost::toggle`.
- **Colors go through the theme**: every color comes from the active `Palette`
  (`app.palette`, resolved from `app.theme`), never a hardcoded `Color`. Draw
  functions that take `app` read `app.palette`; the color-using helpers
  (`title_block`, `pane_title`, `style_for`, `render_vscrollbar`, the popups)
  take a `pal: &Palette`. Adding a new colored element means adding a role to
  `theme.rs` (the `ROLES` table, the `I_*` index, `Palette`, `Default`, and each
  preset) and reading `pal.<role>` — don't reintroduce literal `Color::` values
  in ui.rs. The theme is edited on `Screen::Theme` and persisted to `theme.json`;
  the palette is recomputed (`App::refresh_palette`) on every change for live
  preview.
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
  Payload rendering (e.g. pretty JSON); the raw view always stays available. The
  `InspectorView.label` names the view (e.g. "JSON"). `App::payload_view`
  (`PayloadView`: Auto / Raw / Named) is sticky across messages; `Auto` (default)
  auto-selects the matching view per payload, and `i` toggles only among the
  views the current message can produce plus raw.
- `commands(&self) -> Vec<PluginCommand>` / `invoke(&mut self, id) -> Vec<PluginAction>`
  — contribute entries to the `m` command menu and handle them. Labels are
  computed in `commands()` so they can reflect state ("Stop recording (N msgs)").
- `pane(&self) -> Option<PaneView>` — own a whole screen (a dashboard of styled
  `PaneSpan` lines, still UI-agnostic — no ratatui). Rendered fresh each frame
  in `Screen::PluginPane`. A plugin opens its pane by returning
  `PluginAction::OpenPane` from a command; `App::invoke_plugin_command` resolves
  which plugin (it knows the index) and sets `pane_plugin`. See
  `builtin/analytics.rs` for the traffic-analytics example.

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

`json-schema` follows the same per-connection pattern: topic→schema mappings in
`plugins/schemas/<connection-id>.json` loaded on `Connected` (model + a
dependency-free subset validator in `plugin/schemas.rs`), edited in-app via
`Screen::Schemas`/`SchemaForm` (`S`), which persists and re-dispatches
`Connected` to reload live — exactly like the alerts editor. Both it and
`topic-alerts` match topics via `plugin/topics.rs::matches` (don't duplicate
wildcard logic).

`publish-templates` stores **global** presets in `plugins/publish-templates.json`
(`plugin/templates.rs`) and surfaces each as an `m`-menu command whose `id` is
the template name (this is why `PluginCommand.id`/`MenuAction::Plugin.id` are
`String`, not `&'static str`). Invoking one emits `PluginAction::OpenPublish`,
which the app applies by pre-filling the publish form; `^T` there saves the
current form back as a template.

Multi-line editors (recording JSONL, schema body) share `app/textarea.rs`
(`TextArea`: buffer + char cursor) and render through `ui::common::draw_textarea`
— reuse those rather than re-implementing cursor math.

`topic-recorder` records the active connection's `MessageReceived` events to
`plugins/recordings/<connection-id>-<label>.jsonl` (one `RecordedMessage` per
line with a relative `offset_ms`) and replays a recording for the active
connection on `FrameTick`, emitting `PluginAction::Publish` for each message
whose offset has arrived (`drain_due`). It rewrites topics under a `replay/`
prefix by default and skips recording while replaying, so it never feeds on its
own echoes. `Connected`/`Disconnected` reset record+replay+selection state so
recordings never span connections.

The recording model (`RecordedMessage`) and all file management live in
`plugin/recordings.rs` (list / rename / delete / newest / `path_for` / `read` /
`to_lines` / `parse_lines` / `write_label`), shared by the recorder plugin and
the app — mirroring `alerts_rules`. The app-level recordings picker
(`Screen::Recordings`, opened with `R` or the "Recordings" command) lists a
connection's recordings and edits/renames/deletes them **directly** via
`crate::plugin::{list,read,save,rename,delete}_recording*` — the plugin isn't in
that loop. The `<connection-id>-` filename prefix scopes recordings per
connection and is preserved across renames/edits; only the `label` suffix
changes. Picking a recording to **replay** is the one app→recorder call:
`PluginHost::use_item(RECORDER, label)` → `Plugin::use_item`, which sets the
recorder's `selected` recording and starts replay (falling back to newest if the
selection has since vanished). `use_item` is the generic "act on a named item
chosen from an app management screen" hook; most plugins don't implement it.

`e` in the picker opens a multi-line JSONL editor (`Screen::RecordingEdit`,
state `rec_edit_*` in `App`): the recording is loaded as canonical one-line-per-
message text, edited with a char-addressed cursor (`char_byte_idx` bridges char
columns to the UTF-8 `String`), and saved via `save_recording_text`, which
validates every line through `recordings::parse_lines` before writing so a bad
edit never corrupts the file. `^S` overwrites the current recording; `^N` is
"save as" (a filename prompt in `rec_edit_saveas`) writing a new recording and
leaving the original intact.

## Security note

Broker passwords are currently stored in plaintext in `connections.json`.
This is a known tradeoff for local-dev convenience. If asked to harden it, the
intended path is the OS keyring (`keyring` crate), not encrypting the JSON.

## Scope guidance

Keep additions in the spirit of a fast, keyboard-driven single-binary TUI.
Favor small, focused modules over frameworks. Avoid adding background threads
beyond the MQTT tasks, and avoid anything that blocks the render loop.
