//! Color theming: a set of named color roles the whole UI reads from, a few
//! built-in presets, and persistence to `~/.config/lazymqtt/theme.json`.
//!
//! A [`Theme`] stores one *spec* string per role (a color name like `cyan` or a
//! `#rrggbb` hex value); [`Theme::palette`] resolves those specs into ratatui
//! [`Color`]s for rendering. Keeping the on-disk form as strings makes the file
//! hand-editable and the in-app editor a matter of typing a new spec.

use ratatui::style::Color;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The color roles, in editor order: `(key, human label)`. The key is the field
/// name used in `theme.json`; the array index addresses the role everywhere
/// else (see the `I_*` constants).
pub const ROLES: [(&str, &str); 16] = [
    ("accent", "Accent (titles, highlights)"),
    ("dim", "Dim (borders, hints, meta)"),
    ("text", "Primary text"),
    ("selection_fg", "Selected-row text"),
    ("ok", "OK / connected"),
    ("info", "Info"),
    ("warn", "Warning"),
    ("error", "Error / disconnected"),
    ("payload", "Payload text"),
    ("json_key", "JSON key"),
    ("json_string", "JSON string"),
    ("json_number", "JSON number"),
    ("json_literal", "JSON literal"),
    ("json_punctuation", "JSON punctuation"),
    ("json_plain", "JSON plain"),
    ("status_bar_bg", "Status bar background"),
];

pub const ROLE_COUNT: usize = ROLES.len();

// Role indices — must line up with `ROLES`.
const I_ACCENT: usize = 0;
const I_DIM: usize = 1;
const I_TEXT: usize = 2;
const I_SELECTION_FG: usize = 3;
const I_OK: usize = 4;
const I_INFO: usize = 5;
const I_WARN: usize = 6;
const I_ERROR: usize = 7;
const I_PAYLOAD: usize = 8;
const I_JSON_KEY: usize = 9;
const I_JSON_STRING: usize = 10;
const I_JSON_NUMBER: usize = 11;
const I_JSON_LITERAL: usize = 12;
const I_JSON_PUNCTUATION: usize = 13;
const I_JSON_PLAIN: usize = 14;
const I_STATUS_BAR_BG: usize = 15;

/// A theme: one color spec (name or `#rrggbb`) per role, addressed by index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    specs: [String; ROLE_COUNT],
}

impl Default for Theme {
    fn default() -> Self {
        // The classic LazyMQTT look (cyan accent, terminal-native names so it
        // adapts to the user's palette; JSON colors match the shipped scheme).
        Self::from_specs([
            "cyan",     // accent
            "darkgray", // dim
            "white",    // text
            "black",    // selection_fg
            "green",    // ok
            "cyan",     // info
            "yellow",   // warn
            "red",      // error
            "green",    // payload
            "#7aa2f7",  // json_key
            "green",    // json_string
            "yellow",   // json_number
            "magenta",  // json_literal
            "white",    // json_punctuation
            "gray",     // json_plain
            "#141419",  // status_bar_bg
        ])
    }
}

impl Theme {
    fn from_specs(specs: [&str; ROLE_COUNT]) -> Self {
        Self {
            specs: specs.map(|s| s.to_string()),
        }
    }

    /// The spec string for role `index` (empty if out of range).
    pub fn spec(&self, index: usize) -> &str {
        self.specs.get(index).map(String::as_str).unwrap_or("")
    }

    /// Set role `index`'s spec.
    pub fn set_spec(&mut self, index: usize, spec: String) {
        if let Some(slot) = self.specs.get_mut(index) {
            *slot = spec;
        }
    }

    /// Resolve every spec into a ratatui palette for rendering.
    pub fn palette(&self) -> Palette {
        let c = |i: usize| parse_color(&self.specs[i]);
        Palette {
            accent: c(I_ACCENT),
            dim: c(I_DIM),
            text: c(I_TEXT),
            selection_fg: c(I_SELECTION_FG),
            ok: c(I_OK),
            info: c(I_INFO),
            warn: c(I_WARN),
            error: c(I_ERROR),
            payload: c(I_PAYLOAD),
            json_key: c(I_JSON_KEY),
            json_string: c(I_JSON_STRING),
            json_number: c(I_JSON_NUMBER),
            json_literal: c(I_JSON_LITERAL),
            json_punctuation: c(I_JSON_PUNCTUATION),
            json_plain: c(I_JSON_PLAIN),
            status_bar_bg: c(I_STATUS_BAR_BG),
        }
    }

    /// Load the saved theme, falling back to the default when there's no file
    /// (or it can't be read). Unknown keys are ignored and missing roles keep
    /// their default, so the format tolerates edits and version drift.
    pub fn load() -> Self {
        let mut theme = Theme::default();
        let Ok(text) = std::fs::read_to_string(theme_path()) else {
            return theme;
        };
        if let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(&text) {
            for (i, (key, _)) in ROLES.iter().enumerate() {
                if let Some(spec) = map.get(*key) {
                    theme.specs[i] = spec.clone();
                }
            }
        }
        theme
    }

