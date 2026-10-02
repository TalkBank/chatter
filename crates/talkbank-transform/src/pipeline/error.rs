//! Error types and conversions for this subsystem.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

/// Errors that can occur in pipeline functions
#[derive(Debug)]
pub enum PipelineError {
    /// The tool failed without establishing CHAT validity.
    InternalFailure(talkbank_model::InternalFailure),
    /// I/O error (file reading/writing)
    Io(std::io::Error),
    /// Failed to create parser
    ParserCreation(String),
    /// Parse errors
    Parse(talkbank_model::ParseErrors),
    /// Validation errors
    Validation(Vec<talkbank_model::ParseError>),
    /// Recovered or unknown parse provenance prevented complete validation.
    IncompleteValidation(Box<talkbank_model::validation::ValidationFailure>),
    /// JSON serialization error
    JsonSerialization(String),
    /// Writing the serialized model over its source would DROP part of it.
    ///
    /// Distinct from [`PipelineError::Parse`] and
    /// [`PipelineError::Validation`] on purpose: the source may be perfectly
    /// valid CHAT and still round-trip lossily, and a caller that only
    /// refuses on invalidity will write the loss. See
    /// [`crate::Rewrite`].
    DroppedContent(super::rewrite::DroppedContent),
}

impl std::fmt::Display for PipelineError {
    /// Render a concise pipeline error summary.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::InternalFailure(failure) => write!(f, "{failure}"),
            PipelineError::Io(err) => write!(f, "I/O error: {}", err),
            PipelineError::ParserCreation(msg) => write!(f, "Parser creation failed: {}", msg),
            PipelineError::Parse(errors) => write!(f, "Parse errors: {}", errors),
            PipelineError::Validation(errors) => {
                write!(f, "Validation failed with {} errors", errors.len())
            }
            PipelineError::IncompleteValidation(failure) => write!(f, "{failure}"),
            PipelineError::JsonSerialization(msg) => {
                write!(f, "JSON serialization failed: {}", msg)
            }
            PipelineError::DroppedContent(dropped) => write!(f, "{dropped}"),
        }
    }
}

impl std::error::Error for PipelineError {}

impl PipelineError {
    /// The located diagnostics this failure carries, or `None` for a failure
    /// that has none (an I/O error, a parser that could not be created, a
    /// serialization failure, dropped content). The one place that knows
    /// which variants carry diagnostics, so no reporter can forget one: the
    /// CLI's renderer and speaker identification's recorded codes both read
    /// it, and each used to keep its own partial list.
    pub fn diagnostics(&self) -> Option<&[talkbank_model::ParseError]> {
        match self {
            PipelineError::InternalFailure(failure) => Some(failure.diagnostics()),
            PipelineError::Parse(errors) => Some(&errors.errors),
            PipelineError::Validation(errors) => Some(errors),
            PipelineError::IncompleteValidation(failure) => Some(failure.diagnostics()),
            PipelineError::Io(_)
            | PipelineError::ParserCreation(_)
            | PipelineError::JsonSerialization(_)
            | PipelineError::DroppedContent(_) => None,
        }
    }
}

impl From<std::io::Error> for PipelineError {
    /// Convert filesystem I/O errors into pipeline errors.
    fn from(err: std::io::Error) -> Self {
        PipelineError::Io(err)
    }
}
