//! Alignment visualization command (`chatter show-alignment`).
//!
//! Entry point for the `show-alignment` subcommand. Loads the file via
//! [`load::load_alignment_context`], prints any validation diagnostics first,
//! then delegates to [`render::render_alignments`] for per-utterance tier
//! display. Supports filtering to a single tier (`--tier mor`) and compact
//! output.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod load;
mod render;

use std::path::PathBuf;
use tracing::{Level, error, info, span};

use crate::cli::AlignmentTier;
use crate::output::print_errors;

/// How much of each alignment `show-alignment` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignmentView {
    /// Every aligned pair on its own line, with headings and errors.
    Full,
    /// One line per alignment and an error count (`--compact`).
    Compact,
}

impl crate::cli::FlagMode for AlignmentView {
    const LONG: &'static str = "compact";
    const HELP: &'static str = "Compact output (one line per alignment)";
    const SHORT: Option<char> = Some('c');
    fn absent() -> Self {
        Self::Full
    }
    fn present() -> Self {
        Self::Compact
    }
}

/// Show alignment visualization for debugging.
///
/// The CLI emits this view to help developers compare `%mor`, `%pho`, and `%gra` tiers against the cleaned
/// Main Tier transcript. Validation diagnostics are displayed first (per the CHAT manual’s Main Tier and
/// Dependent Tier sections) so the developer can see why alignment may have failed, and then the `render`
/// modules produce a tier-specific tabular view of aligned words and features. The view can operate in
/// compact mode or limited to a specific `AlignmentTier` filter to keep attention on the relevant CHAT tiers.
pub fn show_alignment(input: &PathBuf, tier_filter: Option<AlignmentTier>, view: AlignmentView) {
    let _span = span!(Level::INFO, "show_alignment", ?input).entered();
    info!("Showing alignment for {:?}", input);

    let context = match load::load_alignment_context(input) {
        Ok(ctx) => ctx,
        Err(err) => {
            error!("{}", err);
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };

    let mut had_validation_errors = false;
    if !context.validation_errors.is_empty() {
        had_validation_errors = true;
        eprintln!(
            "⚠ Alignment validation found {} issue(s) in {}",
            context.validation_errors.len(),
            input.display()
        );
        print_errors(input, &context.content, &context.validation_errors);
    }

    render::render_alignments(
        input,
        &context.chat_file,
        tier_filter,
        view,
        had_validation_errors,
    );
}
