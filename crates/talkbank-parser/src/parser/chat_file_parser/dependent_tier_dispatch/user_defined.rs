//! Dispatch for user-defined and unsupported dependent tiers.
//!
//! A `%x` tier is stored as `UserDefined` with its label and content. Until
//! 2026-09-08 this file also intercepted the labels `modsyl`, `phosyl` and
//! `phoaln` and built the Phon syllable and alignment tiers from them, for
//! the day the `x` prefix would be dropped; that day had come and gone. The
//! grammar's own tier tokens accept both spellings (`%modsyl` and
//! `%xmodsyl`, at higher precedence than the `%x` prefix), so every one of
//! those lines reaches the dedicated tier parser and the interception was
//! unreachable: a whole-workspace coverage run showed its arms unreached,
//! and a differential of thirteen tier shapes in both spellings through the
//! CLI was identical without it.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#User_Defined_Tiers>

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{
    AsRawNode, Never, NoChild, ReadableRecovery, ReadableSlot, SlotView, SourceBound, SourceSlice,
    UnsupportedDependentTierNode, UnsupportedTierPrefixNode, XDependentTierNode,
};
use crate::model::dependent_tier::{DependentTier, DependentTierEntry};
use crate::model::{RecoveryText, Utterance};
use crate::node_types::{
    UNSUPPORTED_DEPENDENT_TIER, UNSUPPORTED_TIER_PREFIX, X_DEPENDENT_TIER, X_TIER_PREFIX,
};
use crate::parser::typed_cst::ExtractCanonical;

use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{
    analyze_dependent_tier_error, check_not_missing, surface_displaced,
};
use crate::parser::typed_cst::{CstFailure, GeneratedPosition, report_cst_failure};
use tree_sitter::Node;

/// The name an unsupported tier's prefix declares: the text of the PRESENT
/// prefix token past its marker, non-empty by the grammar's own token
/// (`/%[a-zA-Z].../`). Every other state is reported and yields no name, so
/// no tier is built from a prefix that is not there: a MISSING placeholder
/// (E342) or an ERROR through the associated slot; a Present token that does
/// not fit its marker, or names nothing after it, is reported rather than
/// assumed; and a required position holding nothing is the walk's fault
/// (`CstFailure::AbsentRequiredPosition`), since the grammar has no
/// fail-closed partials, not a tier without a prefix. Selected ranges are
/// already checked; a failed range admission refuses the owner before this
/// helper runs. This reader once read the MISSING placeholder's empty text
/// and built the tier, through an unchecked constructor, with a label the
/// program had invented. Recovery is not validity, and the recovery node
/// stays reported; but a tier without a label has no honest model value (the
/// model's tier requires one), so the line lowers to nothing here rather
/// than to a tier the parser named. Should the state ever be reached, the
/// change is a label-less tier variant in the model, not a default here.
///
/// Only the unsupported tier reads its prefix through this: its `child_0`
/// is a plain kind slot that admits every state. The `%x` tier's prefix
/// position is closed by required-presence admission (`extract_envelope`),
/// so `apply_x_tier` matches Present and Missing only. Reachability
/// (coverage drive batch 3, 2026-10-19): the unsupported tier takes no
/// whole-tier admission, so these arms are reachable in principle; in a
/// sweep of 1,345 single-edit mutations of an unsupported tier line the
/// prefix position was only ever `Present`. The `Missing` and `Error` arms
/// stay and report the node they are given; no authored file reaches them.
fn tier_name<'tree, 'source, E: ReadableRecovery<View = SourceSlice<'tree, 'source>>>(
    slot: &ReadableSlot<
        SourceBound<'tree, 'source, UnsupportedTierPrefixNode<'tree>>,
        SourceBound<'tree, 'source, UnsupportedTierPrefixNode<'tree>>,
        E,
        Never,
        NoChild,
    >,
    tier: SourceSlice<'tree, 'source>,
    errors: &impl ErrorSink,
) -> Option<RecoveryText> {
    let input = tier.source();
    let tier_node = tier.raw_node();
    let prefix = match slot.view() {
        SlotView::Present(prefix) => *prefix,
        SlotView::Absent(NoChild) => {
            report_cst_failure(
                tier_node,
                input,
                CstFailure::AbsentRequiredPosition {
                    rule: UNSUPPORTED_DEPENDENT_TIER,
                    position: GeneratedPosition::new("UnsupportedDependentTierChildren.child_0"),
                },
                errors,
            );
            return None;
        }
        SlotView::Missing(missing) => {
            check_not_missing(
                missing.raw_node(),
                missing.source(),
                errors,
                UNSUPPORTED_DEPENDENT_TIER,
            );
            return None;
        }
        SlotView::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                bad.source(),
                UNSUPPORTED_DEPENDENT_TIER,
            ));
            return None;
        }
    };
    prefix_name(prefix, errors)
}

