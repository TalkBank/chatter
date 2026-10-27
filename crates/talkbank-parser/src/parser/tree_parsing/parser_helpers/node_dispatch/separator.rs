//! Separator-node parsing utilities.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{
    AsRawNode, Extract, NoChild, NodeSlot, NonColonSeparatorChoice, NonColonSeparatorNode,
    SeparatorChoice, SeparatorNode, SourceBound, extract_non_colon_separator,
};
use crate::model::Separator;
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_displaced};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// The single model mapping for the generated non-colon separator choice.
/// Kind classification belongs to the generator; this match supplies semantics.
fn separator_for(choice: &NonColonSeparatorChoice<'_>) -> Separator {
    // The span comes from the CHOICE, not from each variant's payload. It used
    // to be `span_of(n.raw_node())` written out fourteen times, which is the
    // same fact re-derived per arm: `NonColonSeparatorChoice` implements
    // `AsRawNode` by delegating to whichever variant it holds, so the two are
    // equal by construction and one of them was redundant. Now each arm names
    // only what actually varies, which is the model variant.
    let span = span_of(choice.raw_node());
    match choice {
        NonColonSeparatorChoice::Comma(_) => Separator::Comma {
            location: span.into(),
        },
        NonColonSeparatorChoice::Semicolon(_) => Separator::Semicolon {
            location: span.into(),
        },
        NonColonSeparatorChoice::TagMarker(_) => Separator::Tag {
            location: span.into(),
        },
        NonColonSeparatorChoice::VocativeMarker(_) => Separator::Vocative {
            location: span.into(),
        },
        NonColonSeparatorChoice::CaContinuationMarker(_) => Separator::CaContinuation {
            location: span.into(),
        },
        NonColonSeparatorChoice::UnmarkedEnding(_) => Separator::UnmarkedEnding {
            location: span.into(),
        },
        NonColonSeparatorChoice::UptakeSymbol(_) => Separator::Uptake {
            location: span.into(),
        },
        NonColonSeparatorChoice::CaNoBreak(_) => Separator::CaNoBreak {
            location: span.into(),
        },
        NonColonSeparatorChoice::CaTechnicalBreak(_) => Separator::CaTechnicalBreak {
            location: span.into(),
        },
        NonColonSeparatorChoice::RisingToHigh(_) => Separator::RisingToHigh {
            location: span.into(),
        },
        NonColonSeparatorChoice::RisingToMid(_) => Separator::RisingToMid {
            location: span.into(),
        },
        NonColonSeparatorChoice::LevelPitch(_) => Separator::Level {
            location: span.into(),
        },
        NonColonSeparatorChoice::FallingToMid(_) => Separator::FallingToMid {
            location: span.into(),
        },
        NonColonSeparatorChoice::FallingToLow(_) => Separator::FallingToLow {
            location: span.into(),
        },
    }
}

