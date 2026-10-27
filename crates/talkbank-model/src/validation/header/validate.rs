//! Dispatch header validation to type-specific checkers.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Options_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Number_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Recording_Quality_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Transcription_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Duration_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Start_Header>

use crate::ErrorSink;
use crate::model::Header;
use crate::validation::{Validate, ValidationContext};

use super::checkers::{
    check_birth_date_header, check_comment_warning, check_date_header, check_id_header,
};
use super::metadata::{check_time_duration_format, check_time_start_format};
use super::unknown::check_unknown_header;

/// Validate a parsed `Header` and emit diagnostics through `errors`.
///
/// This is a lightweight dispatcher that routes each header variant to the
/// corresponding checker/trait implementation at the header's location.
pub(crate) fn check_header(
    header: &Header,
    location: crate::Location,
    context: &ValidationContext,
    errors: &impl ErrorSink,
) {
    // Header payloads (participant entries, language codes, option flags)
    // carry no location of their own: they report at the header's line. An
    // unlocated header falls back to whatever the caller had narrowed to.
    let context = context
        .clone()
        .with_field_location(location.or(context.field_location));
    let context = &context;
    match header {
        Header::ID(id_header) => {
            check_id_header(id_header, location, errors);
            // E542: Unsupported sex value
            if let Some(crate::model::Sex::Unsupported(value)) = &id_header.sex {
                report_unsupported_sex(value, location, errors);
            }
            // E546: Unsupported SES value
            if let Some(crate::model::SesValue::Unsupported(value)) = &id_header.ses {
                report_unsupported_ses(value, location, errors);
            }
        }
        Header::Participants { entries } => {
            entries.validate(context, errors);
        }
        Header::UnparsedID(raw) => {
            errors.report(crate::ParseError::at_location(
                crate::ErrorCode::UnparsableContent,
                crate::Severity::Error,
                raw.location().or(location),
                "Unparsed @ID header",
            ));
        }
        Header::UnparsedParticipants(raw) => {
            errors.report(crate::ParseError::at_location(
                crate::ErrorCode::UnparsableContent,
                crate::Severity::Error,
                raw.location().or(location),
                "Unparsed @Participants declaration",
            ));
        }
        Header::UnparsedLanguages(raw) => {
            errors.report(crate::ParseError::at_location(
                match raw.failure() {
                    crate::model::LanguageHeaderFailure::Syntax => {
                        crate::ErrorCode::UnparsableContent
                    }
                    crate::model::LanguageHeaderFailure::List => {
                        crate::ErrorCode::EmptyLanguagesHeader
                    }
                },
                crate::Severity::Error,
                raw.location().or(location),
                "Unparsed @Languages declaration",
            ));
        }
        Header::Languages { codes } => {
            codes.validate(context, errors);
        }
        Header::L1Of { language, .. } => {
            // @L1 of carries an ISO 639-3 code held to the same shared rule
            // set as @Languages / @ID, anchored at this header's location.
            language.report_code_issues(crate::DiagnosticLocation::from(location), errors);
        }
        Header::Date { date } => {
            check_date_header(date.as_str(), location, errors);
        }
        Header::Birth { date, .. } => {
            check_birth_date_header(date.as_str(), location, errors);
        }
        Header::Comment { content } => {
            check_comment_warning(content, location, errors);
            content.segments.validate(context, errors);
        }
        Header::Unknown {
            text,
            parse_reason,
            suggested_fix,
        } => {
            check_unknown_header(
                text.as_str(),
                parse_reason.as_deref(),
                suggested_fix.as_deref(),
                location,
                errors,
            );
        }
        Header::Options { options } => {
            options.validate(context, errors);
            check_unsupported_options(options, location, errors);
        }
        Header::Media(media_header) => {
            check_unsupported_media(media_header, location, errors);
            // E767 and E768 are properties of THIS header's payload, like
            // E535/E536 above, so they belong on this arm rather than in a
            // separate file-level sweep. Placing them here is also what makes
            // them reach `validate_headers_only`, the LSP's entry point: E767
            // was invisible in the editor for its whole first day because the
            // file-level sweep never runs there.
            check_media_whitespace_before_comma(media_header, location, errors);
            media_header
                .filename
                .report_representability_issues(location, errors);
        }
        Header::Number { number } => {
            check_unsupported_number(number, location, errors);
        }
        Header::RecordingQuality { quality } => {
            check_unsupported_recording_quality(quality, location, errors);
        }
        Header::Transcription { transcription } => {
            check_unsupported_transcription(transcription, location, errors);
        }
        Header::TimeDuration { duration } => {
            if let Some(invalid) = duration.invalidity() {
                check_time_duration_format(invalid, location, errors);
            }
        }
        Header::TimeStart { start } => {
            if let Some(invalid) = start.invalidity() {
                check_time_start_format(invalid, location, errors);
            }
        }
        Header::Types(_) => {
            // No validation, @Types fields have no fixed vocabulary.
        }
        _ => {}
    }
}

// ── Unsupported-value validators (E534-E539, E542) ────────────────────

/// E534: Flag each unsupported option flag in `@Options`.
fn check_unsupported_options(
    options: &crate::model::header::ChatOptionFlags,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    for flag in options.iter() {
        if let crate::model::ChatOptionFlag::Unsupported(value) = flag {
            let err = crate::ParseError::new(
                crate::ErrorCode::UnsupportedOption,
                crate::Severity::Error,
                crate::DiagnosticLocation::from(location),
                crate::ErrorContext::new(value, crate::Span::new(0, value.len()), "option_name"),
                format!("Unsupported @Options value: '{}'", value),
            )
            .with_suggestion("Supported options (per CLAN depfile.cut): CA, NoAlign");
            errors.report(err);
        }
    }
}

