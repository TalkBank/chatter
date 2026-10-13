//! Shared helper routines for dependent-tier dispatch.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, RangeAdmittedChildren, Ranged, ReadableSlot, SelectedKindSlot, SourceBound,
    SourceSlice, TextWithBulletsNode,
};
use crate::model::NonEmptyString;
use crate::model::TextTier;
use crate::parser::tree_parsing::parser_helpers::analyze_dependent_tier_error;
use talkbank_model::ParseOutcome;

/// The optional body shape of the range-admitted simple text-like tier carriers.
pub(super) type OptionalTierBodySlot<'tree, 'source> = Option<
    SelectedKindSlot<
        'tree,
        SourceBound<'tree, 'source, TextWithBulletsNode<'tree>>,
        Ranged<'source>,
    >,
>;

/// Project this carrier's body, never independently supplied text.
/// Implementations are emitted beside their concrete appliers in `raw`.
pub(super) trait OptionalTierBody<'tree, 'source> {
    fn body(&self) -> &OptionalTierBodySlot<'tree, 'source>;
}

/// Read the retained text of a range-admitted dependent-tier body slot.
///
/// The raw text-like tiers share the grammar shape
/// `seq(<x>_tier_prefix, tier_sep, optional(text_with_bullets), newline)`.
/// Each concrete applier retains its source-bound tier, extracts under the
/// compiled kind proof, surfaces the displaced sink and consumes range admission.
/// The optional readers receive the whole admitted carrier: its parent and body
/// cannot be supplied independently.
///
/// Since the E756 widening made those bodies optional, NOTHING calls this
/// directly: every caller goes through [`read_optional_tier_body_text`] or
/// [`read_optional_tier_body_raw_text`], which unwrap the `Option` and then
/// delegate here for a body that IS present. This function therefore describes
/// what to do with a body that exists, and the absent case is not its business.
/// The selected slot cannot be `Absent`; optional `None` owns that outcome.
///
/// This replaces the removed `extract_unparsed_tier_content` hand-walk, which
/// located the body by scanning `node.children()` for a child of kind
/// `free_text` / `text_with_bullets` / `text_with_bullets_and_pics`. Behavior is
/// preserved byte for byte:
///
/// - `Present`: the body node's raw text is decoded. A non-empty text yields
///   `Parsed`; an empty text reports "Tier has empty content" at the tier-node
///   span. Source failure has already refused admission with E001 before this
///   reader can run; it cannot become empty content.
/// - `Missing` is diagnosed and rejected; source admission cannot turn a
///   recovered nonterminal into readable tier content.
/// - `Error`: the recovery node is reported first, in the dependent-tier
///   analyzer's words (E316 for text nothing could parse), then "Tier is
///   missing content node" at the tier-node span: E330.md's pair. Until
///   2026-09-08 the utterance parser's pre-attach walk reported the recovery
///   node; with that walk gone, the whole-tree backstop would have dropped it
///   as overlapping the tier-span report, and E330.md#3 lost its E316.
///
/// Selection rules out `Unexpected` and `Absent`; producer faults remain
/// distinct from these source-recovery states.
///
/// The applier surfaces displaced recovery FIRST, before consuming admission,
/// retaining the body-versus-outside-body diagnostic order.
fn read_tier_body_text<'tree, 'source>(
    tier: SourceSlice<'tree, 'source>,
    body: &SelectedKindSlot<
        'tree,
        SourceBound<'tree, 'source, TextWithBulletsNode<'tree>>,
        Ranged<'source>,
    >,
    errors: &impl ErrorSink,
) -> ParseOutcome<NonEmptyString> {
    match body {
        ReadableSlot::Present(text) => decode_body_text(tier, *text, errors),
        ReadableSlot::Error(node) => {
            errors.report(analyze_dependent_tier_error(node.raw_node(), node.source()));
            report_missing_content_node(tier, errors);
            ParseOutcome::rejected()
        }
        ReadableSlot::Missing(missing) => {
            crate::parser::tree_parsing::parser_helpers::check_not_missing(
                missing.raw_node(),
                missing.source(),
                errors,
                "dependent_tier",
            );
            report_missing_content_node(tier, errors);
            ParseOutcome::rejected()
        }
        ReadableSlot::Unexpected(never) | ReadableSlot::Absent(never) => match *never {},
    }
}

