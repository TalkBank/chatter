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
    #[error("Failed to read file {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
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

/// Writes a changed or missing schema; preserves identical files and their mtime.
pub fn write_schema_file(path: &Path, schema_json: &str) -> Result<(), IoError> {
    match fs::read(path) {
        Ok(existing) if existing == schema_json.as_bytes() => return Ok(()),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(IoError::Read {
                path: path.display().to_string(),
                source,
            });
        }
    }
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

/// Identical regeneration must not invalidate include_str! consumers.
#[test]
fn unchanged_schema_preserves_modification_time() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("schema.json");
    write_schema_file(&path, "{}")?;
    let sentinel = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    fs::File::options()
        .write(true)
        .open(&path)?
        .set_times(fs::FileTimes::new().set_modified(sentinel))?;
    let before = fs::metadata(&path)?.modified()?;
    write_schema_file(&path, "{}")?;
    assert_eq!(fs::metadata(&path)?.modified()?, before);
    write_schema_file(&path, "{\"changed\":true}")?;
    assert_eq!(fs::read_to_string(&path)?, "{\"changed\":true}");
    Ok(())
}
