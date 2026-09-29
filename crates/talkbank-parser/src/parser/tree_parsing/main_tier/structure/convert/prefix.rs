//! Prefix-field extraction for `main_tier` conversion.
//!
//! Driven by the generated typed visitor: the speaker prefix is read from the
//! `extract_main_tier` slots (`star`, `speaker`, `colon`, `tab`) instead of a
//! positional `node.kind()` hand-walk. Each slot is matched EXHAUSTIVELY over
//! `KindSlotValue` so a recovery node (MISSING / ERROR) is handled
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
use crate::generated_traversal::{
    AdmittedMainTierChildren, AsRawNode, ColonNode, KindMissing, KindSlotValue, NoChild, NodeSlot,
    RecoveryNode, SourceBound, SourceChildren, SourceField, SourceSlotView, SpeakerNode,
};
use crate::parser::typed_cst::cst_failure_diagnostic;

use super::{PrefixData, ReportedMainTierError, report_missing_child, report_unexpected_child};

/// Admitted nonempty speaker text together with the span that supplied it.
/// Fields are private to the admitting module; conversion consumes the result.
pub(super) struct ParsedSpeakerPrefix {
    code: String,
    span: Span,
}

impl ParsedSpeakerPrefix {
    fn admit<'tree>(
        node: SourceBound<'tree, '_, SpeakerNode<'tree>>,
        errors: &impl ErrorSink,
    ) -> Result<Self, ReportedMainTierError> {
        let raw = node.raw_node();
        if raw.start_byte() == raw.end_byte() {
            return Err(report_empty_speaker(node.node(), node.source(), errors));
        }
        let code = node.text().to_owned();
        Ok(Self {
            code,
            span: Span::new(raw.start_byte() as u32, raw.end_byte() as u32),
        })
    }

    pub(super) fn into_parts(self) -> (String, Span) {
        (self.code, self.span)
    }
}

