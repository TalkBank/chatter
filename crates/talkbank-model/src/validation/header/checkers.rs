//! Field-level validators for specific header payloads.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Role_Field>

use crate::model::{BulletContent, IDHeader, ValidationTagged};
use crate::validation::check_speaker_id;
use crate::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};

use super::metadata;
use super::participant::{is_allowed_participant_role, suggest_similar_role};

/// Validates required `@ID` slots and role/age formatting rules.
pub(super) fn check_id_header(
    id_header: &IDHeader,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    if id_header.language.is_empty() {
        let err = ParseError::new(
            ErrorCode::EmptyIDLanguage,
            Severity::Error,
            DiagnosticLocation::from(location),
            ErrorContext::new("", crate::Span::new(0, 0), "id_language"),
            "ID header language field cannot be empty",
        );
        errors.report(err);
    }

    if id_header.corpus.is_empty() {
        let err = ParseError::new(
            ErrorCode::EmptyIDCorpus,
            Severity::Error,
            DiagnosticLocation::from(location),
            ErrorContext::new("", crate::Span::new(0, 0), "id_corpus"),
            "ID header corpus field (2nd field) cannot be empty: every @ID names its corpus",
        );
        errors.report(err);
    }

    if id_header.speaker.is_empty() {
        let err = ParseError::new(
            ErrorCode::EmptyIDSpeaker,
            Severity::Error,
            DiagnosticLocation::from(location),
            ErrorContext::new(
                id_header.speaker.as_str(),
                crate::Span::new(0, 0),
                "id_speaker",
            ),
            "ID header speaker field cannot be empty",
        );
        errors.report(err);
    }

    check_speaker_id(id_header.speaker.as_str(), location, errors);

    if id_header.role.is_empty() {
        let err = ParseError::new(
            ErrorCode::EmptyIDRole,
            Severity::Error,
            DiagnosticLocation::from(location),
            ErrorContext::new(id_header.role.as_str(), crate::Span::new(0, 0), "id_role"),
            "Empty role field in @ID header: the role (8th field) must not be blank",
        )
        .with_suggestion(
            "Set a valid role such as 'Target_Child', 'Mother', 'Investigator', or 'Participant'",
        );
        errors.report(err);
    } else if !is_allowed_participant_role(id_header.role.as_str()) {
        // E532: Invalid participant role
        let suggested = suggest_similar_role(id_header.role.as_str());
        let err = ParseError::new(
            ErrorCode::InvalidParticipantRole,
            Severity::Error,
            DiagnosticLocation::from(location),
            ErrorContext::new(id_header.role.as_str(), crate::Span::new(0, 0), "id_role"),
            format!("Invalid participant role: '{}'", id_header.role.as_str()),
        )
        .with_suggestion(format!("Use a valid CHAT role such as: {}", suggested));
        errors.report(err);
    }

    if let Some(ref age) = id_header.age {
        if age.has_validation_issue() {
            let raw = age.as_str();
            let err = ParseError::new(
                ErrorCode::InvalidAgeFormat,
                Severity::Error,
                DiagnosticLocation::from(location),
                ErrorContext::new(raw, crate::Span::new(0, raw.len()), "id_age"),
                format!("Age should be in format years;months.days, got: {}", raw),
            );
            errors.report(err);
        } else if age.violates_depfile_pattern() {
            // Mirrors CLAN CHECK error 34 ("Illegal date representation"):
            // depfile.cut admits exactly yy; / yy;mm. / yy;mm.dd, so every
            // other shape, one-digit month, missing trailing period,
            // single-digit day, etc., is a hard error.
            let raw = age.as_str();
            let err = ParseError::new(
                ErrorCode::InvalidAgeFormat,
                Severity::Error,
                DiagnosticLocation::from(location),
                ErrorContext::new(raw, crate::Span::new(0, raw.len()), "id_age"),
                format!(
                    "Illegal date representation: '{}'. Legal forms per CLAN depfile.cut: YY; or YY;MM. or YY;MM.DD (two-digit MM and DD)",
                    raw
                ),
            )
            .with_suggestion(
                "Run \"chstring +q +1\" to zero-pad, or edit manually: 3;0 → 3;00., 2;6 → 2;06.",
            );
            errors.report(err);
        }
    }
}

/// Validates `@Date` payload and delegates format checks.
pub(super) fn check_date_header(date: &str, location: crate::Location, errors: &impl ErrorSink) {
    if date.is_empty() {
        let err = ParseError::new(
            ErrorCode::EmptyDate,
            Severity::Error,
            DiagnosticLocation::from(location),
            ErrorContext::new(date, crate::Span::new(0, 0), "date"),
            "@Date header should not be empty",
        );
        errors.report(err);
    } else {
        metadata::check_date_format(date, location, errors, ErrorCode::InvalidDateFormat);
    }
}

/// Validates `@Birth of <CODE>` payload against the depfile date
/// template. Same component-level rules as `@Date`, but emits E545
/// so users can distinguish which header was wrong.
pub(super) fn check_birth_date_header(
    date: &str,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    if date.is_empty() {
        // Empty @Birth of is allowed (date-unknown), matching the
        // `Option<ChatDate>`-style semantics of participant fields.
        return;
    }
    metadata::check_date_format(date, location, errors, ErrorCode::InvalidBirthDateFormat);
}

/// Intentionally no-op for `@Comment` and `@Warning` payload bodies.
pub(super) fn check_comment_warning(
    _content: &BulletContent,
    _location: crate::Location,
    _errors: &impl ErrorSink,
) {
    // @Comment and @Warning headers are opaque CHAT data, not directives
    // for our validator.  Nothing to check here.
}
