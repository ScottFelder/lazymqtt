//! Mouse-driven actions on the broker screen's three content panes. These are
//! thin state transitions (the `events::mouse` handler maps screen coordinates
//! to pane-local line/column and calls them), kept here so the behavior is
//! testable without a terminal and consistent with the keyboard paths.

use super::{App, Focus};

impl App {
    /// Clamp a column to the last selectable char of the given line (0 if empty).
    fn clamp_col(lines: &[super::DetailLine], line: usize, col: usize) -> usize {
        let max = lines.get(line).map(|l| l.char_len()).unwrap_or(0);
        col.min(max.saturating_sub(1))
    }

    /// Select a visible topic-tree row (index into the flattened rows). Focuses
    /// the tree; resetting the message view when the selection actually moves.
    pub fn mouse_select_tree(&mut self, row: usize) {
        let rows = self.tree.rows(&self.expanded);
        if rows.is_empty() {
            return;
        }
        let row = row.min(rows.len() - 1);
        self.focus = Focus::Tree;
        if self.tree_selected != row {
            self.tree_selected = row;
            self.reset_message_view();
        } else {
            self.reset_selection();
        }
    }

    /// Toggle expand/collapse of the topic-tree row under a double-click.
    pub fn mouse_toggle_tree(&mut self, row: usize) {
        let rows = self.tree.rows(&self.expanded);
        let Some(r) = rows.get(row.min(rows.len().saturating_sub(1))) else {
            return;
        };
        if r.has_children && !self.expanded.remove(&r.path) {
            self.expanded.insert(r.path.clone());
        }
    }

    /// Place the Payload selection cursor at a clicked line/column (focuses the
    /// Payload pane and clears any in-progress selection).
    pub fn mouse_click_payload(&mut self, line: usize, col: usize) {
        self.focus = Focus::Payload;
        let lines = self.payload_lines();
        self.sel_anchor = None;
        if lines.is_empty() {
            self.sel_cursor = (0, 0);
            return;
        }
        let l = line.min(lines.len() - 1);
        self.sel_cursor = (l, Self::clamp_col(&lines, l, col));
    }

    /// Place the History cursor at a clicked line/column and point the Payload
    /// pane at that message.
    pub fn mouse_click_history(&mut self, line: usize, col: usize) {
        self.focus = Focus::History;
        let lines = self.history_lines();
        self.sel_anchor = None;
        if lines.is_empty() {
            self.sel_cursor = (0, 0);
            return;
        }
        let l = line.min(lines.len() - 1);
        self.sel_cursor = (l, Self::clamp_col(&lines, l, col));
        self.sync_history_selected();
    }

    /// Extend a drag-selection in the focused message pane to a line/column,
    /// anchoring at the press point on the first drag so release can yank it.
    pub fn mouse_drag(&mut self, line: usize, col: usize) {
        let lines = match self.focus {
            Focus::History => self.history_lines(),
            Focus::Payload => self.payload_lines(),
            Focus::Tree => return,
        };
        if lines.is_empty() {
            return;
        }
        if self.sel_anchor.is_none() {
            self.sel_anchor = Some(self.sel_cursor);
        }
        let l = line.min(lines.len() - 1);
        self.sel_cursor = (l, Self::clamp_col(&lines, l, col));
    }

    /// Finish a drag: copy the selection if one was actually dragged out,
    /// otherwise drop the bare anchor so the click reads as a plain cursor.
    pub fn mouse_release(&mut self) {
        match self.sel_anchor {
            Some(a) if a != self.sel_cursor => self.sel_yank(),
            _ => self.sel_anchor = None,
        }
    }