/// Preserve canonical ownership until the generated leaf admits its range.
fn read_speaker<'tree, 'source>(
    field: SourceField<'_, 'tree, 'source, SpeakerNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<SourceBound<'tree, 'source, SpeakerNode<'tree>>, ReportedMainTierError> {
    field.read().map_err(|error| {
        ReportedMainTierError::report(
            cst_failure_diagnostic(field.raw_node(), field.source(), error),
            errors,
        )
    })
}

fn report_empty_speaker(
    typed: SpeakerNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ReportedMainTierError {
    let node = typed.raw_node();
    ReportedMainTierError::report(
        ParseError::new(
            ErrorCode::MissingSpeaker,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            "Missing speaker in main tier",
        )
        .with_suggestion("Main tier should start with *SPEAKER:"),
        errors,
    )
}

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
pub(super) fn parse_prefix<'tree>(
    associated: &SourceChildren<'tree, '_, AdmittedMainTierChildren<'tree>>,
    carrier: std::ops::Range<usize>,
    original_input: &str,
    errors: &impl ErrorSink,
) -> PrefixData {
    let main = associated.children();
    let source = associated.source();
    // Present and kind-proven MISSING stars preserve the existing acceptance
    // policy. ERROR and absent slots keep their distinct diagnostics.
    match main.child_0.slot().known_or_placeholder() {
        KindSlotValue::Present(_) | KindSlotValue::Placeholder(_) => {}
        KindSlotValue::Error(node) => {
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
        KindSlotValue::Absent(NoChild) => {
            report_missing_child(
                carrier.clone(),
                original_input,
                errors,
                ErrorCode::MissingSpeaker,
                "Missing star (*) at beginning of main tier",
            );
        }
    }

    // Retain the generated speaker type through checked text admission. A
    // MISSING placeholder is zero-width recovery, never an admitted code.
    let speaker = match associated.field_speaker().slot().view() {
        SourceSlotView::Present(speaker_node) => read_speaker(speaker_node, errors)
            .and_then(|node| ParsedSpeakerPrefix::admit(node, errors)),
        SourceSlotView::Missing(node) => read_speaker(node, errors)
            .and_then(|node| Err(report_empty_speaker(node.node(), node.source(), errors))),
        SourceSlotView::Error(node) => Err(report_unexpected_child(
            node.raw_node(),
            source,
            errors,
            "speaker",
            SPEAKER_POSITION,
        )),
        SourceSlotView::Unexpected(never) => match never {},
        SourceSlotView::Absent(NoChild) => Err(report_missing_child(
            carrier.clone(),
            original_input,
            errors,
            ErrorCode::MissingSpeaker,
            "Missing speaker in main tier",
        )),
    };
    // The producer classifies is_missing before constructing Present. Keep
    // that proof instead of erasing it and rediscovering it from byte width.
    match main.child_2.slot() {
        NodeSlot::Present(_) => {}
        NodeSlot::Missing(colon) => {
            report_empty_colon(*colon, original_input, errors);
        }
        NodeSlot::Error(node) => {
            report_unexpected_child(*node, source, errors, "colon", COLON_POSITION);
        }
        NodeSlot::Unexpected(never) => match *never {},
        NodeSlot::Absent(NoChild) => {
            report_missing_child(
                carrier.clone(),
                original_input,
                errors,
                ErrorCode::MissingColonAfterSpeaker,
                "Missing colon (:) after speaker in main tier",
            );
        }
    }

    // Tab identity also survives MISSING recovery without reclassification.
    match main.child_3.slot().known_or_placeholder() {
        KindSlotValue::Present(_) | KindSlotValue::Placeholder(_) => {}
        KindSlotValue::Error(node) => {
            report_unexpected_child(node, source, errors, "tab", TAB_POSITION);
        }
        KindSlotValue::Absent(NoChild) => {
            report_missing_child(
                carrier,
                original_input,
                errors,
                ErrorCode::StructuralOrderError,
                "Missing tab after colon in main tier",
            );
        }
    }

    PrefixData { speaker }
}

/// Report the `EmptyColon` diagnostic for a zero-width colon slot.
///
/// Reproduces the previous inline diagnostic byte-identically: the location uses
/// the colon node's (zero-width) span, the context spans the full original input,
/// and the suggestion is preserved.
fn report_empty_colon(
    colon: KindMissing<ColonNode<'_>>,
    original_input: &str,
    errors: &impl ErrorSink,
) {
    let node = colon.node();
    errors.report(
        ParseError::new(
            ErrorCode::EmptyColon,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(original_input, 0..original_input.len(), original_input),
            "Empty colon (zero-width node) in main tier".to_string(),
        )
        .with_suggestion("Add ':' after speaker code (e.g., '*CHI:')"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::error::ErrorCollector;
    use crate::generated_traversal::FromNodeKind;

    #[test]
    fn real_speakers_admit_text_and_span_only_from_their_parse_owner() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/content/linkers-multiple.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let foreign = parser
            .parse_source_incremental(source, None)
            .expect("separate parse owner");
        let mut pending = vec![parsed.root_node()];
        let mut checked = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(speaker) = SpeakerNode::from_node(node) else {
                continue;
            };
            let errors = ErrorCollector::new();
            let bound = parsed.bind_typed(speaker).expect("original parse owner");
            let admitted = ParsedSpeakerPrefix::admit(bound, &errors)
                .unwrap_or_else(|_| panic!("fixture speaker must be admitted"));
            let (code, span) = admitted.into_parts();
            assert_eq!(code, &source[node.byte_range()]);
            assert_eq!(
                span,
                Span::new(node.start_byte() as u32, node.end_byte() as u32)
            );
            assert!(errors.to_vec().is_empty());
            assert!(
                foreign.bind_typed(speaker).is_err(),
                "equal text is not shared ownership"
            );
            checked += 1;
        }
        assert!(checked > 0, "fixture must exercise speaker admission");
    }
}
