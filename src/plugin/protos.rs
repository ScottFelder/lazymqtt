//! Protobuf schema mapping + runtime compilation for the `protobuf-view` plugin.
//!
//! Each mapping binds a topic filter to a self-contained `.proto` source and the
//! fully-qualified message type to decode payloads as. Mappings are stored per
//! connection at `<plugins>/protos/<connection-id>.json`:
//!
//! ```json
//! { "mappings": [
//!     { "topic": "sensors/#",
//!       "message_type": "sensors.Reading",
//!       "proto": "syntax = \"proto3\";\npackage sensors;\nmessage Reading { double temp = 1; }" }
//! ] }
//! ```
//!
//! Compilation is pure-Rust: `protox` parses/links the `.proto` (no external
//! `protoc`) into a `prost_reflect::DescriptorPool`, from which we resolve the
//! chosen `MessageDescriptor`. The plugin then decodes payload bytes against it
//! with `prost_reflect::DynamicMessage`.

use prost_reflect::MessageDescriptor;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// One topic-filter → (`.proto`, message type) binding.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProtoMapping {
    pub topic: String,
    pub message_type: String,
    pub proto: String,
}

impl ProtoMapping {
    /// One-line summary for the editor list (the message type it decodes as).
    pub fn summary(&self) -> String {
        if self.message_type.trim().is_empty() {
            "(no message type)".to_string()
        } else {
            format!("→ {}", self.message_type.trim())
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ProtoFile {
    #[serde(default)]
    mappings: Vec<ProtoMapping>,
}

fn protos_path(dir: &Path, connection_id: &str) -> PathBuf {
    dir.join("protos").join(format!("{connection_id}.json"))
}

/// Load a connection's proto mappings (empty if missing or unparseable).
pub fn load(dir: &Path, connection_id: &str) -> Vec<ProtoMapping> {
    fs::read_to_string(protos_path(dir, connection_id))
        .ok()
        .and_then(|s| serde_json::from_str::<ProtoFile>(&s).ok())
        .map(|f| f.mappings)
        .unwrap_or_default()
}

/// Persist a connection's proto mappings to `<dir>/protos/<id>.json`.
pub fn save(dir: &Path, connection_id: &str, mappings: &[ProtoMapping]) -> std::io::Result<()> {
    fs::create_dir_all(dir.join("protos"))?;
    let file = ProtoFile {
        mappings: mappings.to_vec(),
    };
    let json = serde_json::to_string_pretty(&file).unwrap_or_default();
    fs::write(protos_path(dir, connection_id), json)
}

/// Compile one mapping's `.proto` and resolve its message type, returning a
/// ready-to-decode `MessageDescriptor`. On failure returns a human-readable
/// error (syntax error or unknown message type) for the editor to show.
///
/// The source is materialized to a scratch file under
/// `<dir>/protos/.build/<connection-id>/<slot>/` so protox's file resolver (and
/// any co-located imports) work; `slot` keeps concurrent mappings isolated.
pub fn compile(
    dir: &Path,
    connection_id: &str,
    slot: usize,
    mapping: &ProtoMapping,
) -> Result<MessageDescriptor, String> {
    let message_type = mapping.message_type.trim();
    if message_type.is_empty() {
        return Err("message type is required".to_string());
    }
    if mapping.proto.trim().is_empty() {
        return Err("proto schema is empty".to_string());
    }

    let build_dir = dir
        .join("protos")
        .join(".build")
        .join(connection_id)
        .join(slot.to_string());
    fs::create_dir_all(&build_dir).map_err(|e| format!("scratch dir: {e}"))?;
    let file_name = "schema.proto";
    fs::write(build_dir.join(file_name), &mapping.proto)
        .map_err(|e| format!("write proto: {e}"))?;

    let mut compiler = protox::Compiler::new([&build_dir]).map_err(|e| format!("compiler: {e}"))?;
    compiler.open_file(file_name).map_err(|e| format!("{e}"))?;
    let pool = compiler.descriptor_pool();
    pool.get_message_by_name(message_type)
        .ok_or_else(|| format!("message type '{message_type}' not found in schema"))
}
