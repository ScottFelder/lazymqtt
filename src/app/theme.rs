use crate::app::{App, Screen};

impl App {
    // ---- Theme editor ------------------------------------------------------

    /// Number of built-in presets shown above the color roles in the editor.
    pub fn theme_builtins_len(&self) -> usize {
        crate::theme::builtins().len()
    }

    /// Total editor rows: presets followed by the color roles.
    pub fn theme_row_count(&self) -> usize {
        self.theme_builtins_len() + crate::theme::ROLE_COUNT
    }

    /// If the selected row is a color role, its 0-based role index.
    pub fn theme_selected_role(&self) -> Option<usize> {
        self.theme_selected
            .checked_sub(self.theme_builtins_len())
            .filter(|i| *i < crate::theme::ROLE_COUNT)
    }

    pub fn open_theme(&mut self) {
        self.theme_selected = 0;
        self.theme_edit = None;
        self.screen = Screen::Theme;
    }

    /// Recompute the render palette and persist — every theme change sticks
    /// immediately (like the alerts/schema editors), so there's no separate
    /// "save" step to forget. On a write failure the change still previews but
    /// the error is surfaced.
    fn refresh_and_save(&mut self) {
        self.palette = self.theme.palette();
        if let Err(e) = self.theme.save() {
            self.error = Some(format!("theme save failed: {e}"));
        }
    }

    /// Apply the built-in preset at `index` as the theme (persisted).
    pub fn apply_theme_builtin(&mut self, index: usize) {
        if let Some((name, theme)) = crate::theme::builtins().into_iter().nth(index) {
            self.theme = theme;
            self.refresh_and_save();
            self.error
                .get_or_insert_with(|| format!("applied {name} theme"));
        }
    }

    /// Begin editing the selected color role, seeding the buffer with its spec.
    pub fn begin_theme_edit(&mut self) {
        if let Some(role) = self.theme_selected_role() {
            self.theme_edit = Some(self.theme.spec(role).to_string());
        }
    }

    /// Commit the in-progress color edit to the selected role (persisted).
    pub fn apply_theme_edit(&mut self) {
        let Some(spec) = self.theme_edit.take() else {
            return;
        };
        if let Some(role) = self.theme_selected_role() {
            self.theme.set_spec(role, spec.trim().to_string());
            self.refresh_and_save();
        }
    }

    /// Explicit save for the `s` key. Changes already persist on apply, so this
    /// is just reassurance for anyone who reaches for it.
    pub fn save_theme(&mut self) {
        match self.theme.save() {
            Ok(()) => self.error = Some("theme saved".into()),
            Err(e) => self.error = Some(format!("theme save failed: {e}")),
        }
    }
}
