use crate::alignment::location_of;
use crate::{ErrorCode, ErrorLabel, ParseError, Severity};

/// Build a warning emitted when parse-health taint blocks one alignment pass.
/// The spans are as for [`location_of`].
pub(super) fn skipped_alignment_warning(
    alignment_name: &str,
    left_label: &str,
    left_clean: bool,
    left_location: crate::Location,
    right_label: &str,
    right_clean: bool,
    right_location: Option<crate::Location>,
) -> ParseError {
    let tainted = match (!left_clean, !right_clean) {
        (true, true) => format!("{left_label} and {right_label}"),
        (true, false) => left_label.to_string(),
        (false, true) => right_label.to_string(),
        (false, false) => "an internal parse-health gate".to_string(),
    };

    let location = location_of(left_location, right_location);
    let mut error = ParseError::at_location(
        ErrorCode::TierValidationError,
        Severity::Warning,
        location,
        format!(
            "Tier validation warning: skipped {} alignment because {} had parse errors during recovery",
            alignment_name, tainted
        ),
    )
    .with_suggestion("Fix parse errors in the affected tier(s) first, then rerun validation");

    if !left_clean && let Some(left_span) = left_location.span() {
        error.labels.push(ErrorLabel::new(left_span, left_label));
    }
    if !right_clean && let Some(right_span) = right_location.and_then(crate::Location::span) {
        error.labels.push(ErrorLabel::new(right_span, right_label));
    }

    error
}

pub(super) fn build_count_mismatch_error(
    source_count: usize,
    source_location: crate::Location,
    source_label: &str,
    target_count: usize,
    _target_location: crate::Location,
    target_label: &str,
    code: ErrorCode,
) -> ParseError {
    ParseError::at_location(
        code,
        Severity::Error,
        source_location,
        format!(
            "{} has {} words but {} has {}: word counts must match",
            source_label, source_count, target_label, target_count
        ),
    )
    .with_suggestion(format!(
        "Ensure {} and {} have the same number of words",
        source_label, target_label
    ))
}

/// Build the `%xphoaln` ↔ `%mod`/`%pho` word-count mismatch (E727/E728),
/// accounting for one-sided pause words.
///
/// Greg Hedlund's spec (§2 rule 5): a pause word present on only one of
/// `%mod`/`%pho` forms its own `%xphoaln` alignment word and consumes no
/// word slot on the tier that lacks it, so `%xphoaln`'s raw word count may
/// legitimately exceed either source tier's count. `effective_count` is the
/// count after excluding words that consume no slot on `target_label`; the
/// message reports both numbers so it never looks like it disagrees with
/// what the tier literally contains.
pub(super) fn build_phoaln_count_mismatch_error(
    phoaln_raw_count: usize,
    effective_count: usize,
    location: crate::Location,
    target_count: usize,
    target_label: &str,
    code: ErrorCode,
) -> ParseError {
    let detail = if effective_count == phoaln_raw_count {
        format!(
            "%xphoaln has {phoaln_raw_count} words but {target_label} has {target_count}: \
             word counts must match"
        )
    } else {
        format!(
            "%xphoaln has {phoaln_raw_count} words ({effective_count} after excluding one-sided \
             pause words that consume no {target_label} slot) but {target_label} has \
             {target_count}: word counts must match"
        )
    };
    ParseError::at_location(code, Severity::Error, location, detail).with_suggestion(format!(
        "Ensure %xphoaln aligns one-to-one with {target_label}, accounting for any one-sided \
         pause words"
    ))
}
