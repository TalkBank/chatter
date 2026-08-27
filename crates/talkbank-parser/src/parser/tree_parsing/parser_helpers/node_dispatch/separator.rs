//! Separator-node parsing utilities.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, FromNodeKind, NodeSlot, NonColonSeparatorChoice, NonColonSeparatorNode,
    SeparatorChoice, SeparatorNode, SlotValue, extract_non_colon_separator, extract_separator,
};
use crate::model::Separator;
use crate::node_types::COLON;
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// The single mapping from a non-colon separator node to its model variant.
///
/// Exhaustive over the GENERATED `NonColonSeparatorChoice`, which is what makes
/// it the only one. The same fourteen-way mapping used to be written twice in
/// this file as `match node.kind()` arms, once here and once in
/// `parse_separator_like`, with a third partial encoding matching the raw text
/// `":"`, `","`, `";"`. Two of those three are gone. Each was a closed set a
/// new grammar alternative could be added behind without breaking anything:
/// the `_ =>` arms turned an unhandled variant into a RUNTIME rejection, on a
/// file a user is trying to read.
///
/// The raw-text encoding in `parse_separator_like` SURVIVES, and deliberately:
/// it reads a childless `separator` node, which has no child kind to classify,
/// so there is nothing for this mapping to be given. It covers three
/// punctuation marks and does not fall through into the fourteen. Said plainly
/// here because this docstring is what an audit of "is the separator mapping
/// duplicated?" will find, and it previously answered no.
///
/// Now a new alternative in `non_colon_separator` fails to compile here, which
/// is the whole point of the generated traversal and is what
/// `book/src/architecture/parsing.md` means by dispatching on generated types
/// rather than on `node.kind()` strings.
fn separator_for(choice: &NonColonSeparatorChoice<'_>) -> Separator {
    // The span comes from the CHOICE, not from each variant's payload. It used
    // to be `span_of(n.raw_node())` written out fourteen times, which is the
    // same fact re-derived per arm: `NonColonSeparatorChoice` implements
    // `AsRawNode` by delegating to whichever variant it holds, so the two are
    // equal by construction and one of them was redundant. Now each arm names
    // only what actually varies, which is the model variant.
    let span = span_of(choice.raw_node());
    match choice {
        NonColonSeparatorChoice::Comma(_) => Separator::Comma { span },
        NonColonSeparatorChoice::Semicolon(_) => Separator::Semicolon { span },
        NonColonSeparatorChoice::TagMarker(_) => Separator::Tag { span },
        NonColonSeparatorChoice::VocativeMarker(_) => Separator::Vocative { span },
        NonColonSeparatorChoice::CaContinuationMarker(_) => Separator::CaContinuation { span },
        NonColonSeparatorChoice::UnmarkedEnding(_) => Separator::UnmarkedEnding { span },
        NonColonSeparatorChoice::UptakeSymbol(_) => Separator::Uptake { span },
        NonColonSeparatorChoice::CaNoBreak(_) => Separator::CaNoBreak { span },
        NonColonSeparatorChoice::CaTechnicalBreak(_) => Separator::CaTechnicalBreak { span },
        NonColonSeparatorChoice::RisingToHigh(_) => Separator::RisingToHigh { span },
        NonColonSeparatorChoice::RisingToMid(_) => Separator::RisingToMid { span },
        NonColonSeparatorChoice::LevelPitch(_) => Separator::Level { span },
        NonColonSeparatorChoice::FallingToMid(_) => Separator::FallingToMid { span },
        NonColonSeparatorChoice::FallingToLow(_) => Separator::FallingToLow { span },
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
    match children.content.slot() {
        NodeSlot::Present(choice) => ParseOutcome::parsed(separator_for(choice)),
        // Recovery states. The pre-migration code reached these through a
        // `node.child(0)` that could be absent or of an unmodeled kind, and
        // reported one generic message for all of them; the slot names which
        // happened, so the message can too.
        // A CHILDLESS `non_colon_separator`. This used to try to classify the
        // node ITSELF, carried over from the pre-migration
        // `if child_count > 0 { child(0) } else { Some(node) }`. That branch
        // was dead in both forms and is gone: `node` here is always of kind
        // `non_colon_separator`, which is the PARENT rule and not one of the
        // fourteen alternatives, so no classifier can ever answer for it.
        // (The `*CHI:\t.` fixture and the E253/E306 versus E330/E342 shift
        // belong to the MISSING arm below, which is what actually handles them;
        // the comment claiming them here sent readers to the wrong arm.)
        NodeSlot::Absent => reject(node, source, errors, "non_colon_separator has no children"),
        // A MISSING node is classified LIKE A PRESENT ONE, deliberately, and it
        // is the same call `main_tier/structure/contents.rs` records for its
        // own migration: the pre-migration `node.child(0)` never checked
        // `is_missing`, and a MISSING node still carries its kind, so it went
        // down the ordinary path. Recovery is not silently dropped by doing
        // this; the whole-tree recovery backstop still sees the node, and that
        // is the layer that owns "this document needed recovery".
        //
        // Measured: `*CHI:\t.` parses to a zero-width `separator` holding a
        // zero-width MISSING `ca_no_break`. Rejecting here instead moved that
        // fixture from E253/E306 to E330/E342, which the CHECK-parity manifest
        // caught. A migration that changes diagnostics is a behaviour change
        // wearing a refactor's clothes.
        NodeSlot::Missing(missing) => match classify(*missing) {
            Some(separator) => ParseOutcome::parsed(separator),
            None => reject(
                *missing,
                source,
                errors,
                "non_colon_separator is a MISSING placeholder",
            ),
        },
        NodeSlot::Error(error) => reject(
            *error,
            source,
            errors,
            "non_colon_separator contains an ERROR node",
        ),
        NodeSlot::Unexpected(other) => reject(
            *other,
            source,
            errors,
            format!("Unknown non_colon_separator kind '{}'", other.kind()),
        ),
    }
}

/// The `Separator` a node denotes, or `None` if it denotes none.
///
/// The two halves of reaching a model value from a bare node: ask the generated
/// classifier which alternative this kind is, then map that alternative. Both
/// callers want exactly this pair and differ only in what they do with the
/// `None`, so pairing them here keeps [`separator_for`] the sole mapping while
/// giving the classifier one call site per question rather than per caller.
///
/// NOT `NodeSlot::typed_or_placeholder`, which answers this same question for a
/// slot and would collapse the `Present` and `Missing` arms above into one. Its
/// `None` cannot say WHICH state produced it, and the four failing states here
/// carry four different diagnostics: a childless node, a MISSING placeholder of
/// no modeled kind, an ERROR subtree, and an unmodeled kind. Merging the top of
/// the match would force the slot to be re-matched underneath to recover the
/// distinction, with an unreachable arm for the two states already handled.
/// Same trade-off, and the same answer, as `tier_parsers/text/helpers.rs`.
fn classify(node: Node<'_>) -> Option<Separator> {
    NonColonSeparatorChoice::from_node(node).map(|choice| separator_for(&choice))
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
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
        message.into(),
    ));
    ParseOutcome::rejected()
}

