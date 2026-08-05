//! Screen + command enums and the command-menu registry.

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Screen {
    Connections,
    ConnectionForm,
    SubscriptionList,
    SubscriptionForm,
    Broker,
    Publish,
    Subscribe,
    ClearRetained,
    Plugins,
    AlertRules,
    AlertRuleForm,
    Schemas,
    SchemaForm,
    Recordings,
    RecordingEdit,
    Theme,
    PluginPane,
    CommandMenu,
    Help,
}

/// A broker-screen command, invocable by its shortcut key or from the command
/// menu (`m`). One source of truth for both — see `App::run_command`.
#[derive(Clone, Copy)]
pub enum Command {
    Subscribe,
    Unsubscribe,
    Publish,
    ClearRetained,
    ClearTopic,
    ClearTree,
    ExpandSubtree,
    CollapseSubtree,
    AlertRules,
    Schemas,
    Recordings,
    Theme,
    Plugins,
    Help,
    Disconnect,
    Quit,
}

/// The command menu's contents: (command, shortcut label, description).
pub const BROKER_COMMANDS: &[(Command, &str, &str)] = &[
    (Command::Subscribe, "s", "Subscribe to a topic"),
    (
        Command::Unsubscribe,
        "u",
        "Unsubscribe from the selected topic",
    ),
    (Command::Publish, "p", "Publish a message"),
    (
        Command::ClearRetained,
        "r",
        "Clear the retained message on the selected topic",
    ),
    (
        Command::ClearTopic,
        "x",
        "Clear the selected topic from the view",
    ),
    (Command::ClearTree, "c", "Clear the topic tree"),
    (
        Command::ExpandSubtree,
        "E",
        "Expand selected topic and children",
    ),
    (
        Command::CollapseSubtree,
        "C",
        "Collapse selected topic and children",
    ),
    (Command::AlertRules, "A", "Edit alert rules"),
    (Command::Schemas, "S", "Edit JSON Schemas"),
    (
        Command::Recordings,
        "R",
        "Recordings — replay, rename, delete",
    ),
    (Command::Theme, "T", "Theme — colors & presets"),
    (Command::Plugins, "P", "Manage plugins"),
    (Command::Help, "?", "Help"),
    (Command::Disconnect, "Esc", "Disconnect"),
    (Command::Quit, "^q", "Quit"),
];

/// What a command-menu row does when chosen.
#[derive(Clone)]
pub enum MenuAction {
    Core(Command),
    /// Open a plugin's submenu (its own commands live one level down).
    Submenu(usize),
    Plugin {
        plugin: usize,
        id: String,
    },
}

/// One row of the command menu (built fresh each open, and rebuilt after an
/// in-place adjust, so plugin labels reflect current state).
pub struct MenuItem {
    pub key: String,   // shortcut label or plugin glyph
    pub label: String, // what the command does
    pub note: String,  // dim suffix (plugin name), empty for core commands
    pub action: MenuAction,
    /// Cycles through options in place: left/right (or `h`/`l`) adjust it and
    /// the menu stays open, rather than Enter running a one-shot action.
    pub adjustable: bool,
}
