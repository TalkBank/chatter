//! Word-level `%mor` parsing.
//!
//! Parses a morphology token into POS, lemma, and optional feature list.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

use crate::generated_traversal::{
    AsRawNode, KindSlot, MorFeatureNode, MorFeatureValueNode, MorWordNode, NoChild, SourceBound,
    SourceBoundKind, SourceField, SourceSlotView,
};
use talkbank_model::ParseOutcome;
use talkbank_model::model::dependent_tier::{MorFeature, MorWord, PosCategory};
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{
    check_not_missing, expect_delimiter, expect_structure, surface_displaced,
};

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
/// pipe, lemma and feature-repeat positions as typed slots, and every
/// recovery state retains its established missing/error handling. Structural
/// separators use [`expect_structure`]. Tier admission routes parser errors to file-level
/// analysis, but does not narrow the reconstructed slot types. Recovery remains
/// explicit in these shared verbs; absence of a finite-corpus witness is not a
/// proof that a slot state is impossible.
/// Source-binding and reconstruction faults propagate separately as `CstFailure`
/// to the owning item boundary, never as a missing POS/lemma diagnostic.
pub fn parse_mor_word<'tree>(
    typed: SourceBound<'tree, '_, MorWordNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<MorWord>, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();
    let bound_children = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    let children = bound_children.children();
    surface_displaced(&children.unexpected, "mor_word", source, errors);

    let pos = non_empty_text(
        bound_children.field_child_0().slot(),
        "MOR word has empty POS tag",
        errors,
    )?;

    // The pipe separator is purely structural.
    expect_structure(children.child_1.slot(), "mor_word", source, errors, |bad| {
        errors.report(unexpected_node_error(bad, source, "mor_word"));
    });

    let lemma = non_empty_text(
        bound_children.field_child_2().slot(),
        "MOR word has empty lemma",
        errors,
    )?;

    let mut features = Vec::new();
    for element in bound_children.field_child_3().slot().iter() {
        match element.slot().view() {
            SourceSlotView::Present(feature) => {
                if let ParseOutcome::Parsed(Some(feature)) =
                    parse_mor_feature(feature.read()?, errors)?
                {
                    features.push(feature);
                }
            }
            SourceSlotView::Error(bad) => {
                errors.report(unexpected_node_error(bad.raw_node(), source, "mor_word"))
            }
            SourceSlotView::Unexpected(never) => match never {},
            SourceSlotView::Absent(never) => match never {},
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

/// The text of a present POS or lemma node, or a report that it is empty.
/// An empty node is not something the grammar produces for either token; the
/// check remains because the slot's type does not say so.
fn non_empty_text<'tree, 'source, T: SourceBoundKind<'tree>>(
    slot: SourceField<'_, 'tree, 'source, KindSlot<'tree, T>>,
    empty_message: &'static str,
    errors: &impl ErrorSink,
) -> Result<Option<&'source str>, crate::CstFailure> {
    let source = slot.source();
    let bound = match slot.view() {
        SourceSlotView::Present(field) => field.read()?,
        SourceSlotView::Missing(missing) => {
            check_not_missing(missing.raw_node(), source, errors, "mor_word");
            return Ok(None);
        }
        SourceSlotView::Error(bad) => {
            errors.report(unexpected_node_error(bad.raw_node(), source, "mor_word"));
            return Ok(None);
        }
        SourceSlotView::Unexpected(never) => match never {},
        SourceSlotView::Absent(NoChild) => return Ok(None),
    };
    let text = bound.text();
    if text.is_empty() {
        // A successful empty slice is genuinely zero-width; failed reads
        // returned above and cannot masquerade as empty text.
        errors.report(missing_part(bound.raw_node(), empty_message, source));
        return Ok(None);
    }
    Ok(Some(text))
}

/// E342 at `node` for a `%mor` word part the word needs and does not have.
fn missing_part(node: Node, message: &'static str, source: &str) -> ParseError {
    ParseError::new(
        ErrorCode::MissingRequiredElement,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
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
/// hyphen and feature-value positions as typed `Positioned` slots. UNLIKE
/// [`parse_mor_word`] above, the removed walk here called NO `check_not_missing`
/// gate at all: it dispatched purely by `child.kind()`, and a tree-sitter
/// MISSING placeholder still carries its expected kind, so a MISSING hyphen
/// fell into the same no-op arm as a present one, and a MISSING
/// `mor_feature_value` fell into the same `utf8_text` decode as a present one
/// (reading a zero-width MISSING node's text yields an empty string, which the
/// removed code's own "empty" arm already handled). This migration reproduces
/// that distinction faithfully: `Present` and `Missing` share identical
/// handling here, unlike every other position in this file.
fn parse_mor_feature<'tree>(
    typed: SourceBound<'tree, '_, MorFeatureNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<Option<MorFeature>>, crate::CstFailure> {
    let source = typed.source();
    let bound_children = typed.extract()?;
    let children = bound_children.children();
    surface_displaced(&children.unexpected, "mor_feature", source, errors);

    expect_delimiter(children.child_0.slot(), |bad| {
        errors.report(unexpected_node_error(bad, source, "mor_feature"));
    });

    match bound_children.field_child_1().slot().view() {
        SourceSlotView::Present(value_node) | SourceSlotView::Missing(value_node) => {
            if let Some(feature) = decode_feature_value(value_node.read()?, errors) {
                return Ok(ParseOutcome::parsed(Some(feature)));
            }
        }
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(raw.raw_node(), source, "mor_feature"));
        }
        SourceSlotView::Unexpected(never) => match never {},
        SourceSlotView::Absent(NoChild) => {}
    }

    Ok(ParseOutcome::parsed(None))
}

/// Shared decode for a `mor_feature_value` node, applied identically whether
/// the node arrived via a `Present` or a `Missing` slot (see
/// [`parse_mor_feature`]'s doc comment for why both must share this logic).
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
