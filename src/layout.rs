//! Screen geometry shared by the renderer and the mouse-input handler, so both
//! agree on exactly where each pane sits. `ui` draws into these rects; `events`
//! hit-tests mouse clicks against them. Keeping the math in one place means a
//! layout change can't desync what's drawn from what the mouse targets.

use crate::app::PaneFold;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Rows the collapsed message sub-pane shrinks to (title bar only). Mirrors the
/// constant the renderer uses.
const COLLAPSED_ROWS: u16 = 3;

/// Split the full terminal area into (content, status-bar) the way `ui::draw`
/// does: everything above a single status-bar row.
pub fn screen_split(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    (chunks[0], chunks[1])
}

/// The outer (bordered) rects of the broker screen's three content panes, plus
/// which message sub-panes are collapsed. `content` is the area above the
/// status bar (see [`screen_split`]).
pub struct BrokerPanes {
    pub tree: Rect,
    pub messages: Rect,
    pub payload: Rect,
    pub history: Rect,
    pub payload_collapsed: bool,
    pub history_collapsed: bool,
}

pub fn broker_panes(content: Rect, fold: PaneFold) -> BrokerPanes {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(content);
    let (tree, messages) = (cols[0], cols[1]);

    let (constraints, payload_collapsed, history_collapsed) = match fold {
        PaneFold::None => (
            [Constraint::Percentage(50), Constraint::Percentage(50)],
            false,
            false,
        ),
        PaneFold::Payload => (
            [Constraint::Length(COLLAPSED_ROWS), Constraint::Min(1)],
            true,
            false,
        ),
        PaneFold::History => (
            [Constraint::Min(1), Constraint::Length(COLLAPSED_ROWS)],
            false,
            true,
        ),
    };
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(block_inner(messages));

    BrokerPanes {
        tree,
        messages,
        payload: right[0],
        history: right[1],
        payload_collapsed,
        history_collapsed,
    }
}

/// The content area inside a single-cell border, matching
/// `Block::default().borders(Borders::ALL).inner(rect)`.
pub fn block_inner(r: Rect) -> Rect {
    Rect {
        x: r.x.saturating_add(1),
        y: r.y.saturating_add(1),
        width: r.width.saturating_sub(2),
        height: r.height.saturating_sub(2),
    }
}

/// If `(col, row)` lands inside a bordered rect's content area, return the
/// (row, col) offset within that content; otherwise `None` (border or outside).
pub fn hit_inner(r: Rect, col: u16, row: u16) -> Option<(usize, usize)> {
    let inner = block_inner(r);
    if inner.width == 0 || inner.height == 0 {
        return None;
    }
    if col < inner.x || col >= inner.x + inner.width {
        return None;
    }
    if row < inner.y || row >= inner.y + inner.height {
        return None;
    }
    Some(((row - inner.y) as usize, (col - inner.x) as usize))
}

/// The first visible line index given the cursor/selected index and the inner
/// height, matching the renderer's scroll rule (keep the cursor in view, pinned
/// to the bottom once it passes the fold). A `ListState` reset each frame scrolls
/// the topic tree the same way, so this maps clicks in all three panes.
pub fn scroll_offset(index: usize, inner_height: usize) -> usize {
    if inner_height > 0 && index >= inner_height {
        index - inner_height + 1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broker_panes_split_full_and_folded() {
        let content = Rect::new(0, 0, 100, 41);
        let p = broker_panes(content, PaneFold::None);
        // Two equal columns.
        assert_eq!(p.tree, Rect::new(0, 0, 50, 41));
        assert_eq!(p.messages, Rect::new(50, 0, 50, 41));
        // Payload/history split the messages container's inner height evenly.
        let inner = block_inner(p.messages);
        assert_eq!(p.payload.height + p.history.height, inner.height);
        assert_eq!(p.payload.y, inner.y);
        assert_eq!(p.history.y, p.payload.y + p.payload.height);
        assert!(!p.payload_collapsed && !p.history_collapsed);

        // Collapsing the payload pins it to COLLAPSED_ROWS and gives the rest to history.
        let p = broker_panes(content, PaneFold::Payload);
        assert_eq!(p.payload.height, COLLAPSED_ROWS);
        assert!(p.payload_collapsed && !p.history_collapsed);
    }

    #[test]
    fn hit_inner_rejects_border_and_maps_content() {
        // Outer cols 10..=29, rows 5..=14; borders at the edges, so the inner
        // content is cols 11..=28 and rows 6..=13.
        let r = Rect::new(10, 5, 20, 10);
        assert_eq!(hit_inner(r, 10, 5), None, "top-left border");
        assert_eq!(hit_inner(r, 11, 6), Some((0, 0)), "first inner cell");
        assert_eq!(hit_inner(r, 13, 9), Some((3, 2)));
        assert_eq!(hit_inner(r, 28, 13), Some((7, 17)), "last inner cell");
        assert_eq!(hit_inner(r, 29, 13), None, "right border");
        assert_eq!(hit_inner(r, 13, 14), None, "bottom border");
    }

    #[test]
    fn scroll_offset_pins_cursor_to_view() {
        assert_eq!(scroll_offset(0, 10), 0);
        assert_eq!(
            scroll_offset(9, 10),
            0,
            "last fully visible line, no scroll"
        );
        assert_eq!(scroll_offset(10, 10), 1, "one past the fold scrolls by one");
        assert_eq!(scroll_offset(25, 10), 16);
    }
}
