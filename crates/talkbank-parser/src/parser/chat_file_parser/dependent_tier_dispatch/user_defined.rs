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

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, ChildSlot, NoChild, SlotView, UnsupportedDependentTierNode, XDependentTierNode,
    extract_unsupported_dependent_tier, extract_x_dependent_tier,
};
use crate::model::dependent_tier::{DependentTier, DependentTierEntry};
use crate::model::{NonEmptyString, Utterance};
use crate::node_types::{TEXT_WITH_BULLETS, UNSUPPORTED_TIER_PREFIX, X_TIER_PREFIX};

use crate::parser::tree_parsing::parser_helpers::{
    SlotState, after_marker, analyze_dependent_tier_error, expect_present, surface_displaced,
};
use tree_sitter::Node;

/// Which tier prefix a dispatcher reads, and the words its diagnostics use.
struct PrefixKind {
    /// The marker the token begins with, stripped to leave the name.
    marker: &'static str,
    /// The tier rule, named in the recovery reports.
    context: &'static str,
    /// The prefix, as the reports about the token itself call it.
    what: &'static str,
    /// The tier, as the report of an absent prefix calls it.
    tier: &'static str,
    /// The prefix node's kind, for the report's context.
    node_kind: &'static str,
}

/// The `%x` prefix of a user-defined tier.
const X_PREFIX: PrefixKind = PrefixKind {
    marker: "%x",
    context: "x_dependent_tier",
    what: "User-defined tier prefix",
    tier: "user-defined tier",
    node_kind: X_TIER_PREFIX,
};

/// The `%` prefix of a tier no rule names.
const UNSUPPORTED_PREFIX: PrefixKind = PrefixKind {
    marker: "%",
    context: "unsupported_dependent_tier",
    what: "Unsupported tier prefix",
    tier: "unsupported dependent tier",
    node_kind: UNSUPPORTED_TIER_PREFIX,
};

/// The name a tier prefix declares: the text of the PRESENT prefix token
/// past its marker, non-empty by the grammar's own token (`/%x[a-zA-Z].../`
/// and `/%[a-zA-Z].../`). Every other state is reported and yields no name,
/// so no tier is built from a prefix that is not there: a MISSING
/// placeholder (E342) or an ERROR through [`expect_present`]; a position
/// holding nothing, as the old walk reported it; a Present token that is
/// not UTF-8 or does not fit its own grammar, through [`after_marker`], or
/// that names nothing after its marker, reported rather than assumed (no
/// CHAT input reaches any of the three). Until 2026-09-09 both dispatchers read
/// the MISSING placeholder's empty text and built the tier, through an
/// unchecked constructor, with a label the program had invented (`x`, or
/// nothing at all). Recovery is not validity, and the recovery node stays
/// reported; but a tier without a label has no honest model value (the
/// model's tier requires one), so the line lowers to nothing here rather
/// than to a tier the parser named. Should the state ever be reached, the
/// change is a label-less tier variant in the model, not a default here.
fn tier_name<'tree, T: AsRawNode<'tree>>(
    slot: &ChildSlot<'tree, T>,
    kind: &PrefixKind,
    tier_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> Option<NonEmptyString> {
    let prefix = match expect_present(slot, kind.context, input, errors) {
        SlotState::Present(prefix) => prefix.raw_node(),
        SlotState::Absent => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                format!("Missing tier prefix in {}", kind.tier),
            ));
            return None;
        }
        SlotState::Recovered => return None,
    };
    let name = after_marker(prefix, kind.marker, kind.what, input, errors)?;
    match NonEmptyString::new(name) {
        Ok(name) => Some(name),
        Err(_) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(prefix.start_byte(), prefix.end_byte()),
                ErrorContext::new(
                    input,
                    prefix.start_byte()..prefix.end_byte(),
                    kind.node_kind,
                ),
                format!("{} declares no name after '{}'", kind.what, kind.marker),
            ));
            None
        }
    }
}

