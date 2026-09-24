//! Word-level `%mor` parsing.
//!
//! Parses a morphology token into POS, lemma, and optional feature list.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

use crate::generated_traversal::{
    AsRawNode, KindSlotValue, MorFeatureNode, MorFeatureValueNode, MorWordNode, NoChild,
    extract_mor_feature, extract_mor_word,
};
use talkbank_model::ParseOutcome;
use talkbank_model::model::dependent_tier::{MorFeature, MorWord, PosCategory};
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{
    SlotState, expect_delimiter, expect_present, expect_structure, extract_utf8_text,
    surface_displaced,
};
use crate::parser::typed_cst::admit_node_text;

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
/// recovery state is reported through the shared [`expect_present`] and
/// [`expect_structure`] verbs. Tier admission routes parser errors to file-level
/// analysis, but does not narrow the reconstructed slot types. Recovery remains
/// explicit in these shared verbs; absence of a finite-corpus witness is not a
/// proof that a slot state is impossible.
pub fn parse_mor_word(
    typed: MorWordNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorWord> {
    let node = typed.raw_node();
    let children = extract_mor_word(typed);
    surface_displaced(&children.unexpected, "mor_word", source, errors);

    let pos = match expect_present(children.child_0.slot(), "mor_word", source, errors) {
        SlotState::Present(pos_node) => non_empty_text(
            pos_node.raw_node(),
            "MOR word has empty POS tag",
            source,
            errors,
        ),
        SlotState::Absent | SlotState::Recovered => None,
    };

    // The pipe separator is purely structural.
    expect_structure(children.child_1.slot(), "mor_word", source, errors, |bad| {
        errors.report(unexpected_node_error(bad, source, "mor_word"));
    });

    let lemma = match expect_present(children.child_2.slot(), "mor_word", source, errors) {
        SlotState::Present(lemma_node) => non_empty_text(
            lemma_node.raw_node(),
            "MOR word has empty lemma",
            source,
            errors,
        ),
        SlotState::Absent | SlotState::Recovered => None,
    };

    let mut features = Vec::new();
    for element in children.child_3.slot() {
        if let SlotState::Present(feature_node) =
            expect_present(element.slot(), "mor_word", source, errors)
            && let ParseOutcome::Parsed(Some(feature)) =
                parse_mor_feature(*feature_node, source, errors)
        {
            features.push(feature);
        }
    }

    let Some(pos) = pos else {
        errors.report(missing_part(
            node,
            "MOR word is missing required POS tag",
            source,
        ));
        return ParseOutcome::rejected();
    };

    let Some(lemma) = lemma else {
        errors.report(missing_part(
            node,
            "MOR word is missing required lemma",
            source,
        ));
        return ParseOutcome::rejected();
    };

    ParseOutcome::parsed(MorWord::new(PosCategory::new(pos), lemma).with_features(features))
}

/// The text of a present POS or lemma node, or a report that it is empty.
/// An empty node is not something the grammar produces for either token; the
/// check remains because the slot's type does not say so.
fn non_empty_text<'a>(
    node: Node,
    empty_message: &'static str,
    source: &'a str,
    errors: &impl ErrorSink,
) -> Option<&'a str> {
    let talkbank_model::ParseOutcome::Parsed(text) =
        extract_utf8_text(node, source, errors, "mor_word")
    else {
        return None;
    };
    if text.is_empty() {
        // A successful empty slice is genuinely zero-width; failed reads
        // returned above and cannot masquerade as empty text.
        errors.report(missing_part(node, empty_message, source));
        return None;
    }
    Some(text)
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
fn parse_mor_feature(
    typed: MorFeatureNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<MorFeature>> {
    let children = extract_mor_feature(typed);
    surface_displaced(&children.unexpected, "mor_feature", source, errors);

    expect_delimiter(children.child_0.slot(), |bad| {
        errors.report(unexpected_node_error(bad, source, "mor_feature"));
    });

    match children.child_1.slot().known_or_placeholder() {
        KindSlotValue::Present(value_node) | KindSlotValue::Placeholder(value_node) => {
            if let Some(feature) = decode_feature_value(value_node, source, errors) {
                return ParseOutcome::parsed(Some(feature));
            }
        }
        KindSlotValue::Error(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_feature"));
        }
        KindSlotValue::Absent(NoChild) => {}
    }

    ParseOutcome::parsed(None)
}

/// Shared decode for a `mor_feature_value` node, applied identically whether
/// the node arrived via a `Present` or a `Missing` slot (see
/// [`parse_mor_feature`]'s doc comment for why both must share this logic).
fn decode_feature_value(
    typed: MorFeatureValueNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Option<MorFeature> {
    let node = typed.raw_node();
    match admit_node_text(node, source) {
        Ok(text) if !text.is_empty() => Some(MorFeature::new(text)),
        Ok(_) => {
            errors.report(unexpected_node_error(
                node,
                source,
                "mor_feature_value empty",
            ));
            None
        }
        Err(_) => {
            errors.report(unexpected_node_error(
                node,
                source,
                "mor_feature_value utf8 error",
            ));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::generated_traversal::FromNodeKind;
    use talkbank_model::{ErrorCollector, Span};

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
        let foreign = "é".repeat(source.len());
        let mut pending = vec![parsed.root_node()];
        let mut checked = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(feature) = MorFeatureValueNode::from_node(node) else {
                continue;
            };
            if source.get(node.byte_range()) != Some("Fin") {
                continue;
            }
            checked += 1;
            let errors = ErrorCollector::new();
            assert_eq!(
                decode_feature_value(feature, source, &errors),
                Some(MorFeature::new("Fin"))
            );
            assert!(errors.to_vec().is_empty());
            // An odd-width real token cuts a code point in this foreign source.
            // These are boundary witnesses, not grammar-produced invalid values.
            for incompatible in ["", foreign.as_str()] {
                let errors = ErrorCollector::new();
                assert!(decode_feature_value(feature, incompatible, &errors).is_none());
                let diagnostics = errors.into_vec();
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].code, ErrorCode::UnexpectedNodeInContext);
                assert_eq!(
                    diagnostics[0].location.span,
                    Span::from_usize(node.start_byte(), node.end_byte())
                );
                assert!(
                    diagnostics[0]
                        .message
                        .contains("mor_feature_value utf8 error")
                );
            }
        }
        assert!(checked > 0, "fixture must supply finite-verb features");
    }
}
