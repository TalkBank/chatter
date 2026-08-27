//! Debug subcommands for CHAT file inspection.

mod fix_s;
mod join_retrace;
mod linker;
mod overlap;
mod retag_language;
mod sanitize;

pub use fix_s::*;
pub use join_retrace::*;
pub use linker::*;
pub use overlap::*;
pub use retag_language::*;
pub use sanitize::*;

use std::path::{Path, PathBuf};
use talkbank_model::WriteChat;
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

/// A transcript open for editing IN PLACE, whose model provably reproduces it.
///
/// # The operation, not just its precondition
///
/// Three commands edit a transcript in place, and all three were the same six
/// steps written out longhand: read, parse, edit, serialize, compare, write.
/// Naming only the precondition left the tail copied three times, and nothing
/// bound the check to the `fs::write` twenty lines below it. A fourth editor
/// could call the unchecked parse and write, and compile clean, which is how
/// the class reached three commands in the first place.
///
/// So this owns the verb. `fs::write` and `to_chat_string` do not appear in any
/// command in this module any more; [`Self::open`] is the only way to get one
/// of these, and [`Self::commit`] is the only way to write one out.
///
/// # Why the check is BEFORE the edit
///
/// After the edit the model no longer matches its source by design: `fix-s`
/// removes `@s:` markers, `retag-language` changes a language code, and
/// `join-retrace` merges two utterances. A did-anything-vanish test on the
/// RESULT cannot tell an intended change from a dropped region, and a first cut
/// that tried refused every legitimate rewrite all three commands make. The
/// unedited model is the only point where faithfulness is a clean question with
/// a byte-exact answer.
pub(super) struct InPlace {
    path: PathBuf,
    source: String,
    file: talkbank_model::model::ChatFile,
}

/// Whether [`InPlace::commit`] should actually touch the file.
///
/// An enum rather than a `bool` parameter because the two callers that need it
/// read as `commit(Commit::DryRun)` and `commit(Commit::Write)`, where
/// `commit(true)` says nothing at the call site. It also puts the dry run
/// through the SAME change detection as the real write, rather than leaving
/// `--dry-run` to re-derive "would this change anything" and drift.
pub(super) enum Commit {
    /// Write the file.
    Write,
    /// Report what a write would do, and touch nothing.
    DryRun,
}

/// What [`InPlace::commit`] did, so a caller counts what HAPPENED rather than
/// what it asked for.
pub(super) enum Committed {
    /// The file on disk was replaced.
    Wrote,
    /// The edit would have replaced it; [`Commit::DryRun`] stopped it.
    WouldWrite,
    /// The edit produced the bytes already there, so nothing was written and
    /// nothing would have been.
    Unchanged,
}

impl InPlace {
    /// Read and parse `path` for editing, or report why it cannot be edited.
    ///
    /// Returns `None` after reporting, so a caller `continue`s to the next file
    /// exactly as it does for an unparsable one, leaving this one untouched.
    /// One bad file must not kill a multi-file run.
    pub(super) fn open(parser: &talkbank_parser::TreeSitterParser, path: PathBuf) -> Option<Self> {
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(e) => {
                eprintln!("ERROR: cannot read {}: {e}", path.display());
                return None;
            }
        };
        let file = parse_or_report(parser, &path, &source)?;
        if !reproduces(&file, &source) {
            eprintln!(
                "ERROR: refusing to rewrite {}: parsing it does not reproduce it byte for byte,\n\
                 \x20      so an in-place edit would also write whatever the parse dropped.\n\
                 \x20      Run `chatter validate` on it, and `chatter normalize` if the \
                 difference is formatting.",
                path.display()
            );
            return None;
        }
        Some(Self { path, source, file })
    }

    /// The model, for the caller's edit.
    ///
    /// A borrow rather than an `edit(FnOnce)` wrapper: the wrapper gated
    /// nothing, marked nothing dirty and enforced no ordering (`commit`
    /// consumes `self` whether or not it ran), so all three callers paid a
    /// closure to reach a field. What enforces the sequence is that `open` is
    /// the only constructor and `commit` the only writer.
    pub(super) fn model_mut(&mut self) -> &mut talkbank_model::model::ChatFile {
        &mut self.file
    }

    /// Serialize the edited model over the source file.
    ///
    /// The ONLY `fs::write` in this module. An edit that reproduces the source
    /// writes nothing and says so, rather than rewriting identical bytes.
    pub(super) fn commit(self, mode: Commit) -> Committed {
        let rewritten = self.file.to_chat_string();
        if rewritten == self.source {
            return Committed::Unchanged;
        }
        match mode {
            Commit::DryRun => Committed::WouldWrite,
            Commit::Write => {
                if let Err(e) = std::fs::write(&self.path, &rewritten) {
                    die(&format!("cannot write {}: {e}", self.path.display()));
                }
                Committed::Wrote
            }
        }
    }

    /// The path being edited, for a caller's own reporting.
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

/// Whether serializing `file` reproduces `source` byte for byte.
///
/// Streams the comparison instead of building a second copy of the file. The
/// straightforward `file.to_chat_string() == source` allocated a whole extra
/// copy per file, in commands that recurse into directories, and on the passing
/// path that copy is byte-identical to the `source` the caller already holds.
/// This also stops at the FIRST differing byte, where the allocating form
/// serialized the whole model before noticing.
fn reproduces(file: &talkbank_model::model::ChatFile, source: &str) -> bool {
    /// A `fmt::Write` sink that consumes `rest` as the serializer produces it.
    struct CompareTo<'a> {
        rest: &'a str,
    }
    impl std::fmt::Write for CompareTo<'_> {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            match self.rest.strip_prefix(s) {
                Some(rest) => {
                    self.rest = rest;
                    Ok(())
                }
                // Aborts the walk. `write_chat` propagates the error, so the
                // serializer stops at the first divergence.
                None => Err(std::fmt::Error),
            }
        }
    }

    let mut sink = CompareTo { rest: source };
    file.write_chat(&mut sink).is_ok() && sink.rest.is_empty()
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
                // Reports what was OBSERVED and stops there: what happens next
                // differs by caller, so each caller says its own outcome.
                eprintln!(
                    "WARNING: {} parse diagnostic(s) for {}: {}",
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
