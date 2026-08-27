//! Shared helpers for text-like dependent tier parsers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::generated_traversal::{AsRawNode, NamedKind, NodeSlot};
use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use talkbank_model::model::BulletContent;
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

/// Byte span of an entire tier node.
///
/// Re-exported for the seven text-tier parsers that reach for it through this
/// module; the conversion lives in [`crate::parser::node_span`].
pub(super) use crate::parser::node_span::span_of;

/// The tiers whose body this module parses.
///
/// A MARKER trait now: it carries no members, because the one per-tier fact it
/// used to hold turned out not to vary. `MalformedBody` decided whether a
/// malformed body got its OWN diagnostic before the "no content" rejection, and
/// `%act` and `%cod` said no purely because their removed hand-walk loops had
/// none. Nobody chose that, no test asserted it, and converging the nine ONTO
/// reporting keeps E315 ("invalid control character") on all of them where
/// converging the other way lost it. Measured, not assumed: on a tier body
/// holding a bare control character, `%act` and `%cod` now report
/// `E315 E316 E330 E756` exactly as the other seven always did.
///
/// It stays a trait rather than becoming nothing, because the bound is what
/// makes `Self::KIND` available to the shared parser and what keeps a
/// non-tier type out of it.
pub(crate) trait TextTierBody: NamedKind {}

/// The nine tiers this module parses.
mod policies {
    use super::TextTierBody;
    use crate::generated_traversal::{
        ActDependentTierNode, AddDependentTierNode, CodDependentTierNode, ComDependentTierNode,
        ExpDependentTierNode, GpxDependentTierNode, IntDependentTierNode, SitDependentTierNode,
        SpaDependentTierNode,
    };

