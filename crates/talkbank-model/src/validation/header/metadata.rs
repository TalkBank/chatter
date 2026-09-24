//! Validators for metadata header formats (`@Date`, `@Time Duration`, `@Time Start`).
//!
//! This module houses header-field format validators that are easier to keep
//! independent from structural header-order logic in `header/structure`.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Duration_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Start_Header>

use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};

use crate::model::header::{DateDigitError, DateDigits, InvalidTimeDuration, InvalidTimeStart};

/// Validate a `DD-MMM-YYYY` date value against CLAN `depfile.cut`'s
/// `@d<dd-lll-yyyy>` template. Used by both `@Date` (E518) and
/// `@Birth of` (E545), the two headers share the same date format
/// rule per depfile, so the same component-level diagnostic logic
/// applies; only the emitted error code differs.
///
/// The checker reports granular diagnostics per component (day/month/year) so
/// users get actionable fixes instead of a single generic format error.
pub(super) fn check_date_format(
    date: &str,
    span: Span,
    errors: &impl ErrorSink,
    error_code: ErrorCode,
) {
    const VALID_MONTHS: &[&str] = &[
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];

    let make_err = |context_label: &str, message: String| {
        let mut err = ParseError::new(
            error_code,
            Severity::Error,
            SourceLocation::at_offset(span.start as usize),
            ErrorContext::new(date, 0..date.len(), context_label),
            message,
        );
        err.location.span = span;
        err
    };

    let parts: Vec<&str> = date.split('-').collect();

    if parts.len() != 3 {
        errors.report(
            make_err(
                date,
                format!(
                    "Invalid @Date format '{}': expected DD-MMM-YYYY with hyphens",
                    date
                ),
            )
            .with_suggestion(
                "Use format: 01-JAN-2024 (two-digit day, uppercase month, four-digit year)",
            ),
        );
        return;
    }

    let (day_str, month_str, year_str) = (parts[0], parts[1], parts[2]);

    match DateDigits::<2>::parse(day_str) {
        Err(DateDigitError::Width) => {
            errors.report(
                make_err(
                    day_str,
                    format!(
                        "Invalid @Date day '{}': must be exactly two digits",
                        day_str
                    ),
                )
                .with_suggestion("Use two-digit day (e.g., 01, 02, 15)"),
            );
        }
        Ok(digits) => {
            let day = digits.day();
            if !(1..=31).contains(&day) {
                errors.report(
                    make_err(
                        day_str,
                        format!("Invalid @Date day '{}': must be between 01 and 31", day_str),
                    )
                    .with_suggestion("Use a valid day between 01 and 31"),
                );
            }
        }
        Err(DateDigitError::NonDigit) => {
            errors.report(
                make_err(
                    day_str,
                    format!(
                        "Invalid @Date day '{}': must contain only ASCII digits",
                        day_str
                    ),
                )
                .with_suggestion("Day must contain two ASCII digits (01-31), without a sign"),
            );
        }
    }

    if !VALID_MONTHS.contains(&month_str) {
        let suggestion = if month_str.len() == 3 {
            let upper = month_str.to_uppercase();
            if VALID_MONTHS.contains(&upper.as_str()) {
                format!("Use uppercase month: {}", upper)
            } else {
                "Valid months: JAN, FEB, MAR, APR, MAY, JUN, JUL, AUG, SEP, OCT, NOV, DEC"
                    .to_string()
            }
        } else {
            "Month must be three-letter uppercase abbreviation (e.g., JAN, FEB, MAR)".to_string()
        };

        errors.report(
            make_err(
                month_str,
                format!(
                    "Invalid @Date month '{}': must be an uppercase three-letter abbreviation",
                    month_str
                ),
            )
            .with_suggestion(suggestion),
        );
    }

    match DateDigits::<4>::parse(year_str) {
        Err(DateDigitError::Width) => {
            errors.report(
                make_err(
                    year_str,
                    format!(
                        "Invalid @Date year '{}': must be exactly four digits",
                        year_str
                    ),
                )
                .with_suggestion("Use four-digit year (e.g., 2024)"),
            );
        }
        Err(DateDigitError::NonDigit) => {
            errors.report(
                make_err(
                    year_str,
                    format!(
                        "Invalid @Date year '{}': must contain only ASCII digits",
                        year_str
                    ),
                )
                .with_suggestion("Year must contain four ASCII digits, without a sign"),
            );
        }
        Ok(_) => {}
    }
}

// ── Time format validators (E540, E541) ───────────────────────────────
//
// These functions render diagnostics from model-issued refusal capabilities.
// Duration and start assessments own unsupported values, shape, clock range
// and the optional-empty policy; rendering cannot repeat or bypass admission.

/// E540: Emit an error for an invalid `@Time Duration` value.
///
/// The model's refusal proves the duration is nonempty and fails its shape
/// or clock assessment. Rendering cannot accept unassessed text or repeat the
/// optional-empty policy.
pub(super) fn check_time_duration_format(
    invalid: InvalidTimeDuration<'_>,
    span: Span,
    errors: &impl ErrorSink,
) {
    let duration = invalid.into_text();
    let mut err = ParseError::new(
        ErrorCode::InvalidTimeDuration,
        Severity::Error,
        SourceLocation::at_offset(span.start as usize),
        ErrorContext::new(duration, 0..duration.len(), "time_duration"),
        format!(
            "Invalid @Time Duration: '{}'. Must be one of the depfile.cut forms (HH:MM:SS, HH:MM-HH:MM, HH:MM:SS-HH:MM:SS) with legal clock values (hours 00-23, minutes/seconds 00-59)",
            duration
        ),
    )
    .with_suggestion(
        "Use one of: HH:MM:SS (single), HH:MM-HH:MM (range), HH:MM:SS-HH:MM:SS (range), with valid clock values (hours 00-23, minutes/seconds 00-59). No comma-joined segments, no semicolon separator.",
    );
    err.location.span = span;
    errors.report(err);
}

/// E541: Emit an error for an invalid `@Time Start` value.
///
/// Consumes model-issued refusal evidence: callers cannot report E541 from
/// arbitrary unassessed text. The original spelling survives the assessment.
pub(super) fn check_time_start_format(
    invalid: InvalidTimeStart<'_>,
    span: Span,
    errors: &impl ErrorSink,
) {
    let start = invalid.into_text();
    let mut err = ParseError::new(
        ErrorCode::InvalidTimeStart,
        Severity::Error,
        SourceLocation::at_offset(span.start as usize),
        ErrorContext::new(start, 0..start.len(), "time_start"),
        format!(
            "Invalid @Time Start: '{}'. Use HH:MM:SS or MM:SS with legal clock values (hours 00-23, minutes/seconds 00-59)",
            start
        ),
    )
    .with_suggestion("Use HH:MM:SS or MM:SS with hours 00-23 and minutes/seconds 00-59, no millisecond suffix, no range.");
    err.location.span = span;
    errors.report(err);
}
