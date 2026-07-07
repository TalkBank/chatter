//! Prefix-field extraction for `main_tier` conversion.
//!
//! Driven by the generated typed visitor: the speaker prefix is read from the
//! `extract_main_tier` slots (`star`, `speaker`, `colon`, `tab`) instead of a
//! positional `node.kind()` hand-walk. Each slot is matched EXHAUSTIVELY over
//! [`NodeSlot`] so a recovery node (MISSING / ERROR / unexpected kind) is handled
//! explicitly rather than silently dropped. The recovery diagnostics
//! (`MissingSpeaker`, `EmptyColon`, the `StructuralOrderError` "unexpected child"
//! / "missing tab" messages) are reproduced byte-identically from the previous
//! positional implementation. On the valid path (all four slots `Present`) no
//! diagnostic is emitted and the speaker string + span are built exactly as
//! before.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speaker_ID>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{AsRawNode, MainTierChildren, NodeSlot};

use super::{PrefixData, report_missing_child, report_unexpected_child};

/// Positional labels used in the `StructuralOrderError` "unexpected child"
/// diagnostics. The `star` slot keeps its bespoke `(*)` message inline below, so
/// only the speaker/colon/tab positions are named here. These mirror the running
/// child cursor the previous positional walk used (star=0, speaker=1, ...).
const SPEAKER_POSITION: usize = 1;
const COLON_POSITION: usize = 2;
const TAB_POSITION: usize = 3;

/// Parse `*`, speaker code, colon, and tab from the typed `main_tier` slots.
///
/// `main` is the result of `extract_main_tier`: `child_0` (`star`), `speaker`,
/// `child_2` (`colon`), `child_3` (`tab`). The valid path (all four `Present`)
/// emits no diagnostics and yields the speaker string + span; the remaining slot
/// states reproduce the prior recovery behavior.
pub(super) fn parse_prefix(
    main: &MainTierChildren,
    source: &str,
    original_input: &str,
    errors: &impl ErrorSink,
) -> PrefixData {
    // Position 0: star. A `Present` or `Missing` star keeps kind `star`, so the
    // old `child.kind() == STAR` branch accepted both with no diagnostic; a
    // plain `_` binding is sound here even though `Present` now carries a typed
    // `StarNode` while `Missing` carries a bare `Node` under the NEW closed
    // `NodeSlot` (an or-pattern of unbound wildcards does not require its
    // alternatives to share a type). An ERROR / unexpected-kind node reproduces
    // the bespoke "Expected 'star' (*)" structural diagnostic (an ERROR node's
    // `kind()` is "ERROR", matching the old `found '{}'` text); an absent star
    // reproduces the missing-star diagnostic.
    match &main.child_0.slot {
        NodeSlot::Present(_) | NodeSlot::Missing(_) => {}
        NodeSlot::Error(node) | NodeSlot::Unexpected(node) => {
            errors.report(ParseError::new(
                ErrorCode::StructuralOrderError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!(
                    "Expected 'star' (*) at position 0 of main_tier, found '{}'",
                    node.kind()
                ),
            ));
        }
        NodeSlot::Absent => report_missing_child(
            original_input,
            errors,
            ErrorCode::MissingSpeaker,
            "Missing star (*) at beginning of main tier",
        ),
    }

    // Position 1: speaker. `Present` carries a typed `SpeakerNode`, `Missing`
    // carries a bare `Node`: both need the raw node's byte range for the
    // zero-width check, so they are collapsed into one `Option<Node>` up front
    // (exhaustive over all 5 states, no `_ =>`) rather than duplicating the
    // zero-width-check body per arm.
    let mut speaker: Option<String> = None;
    let mut speaker_span = Span::DUMMY;
    let speaker_raw_node = match &main.speaker.slot {
        NodeSlot::Present(speaker_node) => Some(speaker_node.raw_node()),
        NodeSlot::Missing(node) => Some(*node),
        NodeSlot::Error(node) | NodeSlot::Unexpected(node) => {
            report_unexpected_child(*node, source, errors, "speaker", SPEAKER_POSITION);
            None
        }
        NodeSlot::Absent => {
            report_missing_child(
                original_input,
                errors,
                ErrorCode::MissingSpeaker,
                "Missing speaker in main tier",
            );
            None
        }
    };
    // A zero-width `speaker` token (a `Present` empty node or a MISSING
    // placeholder, both zero-width) is reported as missing. A MISSING node is
    // always zero-width, so it shares the diagnostic path of a zero-width
    // `Present`; the old branch keyed on `is_missing() || start == end`.
    if let Some(node) = speaker_raw_node {
        if node.start_byte() == node.end_byte() {
            errors.report(
                ParseError::new(
                    ErrorCode::MissingSpeaker,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                    "Missing speaker in main tier",
                )
                .with_suggestion("Main tier should start with *SPEAKER:"),
            );
        } else {
            speaker = Some(source[node.start_byte()..node.end_byte()].to_string());
            speaker_span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
        }
    }

    // Position 2: colon. Same Present/Missing type-mismatch collapse as speaker
    // above. A zero-width colon (a `Present` empty node or a MISSING
    // placeholder, both zero-width) is reported as `EmptyColon`; the old branch
    // reported `EmptyColon` whenever the colon node was zero width.
    let colon_raw_node = match &main.child_2.slot {
        NodeSlot::Present(colon_node) => Some(colon_node.raw_node()),
        NodeSlot::Missing(node) => Some(*node),
        NodeSlot::Error(node) | NodeSlot::Unexpected(node) => {
            report_unexpected_child(*node, source, errors, "colon", COLON_POSITION);
            None
        }
        NodeSlot::Absent => {
            report_missing_child(
                original_input,
                errors,
                ErrorCode::MissingColonAfterSpeaker,
                "Missing colon (:) after speaker in main tier",
            );
            None
        }
    };
    if let Some(node) = colon_raw_node
        && node.start_byte() == node.end_byte()
    {
        report_empty_colon(node.start_byte(), node.end_byte(), original_input, errors);
    }

    // Position 3: tab. A `Present` or `Missing` tab keeps kind `tab`, so the old
    // `child.kind() == TAB` branch accepted both with no diagnostic (see the
    // star-position note above on why the unbound `_ | _` arm is sound).
    match &main.child_3.slot {
        NodeSlot::Present(_) | NodeSlot::Missing(_) => {}
        NodeSlot::Error(node) | NodeSlot::Unexpected(node) => {
            report_unexpected_child(*node, source, errors, "tab", TAB_POSITION);
        }
        NodeSlot::Absent => report_missing_child(
            original_input,
            errors,
            ErrorCode::StructuralOrderError,
            "Missing tab after colon in main tier",
        ),
    }

    PrefixData {
        speaker,
        speaker_span,
    }
}

/// Report the `EmptyColon` diagnostic for a zero-width colon slot.
///
/// Reproduces the previous inline diagnostic byte-identically: the location uses
/// the colon node's (zero-width) span, the context spans the full original input,
/// and the suggestion is preserved.
fn report_empty_colon(
    colon_start: usize,
    colon_end: usize,
    original_input: &str,
    errors: &impl ErrorSink,
) {
    errors.report(
        ParseError::new(
            ErrorCode::EmptyColon,
            Severity::Error,
            SourceLocation::from_offsets(colon_start, colon_end),
            ErrorContext::new(original_input, 0..original_input.len(), original_input),
            "Empty colon (zero-width node) in main tier".to_string(),
        )
        .with_suggestion("Add ':' after speaker code (e.g., '*CHI:')"),
    );
}
