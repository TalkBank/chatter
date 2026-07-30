//! Helper functions for file collection
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::fs;
use std::path::PathBuf;

// Moved to the crate root (`crate::paths`) so it survives builds with the
// `validation-runner` feature off; re-exported here so this module's public
// surface (and `validation_runner::is_chat_transcript_path` consumers) are
// unchanged.
pub use crate::paths::is_chat_transcript_path;

/// Recursively collect all .cha files from a directory
pub(super) fn collect_cha_files(dir: &PathBuf, recursive: bool, files: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(directory = ?dir, error = %e, "Failed to read directory");
            return;
        }
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            if recursive {
                collect_cha_files(&path, recursive, files);
            }
        } else if is_chat_transcript_path(&path) {
            files.push(path);
        }
    }
}
