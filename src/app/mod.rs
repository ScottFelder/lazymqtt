//! Application state and behavior. `App` is the single state object the render
//! loop owns; its methods live in focused submodules (`connection`, `commands`,
//! `broker`, `alerts`, `recordings`, `theme`), while the state types live in
//! `screen`, `view`, and `forms`. UI reads `App`; events mutate it.

mod alerts;
mod broker;
mod commands;
mod connection;
mod forms;
mod recordings;
mod schemas;
mod screen;
mod textarea;
mod theme;
mod view;

pub use forms::{AlertForm, FormBuffer, PublishBuffer, SchemaForm, Status};
pub use screen::{Command, MenuAction, MenuItem, Screen, BROKER_COMMANDS};
pub use textarea::TextArea;
pub use view::{DetailKind, DetailLine, Focus, PaneFold};

use crate::config::Config;
use crate::mqtt::{Message, MqttHandle};
use crate::plugin::{AlertRule, Annotation, PluginHost, Recording, Severity};
use crate::theme::{Palette, Theme};
use crate::tree::TopicTree;
use std::collections::{HashMap, HashSet};

pub struct App {
    pub config: Config,
    pub screen: Screen,
    pub focus: Focus,
    pub collapsed: PaneFold, // which message sub-pane is collapsed, if any
    pub should_quit: bool,

    pub conn_selected: usize,
    pub form: FormBuffer,
    pub publish: PublishBuffer,
    /// When `Some`, the publish form is prompting for a name to save the current
    /// topic/payload/QoS/retain as a reusable template (the buffer is the name).
    pub publish_save_as: Option<String>,

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

    // Recording editor (multi-line JSONL edit, opened from the picker with `e`).
    // `rec_editor` holds the editable text; `rec_edit_label` is the recording
    // being edited (the Save target); `rec_edit_saveas`, when Some, holds the
    // "save as" filename buffer.
    pub rec_editor: TextArea,
    pub rec_edit_label: String,
    pub rec_edit_error: Option<String>,
    pub rec_edit_saveas: Option<String>,

    // JSON Schema editor (per connection). `schemas` is the working copy for the
    // connection identified by `schema_edit_conn` (name in `schema_edit_name`);
    // `schemas_selected` is the list cursor; `schema_form` backs the add/edit form.
    pub schemas: Vec<crate::plugin::SchemaMapping>,
    pub schemas_selected: usize,
    pub schema_edit_conn: String,
    pub schema_edit_name: String,
    pub schema_form: SchemaForm,

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
            publish_save_as: None,
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
            rec_editor: TextArea::default(),
            rec_edit_label: String::new(),
            rec_edit_error: None,
            rec_edit_saveas: None,
            schemas: Vec::new(),
            schemas_selected: 0,
            schema_edit_conn: String::new(),
            schema_edit_name: String::new(),
            schema_form: SchemaForm::default(),
            payload_view: 0,
            error: None,
            sel_cursor: (0, 0),
            sel_anchor: None,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mqtt::Message;
    use crate::plugin::{AlertCondition, AlertSeverity};
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
