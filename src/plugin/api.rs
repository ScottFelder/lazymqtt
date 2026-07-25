//! Data types exchanged across the plugin boundary.
//!
//! Kept deliberately UI-agnostic: events carry owned data (so dispatch never
//! borrows `App`), and actions are the only way a plugin affects the app.
//!
//! This is the plugin API contract, so it intentionally defines more surface
//! than the built-in demo exercises — extra severities, event fields, and
//! actions exist for plugins and hooks that don't ship yet. `dead_code` is
//! therefore allowed module-wide here.
#![allow(dead_code)]

use std::path::PathBuf;

/// How prominent / alarming an annotation is. The UI maps this to a color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Ok,
    Info,
    Warn,
    Error,
}

/// A note a plugin attaches to a specific message (validation result, decode
/// status, alert, …). Never mutates the original message.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub plugin: &'static str,
    pub severity: Severity,
    pub text: String,
}

/// Something happened that plugins may want to observe. Variants own their data
/// so dispatching one never holds a borrow into `App`.
#[derive(Debug, Clone)]
pub enum PluginEvent {
    /// Carries the active connection's id so plugins can load per-connection
    /// config (e.g. topic-alerts rules).
    Connected {
        connection: String,
    },
    Disconnected(String),
    MessageReceived {
        id: u64,
        topic: String,
        payload: String,
        qos: u8,
        retained: bool,
    },
    /// ~1 Hz — silence detection, periodic status.
    Tick,
    /// ~10 Hz — for time-sensitive plugins (e.g. replay pacing). Kept separate
    /// from `Tick` so per-second logic (alerts' silence counter) stays correct.
    FrameTick,
    Shutdown,
    // Defined for forward-compatibility but not yet dispatched (see the plugin
    // module docs): TopicSelected, MessageSelected, BeforePublish, AfterPublish,
    // SubscriptionChanged. They'll arrive with the inspector/command follow-ups.
}

/// The only ways a plugin can affect the running app. Applied by
/// `App::apply_plugin_actions`.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginAction {
    /// Attach a note to a message (by its stable id).
    Annotate {
        id: u64,
        annotation: Annotation,
    },
    /// Publish a message (record/replay, alerts, generators…).
    Publish {
        topic: String,
        payload: String,
        qos: u8,
        retain: bool,
    },
    Subscribe {
        topic: String,
        qos: u8,
    },
    Unsubscribe {
        topic: String,
    },
    /// Show a transient message in the status bar.
    Status(String),
}

/// Static description of a plugin, shown in the help/plugins listing.
#[derive(Debug, Clone, Copy)]
pub struct PluginMetadata {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
}

/// A command a plugin exposes in the `m` command menu. `label` is computed
/// fresh each time the menu opens, so it can reflect current state (e.g.
/// "Stop recording (12 msgs)"). `id` is the stable handle passed back to
/// `Plugin::invoke`.
#[derive(Debug, Clone)]
pub struct PluginCommand {
    pub id: &'static str,
    pub label: String,
    pub glyph: &'static str,
}

/// Handed to a plugin once at load time. Minimal for now; grows as the API does.
#[derive(Debug, Clone)]
pub struct PluginContext {
    pub config_dir: PathBuf,
}

/// The selected message handed to an inspector provider (`Plugin::inspect`).
#[derive(Debug, Clone)]
pub struct InspectMessage {
    pub topic: String,
    pub payload: String,
    pub qos: u8,
    pub retained: bool,
}

/// Semantic token kind for a styled inspector span; the renderer maps it to a
/// color (kept UI-agnostic here so plugins never name colors directly).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorStyle {
    /// Structural characters: braces, brackets, quotes, colons, commas, indent.
    Punctuation,
    /// An object key.
    Key,
    /// A string value.
    Str,
    /// A numeric value.
    Number,
    /// A literal: true / false / null.
    Literal,
    /// Uncategorized text.
    Plain,
}

/// One styled piece of an inspector line. Concatenating a line's `text`s
/// reproduces the plain rendering, so selection/yank stay intact.
#[derive(Debug, Clone)]
pub struct InspectorSpan {
    pub text: String,
    pub style: InspectorStyle,
}

impl InspectorSpan {
    pub fn new(text: impl Into<String>, style: InspectorStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// An alternative rendering of a payload supplied by a plugin (e.g. pretty
/// JSON). Each line is a sequence of styled spans; the core turns them into
/// Payload-pane rows, and the raw text view always stays available alongside.
#[derive(Debug, Clone)]
pub struct InspectorView {
    pub label: String,
    pub lines: Vec<Vec<InspectorSpan>>,
}