/// Parse a `non_colon_separator` node into a `Separator`.
///
/// **Grammar Rule:**
/// ```text
/// separator: $ => prec(-1, choice($.non_colon_separator, $.colon)),
///
/// non_colon_separator: $ => choice(
///   $.comma, $.semicolon, $.tag_marker, $.vocative_marker,
///   $.ca_continuation_marker, $.unmarked_ending, $.uptake_symbol,
///   $.ca_no_break, $.ca_technical_break,
///   $.rising_to_high, $.rising_to_mid, $.level_pitch,
///   $.falling_to_mid, $.falling_to_low,
/// ),
/// ```
///
/// **Expected Structure:**
/// - Position 0: `non_colon_separator` OR `colon` (the rule has no other member)
fn parse_non_colon_separator_node(
    typed: NonColonSeparatorNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Separator> {
    let node = typed.raw_node();
    let children = extract_non_colon_separator(typed);
    // A MISSING node of a kind the choice does not name is not its own, so it
    // lands here rather than in the slot; surface it like any displaced child.
    surface_displaced(&children.unexpected, "non_colon_separator", source, errors);
    match children.content.slot() {
        NodeSlot::Present(choice) => ParseOutcome::parsed(separator_for(choice)),
        // Missing-kind evidence must never be promoted to a present choice:
        // doing so rendered an unauthored CA marker for a terminator-only tier.
        NodeSlot::Missing(missing) => {
            check_not_missing(missing.raw_node(), source, errors, "non_colon_separator");
            ParseOutcome::rejected()
        }
        // A CHILDLESS `non_colon_separator`. This used to try to classify the
        // node ITSELF, carried over from the pre-migration
        // `if child_count > 0 { child(0) } else { Some(node) }`. That branch
        // was dead in both forms and is gone: `node` here is always of kind
        // `non_colon_separator`, which is the PARENT rule and not one of the
        // fourteen alternatives, so no classifier can ever answer for it.
        NodeSlot::Absent(NoChild) => {
            reject(node, source, errors, "non_colon_separator has no children")
        }
        NodeSlot::Error(error) => reject(
            *error,
            source,
            errors,
            "non_colon_separator contains an ERROR node",
        ),
        NodeSlot::Unexpected(never) => match *never {},
    }
}

/// Report a tree-parsing rejection at `node`.
fn reject(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
    message: impl Into<String>,
) -> ParseOutcome<Separator> {
    errors.report(ParseError::new(
        ErrorCode::TreeParsingError,
        Severity::Error,
        DiagnosticLocation::new(span_of(node)),
        ErrorContext::new(source, span_of(node), ""),
        message.into(),
    ));
    ParseOutcome::rejected()
}

/// Parse a full `separator` CST node.
///
/// Driven by the generated carrier: `separator` is
/// `prec(-1, choice($.non_colon_separator, $.colon))`, and its generated
/// extraction already owns that choice as `SeparatorChoice`. This used to walk
/// `node.child(0)` by hand, classify one alternative, compare the other against
/// a kind constant, and report a MESSAGE that restated the grammar's own choice
/// in prose ("Expected 'non_colon_separator' or 'colon' in separator, found
/// ..."). That message was a third owner of a fact the grammar states.
///
/// Only a present choice can produce a model separator. Missing tokens remain
/// recovery evidence, even when their expected punctuation kind is known.
fn parse_separator_contents(
    typed: SeparatorNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Separator> {
    let node = typed.raw_node();
    // Admitted, so a MISSING `non_colon_separator` (a rule) is an uninhabited
    // type; only a MISSING `colon` can arrive.
    let children = match crate::parser::typed_cst::canonical_grammar()
        .and_then(|grammar| typed.extract_admitted(grammar).map_err(Into::into))
    {
        Ok(children) => children,
        Err(fault) => {
            crate::parser::typed_cst::report_cst_failure(node, source, fault, errors);
            return ParseOutcome::rejected();
        }
    };
    surface_displaced(&children.unexpected, "separator", source, errors);

    let choice = match children.content.slot() {
        NodeSlot::Present(choice) => *choice,
        NodeSlot::Missing(missing) => {
            check_not_missing(missing.raw_node(), source, errors, "separator");
            return ParseOutcome::rejected();
        }
        NodeSlot::Error(error) => {
            return reject(*error, source, errors, "separator contains an ERROR node");
        }
        NodeSlot::Unexpected(never) => match *never {},
        NodeSlot::Absent(NoChild) => {
            return reject(
                node,
                source,
                errors,
                "Separator node has no separator content",
            );
        }
    };
    match choice {
        SeparatorChoice::NonColonSeparator(non_colon) => {
            parse_non_colon_separator_node(non_colon, source, errors)
        }
        SeparatorChoice::Colon(colon) => ParseOutcome::parsed(Separator::Colon {
            location: span_of(colon.raw_node()).into(),
        }),
    }
}

/// Parse the separator alternative produced by `contents`.
///
/// The producer already classified the carrier. Bare colon/non-colon leaves
/// cannot enter this boundary; recovery inside the separator remains explicit.
pub(crate) fn parse_separator_node<'tree>(
    typed: SourceBound<'tree, '_, SeparatorNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Separator> {
    let node = typed.raw_node();
    // Preserve the childless recovery policy: only the three punctuation
    // spellings have a text interpretation, and empty/unknown text rejects.
    if node.child_count() == 0 {
        let span = span_of(node);
        return match typed.text() {
            ":" => ParseOutcome::parsed(Separator::Colon {
                location: span.into(),
            }),
            "," => ParseOutcome::parsed(Separator::Comma {
                location: span.into(),
            }),
            ";" => ParseOutcome::parsed(Separator::Semicolon {
                location: span.into(),
            }),
            _ => ParseOutcome::rejected(),
        };
    }
    parse_separator_contents(typed.node(), typed.source(), errors)
}
