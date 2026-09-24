//! Shared helpers for text-like dependent tier parsers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::generated_traversal::{AsRawNode, KindSlot, NamedKind, NoChild};
use crate::parser::tree_parsing::bullet_content::{BulletTextNode, parse_bullet_content};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
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

/// Parse an extracted text-tier body without erasing its generated kind.
///
/// Only the two bullet-text carriers implement conversion to `BulletTextNode`.
/// Present nodes and kind-admitted MISSING placeholders retain that identity;
/// an unclassified placeholder or ERROR reports the tier fault and no-content
/// diagnostic. Absent required content reports no-content alone. The enclosing
/// optional-body transition handles author-written empty tiers separately.
///
/// Surface displaced children before lowering, preserving the recovery backstop.
fn parse_text_tier_content<'tree, 'source, Tier, Body>(
    tier_node: Node<'tree>,
    body: crate::generated_traversal::SourceField<'_, 'tree, 'source, KindSlot<'tree, Body>>,
    unexpected: &[Node<'tree>],
    errors: &impl ErrorSink,
) -> BulletContent
where
    Tier: TextTierBody,
    Body: crate::generated_traversal::SourceBoundKind<'tree>,
    crate::generated_traversal::SourceBound<'tree, 'source, Body>:
        Into<BulletTextNode<'tree, 'source>>,
{
    let source = body.source();
    surface_displaced(unexpected, Tier::KIND, source, errors);

    use crate::generated_traversal::SourceSlotView;
    match body.view() {
        SourceSlotView::Present(text) | SourceSlotView::Missing(text) => {
            match crate::parser::typed_cst::read_source_field(text, errors) {
                Some(text) => parse_bullet_content(text.into(), errors),
                None => BulletContent::empty(),
            }
        }
        SourceSlotView::Error(node) => {
            errors.report(unexpected_node_error(node.raw_node(), source, Tier::KIND));
            report_missing_text_content::<Tier>(tier_node, source, errors);
            BulletContent::empty()
        }
        SourceSlotView::Absent(NoChild) => {
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
pub(crate) fn parse_optional_text_tier_content<'tree, 'source, Tier, Body>(
    tier: crate::generated_traversal::SourceBound<'tree, 'source, Tier>,
    body: crate::generated_traversal::SourceField<
        '_,
        'tree,
        'source,
        Option<KindSlot<'tree, Body>>,
    >,
    unexpected: &[Node<'tree>],
    errors: &impl ErrorSink,
) -> BulletContent
where
    Tier: TextTierBody + crate::generated_traversal::SourceBoundKind<'tree>,
    Body: crate::generated_traversal::SourceBoundKind<'tree>,
    crate::generated_traversal::SourceBound<'tree, 'source, Body>:
        Into<BulletTextNode<'tree, 'source>>,
{
    // The tier node comes from the tier VALUE, so the span reported and the
    // policy applied are the same tier by construction. They were a `Node` and
    // three loose arguments that a caller paired by hand.
    let tier_node = tier.raw_node();
    let source = tier.source();
    match body.optional() {
        Some(slot) => parse_text_tier_content::<Tier, Body>(tier_node, slot, unexpected, errors),
        None => {
            surface_displaced(unexpected, Tier::KIND, source, errors);
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
