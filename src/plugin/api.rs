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
    /// Open the publish form pre-filled (e.g. from a saved publish template) so
    /// the user can review/tweak before sending.
    OpenPublish {
        topic: String,
        payload: String,
        qos: u8,
        retain: bool,
    },
    /// Open the emitting plugin's pane (see `Plugin::pane`). Returned from a menu
    /// command; the app resolves which plugin from the invoke call site.
    OpenPane,
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
/// fresh each time the menu is (re)built, so it can reflect current state (e.g.
/// "Stop recording (12 msgs)" or "Replay speed: 2x"). `id` is the stable handle
/// passed back to `Plugin::invoke`/`Plugin::adjust`.
///
/// `adjustable` marks a command that cycles through several options in place:
/// the left/right (or `h`/`l`) keys call `Plugin::adjust` and the menu stays
/// open with a refreshed label, instead of Enter running a one-shot action.
#[derive(Debug, Clone)]
pub struct PluginCommand {
    /// Stable handle passed back to `invoke`/`adjust`. `String` (not `&'static
    /// str`) so a plugin can key commands on dynamic items — e.g. a template name.
    pub id: String,
    pub label: String,
    pub glyph: &'static str,
    pub adjustable: bool,
}

impl PluginCommand {
    /// A one-shot command (Enter runs it and closes the menu).
    pub fn action(id: impl Into<String>, glyph: &'static str, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            glyph,
            label: label.into(),
            adjustable: false,
        }
    }

    /// An option-cycling command (left/right or `h`/`l` cycle it in place).
    pub fn option(id: impl Into<String>, glyph: &'static str, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            glyph,
            label: label.into(),
            adjustable: true,
        }
    }
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

/// Semantic style for a piece of a plugin pane; the renderer maps it to a theme
/// color (kept UI-agnostic, like `InspectorStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneStyle {
    /// A section header / title.
    Header,
    /// A dim field label.
    Label,
    /// A normal value.
    Value,
    /// Accented value (e.g. a bar/gauge).
    Accent,
    /// Positive/healthy.
    Good,
    /// Cautionary.
    Warn,
    /// Problem/error.
    Bad,
    /// De-emphasized text.
    Muted,
}

/// One styled piece of a pane line.
#[derive(Debug, Clone)]
pub struct PaneSpan {
    pub text: String,
    pub style: PaneStyle,
}

impl PaneSpan {
    pub fn new(text: impl Into<String>, style: PaneStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// A whole-screen view a plugin owns and renders (e.g. traffic analytics). The
/// core draws it in the `PluginPane` screen, refreshed every frame so live
/// stats update. Plugins build it from their own state — they never touch the
/// terminal.
#[derive(Debug, Clone)]
pub struct PaneView {
    pub title: String,
    pub lines: Vec<Vec<PaneSpan>>,
}
