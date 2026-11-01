//! Validators for metadata header formats (`@Date`, `@Time Duration`, `@Time Start`).
//!
//! This module houses header-field format validators that are easier to keep
//! independent from structural header-order logic in `header/structure`.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Duration_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Start_Header>

use crate::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};

use super::checkers::DateHeader;

use crate::model::header::{
    DateDigitProblem, DateRefusal, DayProblem, InvalidChatDate, InvalidTimeDuration,
    InvalidTimeStart, MonthProblem,
};

/// E518 / E545: render the model's refusal of a `DD-MMM-YYYY` date (CLAN
/// `depfile.cut`'s `@d<dd-lll-yyyy>` template), one diagnostic per wrong
/// component so users get actionable fixes instead of a single generic
/// format error.
///
/// `@Date` and `@Birth of` share the date rule per depfile; `header` chooses
/// the emitted code and the header the message names. The model read the
/// date ([`InvalidChatDate`]); this only words what it found.
pub(super) fn report_invalid_date(
    invalid: &InvalidChatDate,
    header: DateHeader,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    let error_code = header.error_code();
    let name = header.header_name();
    let date = invalid.as_str();
    let make_err = |context_label: &str, message: String| {
        ParseError::new(
            error_code,
            Severity::Error,
            DiagnosticLocation::from(location),
            ErrorContext::new(date, crate::Span::new(0, date.len()), context_label),
            message,
        )
    };
    for refusal in invalid.refusals() {
        let err = match refusal {
            DateRefusal::Shape(text) => make_err(
                text,
                format!("Invalid @{name} format '{text}': expected DD-MMM-YYYY with hyphens"),
            )
            .with_suggestion(
                "Use format: 01-JAN-2024 (two-digit day, uppercase month, four-digit year)",
            ),
            DateRefusal::Day(day, DayProblem::Digits(DateDigitProblem::Width)) => make_err(
                day,
                format!("Invalid @{name} day '{day}': must be exactly two digits"),
            )
            .with_suggestion("Use two-digit day (e.g., 01, 02, 15)"),
            DateRefusal::Day(day, DayProblem::Digits(DateDigitProblem::NonDigit)) => make_err(
                day,
                format!("Invalid @{name} day '{day}': must contain only ASCII digits"),
            )
            .with_suggestion("Day must contain two ASCII digits (01-31), without a sign"),
            DateRefusal::Day(day, DayProblem::OutOfRange) => make_err(
                day,
                format!("Invalid @{name} day '{day}': must be between 01 and 31"),
            )
            .with_suggestion("Use a valid day between 01 and 31"),
            DateRefusal::Month(month, problem) => make_err(
                month,
                format!(
                    "Invalid @{name} month '{month}': must be an uppercase three-letter abbreviation"
                ),
            )
            .with_suggestion(match problem {
                MonthProblem::NotUppercase(upper) => {
                    format!("Use uppercase month: {}", upper.as_str())
                }
                MonthProblem::Unknown => {
                    "Valid months: JAN, FEB, MAR, APR, MAY, JUN, JUL, AUG, SEP, OCT, NOV, DEC"
                        .to_owned()
                }
                MonthProblem::NotAbbreviation => {
                    "Month must be three-letter uppercase abbreviation (e.g., JAN, FEB, MAR)"
                        .to_owned()
                }
            }),
            DateRefusal::Year(year, DateDigitProblem::Width) => make_err(
                year,
                format!("Invalid @{name} year '{year}': must be exactly four digits"),
            )
            .with_suggestion("Use four-digit year (e.g., 2024)"),
            DateRefusal::Year(year, DateDigitProblem::NonDigit) => make_err(
                year,
                format!("Invalid @{name} year '{year}': must contain only ASCII digits"),
            )
            .with_suggestion("Year must contain four ASCII digits, without a sign"),
        };
        errors.report(err);
    }
}

// ── Time format validators (E540, E541) ───────────────────────────────
//
// These functions render diagnostics from model-issued refusal capabilities.
// Duration and start assessments own unsupported values, shape and clock
// range; rendering cannot repeat or bypass admission. Neither value can be
// empty: an empty time header is `Header::EmptyContent` (E553).

/// E540: Emit an error for an invalid `@Time Duration` value.
///
/// The model's refusal proves the duration is nonempty and fails its shape
/// or clock assessment. Rendering cannot accept unassessed text.
pub(super) fn check_time_duration_format(
    invalid: InvalidTimeDuration<'_>,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    let duration = invalid.into_text();
    let err = ParseError::new(
        ErrorCode::InvalidTimeDuration,
        Severity::Error,
        DiagnosticLocation::from(location),
        ErrorContext::new(duration, crate::Span::new(0, duration.len()), "time_duration"),
        format!(
            "Invalid @Time Duration: '{}'. Must be one of the depfile.cut forms (HH:MM:SS, HH:MM-HH:MM, HH:MM:SS-HH:MM:SS) with legal clock values (hours 00-23, minutes/seconds 00-59)",
            duration
        ),
    )
    .with_suggestion(
        "Use one of: HH:MM:SS (single), HH:MM-HH:MM (range), HH:MM:SS-HH:MM:SS (range), with valid clock values (hours 00-23, minutes/seconds 00-59). No comma-joined segments, no semicolon separator.",
    );
    errors.report(err);
}

/// E541: Emit an error for an invalid `@Time Start` value.
///
/// Consumes model-issued refusal evidence: callers cannot report E541 from
/// arbitrary unassessed text. The original spelling survives the assessment.
pub(super) fn check_time_start_format(
    invalid: InvalidTimeStart<'_>,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    let start = invalid.into_text();
    let err = ParseError::new(
        ErrorCode::InvalidTimeStart,
        Severity::Error,
        DiagnosticLocation::from(location),
        ErrorContext::new(start, crate::Span::new(0, start.len()), "time_start"),
        format!(
            "Invalid @Time Start: '{}'. Use HH:MM:SS or MM:SS with legal clock values (hours 00-23, minutes/seconds 00-59)",
            start
        ),
    )
    .with_suggestion("Use HH:MM:SS or MM:SS with hours 00-23 and minutes/seconds 00-59, no millisecond suffix, no range.");
    errors.report(err);
}
