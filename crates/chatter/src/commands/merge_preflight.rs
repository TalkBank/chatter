//! Pre-flight input validation shared by the merge-family commands
//! (`chatter pipeline` and `chatter batch`).
//!
//! Policy (decided 2026-06-10): the merge tooling validates every
//! input as valid CHAT *before* attempting any merge work. Invalid
//! CHAT is cleaned upstream, never fed to the merge. The merge tool
//! refuses invalid input rather than silently merging it. `chatter
//! validate` is the authority on CHAT validity, so the gate runs the
//! same structural + alignment validation that `chatter validate`
//! runs by default (via [`ParseValidateOptions::with_alignment`]),
//! making "valid enough to merge" identical to "valid CHAT".

use std::fs;
use std::path::{Path, PathBuf};

use talkbank_model::ParseValidateOptions;
use talkbank_transform::parse_and_validate;

use crate::exit_codes::EXIT_PRECONDITION;

/// One input that failed the pre-flight gate, with a human-readable
/// reason (unreadable file, parse error, or validation error). The
/// operator uses `path` to locate the file and `reason` to know what
/// to clean.
pub struct InvalidInput {
    /// The offending input file.
    pub path: PathBuf,
    /// Why it is not usable: read failure or the CHAT errors found.
    pub reason: String,
}

/// Validate already-read CHAT content with full `chatter validate`
/// semantics. `Ok(())` when valid; `Err(reason)` carrying the CHAT
/// errors otherwise.
pub fn validate_chat_content(content: &str) -> Result<(), String> {
    // The gate's contract is "input passes `chatter validate`" (the
    // CHAT-validity authority), so it must run exactly what `chatter
    // validate` runs by default: structural validation PLUS cross-tier
    // alignment checks. `.with_alignment()` is that default
    // (non-`--skip-alignment`) level; anything weaker could pass an
    // input the authority would reject.
    let options = ParseValidateOptions::default().with_alignment();
    match parse_and_validate(content, options) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{e}")),
    }
}

/// Validate a CHAT file on disk. Read failures are themselves a
/// gate failure (an input we cannot read is not a usable input).
pub fn validate_chat_input(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|e| format!("cannot read file: {e}"))?;
    validate_chat_content(&content)
}

/// The pre-flight gate's terminal action: if any input failed, report
/// every offending file to stderr in a stable, operator-facing format
/// and exit with the precondition code; otherwise return so the caller
/// proceeds. Folding the report + exit here keeps `pipeline` and
/// `batch` emitting identical diagnostics and sharing one exit code,
/// rather than repeating the `is_empty` / report / exit tail at each
/// call site.
pub fn abort_if_any_invalid(invalid: &[InvalidInput]) {
    if invalid.is_empty() {
        return;
    }
    eprintln!(
        "Refusing to merge: {} input file(s) failed CHAT validation. \
         Clean them to valid CHAT before merging:",
        invalid.len()
    );
    for InvalidInput { path, reason } in invalid {
        eprintln!("  ✗ {}: {}", path.display(), reason);
    }
    std::process::exit(EXIT_PRECONDITION);
}
