use crate::config::{Config, Connection};
use crate::mqtt::{Message, MqttCommand, MqttHandle};
use crate::tree::TopicTree;
use std::collections::HashSet;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Screen {
    Connections,
    ConnectionForm,
    Broker,
    Publish,
    Subscribe,
    Help,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Focus {
    Tree,
    Detail,
}

/// How a detail line should be styled. The text itself is decoration-free so
/// the selection cursor maps 1:1 to characters and yanks copy clean content.
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum DetailKind {
    Header,
    Meta,
    Payload,
    Blank,
}

/// One logical line of the Messages pane, addressable by the selection cursor.
pub struct DetailLine {
    pub text: String,
    pub kind: DetailKind,
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

    pub sub_input: String, // buffer for the subscribe prompt
    pub error: Option<String>,

    // Keyboard text selection in the Messages pane. Both are (line, col) into
    // the rows returned by `detail_lines`. `sel_anchor` is set once visual
    // selection begins; while it is None the cursor just marks a position.
    pub sel_cursor: (usize, usize),
    pub sel_anchor: Option<(usize, usize)>,
}

impl App {
    pub fn new() -> Self {
        Self {
            config: Config::load(),
            screen: Screen::Connections,
            focus: Focus::Tree,
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
            sub_input: String::new(),
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

    pub fn push_message(&mut self, msg: Message) {
        self.tree.insert(msg.clone());
        self.history.push(msg);
        if self.history.len() > 5000 {
            self.history.drain(0..1000);
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
        self.reset_selection();
    }

    pub fn reset_selection(&mut self) {
        self.sel_cursor = (0, 0);
        self.sel_anchor = None;
    }

    /// The Messages pane contents for the currently selected topic, as plain
    /// logical lines. Shared by the renderer and the selection/yank logic so
    /// both agree on line and column positions.
    pub fn detail_lines(&self) -> Vec<DetailLine> {
        let rows = self.tree.rows(&self.expanded);
        let mut out = Vec::new();

        let Some(row) = rows.get(self.tree_selected.min(rows.len().saturating_sub(1))) else {
            out.push(DetailLine {
                text: "Waiting for messages…".into(),
                kind: DetailKind::Blank,
            });
            return out;
        };

        let path = row.path.clone();
        out.push(DetailLine {
            text: path.clone(),
            kind: DetailKind::Header,
        });
        out.push(DetailLine {
            text: String::new(),
            kind: DetailKind::Blank,
        });

        let msgs: Vec<&Message> = self
            .history
            .iter()
            .rev()
            .filter(|m| m.topic == path)
            .take(30)
            .collect();
        if msgs.is_empty() {
            out.push(DetailLine {
                text: "(no messages on this exact topic — it may be a parent node)".into(),
                kind: DetailKind::Blank,
            });
        }
        for m in msgs {
            let retain = if m.retained { " R" } else { "" };
            out.push(DetailLine {
                text: format!(
                    "{}  q{}{}",
                    m.time.format("%m/%d/%Y %H:%M:%S%.3f"),
                    m.qos,
                    retain
                ),
                kind: DetailKind::Meta,
            });
            for line in m.payload.lines() {
                out.push(DetailLine {
                    text: line.to_string(),
                    kind: DetailKind::Payload,
                });
            }
            out.push(DetailLine {
                text: String::new(),
                kind: DetailKind::Blank,
            });
        }
        out
    }

    /// Clamp a column to the last character index of the given line (0 when empty).
    fn max_col(lines: &[DetailLine], line: usize) -> usize {
        lines
            .get(line)
            .map(|l| l.text.chars().count())
            .unwrap_or(0)
            .saturating_sub(1)
    }

    pub fn sel_move_col(&mut self, delta: isize) {
        let lines = self.detail_lines();
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
        let lines = self.detail_lines();
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
        let lines = self.detail_lines();
        if lines.is_empty() {
            return;
        }
        let text = match self.sel_anchor {
            Some(a) => {
                let b = self.sel_cursor;
                let (s, e) = if a <= b { (a, b) } else { (b, a) };
                selection_text(&lines, s, e)
            }
            None => lines[self.sel_cursor.0.min(lines.len() - 1)].text.clone(),
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
            topic: topic.into(),
            payload: payload.into(),
            qos: 0,
            retained: false,
            time: Local::now(),
        }
    }

    #[test]
    fn selection_text_single_line_inclusive() {
        let lines = vec![DetailLine { text: "hello".into(), kind: DetailKind::Payload }];
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
        app.focus = Focus::Detail;

        let payload_line =
            app.detail_lines().iter().position(|l| l.kind == DetailKind::Payload).unwrap();
        for _ in 0..payload_line {
            handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        assert_eq!(app.sel_cursor.0, payload_line, "cursor should be on the payload line");

        handle_key(&mut app, KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));
        for _ in 0..10 {
            handle_key(&mut app, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        }

        let (a, b) = (app.sel_anchor.unwrap(), app.sel_cursor);
        let (s, e) = if a <= b { (a, b) } else { (b, a) };
        assert_eq!(selection_text(&app.detail_lines(), s, e), "hello");
    }

    #[test]
    fn yank_includes_last_char_under_cursor() {
        let mut app = App::new();
        app.tree.insert(msg("t", "hello"));
        app.history.push(msg("t", "hello"));
        app.tree_selected = 0;

        let lines = app.detail_lines();
        let payload_line =
            lines.iter().position(|l| l.kind == DetailKind::Payload).unwrap();
        app.sel_cursor = (payload_line, 0);
        app.sel_toggle_anchor();
        for _ in 0..10 {
            app.sel_move_col(1);
        }
        assert_eq!(app.sel_cursor.1, 4, "cursor should rest on last char index");

        let (a, b) = (app.sel_anchor.unwrap(), app.sel_cursor);
        let (s, e) = if a <= b { (a, b) } else { (b, a) };
        assert_eq!(selection_text(&app.detail_lines(), s, e), "hello");
    }
}

/// Extract the inclusive text span between ordered points `a <= b`.
fn selection_text(lines: &[DetailLine], a: (usize, usize), b: (usize, usize)) -> String {
    let (al, ac) = a;
    let (bl, bc) = b;
    let last = lines.len().saturating_sub(1);
    let (lo, hi) = (al.min(last), bl.min(last));
    let mut out = String::new();
    for (l, dl) in lines.iter().enumerate().take(hi + 1).skip(lo) {
        let chars: Vec<char> = dl.text.chars().collect();
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
