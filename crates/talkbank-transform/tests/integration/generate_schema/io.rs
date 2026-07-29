//! File output for the generated schema.
//!
//! The canonical schema lives at the workspace root `schema/<stem>.json`, which
//! `talkbank_transform::SCHEMA_JSON` embeds via `include_str!`. The test's
//! working directory is this crate's manifest dir, not the workspace root, so
//! the path is resolved from `CARGO_MANIFEST_DIR` up two levels rather than
//! relative to the CWD.

use std::fs;
use std::path::{Path, PathBuf};

/// Enum variants for IoError.
#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("Failed to write file {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
}

/// Canonical schema path for one stem (e.g. `chat-file.schema`), resolved to
/// the workspace `schema/` directory regardless of the test's CWD.
pub fn schema_path_for(schema_stem: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schema")
        .join(format!("{schema_stem}.json"))
}

/// Writes the canonical schema file.
pub fn write_schema_file(path: &Path, schema_json: &str) -> Result<(), IoError> {
    fs::write(path, schema_json).map_err(|source| IoError::Write {
        path: path.display().to_string(),
        source,
    })
}

/// Prints summary.
pub fn print_summary(path: &Path, length: usize) {
    println!("\n========== GENERATED JSON SCHEMA ==========");
    println!("Canonical: {}", path.display());
    println!("Length: {length} bytes");
    println!("==========================================\n");
}
