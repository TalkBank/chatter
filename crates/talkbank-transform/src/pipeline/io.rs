//! Filesystem-oriented pipeline helpers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use std::fs;
use std::path::Path;

use talkbank_model::ChatFile;
use talkbank_model::ParseValidateOptions;

use super::error::PipelineError;
use super::parse::parse_and_validate_named;
use talkbank_model::model::TranscriptName;
use talkbank_parser::TreeSitterParser;

/// Read a CHAT file from disk, then parse/validate using pipeline options.
///
/// # Arguments
///
/// * `path` - Path to CHAT file
/// * `options` - Parsing and validation options
///
/// # Returns
///
/// * `Ok(ChatFile)` - Successfully parsed (and validated if requested)
/// * `Err(PipelineError)` - I/O, parse, or validation errors
pub fn parse_file_and_validate(
    path: &Path,
    options: ParseValidateOptions,
) -> Result<ChatFile, PipelineError> {
    let content = fs::read_to_string(path)?;
    // Reading from disk means the transcript HAS a name, so the rules that
    // compare it against `@Media` (E531) can and must run. This used to call
    // the content-only entry point and lose the path here, which is how
    // `to-json` and every other pipeline consumer silently skipped E531.
    let parser =
        TreeSitterParser::new().map_err(|e| PipelineError::ParserCreation(format!("{e}")))?;
    parse_and_validate_named(&parser, &content, options, TranscriptName::for_path(path))
}
