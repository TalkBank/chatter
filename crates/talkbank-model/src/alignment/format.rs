//! Shared formatting for alignment mismatch and provenance diagnostics.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Timing_Tier>

use super::helpers::TierPosition;
use crate::{DiagnosticLocation, ErrorCode, ErrorLabel, Location, ParseError, Severity};

/// The first located tier side, or an unlocated diagnostic when neither side
/// has a position. An absent grouped partner has no right side at all.
pub(crate) fn location_of(left: Location, right: Option<Location>) -> DiagnosticLocation {
    match [Some(left), right]
        .into_iter()
        .flatten()
        .find_map(Location::span)
    {
        Some(span) => DiagnosticLocation::new(span),
        None => DiagnosticLocation::Unlocated,
    }
}

/// Shared warning for file validation and derived metadata when an alignment
/// is blocked by unknown parse provenance. Only held, known spans get labels.
pub(crate) fn unknown_alignment_warning(
    alignment_name: &str,
    left_label: &str,
    left_location: Location,
    right_label: &str,
    right_location: Option<Location>,
) -> ParseError {
    let mut error = ParseError::at_location(
        ErrorCode::TierValidationError,
        Severity::Warning,
        location_of(left_location, right_location),
        format!(
            "Tier validation warning: skipped {} alignment because parse provenance is unknown for {} and {}",
            alignment_name, left_label, right_label
        ),
    )
    .with_suggestion(
        "Run parser-backed validation to establish parse provenance before alignment checks",
    );

    if let Some(left_span) = left_location.span() {
        error.labels.push(ErrorLabel::new(left_span, left_label));
    }
    if let Some(right_span) = right_location.and_then(Location::span) {
        error.labels.push(ErrorLabel::new(right_span, right_label));
    }
    error
}

/// Format a positional alignment mismatch for cross-domain tiers.
///
/// Uses simple positional pairing. Appropriate for tiers like %mor, %pho,
/// and %sin where the dependent tier items are in a completely different
/// domain (morphological tags, phonological tokens, gestures) and string
/// matching is meaningless.
pub fn format_positional_mismatch(
    left_name: &str,
    right_name: &str,
    left_items: &[TierPosition],
    right_items: &[TierPosition],
) -> String {
    let left_count = left_items.len();
    let right_count = right_items.len();

    let mut msg = format!(
        "{left_name} has {left_count} alignable items, but {right_name} has {right_count} items\n\n",
    );

    msg.push_str(&format!("{:<30} {}\n", left_name, right_name));
    msg.push_str(&format!("{:<30} {}\n", "─".repeat(28), "─".repeat(28)));

    let max_len = left_items.len().max(right_items.len());
    for i in 0..max_len {
        let left_text = left_items.get(i).map_or("-", |item| &item.text);
        let right_text = right_items.get(i).map_or("-", |item| &item.text);

        let marker = if i >= left_items.len() {
            " ⊕"
        } else if i >= right_items.len() {
            " ⊖"
        } else {
            ""
        };

        msg.push_str(&format!("{:<30} {}{}\n", left_text, right_text, marker));
    }

    msg
}