    macro_rules! text_tiers {
        ($($node:ident),* $(,)?) => { $( impl TextTierBody for $node<'_> {} )* };
    }

    text_tiers! {
        ActDependentTierNode,
        AddDependentTierNode,
        CodDependentTierNode,
        ComDependentTierNode,
        ExpDependentTierNode,
        GpxDependentTierNode,
        IntDependentTierNode,
        SitDependentTierNode,
        SpaDependentTierNode,
    }
}

/// The `%xxx` label a dependent-tier rule's diagnostics use.
///
/// Every dependent-tier rule in the grammar is named `<label>_dependent_tier`,
/// so the label is the rule name's prefix and needs no second declaration.
/// `tier_labels_follow_the_rule_naming_convention` pins that for all nine.
///
/// A rule that ever stops following the convention gets its WHOLE kind as the
/// label. That is deliberately conspicuous rather than tidy: a diagnostic
/// reading `%mor_contents tier` is a visible prompt to fix the convention,
/// where a truncated or defaulted label would read as correct and be wrong.
fn tier_label(kind: &str) -> &str {
    match kind.strip_suffix("_dependent_tier") {
        Some(label) => label,
        None => kind,
    }
}

/// Parse the text/bullet payload of a text-like dependent tier from the tier's
/// already-extracted body slot (`child_2` of `extract_<tier>_dependent_tier`)
/// and surface the carrier's `unexpected` sink.
///
/// This is the shared body parser for `%com` / `%exp` / `%add` / `%spa` / `%sit`
/// / `%int` / `%gpx`; each caller extracts its own tier via the generated typed
/// visitor and hands the body slot AND the carrier's `unexpected` sink here, so
/// the concrete body wrapper type (`TextWithBulletsNode`, or
/// `TextWithBulletsAndPicsNode` for `%com`) is abstracted behind [`AsRawNode`],
/// and every caller surfaces its `unexpected` sink uniformly (R2), matching how
/// the sibling carriers `act.rs` / `cod.rs` / gra / pho / sin already surface
/// theirs.
///
/// `unexpected` is surfaced FIRST via [`surface_unexpected`] (a no-op when
/// empty, which is every case on valid input: the tier's own body slot below
/// is the only position that carries content for these grammar rules).
///
/// The `child_2` slot is matched EXHAUSTIVELY over [`NodeSlot`] (no `_`
/// catch-all, no `.ok()`), reproducing the removed hand-walk loop byte for byte:
///
/// - `Present` / `Missing`: the removed loop matched the body by kind
///   (`text_with_bullets` / `text_with_bullets_and_pics`), and a tree-sitter
///   MISSING node carries that expected kind, so BOTH a real body and a MISSING
///   body were handed to [`parse_bullet_content`]. The raw body node is parsed in
///   both arms, and they stay SEPARATE here on purpose. `node_or_placeholder`
///   would merge them, but the arms below bind the offending node to place a
///   diagnostic, and `Error`/`Unexpected` and `Absent` want different ones, so
///   collapsing the top would force the slot to be re-matched underneath with an
///   unreachable case. Two honest arms beat one arm plus an impossible branch.
///   (This paragraph used to give a different reason, that the backend made
///   sharing impossible. That was true when written and is not now.) (This paragraph used to add that an empty `%com:` body was the
///   only reachable malformed case here, landing as `Present` with a MISSING
///   inner `continuation` and recovering to a single `Continuation` segment. The
///   E756 widening abolished that: `%com`'s grammar body is `optional(...)`, so
///   an empty one never reaches this function at all.)
/// - `Error` / `Unexpected`: the removed loop's `_` arm reported
///   [`unexpected_node_error`] for a non-structural, non-text child, then fell
///   through to the end-of-loop "no content" rejection because no text body was
///   found. Both are reproduced, at the same code and span (largely unreachable
///   in practice; the whole-tree recovery backstop covers these).
/// - `Absent`: the removed loop simply never matched a text node and reported the
///   "no content" rejection.
fn parse_text_tier_content<'tree, Tier, Body>(
    tier_node: Node<'tree>,
    body: &NodeSlot<'tree, Body>,
    unexpected: &[Node<'tree>],
    source: &str,
    errors: &impl ErrorSink,
) -> BulletContent
where
    Tier: TextTierBody,
    Body: AsRawNode<'tree>,
{
    surface_unexpected(unexpected, source, errors);

    match body {
        NodeSlot::Present(text) => parse_bullet_content(text.raw_node(), source, errors),
        NodeSlot::Missing(node) => parse_bullet_content(*node, source, errors),
        NodeSlot::Error(node) | NodeSlot::Unexpected(node) => {
            errors.report(unexpected_node_error(*node, source, Tier::KIND));
            report_missing_text_content::<Tier>(tier_node, source, errors);
            BulletContent::empty()
        }
        NodeSlot::Absent => {
            report_missing_text_content::<Tier>(tier_node, source, errors);
            BulletContent::empty()
        }
    }
}

/// Parse the bullet payload of a text-like dependent tier whose grammar makes
/// that body OPTIONAL.
///
/// The nine bullet-payload tiers (`%act %add %cod %com %exp %gpx %int %sit
/// %spa`) gained optional bodies on 2026-08-16 with the E756 widening, joining
/// the ten text-payload tiers that had gained them the day before. A tier line
/// with nothing after the separator is a real, invalid construct a file can
/// contain: before, it failed to parse and recovered generically as E342 while
/// the re2c backend already reported E756 on the same file, so the two backends
/// disagreed on what the file said.
///
/// The absent case lowers to [`BulletContent::empty`] and reports NOTHING from
/// the parse path, because recovery is not validity: the parser says what the
/// file contains and `DependentTier::empty_content_span` lets the validator
/// judge it. That also keeps the tier in the model, so the line survives a
/// roundtrip; a parse-path rejection would drop it.
///
/// `Some(slot)` delegates to [`parse_text_tier_content`] unchanged, so a body
/// that is present but malformed keeps its existing diagnostics.
pub(crate) fn parse_optional_text_tier_content<'tree, Tier, Body>(
    tier: Tier,
    body: &Option<NodeSlot<'tree, Body>>,
    unexpected: &[Node<'tree>],
    source: &str,
    errors: &impl ErrorSink,
) -> BulletContent
where
    Tier: TextTierBody + AsRawNode<'tree>,
    Body: AsRawNode<'tree>,
{
    // The tier node comes from the tier VALUE, so the span reported and the
    // policy applied are the same tier by construction. They were a `Node` and
    // three loose arguments that a caller paired by hand.
    let tier_node = tier.raw_node();
    match body {
        Some(slot) => {
            parse_text_tier_content::<Tier, Body>(tier_node, slot, unexpected, source, errors)
        }
        None => {
            surface_unexpected(unexpected, source, errors);
            BulletContent::empty()
        }
    }
}

/// Report the "no content" rejection for a text-like dependent tier whose body
/// slot carried no parseable `text_with_bullets` node.
///
/// Reproduces the removed hand-walk loop's end-of-loop `TreeParsingError`
/// byte-identically: the same error code, severity, span (the whole tier node),
/// context, and caller-supplied message (for example "Missing content in %com
/// tier").
fn report_missing_text_content<Tier: TextTierBody>(
    tier_node: Node,
    source: &str,
    errors: &impl ErrorSink,
) {
    errors.report(ParseError::new(
        ErrorCode::TreeParsingError,
        Severity::Error,
        SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
        ErrorContext::new(
            source,
            tier_node.start_byte()..tier_node.end_byte(),
            Tier::KIND,
        ),
        format!("Missing content in %{} tier", tier_label(Tier::KIND)),
    ));
}

#[cfg(test)]
mod tests {
    use super::{TextTierBody, tier_label};
    use crate::generated_traversal::{
        ActDependentTierNode, AddDependentTierNode, CodDependentTierNode, ComDependentTierNode,
        ExpDependentTierNode, GpxDependentTierNode, IntDependentTierNode, SitDependentTierNode,
        SpaDependentTierNode,
    };

    /// Every tier's diagnostic label is derived from its rule name, so this is
    /// the one place that knows the convention holds.
    ///
    /// POLICY, not an invariant a type can carry: the grammar is free to name a
    /// rule anything, and `tier_label` deliberately returns the whole kind when
    /// the convention is broken rather than inventing a label. This asserts the
    /// convention DOES hold for all nine, which is what makes the derivation
    /// sound; if a tenth tier arrives named differently, this fails and the
    /// diagnostic it would have produced is visible here instead of in a corpus.
    #[test]
    fn tier_labels_follow_the_rule_naming_convention() {
        fn label_of<T: TextTierBody>() -> &'static str {
            tier_label(T::KIND)
        }
        assert_eq!(label_of::<ActDependentTierNode<'_>>(), "act");
        assert_eq!(label_of::<AddDependentTierNode<'_>>(), "add");
        assert_eq!(label_of::<CodDependentTierNode<'_>>(), "cod");
        assert_eq!(label_of::<ComDependentTierNode<'_>>(), "com");
        assert_eq!(label_of::<ExpDependentTierNode<'_>>(), "exp");
        assert_eq!(label_of::<GpxDependentTierNode<'_>>(), "gpx");
        assert_eq!(label_of::<IntDependentTierNode<'_>>(), "int");
        assert_eq!(label_of::<SitDependentTierNode<'_>>(), "sit");
        assert_eq!(label_of::<SpaDependentTierNode<'_>>(), "spa");
    }

    /// A rule not following the convention keeps its whole kind, conspicuously.
    #[test]
    fn a_rule_outside_the_convention_is_not_given_an_invented_label() {
        assert_eq!(tier_label("mor_contents"), "mor_contents");
    }
}
