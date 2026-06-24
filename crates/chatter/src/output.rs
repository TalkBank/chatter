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
use talkbank_model::{ErrorSink, ParseError};
use talkbank_transform::{RenderMode, render_diagnostics};

/// Print errors to stderr with miette formatting.
///
/// Routes through the shared [`render_diagnostics`] orchestration so the CLI and
/// the desktop GUI render identically (enhance once, then miette). Plain mode
/// produces the same per-error text this function always emitted.
pub fn print_errors(path: &Path, content: &str, errors: &[ParseError]) {
    if !errors.is_empty() {
        eprintln!("✗ Errors found in {}", path.display());
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
            [b'E', b'0'..=b'5', ..] => has_structural = true,
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
        // Print header on first error
        if self
            .header_printed
            .compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            eprintln!("✗ Errors found in {}", self.path.display());
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
