//! Shared helpers for the topic-recorder's on-disk recordings.
//!
//! Recordings live at `<plugin config dir>/recordings/<connection-id>-<label>.jsonl`
//! (one message per line). The `<connection-id>-` prefix scopes them per
//! connection; the `label` is the human-facing part shown in the picker and the
//! only piece a rename changes. Both the recorder plugin and the app-level
//! recordings picker use this module — mirroring how `alerts_rules` is shared by
//! the alerts plugin and its in-app editor.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// One stored recording for a connection.
#[derive(Debug, Clone)]
pub struct Recording {
    pub path: PathBuf,
    /// Filename minus the `<connection-id>-` prefix and `.jsonl` suffix.
    pub label: String,
    /// Number of recorded messages (non-empty lines).
    pub messages: usize,
}

fn dir(config_dir: &Path) -> PathBuf {
    config_dir.join("recordings")
}

/// The file that holds `conn`'s recording named `label`.
pub fn path_for(config_dir: &Path, conn: &str, label: &str) -> PathBuf {
    dir(config_dir).join(format!("{conn}-{label}.jsonl"))
}

/// All recordings for `conn`, newest first (by file modification time).
pub fn list(config_dir: &Path, conn: &str) -> Vec<Recording> {
    let prefix = format!("{conn}-");
    let mut items: Vec<(SystemTime, Recording)> = fs::read_dir(dir(config_dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_name()?.to_str()?.to_string();
            let label = name
                .strip_prefix(&prefix)
                .and_then(|n| n.strip_suffix(".jsonl"))?
                .to_string();
            let mtime = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let messages = count_messages(&path);
            Some((
                mtime,
                Recording {
                    path,
                    label,
                    messages,
                },
            ))
        })
        .collect();
    items.sort_by(|a, b| b.0.cmp(&a.0));
    items.into_iter().map(|(_, r)| r).collect()
}

/// The newest recording for `conn` (by modification time), if any.
pub fn newest(config_dir: &Path, conn: &str) -> Option<PathBuf> {
    list(config_dir, conn).into_iter().next().map(|r| r.path)
}

/// Rename a recording's label, keeping the connection prefix intact. The new
/// label is sanitized; an empty result is rejected.
pub fn rename(
    config_dir: &Path,
    conn: &str,
    old_label: &str,
    new_label: &str,
) -> std::io::Result<()> {
    let clean = sanitize(new_label);
    if clean.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "empty recording name",
        ));
    }
    fs::rename(
        path_for(config_dir, conn, old_label),
        path_for(config_dir, conn, &clean),
    )
}

/// Delete a recording by path.
pub fn delete(path: &Path) -> std::io::Result<()> {
    fs::remove_file(path)
}

fn count_messages(path: &Path) -> usize {
    fs::read_to_string(path)
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

/// Keep labels filesystem-safe and free of the `-`-delimited prefix ambiguity:
/// allow alphanumerics, dot, underscore and dash; collapse anything else (incl.
/// spaces and path separators) to `_`, and trim surrounding separators.
fn sanitize(label: &str) -> String {
    let mapped: String = label
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    mapped.trim_matches(|c| c == '_' || c == '.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_keeps_safe_chars_and_collapses_others() {
        assert_eq!(sanitize("morning run 1"), "morning_run_1");
        assert_eq!(sanitize("a/b\\c"), "a_b_c");
        assert_eq!(sanitize("  _trimmed_  "), "trimmed");
        assert_eq!(sanitize("keep-dots.and_dashes"), "keep-dots.and_dashes");
        assert_eq!(sanitize("***"), "");
    }

    #[test]
    fn path_for_uses_connection_prefix() {
        let p = path_for(Path::new("/cfg"), "conn1", "my-take");
        assert!(p.ends_with("recordings/conn1-my-take.jsonl"));
    }
}
