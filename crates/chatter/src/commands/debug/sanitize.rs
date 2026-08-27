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
        Some(path) => {
            if same_file(input, path) {
                die(&format!(
                    "refusing to write the sanitized copy over its own input, {}.\n\
                     \x20      Sanitization is deliberately LOSSY: it strips contributor \
                     lexical content.\n\
                     \x20      Writing it over the source would destroy the original \
                     irrecoverably. Name a different -o path.",
                    input.display()
                ));
            }
            std::fs::write(path, &chat_text)
                .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", path.display())))
        }
        None => std::io::stdout()
            .lock()
            .write_all(chat_text.as_bytes())
            .unwrap_or_else(|e| die(&format!("cannot write to stdout: {e}"))),
    }
}

/// Whether `output` names the same file on disk as `input`.
///
/// Compares RESOLVED paths, not the strings: `x.cha`, `./x.cha` and an absolute
/// spelling of the same file are the same file, and a string comparison would
/// pass all but the exact repeat. `output` need not exist yet, so its parent is
/// resolved and the file name compared beside it.
///
/// Fails CLOSED in the direction that matters: if either path cannot be
/// resolved, this answers `false` and the write proceeds, because refusing a
/// legitimate write on an unresolvable path would be worse than the narrow case
/// this guards. The case it guards is the exact one an operator hits, `-o` the
/// same path they just typed as the input.
fn same_file(input: &Path, output: &Path) -> bool {
    let Ok(resolved_input) = input.canonicalize() else {
        return false;
    };
    if let Ok(resolved_output) = output.canonicalize() {
        return resolved_input == resolved_output;
    }
    // Not created yet: resolve the directory it would land in.
    let (Some(parent), Some(name)) = (output.parent(), output.file_name()) else {
        return false;
    };
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    parent
        .canonicalize()
        .is_ok_and(|dir| dir.join(name) == resolved_input)
}