/// E535/E536: Flag unsupported media type or status in `@Media`.
fn check_unsupported_media(
    media_header: &crate::model::MediaHeader,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    if let crate::model::MediaType::Unsupported(value) = &media_header.media_type {
        let err = crate::ParseError::new(
            crate::ErrorCode::UnsupportedMediaType,
            crate::Severity::Error,
            crate::DiagnosticLocation::from(location),
            crate::ErrorContext::new(value, crate::Span::new(0, value.len()), "media_type"),
            format!("Unsupported @Media type: '{}'", value),
        )
        .with_suggestion("Supported media types: audio, video, missing");
        errors.report(err);
    }

    if let Some(crate::model::MediaStatus::Unsupported(value)) = &media_header.status {
        let err = crate::ParseError::new(
            crate::ErrorCode::UnsupportedMediaStatus,
            crate::Severity::Error,
            crate::DiagnosticLocation::from(location),
            crate::ErrorContext::new(value, crate::Span::new(0, value.len()), "media_status"),
            format!("Unsupported @Media status: '{}'", value),
        )
        .with_suggestion("Supported media status values: unlinked, missing, notrans");
        errors.report(err);
    }
}

/// E767: whitespace between the `@Media` filename and its comma.
///
/// The span is recorded as parser provenance on the header, so this reads a
/// fact both front ends supply rather than re-deriving it from text; that is
/// what keeps the two parsers reporting the same thing.
fn check_media_whitespace_before_comma(
    media_header: &crate::model::MediaHeader,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    // The whitespace's presence decides the rule; its location only decides
    // where the diagnostic points, falling back to the header's line when a
    // parser recorded the whitespace without an offset.
    let Some(space) = media_header.whitespace_before_comma else {
        return;
    };
    let at = space.start_point().or(location);
    errors.report(crate::ParseError::new(
        crate::ErrorCode::MediaWhitespaceBeforeComma,
        crate::Severity::Error,
        crate::DiagnosticLocation::from(at),
        crate::ErrorContext::new("", crate::Span::new(0, 0), "media_filename"),
        "Whitespace before the comma in @Media: the filename ends where the comma begins",
    ));
}

/// E537: Flag unsupported `@Number` value.
fn check_unsupported_number(
    number: &crate::model::Number,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    if let crate::model::Number::Unsupported(value) = number {
        let err = crate::ParseError::new(
            crate::ErrorCode::UnsupportedNumber,
            crate::Severity::Error,
            crate::DiagnosticLocation::from(location),
            crate::ErrorContext::new(value, crate::Span::new(0, value.len()), "number_option"),
            format!("Unsupported @Number value: '{}'", value),
        )
        .with_suggestion("Supported values: 1, 2, 3, 4, 5, more, audience");
        errors.report(err);
    }
}

/// E538: Flag unsupported `@Recording Quality` value.
fn check_unsupported_recording_quality(
    quality: &crate::model::RecordingQuality,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    if let crate::model::RecordingQuality::Unsupported(value) = quality {
        let err = crate::ParseError::new(
            crate::ErrorCode::UnsupportedRecordingQuality,
            crate::Severity::Error,
            crate::DiagnosticLocation::from(location),
            crate::ErrorContext::new(
                value,
                crate::Span::new(0, value.len()),
                "recording_quality_option",
            ),
            format!("Unsupported @Recording Quality value: '{}'", value),
        )
        .with_suggestion("Supported values: 1, 2, 3, 4, 5");
        errors.report(err);
    }
}

/// E539: Flag unsupported `@Transcription` value.
fn check_unsupported_transcription(
    transcription: &crate::model::Transcription,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    if let crate::model::Transcription::Unsupported(value) = transcription {
        let err = crate::ParseError::new(
            crate::ErrorCode::UnsupportedTranscription,
            crate::Severity::Error,
            crate::DiagnosticLocation::from(location),
            crate::ErrorContext::new(
                value,
                crate::Span::new(0, value.len()),
                "transcription_option",
            ),
            format!("Unsupported @Transcription value: '{}'", value),
        )
        .with_suggestion(
            "Supported values: eye_dialect, partial, full, detailed, coarse, checked, anonymized",
        );
        errors.report(err);
    }
}

/// E546: Flag unsupported SES value in `@ID`.
fn report_unsupported_ses(value: &str, location: crate::Location, errors: &impl ErrorSink) {
    let err = crate::ParseError::new(
        crate::ErrorCode::UnsupportedSesValue,
        crate::Severity::Error,
        crate::DiagnosticLocation::from(location),
        crate::ErrorContext::new(value, crate::Span::new(0, value.len()), "id_ses"),
        format!("Unsupported @ID SES value: '{}'", value),
    )
    .with_suggestion(
        "Supported values: UC, MC, WC, LI, White, Black, Asian, Latino, Native, Multiple, Unknown (or combined e.g. 'White UC')",
    );
    errors.report(err);
}

/// E542: Flag unsupported sex value in `@ID`.
fn report_unsupported_sex(value: &str, location: crate::Location, errors: &impl ErrorSink) {
    let err = crate::ParseError::new(
        crate::ErrorCode::UnsupportedSex,
        crate::Severity::Error,
        crate::DiagnosticLocation::from(location),
        crate::ErrorContext::new(value, crate::Span::new(0, value.len()), "id_sex"),
        format!("Unsupported @ID sex value: '{}'", value),
    )
    .with_suggestion("Supported values: male, female");
    errors.report(err);
}
