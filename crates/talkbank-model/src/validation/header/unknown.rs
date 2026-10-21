//! Handling for unknown/malformed headers during validation.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Warning_Header>

use crate::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};

/// Reports unknown or malformed header labels.
///
/// A header label not recognised by the grammar is always an error,
/// there are no "soft" header kinds in modern CHAT. Earlier code had
/// a special case for a made-up `@OldHeader` label that demoted the
/// diagnostic to Warning severity; that label was never part of the
/// CHAT spec and the special case was removed on 2026-04-22.
pub(super) fn check_unknown_header(
    text: &str,
    parse_reason: Option<&str>,
    suggested_fix: Option<&str>,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    let mut message = format!("Unknown or malformed header: {}", text);
    if let Some(reason) = parse_reason {
        message.push_str(&format!(" ({})", reason));
    }

    let mut error = ParseError::new(
        ErrorCode::UnknownHeader,
        Severity::Error,
        DiagnosticLocation::from(location),
        ErrorContext::new(text, crate::Span::new(0, text.len()), "unknown_header"),
        message,
    );

    if let Some(fix) = suggested_fix {
        error = error.with_suggestion(fix);
    } else {
        error = error.with_suggestion(
            "Check the CHAT manual for valid header types: https://talkbank.org/0info/manuals/CHAT.html#File_Headers",
        );
    }

    errors.report(error);
}
