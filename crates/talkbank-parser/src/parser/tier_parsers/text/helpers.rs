//! Shared helpers for text-like dependent tier parsers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::generated_traversal::{
    AsRawNode, NamedKind, RangeAdmittedChildren, Ranged, ReadableSlot, SelectedNonMissingKindSlot,
    SourceBound, SourceBoundKind, SourceSlice,
};
use crate::parser::tree_parsing::bullet_content::{BulletTextNode, parse_bullet_content};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use talkbank_model::model::BulletContent;
use talkbank_model::{
    DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity,
};

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
/// makes `Self::KIND.name()` available to the shared parser and what keeps a
/// non-tier type out of it.
pub(crate) trait TextTierBody: NamedKind {}

/// Only the whole generated kind/range-admitted tier projects a readable body.
pub(crate) trait AdmittedTextTierBody<'tree, 'source> {
    type Tier: TextTierBody;
    type Body: SourceBoundKind<'tree>;

    fn body(
        &self,
    ) -> &Option<
        SelectedNonMissingKindSlot<'tree, SourceBound<'tree, 'source, Self::Body>, Ranged<'source>>,
    >;
}

/// The nine tiers this module parses.
mod policies {
    use super::{AdmittedTextTierBody, Ranged, TextTierBody};
    use crate::generated_traversal::{
        ActDependentTierNode, AddDependentTierNode, CodDependentTierNode, ComDependentTierNode,
        ExpDependentTierNode, GpxDependentTierNode, IntDependentTierNode, SitDependentTierNode,
        SpaDependentTierNode,
    };

    macro_rules! text_tiers {
        ($($node:ident / $children:ident => $body:ident),* $(,)?) => { $(
            impl TextTierBody for $node<'_> {}
            impl<'tree, 'source, P: crate::generated_traversal::PresenceProof> AdmittedTextTierBody<'tree, 'source>
                for crate::generated_traversal::$children<'tree, crate::generated_traversal::KindAdmitted, Ranged<'source>, P>
            {
                type Tier = $node<'tree>;
                type Body = crate::generated_traversal::$body<'tree>;

                fn body(&self) -> &Option<crate::generated_traversal::SelectedNonMissingKindSlot<
                    'tree,
                    crate::generated_traversal::SourceBound<'tree, 'source, Self::Body>,
                    Ranged<'source>,
                >> {
                    &self.child_2.slot
                }
            }
        )* };
    }

    text_tiers! {
        ActDependentTierNode / ActDependentTierChildren => TextWithBulletsNode,
        AddDependentTierNode / AddDependentTierChildren => TextWithBulletsNode,
        CodDependentTierNode / CodDependentTierChildren => TextWithBulletsNode,
        ComDependentTierNode / ComDependentTierChildren => TextWithBulletsAndPicsNode,
        ExpDependentTierNode / ExpDependentTierChildren => TextWithBulletsNode,
        GpxDependentTierNode / GpxDependentTierChildren => TextWithBulletsNode,
        IntDependentTierNode / IntDependentTierChildren => TextWithBulletsNode,
        SitDependentTierNode / SitDependentTierChildren => TextWithBulletsNode,
        SpaDependentTierNode / SpaDependentTierChildren => TextWithBulletsNode,
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
/// A present node retains that identity; an ERROR reports the tier fault and
/// the no-content diagnostic. The enclosing optional-body transition handles
/// author-written empty tiers separately.
///
/// Selection excludes absence; compiled kind admission separately excludes
/// MISSING for these composite body kinds.
///
/// The concrete owner surfaces displaced children before consuming admission.
fn parse_text_tier_content<'tree, 'source, Tier, Body>(
    tier: SourceSlice<'tree, 'source>,
    body: &SelectedNonMissingKindSlot<'tree, SourceBound<'tree, 'source, Body>, Ranged<'source>>,
    errors: &impl ErrorSink,
) -> Result<BulletContent, crate::CstFailure>
where
    Tier: TextTierBody,
    Body: SourceBoundKind<'tree>,
    SourceBound<'tree, 'source, Body>: Into<BulletTextNode<'tree, 'source>>,
{
    match body {
        ReadableSlot::Present(text) => parse_bullet_content((*text).into(), errors),
        ReadableSlot::Error(node) => {
            errors.report(unexpected_node_error(
                node.raw_node(),
                node.source(),
                Tier::KIND.name(),
            ));
            report_missing_text_content::<Tier>(tier, errors);
            Ok(BulletContent::empty())
        }
        ReadableSlot::Missing(missing) => match missing.0 {},
        ReadableSlot::Unexpected(never) | ReadableSlot::Absent(never) => match *never {},
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
/// file contains and `DependentTier::empty_content_location` lets the validator
/// judge it. That also keeps the tier in the model, so the line survives a
/// roundtrip; a parse-path rejection would drop it.
///
/// `Some(slot)` delegates to [`parse_text_tier_content`] unchanged, so a body
/// that is present but malformed keeps its existing diagnostics.
pub(crate) fn parse_optional_text_tier_content<'tree, 'source, C>(
    tier: &RangeAdmittedChildren<'tree, 'source, C>,
    errors: &impl ErrorSink,
) -> Result<BulletContent, crate::CstFailure>
where
    C: AdmittedTextTierBody<'tree, 'source>,
    SourceBound<'tree, 'source, C::Body>: Into<BulletTextNode<'tree, 'source>>,
{
    // Parent and body are projected from this one consuming admission result.
    match tier.children().body() {
        Some(slot) => parse_text_tier_content::<C::Tier, C::Body>(tier.parent(), slot, errors),
        None => Ok(BulletContent::empty()),
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
    tier: SourceSlice<'_, '_>,
    errors: &impl ErrorSink,
) {
    let tier_node = tier.raw_node();
    errors.report(ParseError::new(
        ErrorCode::TreeParsingError,
        Severity::Error,
        DiagnosticLocation::new(span_of(tier_node)),
        ErrorContext::new(tier.source(), span_of(tier_node), Tier::KIND.name()),
        format!("Missing content in %{} tier", tier_label(Tier::KIND.name())),
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
            tier_label(T::KIND.name())
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
