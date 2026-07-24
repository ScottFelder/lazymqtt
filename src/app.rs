use crate::config::{Config, Connection};
use crate::mqtt::{Message, MqttCommand, MqttHandle};
use crate::plugin::{Annotation, PluginAction, PluginEvent, PluginHost, Severity};
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
    Help,
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

    pub sub_input: String,       // buffer for the subscribe prompt
    pub clear_topic: String,     // topic awaiting retained-message clear confirmation
    pub plugins_selected: usize, // cursor in the Plugins management screen
    pub error: Option<String>,

    // Keyboard text selection in the focused pane (Payload or History). Both
    // are (line, col) into that pane's `active_lines`. `sel_anchor` is set once
    // visual selection begins; while it is None the cursor just marks a position.
    pub sel_cursor: (usize, usize),
    pub sel_anchor: Option<(usize, usize)>,
}

impl App {
    pub fn new() -> Self {
        Self {
            config: Config::load(),
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
                for line in m.payload.lines() {
                    out.push(DetailLine::indented(2, line, DetailKind::Payload));
                }
            }
        }
        out
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
