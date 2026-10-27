//! Speaker codes admitted for insertion at every canonical CHAT token site.
//!
//! Parsed models retain malformed `SpeakerCode`s for diagnostics. Transforms
//! introducing a new code need this stronger, non-deserializable capability.

use talkbank_model::{ErrorCollector, Location, ParseError, SpeakerCode};

#[derive(Debug, Clone)]
pub(crate) struct SpeakerToken(SpeakerCode);

/// Why a newly introduced speaker cannot be represented as a CHAT token.
#[derive(Debug, thiserror::Error)]
pub enum SpeakerTokenError {
    /// A delimiter, empty value or character outside the speaker grammar.
    #[error("speaker token must be nonempty ASCII letters, digits, _, +, - or apostrophe")]
    Unspellable,
    /// The canonical model validator rejected the code; preserve its findings.
    #[error("speaker token violates speaker-code constraints: {0:?}")]
    Invalid(Vec<ParseError>),
}

impl SpeakerToken {
    pub(crate) fn admit(code: SpeakerCode) -> Result<Self, SpeakerTokenError> {
        let text = code.as_str();
        if text.is_empty()
            || !text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'+' | b'-' | b'\''))
        {
            return Err(SpeakerTokenError::Unspellable);
        }
        let errors = ErrorCollector::new();
        talkbank_model::validation::check_speaker_id(text, Location::Unlocated, &errors);
        if !errors.is_empty() {
            return Err(SpeakerTokenError::Invalid(errors.into_vec()));
        }
        Ok(Self(code))
    }

    pub(crate) fn as_code(&self) -> &SpeakerCode {
        &self.0
    }
}
