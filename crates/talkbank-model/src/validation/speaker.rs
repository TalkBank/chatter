//! Speaker-ID validation helpers.
//!
//! This module provides lightweight character-level checks shared by parser and
//! validation flows that need to reject obviously malformed speaker IDs early.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Speaker_ID>

use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};

/// Maximum number of Unicode scalar values in a CHAT speaker ID.
const MAX_SPEAKER_ID_LENGTH: usize = 7;

/// Result of the one authoritative structural assessment of a speaker code.
///
/// Parsed models must retain malformed codes so diagnostics can quote and
/// round-trip recoverable source. A total state enum therefore fits better
/// than pretending every `SpeakerCode` is already valid.
enum SpeakerCodeSyntax {
    /// The code satisfies all structural invariants.
    Valid,
    /// At least one invariant failed; private fields prevent callers from
    /// manufacturing findings independently of [`assess_speaker_code`].
    Invalid(InvalidSpeakerCode),
}

/// Producer-issued evidence that a speaker code is structurally invalid.
enum InvalidSpeakerCode {
    /// Only the length invariant failed.
    TooLong { character_count: usize },
    /// Only the delimiter/whitespace invariant failed.
    ReservedCharacter { character: char },
    /// Both independent invariants failed and both diagnostics are owed.
    TooLongWithReservedCharacter {
        character_count: usize,
        character: char,
    },
}

/// Classify a retained speaker code without emitting diagnostics.
fn assess_speaker_code(speaker: &str) -> SpeakerCodeSyntax {
    let character_count = speaker.chars().count();
    let too_long = (character_count > MAX_SPEAKER_ID_LENGTH).then_some(character_count);
    let reserved_character = has_invalid_speaker_chars(speaker);

    match (too_long, reserved_character) {
        (None, None) => SpeakerCodeSyntax::Valid,
        (Some(character_count), None) => {
            SpeakerCodeSyntax::Invalid(InvalidSpeakerCode::TooLong { character_count })
        }
        (None, Some(character)) => {
            SpeakerCodeSyntax::Invalid(InvalidSpeakerCode::ReservedCharacter { character })
        }
        (Some(character_count), Some(character)) => {
            SpeakerCodeSyntax::Invalid(InvalidSpeakerCode::TooLongWithReservedCharacter {
                character_count,
                character,
            })
        }
    }
}

/// Report the canonical E307 findings for one retained speaker code.
///
/// `span` is the location in the original file. Diagnostic context uses a
/// code-local span because its `source_text` is only `speaker`, not the full
/// file.
pub(crate) fn check_speaker_id(speaker: &str, span: Span, errors: &impl ErrorSink) {
    let SpeakerCodeSyntax::Invalid(invalid) = assess_speaker_code(speaker) else {
        return;
    };
    let local_span = Span::from_usize(0, speaker.len());

    let report_too_long = |character_count| {
        errors.report(ParseError::new(
            ErrorCode::InvalidSpeaker,
            Severity::Error,
            SourceLocation::new(span),
            ErrorContext::new(speaker, local_span, speaker),
            format!(
                "Speaker ID '{}' exceeds maximum length of {} characters (has {})",
                speaker, MAX_SPEAKER_ID_LENGTH, character_count
            ),
        ));
    };

    let report_reserved_character = |invalid_char| {
        errors.report(ParseError::new(
            ErrorCode::InvalidSpeaker,
            Severity::Error,
            SourceLocation::new(span),
            ErrorContext::new(speaker, local_span, speaker),
            format!(
                "Speaker ID '{}' contains invalid character '{}'. Speaker IDs cannot contain colon (:) or whitespace",
                speaker, invalid_char
            ),
        ));
    };

    match invalid {
        InvalidSpeakerCode::TooLong { character_count } => report_too_long(character_count),
        InvalidSpeakerCode::ReservedCharacter { character } => report_reserved_character(character),
        InvalidSpeakerCode::TooLongWithReservedCharacter {
            character_count,
            character,
        } => {
            report_too_long(character_count);
            report_reserved_character(character);
        }
    }
}

/// Returns the first invalid speaker-ID character, if any.
///
/// Invalid characters:
/// - Colon (:) - reserved as speaker ID delimiter
/// - Whitespace (space, tab, newline, etc.)
///
/// All other characters are accepted: lowercase, uppercase, digits, punctuation, Unicode, etc.
/// This lenient approach supports international corpora and various naming conventions.
/// Returning the first offending character allows callers to produce targeted
/// diagnostics without re-scanning the whole identifier.
pub(crate) fn has_invalid_speaker_chars(speaker: &str) -> Option<char> {
    speaker.chars().find(|c| *c == ':' || c.is_whitespace())
}
