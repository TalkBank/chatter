//! `chatter debug sanitize`, protected-corpus redaction.

use std::io::Write;
use std::path::Path;

use super::*;

/// Sanitize a single CHAT file and write the result to `output_path` (or
/// stdout when `None`).
///
/// Implements `chatter debug sanitize`. Runs the strict sanitization
/// policy from `talkbank-transform::redact`. See
/// `talkbank/docs/protected-corpus-debugging-workflow.md` for context.
///
/// A file that parses with diagnostics but still builds a model (a
/// healthy region alongside a malformed one) is sanitized anyway: the
/// diagnostics are reported on stderr as a warning, not treated as fatal.
/// Only a file that cannot build a model at all (see
/// [`talkbank_parser::ParseProduct::Unbuildable`]) aborts the run.
pub fn run_sanitize(input: &Path, output_path: Option<&Path>) {
    let source = std::fs::read_to_string(input)
        .unwrap_or_else(|e| die(&format!("cannot read {}: {e}", input.display())));
    let parser = talkbank_parser::TreeSitterParser::new()
        .unwrap_or_else(|e| die(&format!("parser initialization failed: {e:?}")));
    let Some(parsed) = parse_or_report(&parser, input, &source) else {
        std::process::exit(1);
    };

    let policy = talkbank_transform::redact::SanitizationPolicy::strict();
    let sanitized = talkbank_transform::redact::sanitize(parsed, &policy)
        .unwrap_or_else(|e| die(&format!("sanitize failed for {}: {e}", input.display())));
    let chat_text = sanitized.to_chat_string();

    match output_path {
        Some(path) => std::fs::write(path, &chat_text)
            .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", path.display()))),
        None => std::io::stdout()
            .lock()
            .write_all(chat_text.as_bytes())
            .unwrap_or_else(|e| die(&format!("cannot write to stdout: {e}"))),
    }
}