/// E330 at the tier: the typed walk arrived at the body slot and found no
/// content node there.
fn report_missing_content_node(tier: SourceSlice<'_, '_>, errors: &impl ErrorSink) {
    let tier_node = tier.raw_node();
    errors.report(ParseError::new(
        ErrorCode::TreeParsingError,
        Severity::Error,
        SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
        ErrorContext::new(tier.source(), tier_node.byte_range(), "tier"),
        "Tier is missing content node",
    ));
}

/// Read a text tier's body when the grammar makes that body OPTIONAL.
///
/// The ten text-payload tiers (`%alt %coh %def %eng %err %fac %flo %gls %ort
/// %par`) gained optional bodies on 2026-08-15 with the E756 widening; every
/// other free-text tier followed on 2026-08-16. A tier
/// line with nothing after the separator is a real, invalid construct a file
/// can contain: before, it failed to parse and recovered as E602 "malformed
/// dependent tier header" while the re2c backend read the same file as VALID.
///
/// The empty case lowers to [`TextTier::empty`] rather than reporting from the
/// parse path, because recovery is not validity: the parser says what the file
/// contains and `DependentTier::empty_content_span` lets the validator judge
/// it. That also keeps the tier in the model, so the line survives a roundtrip.
///
/// `TextTier::empty` is called here and in the re2c converter's own lowering;
/// both are parsers saying what the file contains, which is what that
/// constructor's doc asks for.
pub(super) fn read_optional_tier_body_text<'tree, 'source, C>(
    tier: &RangeAdmittedChildren<'tree, 'source, C>,
    errors: &impl ErrorSink,
) -> ParseOutcome<TextTier>
where
    C: OptionalTierBody<'tree, 'source>,
{
    // Defined over the raw reader rather than repeating its `match`, so the
    // absent-body policy (report nothing, return a
    // payload that says it is empty) is stated in exactly one place. Two copies
    // thirty lines apart would agree only by inspection.
    read_optional_tier_body_raw_text(tier, errors).map(|content| match content {
        Some(text) => TextTier::new(text),
        None => TextTier::empty(),
    })
}

/// Read a tier's body when the grammar makes that body OPTIONAL and the caller
/// builds its own model type from the raw text.
///
/// Sibling of [`read_optional_tier_body_text`], which owns [`TextTier`]
/// construction because all ten of its callers build the same type. The five
/// remaining free-text tiers (`%tim %modsyl %phosyl %phoaln %xphoint`) each
/// lower their text differently: `%tim` classifies it into time segments, the
/// Phon tiers run their own fallible content parsers. So this returns the
/// question rather than an answer, and each caller says what an absent body
/// means IN ITS OWN TYPE (`TimTier::empty`, or the Phon tiers' empty word
/// lists), which is the state `DependentTier::empty_content_span` then reads.
///
/// `Parsed(None)` is an absent body, NOT a failure: the outer [`ParseOutcome`]
/// says whether the body could be read, the inner [`Option`] says whether there
/// was one to read. Collapsing the two would make an empty tier
/// indistinguishable from an unreadable one, and they get different diagnostics
/// (E756 versus a parse error).
pub(super) fn read_optional_tier_body_raw_text<'tree, 'source, C>(
    tier: &RangeAdmittedChildren<'tree, 'source, C>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<NonEmptyString>>
where
    C: OptionalTierBody<'tree, 'source>,
{
    match tier.children().body() {
        Some(slot) => read_tier_body_text(tier.parent(), slot, errors).map(Some),
        None => ParseOutcome::Parsed(None),
    }
}

/// Decode a retained body slice; empty content reports at the tier-node span.
fn decode_body_text<'tree, 'source>(
    tier: SourceSlice<'tree, 'source>,
    body: SourceBound<'tree, 'source, TextWithBulletsNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<NonEmptyString> {
    match NonEmptyString::new(body.text()) {
        Ok(content) => ParseOutcome::parsed(content),
        Err(_) => {
            let tier_node = tier.raw_node();
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(tier.source(), tier_node.byte_range(), "tier"),
                "Tier has empty content",
            ));
            ParseOutcome::rejected()
        }
    }
}

// Accepted dispatch callers share the tier parser's unchanged separator policy.
pub(crate) use crate::parser::tier_parsers::dependent_tier_separator;
