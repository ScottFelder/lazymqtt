//! Shared rendering helpers used across the screen modules: bordered blocks,
//! pane titles, the vertical scrollbar, popup centering, and the multi-line
//! text-editor widget.
use crate::app::TextArea;
use crate::theme::Palette;
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

/// Render a [`TextArea`] into `area`, scrolling so the cursor stays visible.
/// When `focused`, the character under the cursor is drawn as a block cursor.
pub(crate) fn draw_textarea(
    f: &mut Frame,
    area: Rect,
    ta: &TextArea,
    focused: bool,
    pal: &Palette,
) {
    let text_h = area.height as usize;
    let text_w = (area.width.max(1) as usize).saturating_sub(1);
    let row_off = ta.row.saturating_sub(text_h.saturating_sub(1));
    let col_off = ta.col.saturating_sub(text_w);

    let mut lines: Vec<Line> = Vec::new();
    for r in row_off..(row_off + text_h).min(ta.lines.len()) {
        let chars: Vec<char> = ta.lines[r].chars().collect();
        if focused && r == ta.row {
            let cur = ta.col;
            let before: String = chars[col_off.min(chars.len())..cur.min(chars.len())]
                .iter()
                .collect();
            let cursor_ch = chars.get(cur).copied().unwrap_or(' ');
            let after: String = if cur < chars.len() {
                chars[(cur + 1)..].iter().take(text_w).collect()
            } else {
                String::new()
            };
            lines.push(Line::from(vec![
                Span::raw(before),
                Span::styled(
                    cursor_ch.to_string(),
                    Style::default().fg(pal.selection_fg).bg(pal.accent),
                ),
                Span::raw(after),
            ]));
        } else {
            lines.push(Line::from(
                chars.iter().skip(col_off).take(text_w).collect::<String>(),
            ));
        }
    }
    f.render_widget(Paragraph::new(lines), area);
}

pub(crate) fn title_block(title: &str, pal: &Palette) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(pal.dim))
        .title(Span::styled(
            format!(" {} ", title),
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        ))
}

/// Draw a vertical scrollbar along the right inner edge of `area` (a
/// bordered pane), but only when `total` items/lines exceed the visible
/// viewport — otherwise the whole pane content already fits and no
/// scrollbar should be shown.
pub(crate) fn render_vscrollbar(
    f: &mut Frame,
    area: Rect,
    total: usize,
    position: usize,
    pal: &Palette,
) {
    let viewport = area.height.saturating_sub(2) as usize;
    if viewport == 0 || total <= viewport {
        return;
    }
    let max_pos = total.saturating_sub(viewport);
    let mut state = ScrollbarState::new(max_pos).position(position.min(max_pos));
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .style(Style::default().fg(pal.dim))
        .thumb_style(Style::default().fg(pal.accent));
    f.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut state,
    );
}

/// A lazygit-style pane title: a dim `[n]` number prefix (the key that jumps
/// to this pane) followed by the accent-colored pane name.
pub(crate) fn pane_title(number: &str, name: &str, pal: &Palette) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" [{}] ", number), Style::default().fg(pal.dim)),
        Span::styled(
            name.to_string(),
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ])
}

pub(crate) fn center_rect(area: Rect, pct_x: u16, pct_y: u16) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(v[1])[1]
}
