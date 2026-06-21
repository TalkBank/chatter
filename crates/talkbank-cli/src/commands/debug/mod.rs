//! Debug subcommands for CHAT file inspection.

mod fix_s;
mod linker;
mod overlap;
mod sanitize;

pub use fix_s::*;
pub use linker::*;
pub use overlap::*;
pub use sanitize::*;

use std::path::PathBuf;
use talkbank_transform::validation_runner::is_chat_transcript_path;

pub(super) fn pct(n: usize, total: usize) -> String {
    if total == 0 {
        "0%".to_owned()
    } else {
        format!("{:.1}%", n as f64 / total as f64 * 100.0)
    }
}

/// Recursively collect .cha files from paths.
pub(super) fn collect_cha_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            collect_recursive(p, &mut files);
        } else if is_chat_transcript_path(p) {
            files.push(p.clone());
        }
    }
    files.sort();
    files
}

/// Print a user-facing error and exit non-zero.
pub(super) fn die(msg: &str) -> ! {
    eprintln!("ERROR: {msg}");
    std::process::exit(1);
}

pub(super) fn collect_recursive(dir: &PathBuf, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_recursive(&path, files);
            } else if is_chat_transcript_path(&path) {
                files.push(path);
            }
        }
    }
}