    /// Wheel scroll within a pane: moves that pane's cursor/selection one step,
    /// which also scrolls the view (matching the keyboard j/k behavior).
    pub fn mouse_scroll(&mut self, pane: Focus, down: bool) {
        match pane {
            Focus::Tree => {
                let len = self.tree.rows(&self.expanded).len();
                self.focus = Focus::Tree;
                if down {
                    if len > 0 {
                        self.tree_selected = (self.tree_selected + 1).min(len - 1);
                    }
                } else {
                    self.tree_selected = self.tree_selected.saturating_sub(1);
                }
                self.reset_message_view();
            }
            Focus::Payload => {
                self.focus = Focus::Payload;
                self.sel_move_line(if down { 1 } else { -1 });
            }
            Focus::History => {
                self.focus = Focus::History;
                self.sel_move_line(if down { 1 } else { -1 });
                self.sync_history_selected();
            }
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
            id: 0,
            topic: topic.into(),
            payload: payload.into(),
            payload_raw: payload.as_bytes().to_vec(),
            qos: 0,
            retained: false,
            time: Local::now(),
        }
    }

    #[test]
    fn click_tree_selects_and_focuses() {
        let mut app = App::new();
        app.push_message(msg("a/b", "1"));
        app.push_message(msg("a/c", "2"));
        app.expanded.insert("a".to_string());
        // rows: a, a/b, a/c
        app.mouse_select_tree(2);
        assert_eq!(app.focus, Focus::Tree);
        assert_eq!(app.selected_topic().as_deref(), Some("a/c"));
    }

    #[test]
    fn double_click_tree_toggles_expansion() {
        let mut app = App::new();
        app.push_message(msg("a/b", "1"));
        assert!(!app.expanded.contains("a"));
        app.mouse_toggle_tree(0); // row 0 == "a" (has children)
        assert!(app.expanded.contains("a"));
        app.mouse_toggle_tree(0);
        assert!(!app.expanded.contains("a"));
    }

    #[test]
    fn click_payload_places_cursor_clamped() {
        let mut app = App::new();
        app.push_message(msg("data", "hello"));
        app.tree_selected = 0;
        // Click line 0 well past its end: the row is kept, the column clamps to
        // that line's last character.
        app.mouse_click_payload(0, 999);
        assert_eq!(app.focus, Focus::Payload);
        let max_col = app.payload_lines()[0].char_len().saturating_sub(1);
        assert_eq!(app.sel_cursor, (0, max_col));
        assert!(app.sel_anchor.is_none());
    }

    #[test]
    fn drag_then_release_yanks_a_real_selection() {
        let mut app = App::new();
        app.push_message(msg("data", "hello world"));
        app.tree_selected = 0;
        app.focus = Focus::Payload;
        // Find the payload line and drag across it.
        let line = app
            .payload_lines()
            .iter()
            .position(|l| l.text().contains("hello"))
            .unwrap();
        app.mouse_click_payload(line, 0);
        app.mouse_drag(line, 4); // select "hello"
        assert_eq!(app.sel_anchor, Some((line, 0)));
        assert_eq!(app.sel_cursor, (line, 4));
        app.mouse_release();
        // Releasing a real drag attempts a yank, which reports via `error`
        // (a "copied …" status on success, or a copy-failure note) — either way
        // proves the drag was treated as a selection, not a bare click.
        assert!(app.error.is_some());
    }

    #[test]
    fn release_without_drag_clears_anchor() {
        let mut app = App::new();
        app.push_message(msg("data", "hi"));
        app.tree_selected = 0;
        app.mouse_click_payload(0, 0);
        app.mouse_release();
        assert!(app.sel_anchor.is_none());
    }

    #[test]
    fn scroll_moves_tree_selection() {
        let mut app = App::new();
        for t in ["a", "b", "c"] {
            app.push_message(msg(t, "x"));
        }
        app.tree_selected = 0;
        app.mouse_scroll(Focus::Tree, true);
        assert_eq!(app.tree_selected, 1);
        app.mouse_scroll(Focus::Tree, false);
        assert_eq!(app.tree_selected, 0);
        app.mouse_scroll(Focus::Tree, false); // saturates at 0
        assert_eq!(app.tree_selected, 0);
    }
}
