//! Output formatting utilities for CLI.
//!
//! These helpers centralize shared output behavior (miette-enhanced errors, progress
//! spinners, json emitters) so commands can call reuse formatting instead of reimplementing it.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use talkbank_model::{ErrorSink, ParseError, Severity};
use talkbank_transform::{RenderMode, render_diagnostics};

/// True when the set contains at least one hard error (`Severity::Error`).
///
/// A file with only warnings is valid CHAT, so the per-file headline must not
/// call it an error; this predicate keys the "errors" vs "warnings" framing on
/// the hard-error count rather than on "has any diagnostic".
pub(crate) fn has_hard_error(errors: &[ParseError]) -> bool {
    errors.iter().any(|error| error.severity == Severity::Error)
}

/// Print errors to stderr with miette formatting.
///
/// Routes through the shared [`render_diagnostics`] orchestration so the CLI and
/// the desktop GUI render identically (enhance once, then miette). Plain mode
/// produces the same per-error text this function always emitted.
pub fn print_errors(path: &Path, content: &str, errors: &[ParseError]) {
    if !errors.is_empty() {
        if has_hard_error(errors) {
            eprintln!("✗ Errors found in {}", path.display());
        } else {
            // Warnings do not make a file invalid: do not headline it as an error.
            eprintln!("⚠ Warnings in {}", path.display());
        }
        eprintln!();
    }

    for diagnostic in render_diagnostics(
        errors,
        &path.display().to_string(),
        content,
        RenderMode::Plain,
    ) {
        eprintln!("{}", diagnostic.text);
    }
}

/// Check whether a set of errors contains structural errors (E0xx-E5xx) but no
/// alignment errors (E7xx). When structural errors taint the parse, alignment
/// checks are silently skipped. This helper detects that situation so callers
/// can emit a hint telling the user to fix structural errors first.
pub fn should_show_cascading_hint(errors: &[ParseError]) -> bool {
    let mut has_structural = false;
    let mut has_alignment = false;

    for error in errors {
        let code_str = error.code.as_str();
        // Structural errors: E0xx, E1xx, E2xx, E3xx, E4xx, E5xx
        // Alignment errors: E7xx
        match code_str.as_bytes() {
            // Only a HARD structural error taints the parse and causes
            // alignment checks to be skipped; a warning-severity code in the
            // structural range does not, so it must not trigger the "fix
            // structural errors first" hint on an otherwise-valid file.
            [b'E', b'0'..=b'5', ..] if error.severity == Severity::Error => has_structural = true,
            // Any alignment diagnostic (error or warning) means alignment
            // actually ran, so the "checks were skipped" hint does not apply.
            [b'E', b'7', ..] => has_alignment = true,
            _ => {}
        }
    }

    has_structural && !has_alignment
}

/// The cascading error hint text, printed to stderr in text mode.
pub const CASCADING_HINT: &str = "  note: Some additional checks may not have run because of structural errors above.\n        Fix the structural errors first, then re-validate.";

/// ErrorSink that prints errors immediately to the terminal using miette rendering.
pub struct TerminalErrorSink {
    path: PathBuf,
    content: String,
    error_count: AtomicUsize,
    header_printed: AtomicUsize,
}

impl TerminalErrorSink {
    /// Create a terminal sink for one file's content.
    ///
    /// The sink keeps the source text so each streamed error can be enhanced
    /// with line/column context before miette rendering.
    pub fn new(path: &Path, content: &str) -> Self {
        Self {
            path: path.to_path_buf(),
            content: content.to_string(),
            error_count: AtomicUsize::new(0),
            header_printed: AtomicUsize::new(0),
        }
    }

    /// Return the number of errors streamed so far.
    pub fn error_count(&self) -> usize {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Prints single error.
    fn print_single_error(&self, error: ParseError) {
        // Print header on first diagnostic. The streaming sink only sees one
        // diagnostic at a time, so it keys the headline on THIS diagnostic's
        // severity: a run whose first (and only) diagnostics are warnings is a
        // valid file and must not be headlined as an error. (Caveat inherent to
        // streaming: if a warning streams before a later hard error, the header
        // reads "Warnings"; the batch `print_errors` path, which sees the whole
        // set at once, has no such ambiguity and is the default validate route.)
        if self
            .header_printed
            .compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            if error.severity == Severity::Error {
                eprintln!("✗ Errors found in {}", self.path.display());
            } else {
                eprintln!("⚠ Warnings in {}", self.path.display());
            }
            eprintln!();
        }

        // Same shared orchestration as batch `print_errors`; a one-element slice
        // keeps the streaming sink byte-identical to the batch path.
        for diagnostic in render_diagnostics(
            std::slice::from_ref(&error),
            &self.path.display().to_string(),
            &self.content,
            RenderMode::Plain,
        ) {
            eprintln!("{}", diagnostic.text);
        }
    }
}

impl ErrorSink for TerminalErrorSink {
    /// Stream one parse error to stderr and increment the counter.
    fn report(&self, error: ParseError) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        self.print_single_error(error);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use talkbank_model::{ErrorCode, SourceLocation};

    /// Build a diagnostic of the given severity for headline-predicate tests.
    fn diagnostic(severity: Severity) -> ParseError {
        ParseError::new(
            ErrorCode::TestError,
            severity,
            SourceLocation::from_offsets_with_position(0, 1, 1, 1),
            Option::<talkbank_model::ErrorContext>::None,
            "synthetic diagnostic for headline tests",
        )
    }

    /// A warnings-only diagnostic set must NOT be headlined as an error:
    /// warnings do not make a file invalid, so `print_errors` takes the
    /// "⚠ Warnings in" branch. This pins the seam end-to-end coverage lost
    /// when the E254 warning was retired (2026-07-15): no default-config
    /// construct currently produces a warning through `chatter validate`,
    /// so the subprocess-level test is ignored and this predicate test is
    /// the regression guard.
    #[test]
    fn warnings_only_set_is_not_a_hard_error() {
        assert!(!has_hard_error(&[diagnostic(Severity::Warning)]));
    }

    /// Any hard error in the set headlines the file as an error, even when
    /// warnings are also present.
    #[test]
    fn any_error_severity_makes_the_set_hard() {
        assert!(has_hard_error(&[
            diagnostic(Severity::Warning),
            diagnostic(Severity::Error),
        ]));
    }

    /// A warnings-only set must not trigger the "fix structural errors
    /// first" cascading hint: that hint is about hard structural errors
    /// tainting the parse.
    #[test]
    fn warnings_only_set_shows_no_cascading_hint() {
        assert!(!should_show_cascading_hint(&[diagnostic(
            Severity::Warning
        )]));
    }
}
