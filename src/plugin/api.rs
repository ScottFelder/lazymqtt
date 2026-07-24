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
#[derive(Debug, Clone)]
pub struct Annotation {
    pub plugin: &'static str,
    pub severity: Severity,
    pub text: String,
}

/// Something happened that plugins may want to observe. Variants own their data
/// so dispatching one never holds a borrow into `App`.
#[derive(Debug, Clone)]
pub enum PluginEvent {
    Connected,
    Disconnected(String),
    MessageReceived {
        id: u64,
        topic: String,
        payload: String,
        qos: u8,
        retained: bool,
    },
    Tick,
    Shutdown,
    // Defined for forward-compatibility but not yet dispatched (see the plugin
    // module docs): TopicSelected, MessageSelected, BeforePublish, AfterPublish,
    // SubscriptionChanged. They'll arrive with the inspector/command follow-ups.
}

/// The only ways a plugin can affect the running app. Applied by
/// `App::apply_plugin_actions`.
#[derive(Debug, Clone)]
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

/// Handed to a plugin once at load time. Minimal for now; grows as the API does.
#[derive(Debug, Clone)]
pub struct PluginContext {
    pub config_dir: PathBuf,
}
