//! Word-level `%mor` parsing.
//!
//! Parses a morphology token into POS, lemma, and optional feature list.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

use crate::generated_traversal::{
    AsRawNode, MorFeatureNode, MorFeatureValueNode, MorWordNode, Never, ReadableSlot, SlotView,
    SourceBound, SourceBoundKind,
};
use talkbank_model::ParseOutcome;
use talkbank_model::model::dependent_tier::{MorFeature, MorWord, PosCategory};
use talkbank_model::{
    DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity,
};
use tree_sitter::Node;

use crate::node_types::{MOR_FEATURE, MOR_WORD};
use crate::parser::node_span::span_of;
use crate::parser::tier_parsers::admission::recovery_inside_healthy_tier;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;

/// A required readable lexical prefix field. Its type admits `Missing`; the
/// tier's admission says it never is (see [`non_empty_text`]).
type RequiredLexicalView<'value, 'tree, 'source, T> = SlotView<
    'value,
    SourceBound<'tree, 'source, T>,
    SourceBound<'tree, 'source, T>,
    Never,
    Never,
    Never,
>;

/// Converts a `mor_word` CST node into `MorWord`.
///
/// **Grammar Rule:**
/// ```text
/// mor_word: $ => seq(
///     $.mor_pos,
///     $.pipe,
///     $.mor_lemma,
///     repeat($.mor_feature)
/// )
/// ```
///
/// Driven by the generated typed visitor: `extract_mor_word` yields the POS,
/// pipe, lemma and feature-repeat positions as typed slots. Compiled admission
/// excludes MISSING feature nodes, and the compiled prefix proof excludes
/// Error and Absent for POS, pipe and lemma. The slot types still admit
/// lexical `Missing` and repeated-feature `Error`, but this word belongs to a
/// tier admitted as recovery-free (see the tier module's doc), so those arms
/// contradict the admission and report the producer fault, then reject the
/// word. The E342 examples 4 and 5 that name these slots are reported by the
/// dispatcher's whole-tier walk before admission. Source-binding and
/// reconstruction faults propagate separately as `CstFailure` to the owning
/// item boundary. Selected ranges are admitted before lexical lowering; this
/// preserves the compiled kind proof, not nonempty token text (an empty POS
/// or lemma text is a lexical refusal the grammar's regexes do not produce;
/// the check stays because the slot type does not say so).
pub fn parse_mor_word<'tree>(
    typed: SourceBound<'tree, '_, MorWordNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<MorWord>, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();
    let bound_children = crate::parser::typed_cst::extract_inner(typed)?;
    surface_displaced(
        &bound_children.children().unexpected,
        "mor_word",
        source,
        errors,
    );
    let bound_children = bound_children.admit_ranges()?;
    let children = bound_children.children();

    let pos = match non_empty_text(
        children.child_0.slot.view(),
        "MOR word has empty POS tag",
        errors,
    ) {
        LexicalRead::Text(text) => Some(text),
        LexicalRead::Empty => None,
        LexicalRead::Contradicted => return Ok(ParseOutcome::rejected()),
    };

    // The pipe separator is purely structural.
    match children.child_1.slot.view() {
        SlotView::Present(_) => {}
        SlotView::Missing(missing) => {
            recovery_inside_healthy_tier(missing.raw_node(), missing.source(), MOR_WORD, errors);
            return Ok(ParseOutcome::rejected());
        }
    }

    let lemma = match non_empty_text(
        children.child_2.slot.view(),
        "MOR word has empty lemma",
        errors,
    ) {
        LexicalRead::Text(text) => Some(text),
        LexicalRead::Empty => None,
        LexicalRead::Contradicted => return Ok(ParseOutcome::rejected()),
    };

    let mut features = Vec::new();
    for element in &children.child_3.slot {
        match &element.slot {
            ReadableSlot::Present(feature) => {
                if let ParseOutcome::Parsed(Some(feature)) = parse_mor_feature(*feature, errors)? {
                    features.push(feature);
                }
            }
            ReadableSlot::Error(bad) => {
                recovery_inside_healthy_tier(bad.raw_node(), bad.source(), MOR_WORD, errors);
                return Ok(ParseOutcome::rejected());
            }
            ReadableSlot::Missing(missing) => match missing.0 {},
            ReadableSlot::Unexpected(never) | ReadableSlot::Absent(never) => match *never {},
        }
    }

    let Some(pos) = pos else {
        errors.report(missing_part(
            node,
            "MOR word is missing required POS tag",
            source,
        ));
        return Ok(ParseOutcome::rejected());
    };

    let Some(lemma) = lemma else {
        errors.report(missing_part(
            node,
            "MOR word is missing required lemma",
            source,
        ));
        return Ok(ParseOutcome::rejected());
    };

    Ok(ParseOutcome::parsed(
        MorWord::new(PosCategory::new(pos), lemma).with_features(features),
    ))
}

