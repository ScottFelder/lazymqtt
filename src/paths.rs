//! Where LazyMQTT keeps its config and state on disk.
//!
//! We deliberately use the XDG layout on *every* platform —
//! `$XDG_CONFIG_HOME/lazymqtt`, falling back to `~/.config/lazymqtt` — so the
//! path is identical on macOS and Linux instead of macOS's
//! `~/Library/Application Support/…`. `connections.json` lives directly in this
//! dir; the plugins keep their state in a `plugins/` subdir.

use std::path::{Path, PathBuf};

/// The base config/state directory: `$XDG_CONFIG_HOME/lazymqtt` if set,
/// otherwise `~/.config/lazymqtt`.
pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("lazymqtt")
}

/// One-time relocation of config from the pre-existing macOS location
/// (`~/Library/Application Support/dev.lazymqtt.lazymqtt`) to the new
/// `~/.config/lazymqtt`. No-op when the new location already holds a
/// `connections.json`, when there's no legacy config to move, or on Linux
/// (which already used `~/.config`). Best-effort: any failure just leaves the
/// app to start with whatever config it can read.
///
/// The guard is the presence of `connections.json`, not of the directory: the
/// config dir gets created eagerly (e.g. the first `Config::load`), so keying on
/// the dir would let an empty one block the move.
pub fn migrate_legacy_config() {
    let new = config_dir();
    if new.join("connections.json").exists() {
        return;
    }
    let Some(old) = legacy_config_dir() else {
        return;
    };
    if old != new && old.join("connections.json").exists() {
        let _ = migrate_dir(&old, &new);
    }
}

/// Move every entry from `old` into `new` (creating `new`), skipping any that
/// already exist there, then drop `old` if it ends up empty. `old` and `new`
/// live under the same home directory, so per-entry renames stay on one
/// filesystem.
fn migrate_dir(old: &Path, new: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(new)?;
    for entry in std::fs::read_dir(old)?.flatten() {
        let dest = new.join(entry.file_name());
        if !dest.exists() {
            let _ = std::fs::rename(entry.path(), dest);
        }
    }
    // Only removes the legacy dir when nothing was left behind.
    let _ = std::fs::remove_dir(old);
    Ok(())
}

#[cfg(target_os = "macos")]
fn legacy_config_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("Library/Application Support/dev.lazymqtt.lazymqtt"))
}

#[cfg(not(target_os = "macos"))]
fn legacy_config_dir() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_dir_moves_contents_into_existing_empty_dir() {
        let base = std::env::temp_dir().join(format!("lazymqtt-mig-{}", std::process::id()));
        let old = base.join("old");
        let new = base.join("new");
        std::fs::create_dir_all(old.join("plugins")).unwrap();
        std::fs::write(old.join("connections.json"), "{}").unwrap();
        std::fs::write(old.join("plugins/config.json"), "{}").unwrap();
        std::fs::create_dir_all(&new).unwrap(); // pre-existing empty dir must not block

        migrate_dir(&old, &new).unwrap();

        assert!(new.join("connections.json").exists());
        assert!(new.join("plugins/config.json").exists());
        assert!(!old.exists(), "empty legacy dir should be removed");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn migrate_dir_never_overwrites_existing_target_files() {
        let base = std::env::temp_dir().join(format!("lazymqtt-mig2-{}", std::process::id()));
        let old = base.join("old");
        let new = base.join("new");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(old.join("connections.json"), "OLD").unwrap();
        std::fs::write(new.join("connections.json"), "NEW").unwrap();

        migrate_dir(&old, &new).unwrap();

        // The new location's file wins; the legacy one is left in place.
        assert_eq!(
            std::fs::read_to_string(new.join("connections.json")).unwrap(),
            "NEW"
        );
        assert!(old.join("connections.json").exists());

        let _ = std::fs::remove_dir_all(&base);
    }
}