/// Decode an unsupported prefix without erasing its source owner.
/// The readable token does not itself carry a proof of its lexical spelling.
fn prefix_name<'tree>(
    prefix: SourceBound<'tree, '_, UnsupportedTierPrefixNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<RecoveryText> {
    let input = prefix.source();
    let Some(name) = prefix.text().strip_prefix('%') else {
        let raw = prefix.raw_node();
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            DiagnosticLocation::new(span_of(raw)),
            ErrorContext::new(prefix.source(), span_of(raw), UNSUPPORTED_TIER_PREFIX),
            "Unsupported tier prefix does not begin with '%'",
        ));
        return None;
    };
    match RecoveryText::new(name) {
        Ok(name) => Some(name),
        Err(_) => {
            let prefix = prefix.raw_node();
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                DiagnosticLocation::new(span_of(prefix)),
                ErrorContext::new(input, span_of(prefix), UNSUPPORTED_TIER_PREFIX),
                "Unsupported tier prefix declares no name after '%'",
            ));
            None
        }
    }
}

/// Handle user-defined %x* tiers (%xfoo, %xpho, %xmod, etc.).
///
/// The grammar uses a single greedy token for the full prefix (e.g. "%xfoo"),
/// so the model admits the full label after the percent sign.
///
/// Driven by the generated typed visitor: `extract_x_dependent_tier` yields the
/// prefix (`child_0`, an `x_tier_prefix`) and the body (`child_2`, a
/// `text_with_bullets`) as typed slots, replacing the removed
/// `find_child_by_kind(tier_node, X_TIER_PREFIX)` / `..(tier_node,
/// TEXT_WITH_BULLETS)` `match child.kind()` scans. The prefix is read as
/// the state its position is in ([`tier_name`]): a tier is built only from
/// a prefix that is there.
pub(super) fn apply_x_tier<'tree>(
    utterance: &mut Utterance,
    node: SourceBound<'tree, '_, XDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) {
    // Grammar: x_dependent_tier = x_tier_prefix, tier_sep, text_with_bullets, newline
    // x_tier_prefix is a single token matching /%x[a-zA-Z][a-zA-Z0-9]*/
    let tier_node = node.raw_node();
    let input = node.source();
    // Extracted under the admitted kind proof: the body is a named nonterminal
    // the compiled grammar proves is never MISSING, so its slot has no
    // `Missing` state to read below.
    let Ok(associated) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(node),
        tier_node,
        input,
        errors,
    ) else {
        return;
    };
    let children = associated.children();
    let Ok(separator) = crate::parser::typed_cst::report_admission(
        super::helpers::dependent_tier_separator(children.child_1.slot()),
        tier_node,
        input,
        errors,
    ) else {
        return;
    };
    surface_displaced(&children.unexpected, "x_dependent_tier", input, errors);
    let Ok(associated) = crate::parser::typed_cst::report_admission(
        associated.admit_ranges().map_err(Into::into),
        tier_node,
        input,
        errors,
    ) else {
        return;
    };
    let children = associated.children();

    let prefix = match children.child_0.slot.view() {
        SlotView::Present(prefix) => *prefix,
        SlotView::Missing(missing) => {
            check_not_missing(
                missing.raw_node(),
                missing.source(),
                errors,
                X_DEPENDENT_TIER,
            );
            return;
        }
    };
    // The model owns custom-label admission, including structured-label
    // exclusions. A mismatch here is a grammar/lowering fault.
    let stored_label = match prefix
        .text()
        .strip_prefix('%')
        .ok_or(crate::model::InvalidUserDefinedDependentTierLabel::InvalidSyntax)
        .and_then(crate::model::UserDefinedDependentTierLabel::new)
    {
        Ok(label) => label,
        Err(error) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                DiagnosticLocation::new(span_of(prefix.raw_node())),
                ErrorContext::new(input, span_of(prefix.raw_node()), X_TIER_PREFIX),
                format!("Invalid generated custom-tier prefix: {error}"),
            ));
            return;
        }
    };

    // Empty user-defined tier. Every free-text tier rule now marks its body
    // `optional(...)`, so the generated visitor types this body slot as
    // `Option<NodeSlot<..>>`: when a `%xLABEL:` line carries nothing but a
    // trailing space, the separator absorbs that space, no `text_with_bullets`
    // child is produced, and the slot is `None`. That is a real (if invalid)
    // construct, and it lowers to a tier whose content says it is absent.
    //
    // It used to report E756 from HERE, out of the parse path, and return
    // WITHOUT pushing the tier. Two things were wrong with that, and both were
    // observable: the tier vanished from the model, so an empty `%xtst:` did
    // not survive a roundtrip while an empty `%eng:` did; and because the
    // report came from parsing rather than validation, `chatter normalize`
    // treated the file as unparseable and emitted NOTHING AT ALL. Recovery is
    // not validity: the parser says what the file contains, and
    // `DependentTier::empty_content_location` hands it to E756 exactly as it does
    // for every other tier kind.
    let body_slot = match &children.child_2.slot {
        Some(slot) => slot,
        None => {
            let span = span_of(tier_node);
            let tier = DependentTier::UserDefined(crate::model::UserDefinedDependentTier {
                label: stored_label,
                content: None,
                location: span.into(),
            });
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(tier, separator));
            return;
        }
    };

    // A recovery node where the body belongs is reported first, in the
    // dependent-tier analyzer's words (E316), then the missing content
    // (E330): the pair E330.md describes. The utterance parser's pre-attach
    // walk used to report the recovery node; it is gone since 2026-09-08. A
    // node of another kind there (none is generated today) is reported by
    // its kind.
    let body = match body_slot {
        ReadableSlot::Present(n) => *n,
        ReadableSlot::Error(n) => {
            errors.report(analyze_dependent_tier_error(n.raw_node(), n.source()));
            report_missing_x_content(tier_node, stored_label.as_str(), input, errors);
            return;
        }
        ReadableSlot::Missing(missing) => match missing.0 {},
        ReadableSlot::Unexpected(never) | ReadableSlot::Absent(never) => match *never {},
    };
    let content_text = body.text();

    // Content must be non-empty. The grammar's `text_with_bullets` is never
    // empty (its first child is required), so this refusal is reached by no
    // file and stays because the slot's text type does not say so.
    let content = match RecoveryText::new(content_text) {
        Ok(c) => c,
        Err(_) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                DiagnosticLocation::new(span_of(tier_node)),
                ErrorContext::new(input, span_of(tier_node), ""),
                format!("Empty content in user-defined tier %{stored_label}"),
            ));
            return;
        }
    };

    let span = span_of(tier_node);

    let tier = DependentTier::UserDefined(crate::model::UserDefinedDependentTier {
        label: stored_label,
        content: Some(content),
        location: span.into(),
    });

    utterance
        .dependent_tiers
        .push(DependentTierEntry::with_separator(tier, separator));
}

