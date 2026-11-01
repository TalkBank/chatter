//! Field-level validators for specific header payloads.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Role_Field>

use crate::model::{EmptyHeader, IDHeader, PlainKind, QualifiedKind, TextKind, ValidationTagged};
use crate::validation::check_speaker_id;
use crate::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};

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

/// The two headers whose payload is a CHAT date, each with its own code.
///
/// Pairs the error code with the header that is named in the message, so a
/// birth-date diagnostic can no longer reuse the `@Date` wording (or code).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DateHeader {
    /// `@Date`, reported as E518.
    Date,
    /// `@Birth of <SPK>`, reported as E545.
    BirthOf,
}

impl DateHeader {
    /// The code a malformed date in this header reports.
    pub(super) fn error_code(self) -> ErrorCode {
        match self {
            Self::Date => ErrorCode::InvalidDateFormat,
            Self::BirthOf => ErrorCode::InvalidBirthDateFormat,
        }
    }

    /// The header named in the diagnostic, without the `@`, from the
    /// header-kind label table.
    pub(super) fn header_name(self) -> &'static str {
        match self {
            Self::Date => TextKind::Date.label(),
            Self::BirthOf => QualifiedKind::Birth.label(),
        }
    }
}

/// E553: a colon-bearing header with nothing after its colon.
///
/// One rule for every header kind, reported at the header. The message names
/// the header as written and says what to do: omit it, or, for a gem header,
/// use its bare form, which has no colon.
pub(super) fn check_empty_header_content(
    empty: &EmptyHeader,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    let written = empty.prefix();
    let remedy = match empty {
        // A gem header has a legal bare form, which has no colon.
        EmptyHeader::Plain {
            kind: PlainKind::Gem(_),
            ..
        } => format!("write the bare {written} with no colon, or omit the header"),
        EmptyHeader::Plain {
            kind: PlainKind::Text(_) | PlainKind::Structured(_),
            ..
        }
        | EmptyHeader::Qualified { .. } => "omit the header".to_owned(),
    };

    errors.report(ParseError::new(
        ErrorCode::EmptyHeaderContent,
        Severity::Error,
        DiagnosticLocation::from(location),
        ErrorContext::new("", crate::Span::new(0, 0), "header"),
        format!("{written} header has no content after its colon; {remedy}"),
    ));
}