    /// Persist this theme to `theme.json` as a `role -> spec` object.
    pub fn save(&self) -> std::io::Result<()> {
        let map: BTreeMap<&str, &str> = ROLES
            .iter()
            .enumerate()
            .map(|(i, (key, _))| (*key, self.specs[i].as_str()))
            .collect();
        let path = theme_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&map).unwrap_or_default();
        std::fs::write(path, json)
    }
}

/// Ratatui colors resolved from a [`Theme`], read by the renderer.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub accent: Color,
    pub dim: Color,
    pub text: Color,
    pub selection_fg: Color,
    pub ok: Color,
    pub info: Color,
    pub warn: Color,
    pub error: Color,
    pub payload: Color,
    pub json_key: Color,
    pub json_string: Color,
    pub json_number: Color,
    pub json_literal: Color,
    pub json_punctuation: Color,
    pub json_plain: Color,
    pub status_bar_bg: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Theme::default().palette()
    }
}

/// The built-in presets shown at the top of the theme editor.
pub fn builtins() -> Vec<(&'static str, Theme)> {
    vec![
        ("Default", Theme::default()),
        (
            "Dracula",
            Theme::from_specs([
                "#bd93f9", "#6272a4", "#f8f8f2", "#282a36", "#50fa7b", "#8be9fd", "#f1fa8c",
                "#ff5555", "#50fa7b", "#8be9fd", "#f1fa8c", "#bd93f9", "#ff79c6", "#f8f8f2",
                "#6272a4", "#21222c",
            ]),
        ),
        (
            "Nord",
            Theme::from_specs([
                "#88c0d0", "#4c566a", "#e5e9f0", "#2e3440", "#a3be8c", "#81a1c1", "#ebcb8b",
                "#bf616a", "#a3be8c", "#88c0d0", "#a3be8c", "#b48ead", "#d08770", "#e5e9f0",
                "#4c566a", "#2e3440",
            ]),
        ),
        (
            "Gruvbox Dark",
            Theme::from_specs([
                "#8ec07c", "#665c54", "#ebdbb2", "#282828", "#b8bb26", "#83a598", "#fabd2f",
                "#fb4934", "#b8bb26", "#83a598", "#b8bb26", "#d3869b", "#fe8019", "#ebdbb2",
                "#928374", "#1d2021",
            ]),
        ),
        (
            "Solarized Dark",
            Theme::from_specs([
                "#2aa198", "#586e75", "#93a1a1", "#002b36", "#859900", "#268bd2", "#b58900",
                "#dc322f", "#859900", "#268bd2", "#2aa198", "#d33682", "#cb4b16", "#93a1a1",
                "#586e75", "#002b36",
            ]),
        ),
    ]
}

/// Parse a color spec: `#rrggbb` hex, a standard ANSI color name, or
/// `default`/`reset` for the terminal's default. Anything unrecognized falls
/// back to the terminal default so a typo never panics.
pub fn parse_color(spec: &str) -> Color {
    let s = spec.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                return Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
            }
        }
        return Color::Reset;
    }
    match s.to_ascii_lowercase().replace(['_', '-', ' '], "").as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "white" => Color::White,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "default" | "reset" | "" => Color::Reset,
        _ => Color::Reset,
    }
}

fn theme_path() -> PathBuf {
    crate::paths::config_dir().join("theme.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_and_names() {
        assert_eq!(parse_color("#7aa2f7"), Color::Rgb(122, 162, 247));
        assert_eq!(parse_color("cyan"), Color::Cyan);
        assert_eq!(parse_color("Dark Gray"), Color::DarkGray);
        assert_eq!(parse_color("default"), Color::Reset);
        assert_eq!(parse_color("bogus"), Color::Reset); // no panic on junk
        assert_eq!(parse_color("#xyz"), Color::Reset);
    }

    #[test]
    fn roles_and_indices_line_up() {
        assert_eq!(ROLES.len(), ROLE_COUNT);
        assert_eq!(ROLES[I_ACCENT].0, "accent");
        assert_eq!(ROLES[I_STATUS_BAR_BG].0, "status_bar_bg");
    }

    #[test]
    fn set_spec_updates_palette() {
        let mut t = Theme::default();
        t.set_spec(I_ACCENT, "#010203".into());
        assert_eq!(t.palette().accent, Color::Rgb(1, 2, 3));
    }

    #[test]
    fn builtins_are_complete() {
        for (_, theme) in builtins() {
            assert!(theme.specs.iter().all(|s| !s.is_empty()));
        }
    }
}