/// E330 at the tier: the typed walk found no content node for a `%x` tier.
fn report_missing_x_content(
    tier_node: Node,
    tier_label: &str,
    input: &str,
    errors: &impl ErrorSink,
) {
    errors.report(ParseError::new(
        ErrorCode::TreeParsingError,
        Severity::Error,
        DiagnosticLocation::new(span_of(tier_node)),
        ErrorContext::new(input, span_of(tier_node), ""),
        format!("Missing content in user-defined tier %{tier_label}"),
    ));
}

/// Handle unsupported dependent tiers (%custom, %foo, etc.) caught by the grammar catch-all.
/// These are stored as Unsupported tiers so the file can still be parsed.
///
/// The PREFIX is driven by the generated typed visitor:
/// `extract_unsupported_dependent_tier`'s `child_0` (`unsupported_tier_prefix`, a
/// real named node) replaces the removed
/// `find_child_by_kind(tier_node, UNSUPPORTED_TIER_PREFIX)` scan.
///
/// The CONTENT is the anonymous regex `/[^\n\r]*/`, which tree-sitter surfaces
/// as no named child; the generator models it as a typed `LeafSpan`
/// (`child_2`), the byte range between the separator and the newline, read
/// from the input (see the inline comment on the content read below).
pub(super) fn apply_unsupported_tier<'tree>(
    utterance: &mut Utterance,
    node: SourceBound<'tree, '_, UnsupportedDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) {
    let tier_node = node.raw_node();
    let input = node.source();
    let extraction = node.extract_canonical();
    let Ok(associated) =
        crate::parser::typed_cst::report_admission(extraction, tier_node, input, errors)
    else {
        return;
    };
    let children = associated.children();
    let Ok(separator) = crate::parser::typed_cst::report_admission(
        super::helpers::dependent_tier_separator(children.child_1.slot()),
        tier_node,
        input,
        errors,
    ) else {
        return;
    };
    surface_displaced(
        &children.unexpected,
        "unsupported_dependent_tier",
        input,
        errors,
    );
    let Ok(associated) = crate::parser::typed_cst::report_admission(
        associated.admit_ranges().map_err(Into::into),
        tier_node,
        input,
        errors,
    ) else {
        return;
    };
    let children = associated.children();

    let Some(label) = tier_name(&children.child_0.slot, associated.parent(), errors) else {
        return;
    };

    // The unsupported-tier body is the anonymous seq-member regex `/[^\n\r]*/`
    // sitting between `tier_sep` (child_1) and `newline` (child_3). The generator
    // now models such an absorbed anonymous seq-token as a typed `LeafSpan`
    // (child_2): a fully-typed inter-sibling byte range, NOT a text-hack. Read the
    // body directly from that span. This closed the pre-migration generator gap
    // where the anonymous token surfaced no child (child_2 was always `Absent`)
    // and the body had to be recovered by string-splitting the tier's source text;
    // the fix is sibling to the sometimes-leaf-choice (`LeafText`) fix. The prefix
    // `child_0`, a real named node, is likewise visitor-driven above. Source
    // identity and readable UTF-8 boundaries are retained by `ReadableLeafSpan`.
    // The consuming transition above refuses the owner with E001 if any selected
    // range is unreadable. This read is infallible, never an empty-body fallback.
    let body = children.child_2.text();

    // An empty tier is KEPT, carrying `None`, rather than dropped with a
    // generic parse error. It used to be dropped because the model could not
    // represent the empty case; now that it can, the tier survives with the
    // truth in it and E756 reports it during validation, which is where a
    // "this line declares nothing" rule belongs. Dropping it also lost the
    // label, and left this parser disagreeing with the re2c one, which kept a
    // fabricated tier instead.
    let content = RecoveryText::new(body.trim()).ok();
    let span = span_of(tier_node);
    let tier = DependentTier::Unsupported(crate::model::UnsupportedDependentTier {
        label,
        content,
        location: span.into(),
    });

    utterance
        .dependent_tiers
        .push(DependentTierEntry::with_separator(tier, separator));
}
