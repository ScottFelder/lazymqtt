use crate::config::{Config, Connection};
use crate::mqtt::{Message, MqttCommand, MqttHandle};
use crate::plugin::{
    AlertCondition, AlertRule, AlertSeverity, Annotation, InspectMessage, InspectorStyle,
    InspectorView, PluginAction, PluginEvent, PluginHost, Recording, Severity,
};
use crate::theme::{Palette, Theme};
use crate::tree::TopicTree;
use std::collections::{HashMap, HashSet};

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Screen {
    Connections,
    ConnectionForm,
    Broker,
    Publish,
    Subscribe,
    ClearRetained,
    Plugins,
    AlertRules,
    AlertRuleForm,
    Recordings,
    RecordingEdit,
    Theme,
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
    AlertRules,
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
    (Command::AlertRules, "A", "Edit alert rules"),
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
#[derive(Clone, Copy)]
pub enum MenuAction {
    Core(Command),
    Plugin { plugin: usize, id: &'static str },
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Focus {
    Tree,
    Payload,
    History,
}

/// Which message sub-pane is collapsed to a title bar. At most one at a time;
/// the other then fills the remaining space.
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum PaneFold {
    None,
    Payload,
    History,
}

/// Semantic role of a piece of a detail line; the renderer maps it to a color.
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum DetailKind {
    Header,
    Meta,
    Payload,
    Toggle,
    Blank,
    /// A plugin annotation, colored by its severity.
    Annotation(Severity),
    /// A token in a plugin inspector view, colored by its syntax kind.
    Syntax(InspectorStyle),
}

/// One logical line of a selectable pane (Payload or History).
///
/// `segs` are the selectable pieces — their concatenation is the line's
/// yankable text, and the selection cursor addresses characters within it.
/// `lead` is a decorative, non-selectable prefix (an indent, or a ▶/▼ toggle)
/// so yanks stay clean. `msg`, when set, is the History message index the line
/// belongs to (used to keep the Payload pane and expand/collapse in sync).
pub struct DetailLine {
    pub lead: String,
    pub lead_kind: DetailKind,
    pub segs: Vec<(String, DetailKind)>,
    pub msg: Option<usize>,
}

impl DetailLine {
    pub fn plain(text: impl Into<String>, kind: DetailKind) -> Self {
        Self {
            lead: String::new(),
            lead_kind: DetailKind::Blank,
            segs: vec![(text.into(), kind)],
            msg: None,
        }
    }

    pub fn indented(indent: usize, text: impl Into<String>, kind: DetailKind) -> Self {
        Self {
            lead: " ".repeat(indent),
            lead_kind: DetailKind::Blank,
            segs: vec![(text.into(), kind)],
            msg: None,
        }
    }

    pub fn blank() -> Self {
        Self {
            lead: String::new(),
            lead_kind: DetailKind::Blank,
            segs: Vec::new(),
            msg: None,
        }
    }

    pub fn with_msg(mut self, m: usize) -> Self {
        self.msg = Some(m);
        self
    }

    /// The selectable text: concatenation of segment strings (no lead).
    pub fn text(&self) -> String {
        self.segs.iter().map(|(s, _)| s.as_str()).collect()
    }

    /// Number of selectable characters on the line.
    pub fn char_len(&self) -> usize {
        self.segs.iter().map(|(s, _)| s.chars().count()).sum()
    }

    /// Kind of the first segment (test-only convenience).
    #[cfg(test)]
    pub fn kind(&self) -> DetailKind {
        self.segs
            .first()
            .map(|(_, k)| *k)
            .unwrap_or(DetailKind::Blank)
    }
}

pub enum Status {
    Idle,
    Connecting,
    Connected,
    Disconnected(String),
}

/// Editable buffer backing the connection form.
#[derive(Default)]
pub struct FormBuffer {
    pub editing_index: Option<usize>,
    pub name: String,
    pub host: String,
    pub port: String,
    pub client_id: String,
    pub username: String,
    pub password: String,
    pub tls: bool,
    pub topics: String, // newline/comma separated
    pub field: usize,   // which field is focused
}

impl FormBuffer {
    pub const FIELD_COUNT: usize = 8;

    pub fn from(conn: &Connection, index: usize) -> Self {
        Self {
            editing_index: Some(index),
            name: conn.name.clone(),
            host: conn.host.clone(),
            port: conn.port.to_string(),
            client_id: conn.client_id.clone(),
            username: conn.username.clone(),
            password: conn.password.clone(),
            tls: conn.tls,
            topics: conn
                .subscriptions
                .iter()
                .map(|s| s.topic.clone())
                .collect::<Vec<_>>()
                .join(", "),
            field: 0,
        }
    }
}

pub struct PublishBuffer {
    pub topic: String,
    pub payload: String,
    pub qos: u8,
    pub retain: bool,
    pub field: usize,
}

impl Default for PublishBuffer {
    fn default() -> Self {
        Self {
            topic: String::new(),
            payload: String::new(),
            qos: 0,
            retain: false,
            field: 0,
        }
    }
}

/// Editable buffer backing the alert-rule form (one rule at a time).
#[derive(Default)]
pub struct AlertForm {
    pub editing_index: Option<usize>, // None = adding a new rule
    pub topic: String,
    pub when: usize,     // 0 above, 1 below, 2 changed, 3 silent
    pub value: String,   // above/below threshold
    pub seconds: String, // silent duration
    pub field: String,   // optional JSON field for above/below
    pub severity: usize, // 0 warn, 1 error
    pub focus: usize,    // focused field, 0..FIELD_COUNT
}

impl AlertForm {
    pub const FIELD_COUNT: usize = 6; // topic, when, value, seconds, field, severity
    pub const WHEN_LABELS: [&'static str; 4] = ["above", "below", "changed", "silent"];
    pub const SEVERITY_LABELS: [&'static str; 2] = ["warn", "error"];

    pub fn from_rule(index: usize, rule: &AlertRule) -> Self {
        let (when, value, seconds) = match rule.cond {
            AlertCondition::Above { value } => (0, value.to_string(), String::new()),
            AlertCondition::Below { value } => (1, value.to_string(), String::new()),
            AlertCondition::Changed => (2, String::new(), String::new()),
            AlertCondition::Silent { seconds } => (3, String::new(), seconds.to_string()),
        };
        Self {
            editing_index: Some(index),
            topic: rule.topic.clone(),
            when,
            value,
            seconds,
            field: rule.field.clone().unwrap_or_default(),
            severity: match rule.severity {
                AlertSeverity::Warn => 0,
                AlertSeverity::Error => 1,
            },
            focus: 0,
        }
    }

    /// Build a rule from the form, validating the inputs.
    pub fn to_rule(&self) -> Result<AlertRule, String> {
        let topic = self.topic.trim();
        if topic.is_empty() {
            return Err("Topic is required".into());
        }
        let cond = match self.when {
            0 | 1 => {
                let v: f64 = self
                    .value
                    .trim()
                    .parse()
                    .map_err(|_| "Value must be a number".to_string())?;
                if self.when == 0 {
                    AlertCondition::Above { value: v }
                } else {
                    AlertCondition::Below { value: v }
                }
            }
            2 => AlertCondition::Changed,
            _ => {
                let s: u64 = self
                    .seconds
                    .trim()
                    .parse()
                    .map_err(|_| "Seconds must be a whole number".to_string())?;
                AlertCondition::Silent { seconds: s }
            }
        };
        let field = {
            let f = self.field.trim();
            (!f.is_empty()).then(|| f.to_string())
        };
        Ok(AlertRule {
            topic: topic.to_string(),
            field,
            severity: if self.severity == 1 {
                AlertSeverity::Error
            } else {
                AlertSeverity::Warn
            },
            cond,
        })
    }
}

pub struct App {
    pub config: Config,
    pub screen: Screen,
    pub focus: Focus,
    pub collapsed: PaneFold, // which message sub-pane is collapsed, if any
    pub should_quit: bool,

    pub conn_selected: usize,
    pub form: FormBuffer,
    pub publish: PublishBuffer,

    pub active: Option<usize>, // index into config.connections
    pub handle: Option<MqttHandle>,
    pub status: Status,

    pub tree: TopicTree,
    pub expanded: HashSet<String>,
    pub tree_selected: usize,
    pub history: Vec<Message>, // recent messages, newest last

    // Plugin host + the data plugins produce. `annotations` maps a message id to
    // the notes plugins attached to it; `next_message_id` hands out the stable
    // monotonic ids that key both annotations and history state.
    pub plugins: PluginHost,
    pub annotations: HashMap<u64, Vec<Annotation>>,
    next_message_id: u64,

    // Which message (within the selected topic's history, newest-first) the
    // Payload pane is showing. 0 is always the latest message.
    pub history_selected: usize,

    // Messages currently expanded inline in the History pane, keyed by their
    // stable message id (collision-free, unlike the old millisecond timestamp).
    pub expanded_history: HashSet<u64>,

    pub sub_input: String,         // buffer for the subscribe prompt
    pub clear_topic: String,       // topic awaiting retained-message clear confirmation
    pub plugins_selected: usize,   // cursor in the Plugins management screen
    pub menu_selected: usize,      // cursor in the command menu
    pub menu_items: Vec<MenuItem>, // command-menu rows, rebuilt each open

    // Alert-rules editor (per connection). `alert_rules` is the working copy for
    // the connection identified by `alert_edit_conn` (name shown via
    // `alert_edit_name`); `alerts_selected` is the list cursor.
    pub alert_rules: Vec<AlertRule>,
    pub alerts_selected: usize,
    pub alert_form: AlertForm,
    pub alert_edit_conn: String,
    pub alert_edit_name: String,

    // Recordings picker (per connection). `recordings` lists the recordings for
    // the connection identified by `recordings_conn` (name shown via
    // `recordings_conn_name`); `recordings_selected` is the list cursor.
    // `recording_rename` is Some(buffer) while renaming the selected recording.
    pub recordings: Vec<Recording>,
    pub recordings_selected: usize,
    pub recordings_conn: String,
    pub recordings_conn_name: String,
    pub recording_rename: Option<String>,

    // Recording editor (multi-line text edit of one recording's JSONL, opened
    // from the picker with `e`). `rec_edit_lines` is the editable content — one
    // message per line — and `rec_edit_row`/`rec_edit_col` the char-addressed
    // cursor. `rec_edit_label` is the recording being edited (the Save target).
    // `rec_edit_saveas`, when Some, holds the "save as" filename buffer.
    pub rec_edit_lines: Vec<String>,
    pub rec_edit_row: usize,
    pub rec_edit_col: usize,
    pub rec_edit_label: String,
    pub rec_edit_error: Option<String>,
    pub rec_edit_saveas: Option<String>,

    // Theming. `theme` is the editable spec set; `palette` is its resolved
    // ratatui colors, which the renderer reads (recomputed on every change).
    // `theme_selected` is the editor cursor over [built-in presets ++ color
    // roles]; `theme_edit`, when Some, holds the spec being typed for the
    // selected role.
    pub theme: Theme,
    pub palette: Palette,
    pub theme_selected: usize,
    pub theme_edit: Option<String>,
    // Which Payload-pane view is active: 0 = raw text, 1.. = plugin inspector
    // views (in plugin order). Clamped when fewer views are available.
    pub payload_view: usize,
    pub error: Option<String>,

    // Keyboard text selection in the focused pane (Payload or History). Both
    // are (line, col) into that pane's `active_lines`. `sel_anchor` is set once
    // visual selection begins; while it is None the cursor just marks a position.
    pub sel_cursor: (usize, usize),
    pub sel_anchor: Option<(usize, usize)>,
}

impl App {
    pub fn new() -> Self {
        // Tests must not read the user's real theme file (mirrors the plugin
        // config isolation).
        let theme = if cfg!(test) {
            Theme::default()
        } else {
            Theme::load()
        };
        let palette = theme.palette();
        Self {
            config: Config::load(),
            theme,
            palette,
            theme_selected: 0,
            theme_edit: None,
            screen: Screen::Connections,
            focus: Focus::Tree,
            collapsed: PaneFold::None,
            should_quit: false,
            conn_selected: 0,
            form: FormBuffer::default(),
            publish: PublishBuffer::default(),
            active: None,
            handle: None,
            status: Status::Idle,
            tree: TopicTree::default(),
            expanded: HashSet::new(),
            tree_selected: 0,
            history: Vec::new(),
            plugins: PluginHost::with_builtins(),
            annotations: HashMap::new(),
            next_message_id: 0,
            history_selected: 0,
            expanded_history: HashSet::new(),
            sub_input: String::new(),
            clear_topic: String::new(),
            plugins_selected: 0,
            menu_selected: 0,
            menu_items: Vec::new(),
            alert_rules: Vec::new(),
            alerts_selected: 0,
            alert_form: AlertForm::default(),
            alert_edit_conn: String::new(),
            alert_edit_name: String::new(),
            recordings: Vec::new(),
            recordings_selected: 0,
            recordings_conn: String::new(),
            recordings_conn_name: String::new(),
            recording_rename: None,
            rec_edit_lines: Vec::new(),
            rec_edit_row: 0,
            rec_edit_col: 0,
            rec_edit_label: String::new(),
            rec_edit_error: None,
            rec_edit_saveas: None,
            payload_view: 0,
            error: None,
            sel_cursor: (0, 0),
            sel_anchor: None,
        }
    }

    pub fn active_conn(&self) -> Option<&Connection> {
        self.active.and_then(|i| self.config.connections.get(i))
    }

    pub fn send(&self, cmd: MqttCommand) {
        if let Some(h) = &self.handle {
            let _ = h.commands.send(cmd);
        }
    }

    pub fn push_message(&mut self, mut msg: Message) {
        self.next_message_id += 1;
        msg.id = self.next_message_id;

        // Build the plugin event from owned data so dispatch never borrows self.
        let event = PluginEvent::MessageReceived {
            id: msg.id,
            topic: msg.topic.clone(),
            payload: msg.payload.clone(),
            qos: msg.qos,
            retained: msg.retained,
        };

        self.tree.insert(msg.clone());
        self.history.push(msg);
        if self.history.len() > 5000 {
            self.history.drain(0..1000);
            // Drop annotations for messages that just fell out of history.
            if let Some(oldest) = self.history.first().map(|m| m.id) {
                self.annotations.retain(|&id, _| id >= oldest);
            }
        }

        let actions = self.plugins.dispatch(&event);
        self.apply_plugin_actions(actions);
    }

    /// Dispatch a plugin event and apply whatever actions come back. Used for
    /// the lifecycle/tick/shutdown hooks driven from the main loop.
    pub fn dispatch_plugin(&mut self, event: PluginEvent) {
        let actions = self.plugins.dispatch(&event);
        self.apply_plugin_actions(actions);
    }

    fn apply_plugin_actions(&mut self, actions: Vec<PluginAction>) {
        for action in actions {
            match action {
                PluginAction::Annotate { id, annotation } => {
                    self.annotations.entry(id).or_default().push(annotation);
                }
                PluginAction::Publish {
                    topic,
                    payload,
                    qos,
                    retain,
                } => {
                    self.send(MqttCommand::Publish {
                        topic,
                        payload,
                        qos,
                        retain,
                    });
                }
                PluginAction::Subscribe { topic, qos } => {
                    self.send(MqttCommand::Subscribe { topic, qos });
                }
                PluginAction::Unsubscribe { topic } => {
                    self.send(MqttCommand::Unsubscribe { topic });
                }
                PluginAction::Status(text) => self.error = Some(text),
            }
        }
    }

    /// Annotations attached to a given message id, in the order added.
    pub fn annotations_for(&self, id: u64) -> &[Annotation] {
        self.annotations
            .get(&id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// The path of the currently selected topic-tree row, if any.
    pub fn selected_topic(&self) -> Option<String> {
        let rows = self.tree.rows(&self.expanded);
        rows.get(self.tree_selected.min(rows.len().saturating_sub(1)))
            .map(|r| r.path.clone())
    }

    /// Build the command-menu rows: the core `BROKER_COMMANDS` followed by each
    /// enabled plugin's commands (labels computed fresh, so they reflect current
    /// state). Shared by `open_command_menu` and the in-place adjust refresh.
    fn build_menu_items(&self) -> Vec<MenuItem> {
        let mut items: Vec<MenuItem> = BROKER_COMMANDS
            .iter()
            .map(|(cmd, key, desc)| MenuItem {
                key: key.to_string(),
                label: desc.to_string(),
                note: String::new(),
                action: MenuAction::Core(*cmd),
                adjustable: false,
            })
            .collect();
        let metadata = self.plugins.metadata();
        for (plugin, cmd) in self.plugins.commands() {
            let note = metadata
                .get(plugin)
                .map(|m| m.name.to_string())
                .unwrap_or_default();
            items.push(MenuItem {
                key: cmd.glyph.to_string(),
                label: cmd.label,
                note,
                action: MenuAction::Plugin { plugin, id: cmd.id },
                adjustable: cmd.adjustable,
            });
        }
        items
    }

    /// Build the command menu (core commands + enabled plugins' commands) and
    /// open it.
    pub fn open_command_menu(&mut self) {
        self.menu_items = self.build_menu_items();
        self.menu_selected = 0;
        self.screen = Screen::CommandMenu;
    }

    /// Invoke a plugin command from the menu and apply its actions.
    pub fn invoke_plugin_command(&mut self, plugin: usize, id: &str) {
        let actions = self.plugins.invoke(plugin, id);
        self.apply_plugin_actions(actions);
    }

    /// Cycle the selected menu item's option in a direction (`forward` = right/
    /// `l`, else left/`h`), applying the plugin's actions and refreshing the
    /// menu labels in place. No-op for non-adjustable rows.
    pub fn adjust_selected_menu_item(&mut self, forward: bool) {
        let Some(item) = self.menu_items.get(self.menu_selected) else {
            return;
        };
        if !item.adjustable {
            return;
        }
        if let MenuAction::Plugin { plugin, id } = item.action {
            let actions = self.plugins.adjust(plugin, id, forward);
            self.apply_plugin_actions(actions);
            // Rebuild so the row's label reflects the new option, keeping the
            // cursor where it is.
            self.menu_items = self.build_menu_items();
            self.menu_selected = self
                .menu_selected
                .min(self.menu_items.len().saturating_sub(1));
        }
    }

    /// Run a broker command — the shared entry point for both its shortcut key
    /// and the command menu.
    pub fn run_command(&mut self, cmd: Command) {
        match cmd {
            Command::Subscribe => {
                self.sub_input.clear();
                self.screen = Screen::Subscribe;
            }
            Command::Unsubscribe => self.unsubscribe_selected(),
            Command::Publish => self.open_publish(),
            Command::ClearRetained => {
                if let Some(topic) = self.selected_topic() {
                    self.clear_topic = topic;
                    self.screen = Screen::ClearRetained;
                }
            }
            Command::ClearTopic => {
                if let Some(topic) = self.selected_topic() {
                    self.clear_topic_subtree(&topic);
                }
            }
            Command::ClearTree => {
                self.tree.clear();
                self.history.clear();
                self.expanded.clear();
                self.tree_selected = 0;
                self.reset_message_view();
            }
            Command::AlertRules => self.open_alerts_editor(),
            Command::Recordings => self.open_recordings(),
            Command::Theme => self.open_theme(),
            Command::Plugins => {
                self.plugins_selected = 0;
                self.screen = Screen::Plugins;
            }
            Command::Help => self.screen = Screen::Help,
            Command::Disconnect => {
                self.disconnect();
                self.screen = Screen::Connections;
            }
            Command::Quit => self.should_quit = true,
        }
    }

    /// Remove a topic and its subtree from the local view: the tree node, the
    /// history messages under it (with their annotations), and any saved
    /// expansion state. Purely local — it does not unsubscribe or touch the
    /// broker, so new messages on the topic repopulate it.
    fn clear_topic_subtree(&mut self, topic: &str) {
        if !self.tree.remove(topic) {
            return;
        }
        let prefix = format!("{topic}/");
        let under = |t: &str| t == topic || t.starts_with(prefix.as_str());
        for m in self.history.iter().filter(|m| under(&m.topic)) {
            self.annotations.remove(&m.id);
        }
        self.history.retain(|m| !under(&m.topic));
        self.expanded
            .retain(|p| !(p.as_str() == topic || p.starts_with(prefix.as_str())));
        let rows = self.tree.rows(&self.expanded);
        self.tree_selected = self.tree_selected.min(rows.len().saturating_sub(1));
        self.reset_message_view();
    }

    fn unsubscribe_selected(&mut self) {
        let Some(topic) = self.selected_topic() else {
            return;
        };
        self.send(MqttCommand::Unsubscribe {
            topic: topic.clone(),
        });
        if let Some(idx) = self.active {
            if let Some(c) = self.config.connections.get_mut(idx) {
                c.subscriptions.retain(|s| s.topic != topic);
                let _ = self.config.save();
            }
        }
        self.error = Some(format!("unsubscribed from {}", topic));
    }

    fn open_publish(&mut self) {
        self.publish = PublishBuffer::default();
        if let Some(topic) = self.selected_topic() {
            self.publish.topic = topic;
        }
        self.screen = Screen::Publish;
    }

    /// Open the alert-rules editor for the active connection (if connected) or
    /// the selected one otherwise. Alerts are per connection.
    pub fn open_alerts_editor(&mut self) {
        let target = if self.handle.is_some() {
            self.active_conn().map(|c| (c.id.clone(), c.name.clone()))
        } else {
            self.config
                .connections
                .get(self.conn_selected)
                .map(|c| (c.id.clone(), c.name.clone()))
        };
        let Some((id, name)) = target else {
            self.error = Some("no connection to edit alerts for".into());
            return;
        };
        self.alert_rules = crate::plugin::load_alert_rules(&id);
        self.alert_edit_conn = id;
        self.alert_edit_name = name;
        self.alerts_selected = 0;
        self.screen = Screen::AlertRules;
    }

    /// Open the recordings picker for the active connection (if connected) or the
    /// selected one otherwise. Recordings are per connection.
    pub fn open_recordings(&mut self) {
        let target = if self.handle.is_some() {
            self.active_conn().map(|c| (c.id.clone(), c.name.clone()))
        } else {
            self.config
                .connections
                .get(self.conn_selected)
                .map(|c| (c.id.clone(), c.name.clone()))
        };
        let Some((id, name)) = target else {
            self.error = Some("no connection to manage recordings for".into());
            return;
        };
        self.recordings_conn = id;
        self.recordings_conn_name = name;
        self.recording_rename = None;
        self.reload_recordings();
        self.recordings_selected = 0;
        self.screen = Screen::Recordings;
    }

    /// Re-read the recordings list for the picker's connection, keeping the
    /// cursor in range.
    pub fn reload_recordings(&mut self) {
        self.recordings = crate::plugin::list_recordings(&self.recordings_conn);
        self.recordings_selected = self
            .recordings_selected
            .min(self.recordings.len().saturating_sub(1));
    }

    /// The recording the picker cursor is on, if any.
    pub fn selected_recording(&self) -> Option<&Recording> {
        self.recordings.get(self.recordings_selected)
    }

    /// Replay the selected recording now (hands it to the recorder plugin), then
    /// return to the broker. Requires the recorder plugin to be enabled and a
    /// live connection.
    pub fn replay_selected_recording(&mut self) {
        let Some(label) = self.selected_recording().map(|r| r.label.clone()) else {
            return;
        };
        if !self.plugins.is_enabled(crate::plugin::RECORDER) {
            self.error = Some("enable the topic-recorder plugin to replay".into());
            return;
        }
        let actions = self.plugins.use_item(crate::plugin::RECORDER, &label);
        self.apply_plugin_actions(actions);
        self.screen = Screen::Broker;
    }

    /// Begin renaming the selected recording (seeds the buffer with its label).
    pub fn begin_recording_rename(&mut self) {
        if let Some(rec) = self.selected_recording() {
            self.recording_rename = Some(rec.label.clone());
        }
    }

    /// Apply the in-progress rename to the selected recording, then reload.
    pub fn apply_recording_rename(&mut self) {
        let Some(new_label) = self.recording_rename.take() else {
            return;
        };
        let Some(old_label) = self.selected_recording().map(|r| r.label.clone()) else {
            return;
        };
        if new_label.trim().is_empty() || new_label == old_label {
            self.reload_recordings();
            return;
        }
        match crate::plugin::rename_recording(&self.recordings_conn, &old_label, &new_label) {
            Ok(()) => self.error = Some(format!("renamed to {new_label}")),
            Err(e) => self.error = Some(format!("rename failed: {e}")),
        }
        self.reload_recordings();
    }

    /// Delete the selected recording, then reload.
    pub fn delete_selected_recording(&mut self) {
        let Some(rec) = self.selected_recording() else {
            return;
        };
        let (path, label) = (rec.path.clone(), rec.label.clone());
        match crate::plugin::delete_recording(&path) {
            Ok(()) => self.error = Some(format!("deleted {label}")),
            Err(e) => self.error = Some(format!("delete failed: {e}")),
        }
        self.reload_recordings();
    }

    // ---- Recording editor -------------------------------------------------

    /// Open the multi-line editor on the selected recording's contents.
    pub fn open_recording_edit(&mut self) {
        let Some(label) = self.selected_recording().map(|r| r.label.clone()) else {
            return;
        };
        let mut lines = crate::plugin::read_recording(&self.recordings_conn, &label);
        if lines.is_empty() {
            lines.push(String::new()); // always have a line to edit
        }
        self.rec_edit_lines = lines;
        self.rec_edit_row = 0;
        self.rec_edit_col = 0;
        self.rec_edit_label = label;
        self.rec_edit_error = None;
        self.rec_edit_saveas = None;
        self.screen = Screen::RecordingEdit;
    }

    /// Char length of an editor line (0 for an out-of-range row).
    fn rec_line_len(&self, row: usize) -> usize {
        self.rec_edit_lines
            .get(row)
            .map(|l| l.chars().count())
            .unwrap_or(0)
    }

    /// Insert a character at the cursor.
    pub fn rec_edit_insert(&mut self, c: char) {
        let (row, col) = (self.rec_edit_row, self.rec_edit_col);
        if let Some(line) = self.rec_edit_lines.get_mut(row) {
            line.insert(char_byte_idx(line, col), c);
            self.rec_edit_col += 1;
            self.rec_edit_error = None;
        }
    }

    /// Delete the character before the cursor, joining lines at column 0.
    pub fn rec_edit_backspace(&mut self) {
        if self.rec_edit_col > 0 {
            let (row, col) = (self.rec_edit_row, self.rec_edit_col);
            if let Some(line) = self.rec_edit_lines.get_mut(row) {
                let start = char_byte_idx(line, col - 1);
                let end = char_byte_idx(line, col);
                line.replace_range(start..end, "");
                self.rec_edit_col -= 1;
            }
        } else if self.rec_edit_row > 0 {
            let cur = self.rec_edit_lines.remove(self.rec_edit_row);
            self.rec_edit_row -= 1;
            self.rec_edit_col = self.rec_line_len(self.rec_edit_row);
            self.rec_edit_lines[self.rec_edit_row].push_str(&cur);
        }
        self.rec_edit_error = None;
    }

    /// Split the current line at the cursor into two lines.
    pub fn rec_edit_newline(&mut self) {
        let (row, col) = (self.rec_edit_row, self.rec_edit_col);
        let line = self.rec_edit_lines.get(row).cloned().unwrap_or_default();
        let at = char_byte_idx(&line, col);
        let (left, right) = line.split_at(at);
        self.rec_edit_lines[row] = left.to_string();
        self.rec_edit_lines.insert(row + 1, right.to_string());
        self.rec_edit_row += 1;
        self.rec_edit_col = 0;
        self.rec_edit_error = None;
    }

    /// Move the cursor horizontally (wrapping across line ends).
    pub fn rec_edit_move_h(&mut self, forward: bool) {
        if forward {
            if self.rec_edit_col < self.rec_line_len(self.rec_edit_row) {
                self.rec_edit_col += 1;
            } else if self.rec_edit_row + 1 < self.rec_edit_lines.len() {
                self.rec_edit_row += 1;
                self.rec_edit_col = 0;
            }
        } else if self.rec_edit_col > 0 {
            self.rec_edit_col -= 1;
        } else if self.rec_edit_row > 0 {
            self.rec_edit_row -= 1;
            self.rec_edit_col = self.rec_line_len(self.rec_edit_row);
        }
    }

    /// Move the cursor vertically, clamping the column to the new line.
    pub fn rec_edit_move_v(&mut self, down: bool) {
        if down {
            if self.rec_edit_row + 1 < self.rec_edit_lines.len() {
                self.rec_edit_row += 1;
            }
        } else {
            self.rec_edit_row = self.rec_edit_row.saturating_sub(1);
        }
        self.rec_edit_col = self.rec_edit_col.min(self.rec_line_len(self.rec_edit_row));
    }

    pub fn rec_edit_home(&mut self) {
        self.rec_edit_col = 0;
    }

    pub fn rec_edit_end(&mut self) {
        self.rec_edit_col = self.rec_line_len(self.rec_edit_row);
    }

    /// Insert pasted text at the cursor, honoring embedded newlines.
    pub fn rec_edit_paste(&mut self, data: &str) {
        for (i, part) in data.split('\n').enumerate() {
            if i > 0 {
                self.rec_edit_newline();
            }
            for c in part.chars().filter(|c| *c != '\r') {
                self.rec_edit_insert(c);
            }
        }
    }

    /// Save the editor's contents to `label`. Validates the JSONL first; on
    /// failure keeps the editor open with the error shown, otherwise reloads the
    /// picker and returns to it.
    pub fn save_recording_edit(&mut self, label: &str) {
        match crate::plugin::save_recording_text(&self.recordings_conn, label, &self.rec_edit_lines)
        {
            Ok(()) => {
                self.error = Some(format!("saved {label}"));
                self.rec_edit_label = label.to_string();
                self.reload_recordings();
                self.screen = Screen::Recordings;
            }
            Err(e) => self.rec_edit_error = Some(e),
        }
    }

    /// Save to the recording currently being edited (overwrite).
    pub fn save_recording_edit_current(&mut self) {
        self.save_recording_edit(&self.rec_edit_label.clone());
    }

    /// Enter "save as" mode, seeding the filename with an `-edited` suffix.
    pub fn begin_recording_saveas(&mut self) {
        self.rec_edit_saveas = Some(format!("{}-edited", self.rec_edit_label));
    }

    /// Commit the "save as": write the editor contents to the new label.
    pub fn commit_recording_saveas(&mut self) {
        let Some(label) = self.rec_edit_saveas.take() else {
            return;
        };
        if label.trim().is_empty() {
            self.rec_edit_error = Some("name required".into());
            return;
        }
        self.save_recording_edit(&label);
    }

    // ---- Theme editor ------------------------------------------------------

    /// Number of built-in presets shown above the color roles in the editor.
    pub fn theme_builtins_len(&self) -> usize {
        crate::theme::builtins().len()
    }

    /// Total editor rows: presets followed by the color roles.
    pub fn theme_row_count(&self) -> usize {
        self.theme_builtins_len() + crate::theme::ROLE_COUNT
    }

    /// If the selected row is a color role, its 0-based role index.
    pub fn theme_selected_role(&self) -> Option<usize> {
        self.theme_selected
            .checked_sub(self.theme_builtins_len())
            .filter(|i| *i < crate::theme::ROLE_COUNT)
    }

    pub fn open_theme(&mut self) {
        self.theme_selected = 0;
        self.theme_edit = None;
        self.screen = Screen::Theme;
    }

    /// Recompute the render palette after any change to `theme`.
    fn refresh_palette(&mut self) {
        self.palette = self.theme.palette();
    }

    /// Apply the built-in preset at `index` as the working theme (live preview).
    pub fn apply_theme_builtin(&mut self, index: usize) {
        if let Some((name, theme)) = crate::theme::builtins().into_iter().nth(index) {
            self.theme = theme;
            self.refresh_palette();
            self.error = Some(format!("applied {name} theme (T then s to save)"));
        }
    }

    /// Begin editing the selected color role, seeding the buffer with its spec.
    pub fn begin_theme_edit(&mut self) {
        if let Some(role) = self.theme_selected_role() {
            self.theme_edit = Some(self.theme.spec(role).to_string());
        }
    }

    /// Commit the in-progress color edit to the selected role (live preview).
    pub fn apply_theme_edit(&mut self) {
        let Some(spec) = self.theme_edit.take() else {
            return;
        };
        if let Some(role) = self.theme_selected_role() {
            self.theme.set_spec(role, spec.trim().to_string());
            self.refresh_palette();
        }
    }

    /// Persist the working theme to `theme.json`.
    pub fn save_theme(&mut self) {
        match self.theme.save() {
            Ok(()) => self.error = Some("theme saved".into()),
            Err(e) => self.error = Some(format!("theme save failed: {e}")),
        }
    }

    /// Persist the working alert rules for the edited connection, and — if that
    /// connection is the live one — reload them into the plugin immediately.
    pub fn persist_alert_rules(&mut self) {
        if let Err(e) = crate::plugin::save_alert_rules(&self.alert_edit_conn, &self.alert_rules) {
            self.error = Some(format!("save failed: {}", e));
            return;
        }
        let is_active =
            self.active_conn().map(|c| c.id.as_str()) == Some(self.alert_edit_conn.as_str());
        if is_active {
            let id = self.alert_edit_conn.clone();
            self.dispatch_plugin(PluginEvent::Connected { connection: id });
        }
    }

    pub fn disconnect(&mut self) {
        self.send(MqttCommand::Disconnect);
        self.handle = None;
        self.active = None;
        self.status = Status::Idle;
        self.tree.clear();
        self.history.clear();
        self.expanded.clear();
        self.tree_selected = 0;
        self.annotations.clear();
        self.reset_message_view();
    }

    pub fn reset_selection(&mut self) {
        self.sel_cursor = (0, 0);
        self.sel_anchor = None;
    }

    /// Call whenever the selected topic changes: jumps the History pane back
    /// to the latest message, collapses any expanded entries, and clears any
    /// in-progress Payload selection.
    pub fn reset_message_view(&mut self) {
        self.history_selected = 0;
        self.expanded_history.clear();
        self.payload_view = 0;
        self.reset_selection();
    }

    /// Key used to track a message's inline-expanded state in the History pane.
    /// The stable message id (not the timestamp) so it can't collide.
    pub fn history_key(msg: &Message) -> u64 {
        msg.id
    }

    /// Whether the given message is currently expanded in the History pane.
    pub fn is_history_expanded(&self, msg: &Message) -> bool {
        self.expanded_history.contains(&Self::history_key(msg))
    }

    /// All messages received for the currently selected tree topic, newest first.
    pub fn topic_messages(&self) -> Vec<&Message> {
        let rows = self.tree.rows(&self.expanded);
        let Some(row) = rows.get(self.tree_selected.min(rows.len().saturating_sub(1))) else {
            return Vec::new();
        };
        self.history
            .iter()
            .rev()
            .filter(|m| m.topic == row.path)
            .collect()
    }

    /// The message currently shown in the Payload pane: whichever entry is
    /// selected in the History pane (0 = latest).
    pub fn selected_message(&self) -> Option<&Message> {
        let msgs = self.topic_messages();
        if msgs.is_empty() {
            return None;
        }
        msgs.into_iter().nth(self.history_selected)
    }

    /// The Payload pane contents for the currently selected message, as plain
    /// logical lines. Shared by the renderer and the selection/yank logic so
    /// both agree on line and column positions.
    pub fn payload_lines(&self) -> Vec<DetailLine> {
        let rows = self.tree.rows(&self.expanded);
        let mut out = Vec::new();

        let Some(row) = rows.get(self.tree_selected.min(rows.len().saturating_sub(1))) else {
            out.push(DetailLine::plain(
                "Waiting for messages…",
                DetailKind::Blank,
            ));
            return out;
        };

        out.push(DetailLine::plain(row.path.clone(), DetailKind::Header));
        out.push(DetailLine::blank());

        match self.selected_message() {
            None => out.push(DetailLine::plain(
                "(no messages on this exact topic — it may be a parent node)",
                DetailKind::Blank,
            )),
            Some(m) => {
                let retain = if m.retained { " R" } else { "" };
                out.push(DetailLine::plain(
                    format!(
                        "{}  q{}{}",
                        m.time.format("%m/%d/%Y %H:%M:%S%.3f"),
                        m.qos,
                        retain
                    ),
                    DetailKind::Meta,
                ));
                for a in self.annotations_for(m.id) {
                    out.push(DetailLine::plain(
                        format!("{} {}: {}", severity_glyph(a.severity), a.plugin, a.text),
                        DetailKind::Annotation(a.severity),
                    ));
                }
                out.push(DetailLine::blank());

                // Body: the raw payload (view 0) or a plugin inspector view.
                let views = self.current_inspector_views();
                match self.payload_view.min(views.len()).checked_sub(1) {
                    None => {
                        for line in m.payload.lines() {
                            out.push(DetailLine::indented(2, line, DetailKind::Payload));
                        }
                    }
                    Some(i) => {
                        for line in &views[i].lines {
                            let segs = line
                                .iter()
                                .map(|sp| (sp.text.clone(), DetailKind::Syntax(sp.style)))
                                .collect();
                            out.push(DetailLine {
                                lead: "  ".to_string(),
                                lead_kind: DetailKind::Blank,
                                segs,
                                msg: None,
                            });
                        }
                    }
                }
            }
        }
        out
    }

    /// Inspector views offered by enabled plugins for the selected message.
    fn current_inspector_views(&self) -> Vec<InspectorView> {
        match self.selected_message() {
            Some(m) => self.plugins.inspect(&InspectMessage {
                topic: m.topic.clone(),
                payload: m.payload.clone(),
                qos: m.qos,
                retained: m.retained,
            }),
            None => Vec::new(),
        }
    }

    /// Cycle the Payload pane through raw text and each available inspector
    /// view. The line set changes, so any in-progress selection is reset.
    pub fn cycle_payload_view(&mut self) {
        let count = self.current_inspector_views().len() + 1; // +1 for raw
        self.payload_view = (self.payload_view + 1) % count;
        self.reset_selection();
    }

    /// Label of the active Payload view, or `None` when only the raw view
    /// exists (nothing to switch to).
    pub fn payload_view_label(&self) -> Option<String> {
        let views = self.current_inspector_views();
        if views.is_empty() {
            return None;
        }
        Some(match self.payload_view.min(views.len()).checked_sub(1) {
            None => "raw".to_string(),
            Some(i) => views[i].label.clone(),
        })
    }

    /// The History pane contents: every message for the selected topic, newest
    /// first, honoring per-message expand/collapse. Each line records its source
    /// message index (`msg`) so cursor movement can keep the Payload pane in sync
    /// and Enter can expand/collapse the entry under the cursor. Shared by the
    /// renderer and the selection/yank logic.
    pub fn history_lines(&self) -> Vec<DetailLine> {
        let msgs = self.topic_messages();
        let mut out = Vec::new();
        if msgs.is_empty() {
            out.push(DetailLine::plain(
                "No messages on this exact topic yet.",
                DetailKind::Blank,
            ));
            return out;
        }

        for (i, m) in msgs.iter().enumerate() {
            let retain = if m.retained { " R" } else { "" };
            let expanded = self.is_history_expanded(m);
            let arrow = if expanded { "▼ " } else { "▶ " };
            let marker = annotation_marker(self.annotations_for(m.id));

            if expanded {
                let meta = format!(
                    "{}  q{}{}",
                    m.time.format("%m/%d/%Y %H:%M:%S%.3f"),
                    m.qos,
                    retain
                );
                let mut segs = vec![(meta, DetailKind::Meta)];
                segs.extend(marker);
                out.push(DetailLine {
                    lead: arrow.into(),
                    lead_kind: DetailKind::Toggle,
                    segs,
                    msg: Some(i),
                });
                for line in m.payload.lines() {
                    out.push(DetailLine::indented(4, line, DetailKind::Payload).with_msg(i));
                }
                out.push(DetailLine::blank().with_msg(i));
            } else {
                let meta = format!("{}  q{}{}", m.time.format("%H:%M:%S%.3f"), m.qos, retain);
                let preview: String = m
                    .payload
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(40)
                    .collect();
                let mut segs = vec![
                    (meta, DetailKind::Meta),
                    (format!("  {}", preview), DetailKind::Payload),
                ];
                segs.extend(marker);
                out.push(DetailLine {
                    lead: arrow.into(),
                    lead_kind: DetailKind::Toggle,
                    segs,
                    msg: Some(i),
                });
            }
        }
        out
    }

    /// Lines the selection cursor currently operates on, per focused pane.
    fn active_lines(&self) -> Vec<DetailLine> {
        match self.focus {
            Focus::History => self.history_lines(),
            _ => self.payload_lines(),
        }
    }

    /// First History line index belonging to the given message (0 if none).
    fn history_header_line(&self, msg_idx: usize) -> usize {
        self.history_lines()
            .iter()
            .position(|l| l.msg == Some(msg_idx))
            .unwrap_or(0)
    }

    /// Focus the History pane, parking the cursor on the currently selected
    /// message so the Payload pane keeps showing it.
    pub fn focus_history(&mut self) {
        self.focus = Focus::History;
        self.reset_selection();
        self.sel_cursor = (self.history_header_line(self.history_selected), 0);
    }

    /// Collapse/expand the focused message sub-pane. Collapsing one implicitly
    /// expands the other (the fold state holds at most one collapsed pane), and
    /// toggling the already-collapsed pane restores the even split. Does nothing
    /// when the Topics pane is focused.
    pub fn toggle_pane_fold(&mut self) {
        let target = match self.focus {
            Focus::Payload => PaneFold::Payload,
            Focus::History => PaneFold::History,
            Focus::Tree => return,
        };
        self.collapsed = if self.collapsed == target {
            PaneFold::None
        } else {
            target
        };
    }

    /// Keep the Payload pane (and `history_selected`) pointed at whichever
    /// message the History cursor is currently on.
    pub fn sync_history_selected(&mut self) {
        if let Some(m) = self
            .history_lines()
            .get(self.sel_cursor.0)
            .and_then(|l| l.msg)
        {
            self.history_selected = m;
        }
    }

    /// Toggle expand/collapse of the History message under the cursor, then park
    /// the cursor on that message's header (its line count just changed).
    pub fn toggle_history_at_cursor(&mut self) {
        let Some(idx) = self
            .history_lines()
            .get(self.sel_cursor.0)
            .and_then(|l| l.msg)
        else {
            return;
        };
        if let Some(key) = self.topic_messages().get(idx).map(|m| Self::history_key(m)) {
            if !self.expanded_history.remove(&key) {
                self.expanded_history.insert(key);
            }
        }
        self.history_selected = idx;
        self.sel_cursor = (self.history_header_line(idx), 0);
        self.sel_anchor = None;
    }

    /// Clamp a column to the last character index of the given line (0 when empty).
    fn max_col(lines: &[DetailLine], line: usize) -> usize {
        lines
            .get(line)
            .map(|l| l.char_len())
            .unwrap_or(0)
            .saturating_sub(1)
    }

    pub fn sel_move_col(&mut self, delta: isize) {
        let lines = self.active_lines();
        if lines.is_empty() {
            return;
        }
        let (l, c) = self.sel_cursor;
        let l = l.min(lines.len() - 1);
        let max = Self::max_col(&lines, l);
        let c = if delta < 0 {
            c.saturating_sub(1)
        } else {
            (c + 1).min(max)
        };
        self.sel_cursor = (l, c.min(max));
    }

    pub fn sel_move_line(&mut self, delta: isize) {
        let lines = self.active_lines();
        if lines.is_empty() {
            return;
        }
        let (l, c) = self.sel_cursor;
        let l = if delta < 0 {
            l.saturating_sub(1)
        } else {
            (l + 1).min(lines.len() - 1)
        };
        self.sel_cursor = (l, c.min(Self::max_col(&lines, l)));
    }

    pub fn sel_toggle_anchor(&mut self) {
        self.sel_anchor = if self.sel_anchor.is_some() {
            None
        } else {
            Some(self.sel_cursor)
        };
    }

    pub fn sel_yank(&mut self) {
        let lines = self.active_lines();
        if lines.is_empty() {
            return;
        }
        let text = match self.sel_anchor {
            Some(a) => {
                let b = self.sel_cursor;
                let (s, e) = if a <= b { (a, b) } else { (b, a) };
                selection_text(&lines, s, e)
            }
            None => lines[self.sel_cursor.0.min(lines.len() - 1)].text(),
        };
        if text.is_empty() {
            self.error = Some("nothing to copy".into());
            return;
        }
        match crate::clipboard::copy(&text) {
            Ok(()) => {
                let n = text.chars().count();
                self.error = Some(format!(
                    "copied {} char{} to clipboard",
                    n,
                    if n == 1 { "" } else { "s" }
                ));
                self.sel_anchor = None;
            }
            Err(e) => self.error = Some(format!("copy failed: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mqtt::Message;
    use chrono::Local;

    fn msg(topic: &str, payload: &str) -> Message {
        Message {
            id: 0, // push_message assigns the real id
            topic: topic.into(),
            payload: payload.into(),
            qos: 0,
            retained: false,
            time: Local::now(),
        }
    }

    #[test]
    fn selection_text_single_line_inclusive() {
        let lines = vec![DetailLine::plain("hello", DetailKind::Payload)];
        assert_eq!(selection_text(&lines, (0, 0), (0, 4)), "hello");
        assert_eq!(selection_text(&lines, (0, 1), (0, 3)), "ell");
    }

    #[test]
    fn full_key_flow_selects_last_char() {
        use crate::app::{Focus, Screen};
        use crate::events::handle_key;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut app = App::new();
        app.tree.insert(msg("t", "hello"));
        app.history.push(msg("t", "hello"));
        app.tree_selected = 0;
        app.screen = Screen::Broker;
        app.focus = Focus::Payload;

        let payload_line = app
            .payload_lines()
            .iter()
            .position(|l| l.kind() == DetailKind::Payload)
            .unwrap();
        for _ in 0..payload_line {
            handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        assert_eq!(
            app.sel_cursor.0, payload_line,
            "cursor should be on the payload line"
        );

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
        );
        for _ in 0..10 {
            handle_key(&mut app, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        }

        let (a, b) = (app.sel_anchor.unwrap(), app.sel_cursor);
        let (s, e) = if a <= b { (a, b) } else { (b, a) };
        assert_eq!(selection_text(&app.payload_lines(), s, e), "hello");
    }

    #[test]
    fn yank_includes_last_char_under_cursor() {
        let mut app = App::new();
        app.tree.insert(msg("t", "hello"));
        app.history.push(msg("t", "hello"));
        app.tree_selected = 0;

        let lines = app.payload_lines();
        let payload_line = lines
            .iter()
            .position(|l| l.kind() == DetailKind::Payload)
            .unwrap();
        app.sel_cursor = (payload_line, 0);
        app.sel_toggle_anchor();
        for _ in 0..10 {
            app.sel_move_col(1);
        }
        assert_eq!(app.sel_cursor.1, 4, "cursor should rest on last char index");

        let (a, b) = (app.sel_anchor.unwrap(), app.sel_cursor);
        let (s, e) = if a <= b { (a, b) } else { (b, a) };
        assert_eq!(selection_text(&app.payload_lines(), s, e), "hello");
    }

    #[test]
    fn history_yank_selects_from_active_pane() {
        use crate::app::Focus;

        let mut app = App::new();
        app.tree.insert(msg("t", "world"));
        app.history.push(msg("t", "world"));
        app.tree_selected = 0;

        // Focus History and expand the entry so its payload is on its own line.
        app.focus_history();
        app.toggle_history_at_cursor();

        // The selection cursor now operates on the History lines.
        let lines = app.history_lines();
        let payload_line = lines
            .iter()
            .position(|l| l.kind() == DetailKind::Payload)
            .expect("expanded history should expose a payload line");
        assert_eq!(app.focus, Focus::History);

        app.sel_cursor = (payload_line, 0);
        app.sel_toggle_anchor();
        for _ in 0..10 {
            app.sel_move_col(1);
        }

        let (a, b) = (app.sel_anchor.unwrap(), app.sel_cursor);
        let (s, e) = if a <= b { (a, b) } else { (b, a) };
        assert_eq!(selection_text(&app.history_lines(), s, e), "world");
    }

    #[test]
    fn push_message_assigns_ids_and_plugin_annotates() {
        let mut app = App::new();

        app.push_message(msg("t", r#"{"a":1}"#));
        app.push_message(msg("t", "plain text"));

        // Stable, monotonic ids assigned in order.
        assert_eq!(app.history[0].id, 1);
        assert_eq!(app.history[1].id, 2);

        // The built-in json-marker annotated each by parseability.
        let first = app.annotations_for(1);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].severity, Severity::Ok);

        let second = app.annotations_for(2);
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].severity, Severity::Info);
    }

    #[test]
    fn annotations_pruned_when_history_trims() {
        let mut app = App::new();
        for _ in 0..5001 {
            app.push_message(msg("t", "hello")); // each gets an Info annotation
        }
        // The trim dropped the oldest 1000 (ids 1..=1000); their annotations go too.
        assert!(app.annotations_for(1).is_empty());
        assert_eq!(app.history.first().unwrap().id, 1001);
        assert!(!app.annotations_for(1001).is_empty());
    }

    #[test]
    fn clear_topic_removes_only_selected_subtree() {
        let mut app = App::new();
        app.push_message(msg("sensor/temp", "1"));
        app.push_message(msg("sensor/humidity", "2"));
        app.push_message(msg("status", "up"));
        app.expanded.insert("sensor".to_string());

        // Select the "sensor" parent row (rows are sorted: sensor, sensor/humidity,
        // sensor/temp, status) and clear it.
        app.tree_selected = 0;
        assert_eq!(app.selected_topic().as_deref(), Some("sensor"));
        app.run_command(Command::ClearTopic);

        // The whole sensor subtree is gone from the tree, its history, its
        // annotations, and its saved expansion — but "status" survives.
        let rows: Vec<String> = app
            .tree
            .rows(&app.expanded)
            .into_iter()
            .map(|r| r.path)
            .collect();
        assert_eq!(rows, vec!["status".to_string()]);
        assert!(app.history.iter().all(|m| m.topic == "status"));
        assert!(!app.expanded.contains("sensor"));
        assert!(app.annotations_for(1).is_empty()); // sensor/temp's annotation
        assert!(app.annotations_for(2).is_empty()); // sensor/humidity's annotation
        assert!(!app.annotations_for(3).is_empty()); // status kept its annotation
    }

    #[test]
    fn char_byte_idx_handles_unicode() {
        assert_eq!(char_byte_idx("aé", 0), 0);
        assert_eq!(char_byte_idx("aé", 1), 1);
        assert_eq!(char_byte_idx("aé", 2), 3); // é is two bytes
        assert_eq!(char_byte_idx("aé", 9), 3); // past the end clamps to len
    }

    #[test]
    fn recording_editor_insert_split_and_join() {
        let mut app = App::new();
        app.rec_edit_lines = vec!["ab".to_string()];
        app.rec_edit_row = 0;
        app.rec_edit_col = 1; // between 'a' and 'b'

        app.rec_edit_insert('X');
        assert_eq!(app.rec_edit_lines, vec!["aXb".to_string()]);
        assert_eq!(app.rec_edit_col, 2);

        // Enter splits the line at the cursor.
        app.rec_edit_newline();
        assert_eq!(app.rec_edit_lines, vec!["aX".to_string(), "b".to_string()]);
        assert_eq!((app.rec_edit_row, app.rec_edit_col), (1, 0));

        // Backspace at column 0 joins with the previous line.
        app.rec_edit_backspace();
        assert_eq!(app.rec_edit_lines, vec!["aXb".to_string()]);
        assert_eq!((app.rec_edit_row, app.rec_edit_col), (0, 2));
    }

    #[test]
    fn plugin_host_dispatch_collects_actions() {
        use crate::plugin::{PluginAction, PluginEvent, PluginHost};

        let mut host = PluginHost::with_builtins();
        let actions = host.dispatch(&PluginEvent::MessageReceived {
            id: 7,
            topic: "t".into(),
            payload: r#"{"ok":true}"#.into(),
            qos: 0,
            retained: false,
        });
        assert!(actions
            .iter()
            .any(|a| matches!(a, PluginAction::Annotate { id: 7, .. })));
    }

    #[test]
    fn payload_view_cycles_between_raw_and_json() {
        let mut app = App::new();
        app.push_message(msg("data", r#"{"a":1}"#)); // slash-free topic => row is the leaf
        app.tree_selected = 0;

        // Raw by default; a JSON view is available (label present).
        assert_eq!(app.payload_view_label().as_deref(), Some("raw"));

        // Cycle to the JSON view: payload_lines now shows pretty-printed JSON.
        app.cycle_payload_view();
        assert_eq!(app.payload_view_label().as_deref(), Some("JSON"));
        let body: String = app
            .payload_lines()
            .iter()
            .map(|l| l.text())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            body.contains("\"a\": 1"),
            "expected pretty JSON, got: {body}"
        );

        // Cycling wraps back to raw.
        app.cycle_payload_view();
        assert_eq!(app.payload_view_label().as_deref(), Some("raw"));
    }

    #[test]
    fn non_json_payload_has_no_alternate_view() {
        let mut app = App::new();
        app.push_message(msg("data", "plain text"));
        app.tree_selected = 0;

        assert_eq!(app.payload_view_label(), None);
        app.cycle_payload_view(); // no-op: only the raw view exists
        assert_eq!(app.payload_view, 0);
    }

    #[test]
    fn alert_form_builds_valid_rules() {
        let mut form = AlertForm {
            topic: "f/temp".into(),
            when: 0, // above
            value: "80".into(),
            severity: 1, // error
            ..Default::default()
        };
        let rule = form.to_rule().expect("valid above rule");
        assert_eq!(rule.topic, "f/temp");
        assert!(matches!(rule.cond, AlertCondition::Above { value } if value == 80.0));
        assert_eq!(rule.severity, AlertSeverity::Error);

        form.when = 3; // silent
        form.seconds = "30".into();
        assert!(matches!(
            form.to_rule().unwrap().cond,
            AlertCondition::Silent { seconds: 30 }
        ));
    }

    #[test]
    fn alert_form_rejects_bad_input() {
        // Empty topic.
        assert!(AlertForm {
            topic: "".into(),
            when: 2,
            ..Default::default()
        }
        .to_rule()
        .is_err());
        // Non-numeric threshold.
        assert!(AlertForm {
            topic: "t".into(),
            when: 0,
            value: "hot".into(),
            ..Default::default()
        }
        .to_rule()
        .is_err());
        // Non-integer seconds.
        assert!(AlertForm {
            topic: "t".into(),
            when: 3,
            seconds: "1.5".into(),
            ..Default::default()
        }
        .to_rule()
        .is_err());
    }

    #[test]
    fn alert_form_from_rule_round_trips() {
        let rule = AlertRule {
            topic: "sensors/#".into(),
            field: Some("temp".into()),
            severity: AlertSeverity::Error,
            cond: AlertCondition::Below { value: 0.0 },
        };
        let form = AlertForm::from_rule(2, &rule);
        assert_eq!(form.editing_index, Some(2));
        assert_eq!(form.when, 1); // below
        assert_eq!(form.value, "0");
        assert_eq!(form.field, "temp");
        assert_eq!(form.severity, 1); // error
        assert_eq!(form.to_rule().unwrap(), rule);
    }
}

/// Glyph shown for an annotation severity (color is applied by the renderer).
pub fn severity_glyph(severity: Severity) -> &'static str {
    match severity {
        Severity::Ok => "✓",
        Severity::Info => "•",
        Severity::Warn => "⚠",
        Severity::Error => "✗",
    }
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Ok => 0,
        Severity::Info => 1,
        Severity::Warn => 2,
        Severity::Error => 3,
    }
}

/// Byte index of the `col`-th character in `s`, or `s.len()` if past the end.
/// Lets the recording editor address the cursor in characters while editing a
/// UTF-8 `String`.
fn char_byte_idx(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map(|(b, _)| b).unwrap_or(s.len())
}

/// A single header marker segment summarizing a message's annotations, taking
/// the most severe. `None` when there are no annotations. Returned as a
/// one-element iterator so callers can `segs.extend(...)` it.
fn annotation_marker(annotations: &[Annotation]) -> Option<(String, DetailKind)> {
    let severity = annotations
        .iter()
        .map(|a| a.severity)
        .max_by_key(|s| severity_rank(*s))?;
    Some((
        format!("  {}", severity_glyph(severity)),
        DetailKind::Annotation(severity),
    ))
}

/// Extract the inclusive text span between ordered points `a <= b`.
fn selection_text(lines: &[DetailLine], a: (usize, usize), b: (usize, usize)) -> String {
    let (al, ac) = a;
    let (bl, bc) = b;
    let last = lines.len().saturating_sub(1);
    let (lo, hi) = (al.min(last), bl.min(last));
    let mut out = String::new();
    for (l, dl) in lines.iter().enumerate().take(hi + 1).skip(lo) {
        let chars: Vec<char> = dl.text().chars().collect();
        if !chars.is_empty() {
            let start = (if l == al { ac } else { 0 }).min(chars.len() - 1);
            let end = (if l == bl { bc } else { chars.len() - 1 }).min(chars.len() - 1);
            if start <= end {
                out.extend(&chars[start..=end]);
            }
        }
        if l != bl {
            out.push('\n');
        }
    }
    out
}
