use super::{annotation_marker, selection_text, severity_glyph};
use crate::app::{App, DetailKind, DetailLine, Focus, PaneFold, PublishBuffer, Screen};
use crate::mqtt::{Message, MqttCommand};
use crate::plugin::{InspectMessage, InspectorView};

impl App {
    /// The path of the currently selected topic-tree row, if any.
    pub fn selected_topic(&self) -> Option<String> {
        let rows = self.tree.rows(&self.expanded);
        rows.get(self.tree_selected.min(rows.len().saturating_sub(1)))
            .map(|r| r.path.clone())
    }

    /// Remove a topic and its subtree from the local view: the tree node, the
    /// history messages under it (with their annotations), and any saved
    /// expansion state. Purely local — it does not unsubscribe or touch the
    /// broker, so new messages on the topic repopulate it.
    pub(crate) fn clear_topic_subtree(&mut self, topic: &str) {
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

    /// Expand the selected topic and every node beneath it. Keeps the cursor on
    /// the selected topic.
    pub fn expand_subtree(&mut self) {
        let Some(root) = self.selected_topic() else {
            return;
        };
        let prefix = format!("{root}/");
        let under: Vec<String> = self
            .tree
            .all_parent_paths()
            .into_iter()
            .filter(|p| *p == root || p.starts_with(&prefix))
            .collect();
        self.expanded.extend(under);
        self.select_tree_path(Some(root));
    }

    /// Collapse the selected topic and every node beneath it. The selected topic
    /// itself stays visible (and selected), just closed.
    pub fn collapse_subtree(&mut self) {
        let Some(root) = self.selected_topic() else {
            return;
        };
        let prefix = format!("{root}/");
        self.expanded
            .retain(|p| !(*p == root || p.starts_with(&prefix)));
        self.select_tree_path(Some(root));
    }

    /// Move the tree cursor to `path` if visible, else to its nearest visible
    /// ancestor; clamps to the row count. Refreshes the message view for the new
    /// selection.
    fn select_tree_path(&mut self, path: Option<String>) {
        let rows = self.tree.rows(&self.expanded);
        let idx = path.and_then(|mut p| loop {
            if let Some(i) = rows.iter().position(|r| r.path == p) {
                return Some(i);
            }
            match p.rsplit_once('/') {
                Some((parent, _)) => p = parent.to_string(),
                None => return None,
            }
        });
        self.tree_selected = idx
            .unwrap_or(self.tree_selected)
            .min(rows.len().saturating_sub(1));
        self.reset_message_view();
    }

    pub(crate) fn unsubscribe_selected(&mut self) {
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

    pub(crate) fn open_publish(&mut self) {
        self.publish = PublishBuffer::default();
        self.publish_save_as = None;
        if let Some(topic) = self.selected_topic() {
            self.publish.topic = topic;
        }
        self.screen = Screen::Publish;
    }

    /// Save the current publish form as a named, reusable template.
    pub fn commit_publish_template(&mut self) {
        let Some(name) = self.publish_save_as.take() else {
            return;
        };
        let name = name.trim().to_string();
        if name.is_empty() {
            self.error = Some("template name required".into());
            return;
        }
        let template = crate::plugin::PublishTemplate {
            name: name.clone(),
            topic: self.publish.topic.clone(),
            payload: self.publish.payload.clone(),
            qos: self.publish.qos,
            retain: self.publish.retain,
        };
        match crate::plugin::save_publish_template(template) {
            Ok(()) => self.error = Some(format!("saved template “{name}”")),
            Err(e) => self.error = Some(format!("save failed: {e}")),
        }
    }

    pub fn reset_selection(&mut self) {
        self.sel_cursor = (0, 0);
        self.sel_anchor = None;
    }

    /// Call whenever the selected topic changes: jumps the History pane back
    /// to the latest message, collapses any expanded entries, and clears any
    /// in-progress Payload selection. The chosen payload view (`payload_view`)
    /// is intentionally left untouched so it stays sticky across messages.
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
                        "{}  QoS {}{}",
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

                // Body: the raw payload, or the preferred plugin inspector view
                // when this message can produce it (else fall back to raw).
                match self.active_inspector_view() {
                    None => {
                        for line in m.payload.lines() {
                            out.push(DetailLine::indented(2, line, DetailKind::Payload));
                        }
                    }
                    Some(view) => {
                        for line in &view.lines {
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

    /// The preferred inspector view for the selected message, if the preference
    /// is set and this message can actually produce that view. Returns `None`
    /// (render raw) when the preference is raw or unavailable for this message.
    fn active_inspector_view(&self) -> Option<InspectorView> {
        let want = self.payload_view.as_deref()?;
        self.current_inspector_views()
            .into_iter()
            .find(|v| v.label == want)
    }

    /// Cycle the preferred Payload view through raw and each enabled view
    /// plugin's label, in plugin order (independent of the current message so
    /// the order is stable). The line set may change, so selection is reset.
    pub fn cycle_payload_view(&mut self) {
        let labels = self.plugins.inspector_labels();
        // Current position in the cycle: 0 = raw, 1.. = labels; an unknown
        // preference (its plugin was disabled) restarts from raw.
        let pos = match self.payload_view.as_deref() {
            None => 0,
            Some(cur) => labels
                .iter()
                .position(|l| *l == cur)
                .map(|i| i + 1)
                .unwrap_or(0),
        };
        let next = (pos + 1) % (labels.len() + 1);
        self.payload_view = (next != 0).then(|| labels[next - 1].to_string());
        self.reset_selection();
    }

    /// Label shown in the Payload pane title: the effective view for the
    /// selected message ("raw" or a plugin label), or `None` when this message
    /// offers no alternate views and the preference isn't rendering (nothing to
    /// hint).
    pub fn payload_view_label(&self) -> Option<String> {
        match self.active_inspector_view() {
            Some(view) => Some(view.label),
            None if self.current_inspector_views().is_empty() => None,
            None => Some("raw".to_string()),
        }
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
                    "{}  QoS {}{}",
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
                let meta = format!("{}  QoS {}{}", m.time.format("%H:%M:%S%.3f"), m.qos, retain);
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
