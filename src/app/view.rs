//! View-model types for the message panes: focus, pane folding, and the
//! styled `DetailLine` model the Payload/History panes render from.

use crate::plugin::{InspectorStyle, Severity};

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
