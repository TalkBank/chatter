//! Load, parse, and pre-validate input for alignment visualization.
//!
//! Reads the file, parses via [`TreeSitterParser`], and runs
//! [`validate_with_alignment`](talkbank_model::ChatFile::validate_with_alignment)
//! to populate the `AlignmentSet` on each utterance. The resulting
//! [`AlignmentContext`] bundles the parsed `ChatFile`, source text, and any
//! validation errors so the renderer can display aligned tiers alongside
//! diagnostics.

use std::fs;
use std::path::PathBuf;
use talkbank_model::model::TranscriptName;

use talkbank_model::ChatFile;
use talkbank_model::{ErrorCollector, ParseError};
use talkbank_parser::TreeSitterParser;

/// Parsed transcript, original text, and validation diagnostics for rendering.
pub(super) struct AlignmentContext {
    pub content: String,
    pub chat_file: ChatFile,
    pub validation_errors: Vec<ParseError>,
}

/// Read, parse, and validate a transcript before showing tier alignments.
///
/// This function mirrors the CLI’s validation path (including `%mor/%gra/%pho` alignment) so the rendered
/// output is grounded in the structured CHAT rules described in the manual. It writes the original text,
/// the parsed `ChatFile`, and any validation errors into an `AlignmentContext` so the caller can highlight
/// misalignments exactly where the main-tier content differs from the dependent tiers.
///
/// A parse that produced diagnostics but still built a model (a healthy
/// region alongside a malformed one) is not an error here: the parse
/// diagnostics are folded into `validation_errors` alongside the
/// alignment-validation errors, and the caller still gets a
/// [`ChatFile`] to render, healthy utterances included. Only a document
/// that could not build a model at all
/// ([`talkbank_parser::ParseProduct::Unbuildable`]) is a hard `Err`.
pub(super) fn load_alignment_context(input: &PathBuf) -> Result<AlignmentContext, String> {
    // Read file
    let content =
        fs::read_to_string(input).map_err(|e| format!("Error reading file {:?}: {}", input, e))?;

    // Parse file
    let parser = TreeSitterParser::new().map_err(|e| format!("Error creating parser: {}", e))?;

    let (mut chat_file, mut validation_errors) = match parser.parse_chat_file(&content) {
        talkbank_parser::ParseProduct::Built { file, diagnostics } => (file, diagnostics),
        talkbank_parser::ParseProduct::Unbuildable { diagnostics } => {
            return Err(format!(
                "Error parsing file {:?}: {}",
                input,
                talkbank_model::ParseErrors::from(diagnostics)
            ));
        }
    };

    // Compute alignments for all utterances and report validation issues,
    // appended after the parse diagnostics collected above.
    let errors = ErrorCollector::new();
    // `TranscriptName::for_path`, not `input.to_str()`. This used to pass the
    // WHOLE PATH where E531 expects the transcript's stem, so `@Media: foo`
    // was compared against `/corpus/eng/foo.cha` and could never match: every
    // media-linked transcript shown through this command reported a spurious
    // filename mismatch. The type is what surfaced it; an `Option<&str>`
    // accepted both strings equally.
    chat_file.validate_with_alignment(&errors, TranscriptName::for_path(input));
    validation_errors.extend(errors.into_vec());

    Ok(AlignmentContext {
        content,
        chat_file,
        validation_errors,
    })
}
