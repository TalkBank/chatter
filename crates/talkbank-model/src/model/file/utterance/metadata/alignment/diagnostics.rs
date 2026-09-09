use crate::{ErrorCode, ErrorContext, ErrorLabel, ParseError, Severity, SourceLocation, Span};

/// Where a warning about two tier sides points: the first side with a known
/// location, and `Span::DUMMY` when neither has one.
///
/// `left` is the span of a tier the caller holds; `right` is `None` when the
/// partner side is a group of tiers none of which is present (`%xphoaln`
/// with neither `%mod` nor `%pho`). A HELD tier's span can still be `0..0`:
/// the re2c backend sets no tier spans yet, so every tier it builds carries
/// the sentinel, and a `SourceLocation` has no way to say "unknown". That
/// producer is the remaining hole, recorded in the fabrication handoff, not
/// this module's to close; until it is, the sentinel is what such a warning
/// carries, and the labels below are gated the same way so that no label
/// points at byte 0 for a tier whose location is unknown.
fn location_of(left: Span, right: Option<Span>) -> Span {
    match [Some(left), right]
        .into_iter()
        .flatten()
        .find(|span| !span.is_dummy())
    {
        Some(span) => span,
        None => Span::DUMMY,
    }
}

/// A tier's span as a location: `None` when the tier carries the sentinel
/// (see [`location_of`]).
pub(super) fn known_span(span: Span) -> Option<Span> {
    (!span.is_dummy()).then_some(span)
}

/// Build a warning emitted when parse-health taint blocks one alignment pass.
/// The spans are as for [`location_of`].
pub(super) fn skipped_alignment_warning(
    alignment_name: &str,
    left_label: &str,
    left_clean: bool,
    left_span: Span,
    right_label: &str,
    right_clean: bool,
    right_span: Option<Span>,
) -> ParseError {
    let tainted = match (!left_clean, !right_clean) {
        (true, true) => format!("{left_label} and {right_label}"),
        (true, false) => left_label.to_string(),
        (false, true) => right_label.to_string(),
        (false, false) => "an internal parse-health gate".to_string(),
    };

    let location = location_of(left_span, right_span);
    let mut error = ParseError::new(
        ErrorCode::TierValidationError,
        Severity::Warning,
        SourceLocation::new(location),
        ErrorContext::new("", location.to_range(), ""),
        format!(
            "Tier validation warning: skipped {} alignment because {} had parse errors during recovery",
            alignment_name, tainted
        ),
    )
    .with_suggestion("Fix parse errors in the affected tier(s) first, then rerun validation");

    if !left_clean && let Some(left_span) = known_span(left_span) {
        error.labels.push(ErrorLabel::new(left_span, left_label));
    }
    if !right_clean && let Some(right_span) = right_span.and_then(known_span) {
        error.labels.push(ErrorLabel::new(right_span, right_label));
    }

    error
}

/// Build a warning emitted when alignment is blocked by missing parse
/// provenance. The spans are as for [`location_of`].
pub(super) fn unknown_alignment_warning(
    alignment_name: &str,
    left_label: &str,
    left_span: Span,
    right_label: &str,
    right_span: Option<Span>,
) -> ParseError {
    let location = location_of(left_span, right_span);
    let mut error = ParseError::new(
        ErrorCode::TierValidationError,
        Severity::Warning,
        SourceLocation::new(location),
        ErrorContext::new("", location.to_range(), ""),
        format!(
            "Tier validation warning: skipped {} alignment because parse provenance is unknown for {} and {}",
            alignment_name, left_label, right_label
        ),
    )
    .with_suggestion(
        "Run parser-backed validation or explicitly mark parse provenance before alignment checks",
    );

    if let Some(left_span) = known_span(left_span) {
        error.labels.push(ErrorLabel::new(left_span, left_label));
    }
    if let Some(right_span) = right_span.and_then(known_span) {
        error.labels.push(ErrorLabel::new(right_span, right_label));
    }

    error
}

pub(super) fn build_count_mismatch_error(
    source_count: usize,
    source_span: Span,
    source_label: &str,
    target_count: usize,
    _target_span: Span,
    target_label: &str,
    code: ErrorCode,
) -> ParseError {
    ParseError::new(
        code,
        Severity::Error,
        SourceLocation::new(source_span),
        ErrorContext::new("", source_span.to_range(), ""),
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
    phoaln_span: Span,
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
    ParseError::new(
        code,
        Severity::Error,
        SourceLocation::new(phoaln_span),
        ErrorContext::new("", phoaln_span.to_range(), ""),
        detail,
    )
    .with_suggestion(format!(
        "Ensure %xphoaln aligns one-to-one with {target_label}, accounting for any one-sided \
         pause words"
    ))
}
