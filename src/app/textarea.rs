//! A minimal multi-line text buffer with a char-addressed cursor, shared by the
//! screens that edit free text (the recording JSONL editor and the schema-body
//! editor). Pure model — rendering lives in `ui`, key mapping in `events`.

/// Editable multi-line text: `lines` never empty, `row`/`col` a character cursor.
#[derive(Default)]
pub struct TextArea {
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
}

impl TextArea {
    /// Build from existing lines (an empty input still yields one blank line so
    /// there is always somewhere to type).
    pub fn from_lines(mut lines: Vec<String>) -> Self {
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self {
            lines,
            row: 0,
            col: 0,
        }
    }

    /// Build from a blob of text, split on newlines.
    pub fn from_text(text: &str) -> Self {
        Self::from_lines(text.split('\n').map(str::to_string).collect())
    }

    /// The buffer joined back into a single string.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Character length of a line (0 for an out-of-range row).
    pub fn line_len(&self, row: usize) -> usize {
        self.lines.get(row).map(|l| l.chars().count()).unwrap_or(0)
    }

    /// Insert a character at the cursor.
    pub fn insert(&mut self, c: char) {
        let (row, col) = (self.row, self.col);
        if let Some(line) = self.lines.get_mut(row) {
            line.insert(char_byte_idx(line, col), c);
            self.col += 1;
        }
    }

    /// Delete the character before the cursor, joining lines at column 0.
    pub fn backspace(&mut self) {
        if self.col > 0 {
            let (row, col) = (self.row, self.col);
            if let Some(line) = self.lines.get_mut(row) {
                let start = char_byte_idx(line, col - 1);
                let end = char_byte_idx(line, col);
                line.replace_range(start..end, "");
                self.col -= 1;
            }
        } else if self.row > 0 {
            let cur = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.line_len(self.row);
            self.lines[self.row].push_str(&cur);
        }
    }

    /// Split the current line at the cursor into two lines.
    pub fn newline(&mut self) {
        let (row, col) = (self.row, self.col);
        let line = self.lines.get(row).cloned().unwrap_or_default();
        let at = char_byte_idx(&line, col);
        let (left, right) = line.split_at(at);
        self.lines[row] = left.to_string();
        self.lines.insert(row + 1, right.to_string());
        self.row += 1;
        self.col = 0;
    }

    /// Move the cursor horizontally, wrapping across line ends.
    pub fn move_h(&mut self, forward: bool) {
        if forward {
            if self.col < self.line_len(self.row) {
                self.col += 1;
            } else if self.row + 1 < self.lines.len() {
                self.row += 1;
                self.col = 0;
            }
        } else if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.line_len(self.row);
        }
    }

    /// Move the cursor vertically, clamping the column to the new line.
    pub fn move_v(&mut self, down: bool) {
        if down {
            if self.row + 1 < self.lines.len() {
                self.row += 1;
            }
        } else {
            self.row = self.row.saturating_sub(1);
        }
        self.col = self.col.min(self.line_len(self.row));
    }

    pub fn home(&mut self) {
        self.col = 0;
    }

    pub fn end(&mut self) {
        self.col = self.line_len(self.row);
    }

    /// Insert pasted text at the cursor, honoring embedded newlines.
    pub fn paste(&mut self, data: &str) {
        for (i, part) in data.split('\n').enumerate() {
            if i > 0 {
                self.newline();
            }
            for c in part.chars().filter(|c| *c != '\r') {
                self.insert(c);
            }
        }
    }
}

/// Byte index of the `col`-th character in `s`, or `s.len()` if past the end.
/// Bridges character columns to the UTF-8 `String` the buffer stores.
fn char_byte_idx(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map(|(b, _)| b).unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_byte_idx_handles_unicode() {
        assert_eq!(char_byte_idx("aé", 0), 0);
        assert_eq!(char_byte_idx("aé", 1), 1);
        assert_eq!(char_byte_idx("aé", 2), 3); // é is two bytes
        assert_eq!(char_byte_idx("aé", 9), 3); // past the end clamps to len
    }

    #[test]
    fn insert_split_and_join() {
        let mut t = TextArea::from_lines(vec!["ab".to_string()]);
        t.col = 1; // between 'a' and 'b'

        t.insert('X');
        assert_eq!(t.lines, vec!["aXb".to_string()]);
        assert_eq!(t.col, 2);

        // Enter splits the line at the cursor.
        t.newline();
        assert_eq!(t.lines, vec!["aX".to_string(), "b".to_string()]);
        assert_eq!((t.row, t.col), (1, 0));

        // Backspace at column 0 joins with the previous line.
        t.backspace();
        assert_eq!(t.lines, vec!["aXb".to_string()]);
        assert_eq!((t.row, t.col), (0, 2));
    }

    #[test]
    fn from_text_and_text_round_trip() {
        let t = TextArea::from_text("one\ntwo\nthree");
        assert_eq!(t.lines.len(), 3);
        assert_eq!(t.text(), "one\ntwo\nthree");
        assert!(!TextArea::from_lines(Vec::new()).lines.is_empty()); // never empty
    }
}
