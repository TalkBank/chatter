//! Domain error type for sanitization failures.

use thiserror::Error;

/// Errors that can arise while sanitizing a [`ChatFile`](talkbank_model::ChatFile).
///
/// Raw unparsed morphology cannot be safely sanitized as parsed lexical data.
#[derive(Debug, Error)]
pub enum RedactError {
    /// The replacement text must satisfy the event-description grammar.
    #[error("invalid event replacement: {0}")]
    EventReplacement(#[from] talkbank_model::InvalidEventType),
    /// Recovery has no admitted freecode payload to sanitize safely.
    #[error("cannot sanitize an unparsed freecode; repair its syntax first")]
    UnparsedFreecode,
    /// Recovery has no admitted postcode payload to preserve safely.
    #[error("cannot sanitize an unparsed postcode; repair its syntax first")]
    UnparsedPostcode,
    /// Unattached annotation source has no safely admitted payload.
    #[error("cannot sanitize an annotation without a host; repair its syntax first")]
    OrphanAnnotation,
    /// Rejected ID syntax has no safely accessible demographic fields.
    #[error("cannot sanitize an unparsed ID header; repair its syntax first")]
    UnparsedID,
    /// Rejected participant syntax may contain names with no safe field boundaries.
    #[error("cannot sanitize an unparsed participant header; repair its syntax first")]
    UnparsedParticipants,
    /// Morphology could not be parsed, so its lexical fields are unknown.
    #[error("cannot sanitize unparsed morphology; repair or regenerate the MOR tier first")]
    UnparsedMorphology,
    /// Grammar recovery contains raw text whose lexical fields are unknown.
    #[error("cannot sanitize unparsed grammar; repair or regenerate the GRA tier first")]
    UnparsedGrammar,
    /// A `WordContents` SmallVec was empty when the placeholder
    /// generator expected at least one segment to replace.
    #[error("empty word content at utterance {utterance_index}, word {word_index}")]
    EmptyWordContent {
        /// Zero-based utterance index in document order.
        utterance_index: usize,
        /// Zero-based word index within the utterance (post-walk-words).
        word_index: usize,
    },
}

/// Refusal before source-bound sanitization can produce output.
#[derive(Debug, Error)]
pub enum SourceSanitizationError {
    /// A producer fault is distinct from malformed CHAT.
    #[error(transparent)]
    Internal(#[from] talkbank_model::InternalFailure),
    /// No model was available; diagnostics are forwarded to the caller's sink.
    #[error("source parsing could not build a document")]
    Unbuildable,
    /// The unedited serialization lost or reordered source content.
    #[error("cannot sanitize a partial source model: {0}")]
    DroppedContent(#[from] crate::DroppedContent),
    /// Source survived, but its lexical fields cannot be safely redacted.
    #[error(transparent)]
    Redaction(#[from] RedactError),
}
