//! Word-level `%mor` parsing.
//!
//! Parses a morphology token into POS, lemma, and optional feature list.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

use crate::generated_traversal::{
    MorFeatureNode, MorWordNode, NodeSlot, extract_mor_feature, extract_mor_word,
};
use talkbank_model::ParseOutcome;
use talkbank_model::model::dependent_tier::{MorFeature, MorWord, PosCategory};
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_unexpected};

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
/// Driven by the generated typed visitor: `extract_mor_word` yields the POS /
/// pipe / lemma / feature-repeat positions as typed `Positioned` slots,
/// replacing the removed flat `while node.child(idx)` walk that ran
/// `check_not_missing` FIRST for every child (before any kind dispatch), then
/// matched by `child.kind()`. Each position below reproduces that exactly:
/// `Missing` reports the SAME `check_not_missing` diagnostic the removed loop
/// reported and skips any further decode (the removed loop never attempted a
/// `utf8_text` read on a child that failed `check_not_missing`); `Error` /
/// `Unexpected` fall through to the removed loop's `_ =>` arm
/// ([`unexpected_node_error`]).
pub fn parse_mor_word(node: Node, source: &str, errors: &impl ErrorSink) -> ParseOutcome<MorWord> {
    let children = extract_mor_word(MorWordNode(node));
    surface_unexpected(&children.unexpected, source, errors);

    let pos = match children.child_0.slot {
        NodeSlot::Present(pos_node) => {
            let field = pos_node.0;
            match field.utf8_text(source.as_bytes()) {
                Ok(text) if !text.is_empty() => Some(text),
                Ok(_) => {
                    errors.report(ParseError::new(
                        ErrorCode::MissingRequiredElement,
                        Severity::Error,
                        SourceLocation::from_offsets(field.start_byte(), field.end_byte()),
                        ErrorContext::new(
                            source,
                            field.start_byte()..field.end_byte(),
                            field.kind(),
                        ),
                        "MOR word has empty POS tag",
                    ));
                    None
                }
                Err(e) => {
                    errors.report(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(field.start_byte(), field.end_byte()),
                        ErrorContext::new(
                            source,
                            field.start_byte()..field.end_byte(),
                            field.kind(),
                        ),
                        format!("Failed to read MOR POS text: {e}"),
                    ));
                    None
                }
            }
        }
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, "mor_word");
            None
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_word"));
            None
        }
        NodeSlot::Absent => None,
    };

    // The pipe separator is purely structural (removed loop: `kind::PIPE =>
    // {}`); Missing/Error/Unexpected still report, matching the removed
    // loop's uniform per-child gate.
    match children.child_1.slot {
        NodeSlot::Present(_) | NodeSlot::Absent => {}
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, "mor_word");
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_word"));
        }
    }

    let lemma = match children.child_2.slot {
        NodeSlot::Present(lemma_node) => {
            let field = lemma_node.0;
            match field.utf8_text(source.as_bytes()) {
                Ok(text) if !text.is_empty() => Some(text),
                Ok(_) => {
                    errors.report(ParseError::new(
                        ErrorCode::MissingRequiredElement,
                        Severity::Error,
                        SourceLocation::from_offsets(field.start_byte(), field.end_byte()),
                        ErrorContext::new(
                            source,
                            field.start_byte()..field.end_byte(),
                            field.kind(),
                        ),
                        "MOR word has empty lemma",
                    ));
                    None
                }
                Err(e) => {
                    errors.report(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(field.start_byte(), field.end_byte()),
                        ErrorContext::new(
                            source,
                            field.start_byte()..field.end_byte(),
                            field.kind(),
                        ),
                        format!("Failed to read MOR lemma text: {e}"),
                    ));
                    None
                }
            }
        }
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, "mor_word");
            None
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_word"));
            None
        }
        NodeSlot::Absent => None,
    };

    let mut features = Vec::new();
    for element in children.child_3.slot {
        match element.slot {
            NodeSlot::Present(feature_node) => {
                if let ParseOutcome::Parsed(Some(feature)) =
                    parse_mor_feature(feature_node.0, source, errors)
                {
                    features.push(feature);
                }
            }
            // The removed loop's `check_not_missing`-first gate ran for
            // EVERY child, including feature positions, so a MISSING
            // `mor_feature` never reached `parse_mor_feature` at all.
            NodeSlot::Missing(raw) => {
                check_not_missing(raw, source, errors, "mor_word");
            }
            NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
                errors.report(unexpected_node_error(raw, source, "mor_word"));
            }
            NodeSlot::Absent => {}
        }
    }

    let Some(pos) = pos else {
        errors.report(ParseError::new(
            ErrorCode::MissingRequiredElement,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            "MOR word is missing required POS tag",
        ));
        return ParseOutcome::rejected();
    };

    let Some(lemma) = lemma else {
        errors.report(ParseError::new(
            ErrorCode::MissingRequiredElement,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            "MOR word is missing required lemma",
        ));
        return ParseOutcome::rejected();
    };

    ParseOutcome::parsed(MorWord::new(PosCategory::new(pos), lemma).with_features(features))
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
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<MorFeature>> {
    let children = extract_mor_feature(MorFeatureNode(node));
    surface_unexpected(&children.unexpected, source, errors);

    match children.child_0.slot {
        NodeSlot::Present(_) | NodeSlot::Missing(_) | NodeSlot::Absent => {}
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_feature"));
        }
    }

    match children.child_1.slot {
        NodeSlot::Present(value_node) => {
            if let Some(feature) = decode_feature_value(value_node.0, source, errors) {
                return ParseOutcome::parsed(Some(feature));
            }
        }
        NodeSlot::Missing(raw) => {
            if let Some(feature) = decode_feature_value(raw, source, errors) {
                return ParseOutcome::parsed(Some(feature));
            }
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_feature"));
        }
        NodeSlot::Absent => {}
    }

    ParseOutcome::parsed(None)
}

/// Shared decode for a `mor_feature_value` node, applied identically whether
/// the node arrived via a `Present` or a `Missing` slot (see
/// [`parse_mor_feature`]'s doc comment for why both must share this logic).
fn decode_feature_value(node: Node, source: &str, errors: &impl ErrorSink) -> Option<MorFeature> {
    match node.utf8_text(source.as_bytes()) {
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
