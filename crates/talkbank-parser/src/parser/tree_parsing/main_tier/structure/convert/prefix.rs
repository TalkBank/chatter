//! Prefix-field extraction for `main_tier` conversion.
//!
//! Driven by the generated typed visitor: the speaker prefix is read from the
//! `extract_main_tier` slots (`star`, `speaker`, `colon`, `tab`) instead of a
//! positional `node.kind()` hand-walk. Per-rule admission excludes Error and
//! Absent at these required positions, not lexical MISSING recovery. Missing
//! speaker and colon diagnostics retain their codes, wording and owning source;
//! star/tab placeholders retain their existing acceptance policy. Parent-level
//! recovery and the unexpected sink remain the converter's responsibility.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speaker_ID>

use crate::error::{
    DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, Span,
};
use crate::generated_traversal::{
    AsRawNode, ColonNode, SourceBound, SourceChildren, SourceField, SourceSlotView, SpeakerNode,
};
use crate::parser::typed_cst::cst_failure_diagnostic;

use super::{PrefixData, ReportedMainTierError, RequiredMainTierChildren};
use crate::parser::node_span::span_of;

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
            span: span_of(raw),
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
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(source, span_of(node), ""),
            "Missing speaker in main tier",
        )
        .with_suggestion("Main tier should start with *SPEAKER:"),
        errors,
    )
}

/// Parse `*`, speaker code, colon, and tab from the typed `main_tier` slots.
///
/// `main` is the result of `extract_main_tier`: `child_0` (`star`), `speaker`,
/// `child_2` (`colon`), `child_3` (`tab`). The valid path (all four `Present`)
/// emits no diagnostics and yields the speaker string + span; the remaining slot
/// states reproduce the prior recovery behavior.
pub(super) fn parse_prefix<'tree>(
    associated: &SourceChildren<'tree, '_, RequiredMainTierChildren<'tree>>,
    errors: &impl ErrorSink,
) -> PrefixData {
    // Required-position admission excludes ERROR and absence. Present and
    // lexical MISSING stars/tabs retain the existing acceptance policy.

    // Retain the generated speaker type through checked text admission. A
    // MISSING placeholder is zero-width recovery, never an admitted code.
    let speaker = match associated.field_speaker().slot().view() {
        SourceSlotView::Present(speaker_node) => read_speaker(speaker_node, errors)
            .and_then(|node| ParsedSpeakerPrefix::admit(node, errors)),
        SourceSlotView::Missing(node) => read_speaker(node, errors)
            .and_then(|node| Err(report_empty_speaker(node.node(), node.source(), errors))),
    };
    // The producer classifies is_missing before constructing Present. Keep
    // that proof instead of erasing it and rediscovering it from byte width.
    match associated.field_child_2().slot().view() {
        SourceSlotView::Present(_) => {}
        SourceSlotView::Missing(colon) => {
            report_empty_colon(colon, errors);
        }
    }

    PrefixData { speaker }
}

/// Report the `EmptyColon` diagnostic for a zero-width colon slot.
///
/// Location and context use the same zero-width colon range in the complete
/// parse source. The diagnostic code, message and suggestion are preserved.
fn report_empty_colon(colon: SourceField<'_, '_, '_, ColonNode<'_>>, errors: &impl ErrorSink) {
    let node = colon.raw_node();
    errors.report(
        ParseError::new(
            ErrorCode::EmptyColon,
            Severity::Error,
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(colon.source(), span_of(node), ""),
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
            assert_eq!(span, span_of(node));
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
