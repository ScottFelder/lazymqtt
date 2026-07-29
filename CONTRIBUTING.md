# Contributing to LazyMQTT

Thanks for your interest in LazyMQTT! Contributions of all kinds are welcome —
bug reports, feature ideas, documentation fixes, new plugins, and code.

## Ways to contribute

- **Report a bug** — open a [GitHub issue](https://github.com/ScottFelder/lazymqtt/issues)
  with what you did, what you expected, and what happened. Include your OS, how
  you installed lazymqtt (`brew` / `cargo` / source), and the version
  (`lazymqtt --version`). A minimal repro (broker, topic, payload) helps a lot.
- **Suggest a feature** — open an issue describing the use case first, so we can
  agree on the shape before you write code. `FEATURES.md` tracks ideas already
  on the radar.
- **Improve the docs** — README, `AGENTS.md`, this file, or code comments.
- **Write a plugin** — the in-process plugin system is the easiest place to add
  value without touching the core (see below).
- **Fix a bug or build a feature** — see the workflow below.

## Development setup

You need a stable Rust toolchain (install via [rustup](https://rustup.rs/)).

```bash
git clone https://github.com/ScottFelder/lazymqtt
cd lazymqtt
cargo run                  # debug build, launches the TUI
cargo build --release      # optimized build (LTO, stripped)
```

To exercise it you'll want an MQTT broker. A local [Mosquitto](https://mosquitto.org/)
(e.g. `docker run -it -p 1883:1883 eclipse-mosquitto`) with no auth on
`localhost:1883` is the simplest, and you can publish test traffic with
`mosquitto_pub`.

## Before you open a pull request

CI runs on every push and PR and must be green. Run the same checks locally
first — this is the whole gate:

```bash
cargo fmt --all            # format (rustfmt defaults)
cargo clippy --all-targets --all-features -- -D warnings   # lint; warnings fail CI
cargo test                 # unit tests must pass
```

- **Formatting**: rustfmt defaults, no exceptions.
- **Clippy**: the tree is warning-free and CI treats warnings as errors — keep
  it that way.
- **Tests**: there's a `#[cfg(test)]` suite (tree building, topic parsing,
  config round-trips, selection math, plugin behavior). Add tests in the
  relevant module when you add logic worth testing.

## Coding conventions

- **Match the surrounding code** — naming, comment density, and idioms. Read a
  neighboring module before adding a new one.
- **Respect the architecture boundaries.** The app is a single-threaded UI loop
  plus async MQTT tasks over channels. Keep that split clean:
  - `ui/` renders and **never mutates** state.
  - `events/` handles input and **never renders**.
  - `app/` owns the state and its behavior.
  - `mqtt.rs` is the only place that talks to the broker.
- **Prefer small, cohesive modules** — one concern per file, low coupling.
- `AGENTS.md` is the detailed map of the codebase (module layout, the plugin
  API, the theming model, the command-menu registry). Read it before a
  non-trivial change.

## Writing a plugin

Plugins are in-process and implement the `Plugin` trait (`src/plugin/mod.rs`);
all methods are defaulted, so you only implement what you need — observe the
message stream (`on_event`), annotate or re-render messages (`inspect`),
contribute commands to the `m` menu (`commands` / `invoke`), or own a pane
(`pane`). Built-ins live in `src/plugin/builtin/` and are good templates.
Plugins are opt-in (disabled by default) and toggled on the Plugins screen. See
the plugin section of `AGENTS.md` for the full API and the event/action flow.

## Commit & pull request process

1. Fork the repo and create a topic branch off `master`
   (`git checkout -b fix-topic-tree-crash`).
2. Make focused commits with clear messages (imperative mood, e.g.
   "Fix panic when a retained payload is empty").
3. If your change is user-visible, add a bullet under `## [Unreleased]` in
   `CHANGELOG.md`.
4. Make sure `fmt`, `clippy`, and `test` all pass locally.
5. Open a PR against `master` describing **what** changed and **why**. Link any
   related issue. Screenshots or a short clip help for UI changes.
6. CI must pass and a maintainer will review. Expect some back-and-forth — it's
   normal and appreciated.

Releases and versioning are handled by maintainers; see `RELEASING.md`.

## License

LazyMQTT is licensed under the [MIT License](LICENSE). By contributing, you
agree that your contributions are licensed under the same terms.
