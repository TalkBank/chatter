//! Debug subcommands for CHAT file inspection.

mod fix_s;
mod join_retrace;
mod linker;
mod overlap;
mod sanitize;

pub use fix_s::*;
pub use join_retrace::*;
pub use linker::*;
pub use overlap::*;
pub use sanitize::*;

use std::path::{Path, PathBuf};
use talkbank_transform::paths::is_chat_transcript_path;

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

/// Parse `source` (already read from `path`) and report, rather than abort,
/// on failure.
///
/// A multi-file command must not let one unparsable file kill the whole
/// run: that was the `fix-s`/`join-retrace` defect (`die()` on the first
/// bad file aborted every remaining path argument). A
/// [`talkbank_parser::ParseProduct::Built`] hands back its `ChatFile`
/// unconditionally, since a document that needed recovery is invalid but
/// not empty; its diagnostics are reported as a warning rather than
/// dropped. A [`talkbank_parser::ParseProduct::Unbuildable`] is reported
/// as an error and this returns `None`, so the caller can `continue` to
/// the next file.
pub(super) fn parse_or_report(
    parser: &talkbank_parser::TreeSitterParser,
    path: &Path,
    source: &str,
) -> Option<talkbank_model::model::ChatFile> {
    match parser.parse_chat_file(source) {
        talkbank_parser::ParseProduct::Built { file, diagnostics } => {
            if !diagnostics.is_empty() {
                eprintln!(
                    "WARNING: {} parse diagnostic(s) for {}; proceeding with the built model: {}",
                    diagnostics.len(),
                    path.display(),
                    talkbank_model::ParseErrors::from(diagnostics)
                );
            }
            Some(file)
        }
        talkbank_parser::ParseProduct::Unbuildable { diagnostics } => {
            eprintln!(
                "ERROR: parse failed for {}: {}",
                path.display(),
                talkbank_model::ParseErrors::from(diagnostics)
            );
            None
        }
    }
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