/// Handle user-defined %x* tiers (%xfoo, %xpho, %xmod, etc.).
///
/// The grammar uses a single greedy token for the full prefix (e.g. "%xfoo"),
/// so the label is extracted by stripping the "%x" prefix from the token text.
///
/// Driven by the generated typed visitor: `extract_x_dependent_tier` yields the
/// prefix (`child_0`, an `x_tier_prefix`) and the body (`child_2`, a
/// `text_with_bullets`) as typed slots, replacing the removed
/// `find_child_by_kind(tier_node, X_TIER_PREFIX)` / `..(tier_node,
/// TEXT_WITH_BULLETS)` `match child.kind()` scans. The prefix is read as
/// the state its position is in ([`tier_name`]): a tier is built only from
/// a prefix that is there.
pub(super) fn apply_x_tier(
    utterance: &mut Utterance,
    node: XDependentTierNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) {
    // Grammar: x_dependent_tier = x_tier_prefix, tier_sep, text_with_bullets, newline
    // x_tier_prefix is a single token matching /%x[a-zA-Z][a-zA-Z0-9]*/
    let tier_node = node.raw_node();
    let children = extract_x_dependent_tier(node);
    let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
    surface_displaced(&children.unexpected, "x_dependent_tier", input, errors);

    let Some(name) = tier_name(children.child_0.slot(), &X_PREFIX, tier_node, input, errors) else {
        return;
    };
    let tier_label = name.as_str();

    // For UserDefined tiers the stored label keeps the leading 'x' to avoid
    // colliding with built-in tiers (`%xmor` stores "xmor", not "mor"). Built
    // once: both the empty-body path below and the content path at the end of
    // this function need the identical value, and building it twice is how the
    // two drift. The `x` is the proof of non-emptiness, so nothing is checked.
    let stored_label = NonEmptyString::from_head_and_tail('x', tier_label);

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
    // `DependentTier::empty_content_span` hands it to E756 exactly as it does
    // for every other tier kind.
    let body_slot = match children.child_2.slot().as_ref() {
        Some(slot) => slot,
        None => {
            let span =
                crate::error::Span::new(tier_node.start_byte() as u32, tier_node.end_byte() as u32);
            let tier = DependentTier::UserDefined(crate::model::UserDefinedDependentTier {
                label: stored_label,
                content: None,
                span,
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
    let body_node = match body_slot.view() {
        SlotView::Present(n) => n.raw_node(),
        SlotView::Missing(n) => n,
        SlotView::Error(n) => {
            errors.report(analyze_dependent_tier_error(n, input));
            report_missing_x_content(tier_node, tier_label, input, errors);
            return;
        }
        SlotView::Absent(NoChild) => {
            report_missing_x_content(tier_node, tier_label, input, errors);
            return;
        }
    };
    let content_text = match body_node.utf8_text(input.as_bytes()) {
        Ok(text) => text,
        Err(_) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(body_node.start_byte(), body_node.end_byte()),
                ErrorContext::new(
                    input,
                    body_node.start_byte()..body_node.end_byte(),
                    TEXT_WITH_BULLETS,
                ),
                "User-defined tier content is not valid UTF-8",
            ));
            return;
        }
    };

    // Content must be non-empty
    let content = match NonEmptyString::new(content_text) {
        Ok(c) => c,
        Err(_) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                format!("Empty content in user-defined tier %x{}", tier_label),
            ));
            return;
        }
    };

    let span = crate::error::Span::new(tier_node.start_byte() as u32, tier_node.end_byte() as u32);

    let tier = DependentTier::UserDefined(crate::model::UserDefinedDependentTier {
        label: stored_label,
        content: Some(content),
        span,
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
        SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
        ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
        format!("Missing content in user-defined tier %x{tier_label}"),
    ));
}

/// Handle unsupported dependent tiers (%custom, %foo, etc.) caught by the grammar catch-all.
/// These are stored as UserDefined tiers so the file can still be parsed.
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
pub(super) fn apply_unsupported_tier(
    utterance: &mut Utterance,
    node: UnsupportedDependentTierNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) {
    let tier_node = node.raw_node();
    let children = extract_unsupported_dependent_tier(node);
    let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
    surface_displaced(
        &children.unexpected,
        "unsupported_dependent_tier",
        input,
        errors,
    );

    let Some(label) = tier_name(
        children.child_0.slot(),
        &UNSUPPORTED_PREFIX,
        tier_node,
        input,
        errors,
    ) else {
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
    // `child_0`, a real named node, is likewise visitor-driven above. `input` is
    // `&str` (already valid UTF-8), so slicing at the span's byte boundaries cannot
    // fail; a range outside the input is the traversal's own failure (E330, the
    // internal code), reported as such rather than read as an empty body, which
    // is the file's fact and E756's to name: no tier claims a content the parser
    // did not read.
    let Some(body) = input.get(children.child_2.range.clone()) else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
            ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
            "Unsupported tier body span lies outside the input",
        ));
        return;
    };

    // An empty tier is KEPT, carrying `None`, rather than dropped with a
    // generic parse error. It used to be dropped because the model could not
    // represent the empty case; now that it can, the tier survives with the
    // truth in it and E756 reports it during validation, which is where a
    // "this line declares nothing" rule belongs. Dropping it also lost the
    // label, and left this parser disagreeing with the re2c one, which kept a
    // fabricated tier instead.
    let content = NonEmptyString::new(body.trim()).ok();
    let span = crate::error::Span::new(tier_node.start_byte() as u32, tier_node.end_byte() as u32);
    let tier = DependentTier::Unsupported(crate::model::UserDefinedDependentTier {
        label,
        content,
        span,
    });

    utterance
        .dependent_tiers
        .push(DependentTierEntry::with_separator(tier, separator));
}