/// What reading a required lexical slot found. Three outcomes, because the
/// two refusals must not be confused: an empty text is a lexical refusal the
/// word reports as E342, while a `Missing` slot contradicts the tier's
/// admission, has already been reported as the producer fault, and must not
/// be followed by a CHAT diagnostic about a POS or lemma nobody read.
enum LexicalRead<'source> {
    Text(&'source str),
    Empty,
    Contradicted,
}

/// The text of a present POS or lemma node, or why there is none.
/// An empty node is not something the grammar produces for either token; the
/// check remains because the slot's type does not say so.
fn non_empty_text<'tree, 'source, T: SourceBoundKind<'tree>>(
    slot: RequiredLexicalView<'_, 'tree, 'source, T>,
    empty_message: &'static str,
    errors: &impl ErrorSink,
) -> LexicalRead<'source> {
    let bound = match slot {
        SlotView::Present(field) => *field,
        SlotView::Missing(missing) => {
            recovery_inside_healthy_tier(missing.raw_node(), missing.source(), MOR_WORD, errors);
            return LexicalRead::Contradicted;
        }
    };
    let text = bound.text();
    if text.is_empty() {
        // Range admission is not nonempty admission. A checked empty slice
        // remains a lexical refusal, never a manufactured source-read result.
        errors.report(missing_part(
            bound.raw_node(),
            empty_message,
            bound.source(),
        ));
        return LexicalRead::Empty;
    }
    LexicalRead::Text(text)
}

/// E342 at `node` for a `%mor` word part the word needs and does not have.
fn missing_part(node: Node, message: &'static str, source: &str) -> ParseError {
    ParseError::new(
        ErrorCode::MissingRequiredElement,
        Severity::Error,
        DiagnosticLocation::new(span_of(node)),
        ErrorContext::new(source, span_of(node), node.kind()),
        message,
    )
}

/// Converts one `mor_feature` CST node (`-feature`).
///
/// **Grammar Rule:**
/// ```text
/// mor_feature: $ => seq($.hyphen, $.mor_feature_value)
/// ```
///
/// Returns a [`MorFeature`] wrapping the feature value text (without the leading hyphen).
///
/// Driven by the generated typed visitor: `extract_mor_feature` yields the
/// hyphen and feature-value positions as typed `Positioned` slots. A MISSING
/// node at either position contradicts the tier's admission as recovery-free
/// and reports the producer fault, like every other position in this file.
fn parse_mor_feature<'tree>(
    typed: SourceBound<'tree, '_, MorFeatureNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<Option<MorFeature>>, crate::CstFailure> {
    let source = typed.source();
    let bound_children = crate::parser::typed_cst::extract_inner(typed)?;
    surface_displaced(
        &bound_children.children().unexpected,
        "mor_feature",
        source,
        errors,
    );
    let bound_children = bound_children.admit_ranges()?;
    let children = bound_children.children();

    match children.child_0.slot.view() {
        SlotView::Present(_) => {}
        SlotView::Missing(missing) => {
            recovery_inside_healthy_tier(missing.raw_node(), missing.source(), MOR_FEATURE, errors);
            return Ok(ParseOutcome::rejected());
        }
    }

    match children.child_1.slot.view() {
        SlotView::Present(value_node) => {
            if let Some(feature) = decode_feature_value(*value_node, errors) {
                return Ok(ParseOutcome::parsed(Some(feature)));
            }
        }
        SlotView::Missing(missing) => {
            recovery_inside_healthy_tier(missing.raw_node(), missing.source(), MOR_FEATURE, errors);
            return Ok(ParseOutcome::rejected());
        }
    }

    Ok(ParseOutcome::parsed(None))
}

/// Decode a present `mor_feature_value` node; an empty text is a lexical
/// refusal the grammar's regex should make impossible, reported rather than
/// manufactured into a feature.
fn decode_feature_value<'tree>(
    typed: SourceBound<'tree, '_, MorFeatureValueNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<MorFeature> {
    let node = typed.raw_node();
    let text = typed.text();
    if !text.is_empty() {
        Some(MorFeature::new(text))
    } else {
        errors.report(unexpected_node_error(
            node,
            typed.source(),
            "mor_feature_value empty",
        ));
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::generated_traversal::SourceBindingError;
    use talkbank_model::ErrorCollector;

    #[test]
    fn real_feature_values_require_readable_source() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/tiers/mor-gra.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let other = parser
            .parse_source_incremental(source, None)
            .expect("independent owner");
        let mut checked = 0;
        for node in parsed.root().expect("root").descendants() {
            let Some(feature) = node.expect("readable node").typed::<MorFeatureValueNode>() else {
                continue;
            };
            if feature.text() != "Fin" {
                continue;
            }
            checked += 1;
            let errors = ErrorCollector::new();
            assert_eq!(
                decode_feature_value(feature, &errors),
                Some(MorFeature::new("Fin"))
            );
            assert!(errors.to_vec().is_empty());
            // The decoder has no independent text parameter. Equal bytes in
            // another parse cannot supply the ownership needed to call it.
            assert!(matches!(
                other.bind(feature.raw_node()),
                Err(SourceBindingError::ForeignTree)
            ));
        }
        assert!(checked > 0, "fixture must supply finite-verb features");
    }
}
