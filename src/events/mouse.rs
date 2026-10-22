//! Mouse input for the Connections list and the broker screen's three content
//! panes. Translates screen coordinates into a row / pane-local line+column
//! (via `crate::layout`) and drives the matching `App` mouse action; it never
//! renders and holds no logic of its own beyond hit-testing and double-click
//! detection.

use crate::app::{App, Focus, Screen};
use crate::layout::{self, BrokerPanes};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

const DOUBLE_CLICK: Duration = Duration::from_millis(400);

pub(super) fn handle_mouse(app: &mut App, me: MouseEvent, full: Rect) {
    match app.screen {
        Screen::Connections => connections_mouse(app, me, full),
        Screen::Broker => broker_mouse(app, me, full),
        _ => {}
    }
}

/// The Connections list fills the content area with a one-cell border: click a
/// row to select it, double-click to connect, wheel to move the selection.
fn connections_mouse(app: &mut App, me: MouseEvent, full: Rect) {
    let (content, _) = layout::screen_split(full);
    let (col, row) = (me.column, me.row);
    match me.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            app.error = None;
            let double = is_double_click(app, col, row);
            if let Some((r, _)) = layout::hit_inner(content, col, row) {
                let idx = layout::scroll_offset(app.conn_selected, inner_h(content)) + r;
                if idx < app.config.connections.len() {
                    app.mouse_select_connection(idx);
                    if double {
                        app.connect_selected();
                    }
                }
            }
        }
        MouseEventKind::ScrollDown => app.mouse_scroll_connections(true),
        MouseEventKind::ScrollUp => app.mouse_scroll_connections(false),
        _ => {}
    }
}

fn broker_mouse(app: &mut App, me: MouseEvent, full: Rect) {
    let (content, _) = layout::screen_split(full);
    let panes = layout::broker_panes(content, app.collapsed);
    let (col, row) = (me.column, me.row);

    match me.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            app.error = None;
            let double = is_double_click(app, col, row);
            match locate(app, &panes, col, row) {
                Some(Target::Tree(idx)) => {
                    app.mouse_select_tree(idx);
                    if double {
                        app.mouse_toggle_tree(idx);
                    }
                }
                Some(Target::Payload(l, c)) => app.mouse_click_payload(l, c),
                Some(Target::History(l, c)) => {
                    app.mouse_click_history(l, c);
                    if double {
                        app.toggle_history_at_cursor();
                    }
                }
                None => {}
            }
        }
        // Dragging extends the selection in the focused message pane, clamped so
        // it keeps tracking even when the pointer strays past the pane edges.
        MouseEventKind::Drag(MouseButton::Left) => match app.focus {
            Focus::Payload if !panes.payload_collapsed => {
                let (l, c) = clamp_point(app, panes.payload, col, row);
                app.mouse_drag(l, c);
            }
            Focus::History if !panes.history_collapsed => {
                let (l, c) = clamp_point(app, panes.history, col, row);
                app.mouse_drag(l, c);
            }
            _ => {}
        },
        MouseEventKind::Up(MouseButton::Left) => app.mouse_release(),
        MouseEventKind::ScrollDown => {
            if let Some(pane) = pane_at(&panes, col, row) {
                app.mouse_scroll(pane, true);
            }
        }
        MouseEventKind::ScrollUp => {
            if let Some(pane) = pane_at(&panes, col, row) {
                app.mouse_scroll(pane, false);
            }
        }
        _ => {}
    }
}

/// What a point resolves to: a tree row, or a line/column in a message pane
/// (already adjusted for that pane's scroll offset).
enum Target {
    Tree(usize),
    Payload(usize, usize),
    History(usize, usize),
}

fn locate(app: &App, panes: &BrokerPanes, col: u16, row: u16) -> Option<Target> {
    if let Some((r, _)) = layout::hit_inner(panes.tree, col, row) {
        // The tree List always scrolls to keep the selected row visible,
        // regardless of focus.
        let off = layout::scroll_offset(app.tree_selected, inner_h(panes.tree));
        return Some(Target::Tree(off + r));
    }
    if !panes.payload_collapsed {
        if let Some((r, c)) = layout::hit_inner(panes.payload, col, row) {
            return Some(Target::Payload(
                pane_offset(app, Focus::Payload, panes.payload) + r,
                c,
            ));
        }
    }
    if !panes.history_collapsed {
        if let Some((r, c)) = layout::hit_inner(panes.history, col, row) {
            return Some(Target::History(
                pane_offset(app, Focus::History, panes.history) + r,
                c,
            ));
        }
    }
    None
}

/// The whole (bordered) pane a point falls in, for wheel routing.
fn pane_at(panes: &BrokerPanes, col: u16, row: u16) -> Option<Focus> {
    if contains(panes.tree, col, row) {
        Some(Focus::Tree)
    } else if !panes.payload_collapsed && contains(panes.payload, col, row) {
        Some(Focus::Payload)
    } else if !panes.history_collapsed && contains(panes.history, col, row) {
        Some(Focus::History)
    } else {
        None
    }
}

/// Map a (possibly out-of-bounds) point to a line/column in `rect`, clamped into
/// the pane — used while dragging so the selection keeps extending at the edges.
fn clamp_point(app: &App, rect: Rect, col: u16, row: u16) -> (usize, usize) {
    let inner = layout::block_inner(rect);
    let off = layout::scroll_offset(app.sel_cursor.0, inner_h(rect));
    let r = row
        .saturating_sub(inner.y)
        .min(inner.height.saturating_sub(1)) as usize;
    let c = col.saturating_sub(inner.x) as usize; // mouse_drag clamps to line length
    (off + r, c)
}

/// First visible line of a message pane: it renders from the top unless it has
/// focus, in which case it scrolls to keep the cursor in view.
fn pane_offset(app: &App, pane: Focus, rect: Rect) -> usize {
    if app.focus == pane {
        layout::scroll_offset(app.sel_cursor.0, inner_h(rect))
    } else {
        0
    }
}

fn inner_h(r: Rect) -> usize {
    r.height.saturating_sub(2) as usize
}

fn contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

/// A left press in the same cell as the previous one, within the threshold.
/// Records this press as the new baseline either way.
fn is_double_click(app: &mut App, col: u16, row: u16) -> bool {
    let now = Instant::now();
    let double = app
        .last_click
        .map(|(t, c, r)| c == col && r == row && now.duration_since(t) <= DOUBLE_CLICK)
        .unwrap_or(false);
    // A completed double-click resets the baseline so a third click starts fresh.
    app.last_click = if double { None } else { Some((now, col, row)) };
    double
}
