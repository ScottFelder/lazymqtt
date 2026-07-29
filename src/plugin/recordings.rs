//! Shared helpers for the topic-recorder's on-disk recordings.
//!
//! Recordings live at `<plugin config dir>/recordings/<connection-id>-<label>.jsonl`
//! (one message per line). The `<connection-id>-` prefix scopes them per
//! connection; the `label` is the human-facing part shown in the picker and the
//! only piece a rename changes. Both the recorder plugin and the app-level
//! recordings picker use this module — mirroring how `alerts_rules` is shared by
//! the alerts plugin and its in-app editor.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// One recorded message: a line in a recording's JSONL file. `offset_ms` is the
/// time since the recording started; replay uses it to preserve timing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedMessage {
    pub offset_ms: u64,
    pub topic: String,
    pub payload: String,
    pub qos: u8,
    pub retain: bool,
}

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
    items.sort_by_key(|item| std::cmp::Reverse(item.0));
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

/// Parse a recording file into its messages, skipping unparseable lines.
pub fn read(path: &Path) -> Vec<RecordedMessage> {
    fs::read_to_string(path)
        .map(|s| {
            s.lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Render messages as one canonical compact-JSON line each (the on-disk format,
/// and what the in-app editor shows).
pub fn to_lines(msgs: &[RecordedMessage]) -> Vec<String> {
    msgs.iter()
        .filter_map(|m| serde_json::to_string(m).ok())
        .collect()
}

/// Parse edited text (one message per line) back into messages, validating each
/// non-empty line. On failure returns the 0-based line index and the error, so
/// the editor can point at the offending line.
pub fn parse_lines(lines: &[String]) -> Result<Vec<RecordedMessage>, (usize, String)> {
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<RecordedMessage>(line) {
            Ok(m) => out.push(m),
            Err(e) => return Err((i, e.to_string())),
        }
    }
    Ok(out)
}

/// Write messages to `conn`'s recording named `label` (one JSON line each),
/// creating the recordings dir if needed. The label is sanitized; an empty
/// result is rejected.
pub fn write_label(
    config_dir: &Path,
    conn: &str,
    label: &str,
    msgs: &[RecordedMessage],
) -> std::io::Result<()> {
    let clean = sanitize(label);
    if clean.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "empty recording name",
        ));
    }
    fs::create_dir_all(dir(config_dir))?;
    let mut file = fs::File::create(path_for(config_dir, conn, &clean))?;
    for line in to_lines(msgs) {
        writeln!(file, "{line}")?;
    }
    Ok(())
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

    fn m(offset_ms: u64, payload: &str) -> RecordedMessage {
        RecordedMessage {
            offset_ms,
            topic: "t".into(),
            payload: payload.into(),
            qos: 0,
            retain: false,
        }
    }

    #[test]
    fn to_lines_and_parse_lines_round_trip() {
        let msgs = vec![m(0, "a"), m(500, "b")];
        let lines = to_lines(&msgs);
        assert_eq!(lines.len(), 2);
        assert_eq!(parse_lines(&lines).unwrap(), msgs);
    }

    #[test]
    fn parse_lines_skips_blanks_and_reports_bad_line() {
        let good = to_lines(&[m(0, "a")])[0].clone();
        // Blank lines are ignored; a malformed line is reported by index.
        let lines = vec![good.clone(), "   ".to_string(), "not json".to_string()];
        match parse_lines(&lines) {
            Err((idx, _)) => assert_eq!(idx, 2),
            Ok(_) => panic!("expected a parse error"),
        }
    }

    #[test]
    fn list_returns_newest_recording_first() {
        let tmp =
            std::env::temp_dir().join(format!("lazymqtt-recordings-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&tmp);

        write_label(&tmp, "conn1", "older", &[m(0, "a")]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_label(&tmp, "conn1", "newer", &[m(1, "b")]).unwrap();

        let items = list(&tmp, "conn1");
        assert_eq!(
            items.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
            vec!["newer", "older"]
        );

        fs::remove_dir_all(&tmp).unwrap();
    }
}
