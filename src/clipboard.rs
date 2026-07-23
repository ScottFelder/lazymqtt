//! System clipboard access via the platform's CLI utility.
//!
//! We shell out rather than pull in a clipboard crate: it keeps the binary
//! dependency-free and matches the "single-binary TUI" spirit. `pbcopy` covers
//! macOS; the Wayland/X11 helpers are tried in turn on Linux.

use anyhow::{anyhow, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// Copy `text` to the system clipboard. Tries each known utility in order and
/// returns the last error if none are available.
pub fn copy(text: &str) -> Result<()> {
    const CANDIDATES: &[(&str, &[&str])] = &[
        ("pbcopy", &[]),
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ];

    let mut last_err = None;
    for (cmd, args) in CANDIDATES {
        match try_copy(cmd, args, text) {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("no clipboard utility found")))
}

fn try_copy(cmd: &str, args: &[&str], text: &str) -> Result<()> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow!("{cmd}: no stdin"))?
        .write_all(text.as_bytes())?;
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("{cmd} exited with {status}"))
    }
}