/// Parse a full `separator` CST node.
///
/// Driven by the generated carrier: `separator` is
/// `prec(-1, choice($.non_colon_separator, $.colon))`, and `extract_separator`
/// already owns that choice as `SeparatorChoice`. This used to walk
/// `node.child(0)` by hand, classify one alternative, compare the other against
/// a kind constant, and report a MESSAGE that restated the grammar's own choice
/// in prose ("Expected 'non_colon_separator' or 'colon' in separator, found
/// ..."). That message was a third owner of a fact the grammar states.
///
/// The five slot states replace three hand-rolled outcomes, and say more than
/// they did. `Missing` is classified LIKE `Present`, the same call this file
/// already documents for `non_colon_separator`: a MISSING node carries the kind
/// the parser expected, and rejecting it instead moved `*CHI:\t.` from E253/E306
/// to E330/E342 once before.
fn parse_separator_node(
    typed: SeparatorNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Separator> {
    let node = typed.raw_node();
    let children = extract_separator(typed);
    surface_unexpected(&children.unexpected, source, errors);

    match children.content.slot().typed_or_placeholder() {
        SlotValue::Present(choice) | SlotValue::Placeholder(choice) => match choice {
            SeparatorChoice::NonColonSeparator(non_colon) => {
                parse_non_colon_separator_node(non_colon, source, errors)
            }
            SeparatorChoice::Colon(colon) => ParseOutcome::parsed(Separator::Colon {
                span: span_of(colon.raw_node()),
            }),
        },
        // A MISSING placeholder of a kind the choice does not name, or a present
        // node of one. The removed hand-walk reported both as "Expected
        // 'non_colon_separator' or 'colon'"; the kind is still named, without
        // this file restating what the grammar admits.
        SlotValue::UnclassifiedPlaceholder(other) | SlotValue::Unexpected(other) => reject(
            other,
            source,
            errors,
            format!("Unknown separator content '{}'", other.kind()),
        ),
        SlotValue::Error(error) => {
            reject(error, source, errors, "separator contains an ERROR node")
        }
        SlotValue::Absent => reject(
            node,
            source,
            errors,
            "Separator node has no separator content",
        ),
    }
}

/// Parse either `separator` or separator-like leaf nodes.
pub(crate) fn parse_separator_like(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Separator> {
    // Same hoist as in [`parse_separator_node`], for the same reason.
    if let Some(non_colon) = NonColonSeparatorNode::from_node(node) {
        return parse_non_colon_separator_node(non_colon, source, errors);
    }
    // `SeparatorNode::from_node` in place of the `SEPARATOR` constant, for the
    // same reason as the hoist above.
    if let Some(separator) = SeparatorNode::from_node(node) {
        // A childless `separator` is a recovery shape: the node exists but
        // holds no alternative, so the typed extractor has nothing to
        // classify and the surface text is the only evidence left. Kept
        // deliberately, and narrow: three punctuation marks, no fallthrough
        // into the general mapping.
        if node.child_count() == 0
            && let Ok(text) = node.utf8_text(source.as_bytes())
        {
            let span = span_of(node);
            return match text {
                ":" => ParseOutcome::parsed(Separator::Colon { span }),
                "," => ParseOutcome::parsed(Separator::Comma { span }),
                ";" => ParseOutcome::parsed(Separator::Semicolon { span }),
                _ => ParseOutcome::rejected(),
            };
        }
        return parse_separator_node(separator, source, errors);
    }
    match node.kind() {
        COLON => ParseOutcome::parsed(Separator::Colon {
            span: span_of(node),
        }),
        // A BARE LEAF: a `comma` or `rising_to_high` node reached directly,
        // not through its `non_colon_separator` parent. The generated
        // classifier lifts it into the choice, so the ONE mapping below still
        // does the work; a leaf of no modeled kind answers `None` and is
        // rejected, exactly as the pre-migration catch-all arm did.
        _ => match classify(node) {
            Some(separator) => ParseOutcome::parsed(separator),
            None => ParseOutcome::rejected(),
        },
    }
}
