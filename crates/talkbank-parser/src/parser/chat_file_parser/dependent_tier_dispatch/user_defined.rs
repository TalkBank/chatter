//! Dispatch for user-defined and unsupported dependent tiers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#User_Defined_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, NodeSlot, UnsupportedDependentTierNode, XDependentTierNode,
    extract_unsupported_dependent_tier, extract_x_dependent_tier,
};
use crate::model::dependent_tier::{DependentTier, DependentTierEntry};
use crate::model::{NonEmptyString, Utterance};
use crate::node_types::{
    TEXT_WITH_BULLETS, UNSUPPORTED_DEPENDENT_TIER, UNSUPPORTED_TIER_PREFIX, X_DEPENDENT_TIER,
    X_TIER_PREFIX,
};
use talkbank_model::model::dependent_tier::{
    PhoalnTier, SylTier, SylTierType, parse_phoaln_content, parse_syl_content,
};
use tree_sitter::Node;

use crate::parser::tree_parsing::parser_helpers::surface_unexpected;

/// Parse and attach user-defined/unsupported dependent tiers.
pub(super) fn apply_user_defined_tier(
    utterance: &mut Utterance,
    tier_kind: &str,
    tier_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> bool {
    match tier_kind {
        X_DEPENDENT_TIER => apply_x_tier(utterance, tier_node, input, errors),
        UNSUPPORTED_DEPENDENT_TIER => apply_unsupported_tier(utterance, tier_node, input, errors),
        _ => false,
    }
}

/// The node a positional slot resolves to, treating a `Present` node and a
/// tree-sitter `Missing` placeholder as "found" and every other recovery state
/// as "not found".
///
/// This reproduces the removed `find_child_by_kind` helper byte for byte: that
/// helper located the FIRST direct child whose `kind()` equalled a target, and a
/// MISSING node still carries its expected kind (so it was found and its empty
/// text read), while an `ERROR` node (kind `ERROR`), an unexpected-kind node, or
/// an absent child never satisfied the kind filter. Callers below then apply the
/// same `utf8_text` decode and the same diagnostics the removed code did.
fn found_node<'tree, T>(slot: &NodeSlot<'tree, T>) -> Option<Node<'tree>>
where
    T: AsRawNode<'tree>,
{
    match slot {
        NodeSlot::Present(node) => Some(node.raw_node()),
        NodeSlot::Missing(raw) => Some(*raw),
        NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent => None,
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
/// TEXT_WITH_BULLETS)` `match child.kind()` scans (see [`found_node`]).
fn apply_x_tier(
    utterance: &mut Utterance,
    tier_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> bool {
    // Grammar: x_dependent_tier = x_tier_prefix, tier_sep, text_with_bullets, newline
    // x_tier_prefix is a single token matching /%x[a-zA-Z][a-zA-Z0-9]*/
    let children = extract_x_dependent_tier(XDependentTierNode(tier_node));
    let separator = super::helpers::dependent_tier_separator(&children.child_1.slot);
    surface_unexpected(&children.unexpected, input, errors);

    let full_prefix = match found_node(&children.child_0.slot) {
        Some(n) => match n.utf8_text(input.as_bytes()) {
            Ok(text) => text,
            Err(_) => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(n.start_byte(), n.end_byte()),
                    ErrorContext::new(input, n.start_byte()..n.end_byte(), X_TIER_PREFIX),
                    "User-defined tier prefix is not valid UTF-8",
                ));
                return true;
            }
        },
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                "Missing tier prefix in user-defined tier",
            ));
            return true;
        }
    };

    // Extract label by stripping "%x" prefix (e.g. "%xfoo" → "foo")
    let tier_label = full_prefix.strip_prefix("%x").unwrap_or(full_prefix);

    // Empty user-defined tier (E756). The grammar makes ONLY this rule's body
    // optional (see grammar.js `x_dependent_tier`), so the generated visitor
    // types the body slot as `Option<NodeSlot<..>>`: when a `%xLABEL:` line
    // carries nothing but a trailing space, the separator absorbs that space
    // and NO `text_with_bullets` child is produced, so the body slot is
    // `None`. This is a real (if invalid) construct that must lower to an
    // empty user-defined tier and flag E756, not recover via a spurious
    // E342/E330. Route it through the shared empty-tier check so the parse
    // path and the validation path emit the identical E756 diagnostic. The
    // label carries the leading 'x' (a `%xtst` tier validates as "xtst"),
    // matching the label stored on a pushed `UserDefined` tier below.
    let body_slot = match children.child_2.slot.as_ref() {
        Some(slot) => slot,
        None => {
            let span =
                crate::error::Span::new(tier_node.start_byte() as u32, tier_node.end_byte() as u32);
            let e756_label = format!("x{}", tier_label);
            talkbank_model::validation::check_user_defined_tier_content(
                &e756_label,
                "",
                span,
                errors,
            );
            return true;
        }
    };

    let content_text = match found_node(body_slot) {
        Some(n) => match n.utf8_text(input.as_bytes()) {
            Ok(text) => text,
            Err(_) => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(n.start_byte(), n.end_byte()),
                    ErrorContext::new(input, n.start_byte()..n.end_byte(), TEXT_WITH_BULLETS),
                    "User-defined tier content is not valid UTF-8",
                ));
                return true;
            }
        },
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                format!("Missing content in user-defined tier %x{}", tier_label),
            ));
            return true;
        }
    };

    // Content must be non-empty
    let content = match NonEmptyString::new(content_text) {
        Some(c) => c,
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                format!("Empty content in user-defined tier %x{}", tier_label),
            ));
            return true;
        }
    };

    let span = crate::error::Span::new(tier_node.start_byte() as u32, tier_node.end_byte() as u32);

    // Intercept Phon project tiers (%xmodsyl, %xphosyl, %xphoaln) and route
    // them to structured types. When the 'x' prefix is eventually dropped
    // (global replace %xmodsyl → %modsyl), the grammar rules in raw.rs
    // take over seamlessly, both paths produce the same model types.
    match tier_label {
        "modsyl" => {
            let words = parse_syl_content(content.as_str());
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Modsyl(SylTier::new(SylTierType::Modsyl, words).with_span(span)),
                    separator,
                ));
            return true;
        }
        "phosyl" => {
            let words = parse_syl_content(content.as_str());
            utterance
                .dependent_tiers
                .push(DependentTierEntry::with_separator(
                    DependentTier::Phosyl(SylTier::new(SylTierType::Phosyl, words).with_span(span)),
                    separator,
                ));
            return true;
        }
        "phoaln" => {
            match parse_phoaln_content(content.as_str()) {
                Ok(words) => {
                    utterance
                        .dependent_tiers
                        .push(DependentTierEntry::with_separator(
                            DependentTier::Phoaln(PhoalnTier::new(words).with_span(span)),
                            separator,
                        ));
                }
                Err(e) => {
                    errors.report(ParseError::new(
                        ErrorCode::InvalidDependentTier,
                        Severity::Error,
                        SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                        ErrorContext::new(
                            input,
                            tier_node.start_byte()..tier_node.end_byte(),
                            "%xphoaln",
                        ),
                        format!("malformed %xphoaln content: {}", e),
                    ));
                }
            }
            return true;
        }
        _ => {}
    }

    // For UserDefined tiers, prepend 'x' to the label to avoid collision with built-in tiers
    // e.g., %xmor stores label="xmor" not "mor" to avoid collision with %mor
    let label = NonEmptyString::new_unchecked(format!("x{}", tier_label));

    let tier = DependentTier::UserDefined(crate::model::UserDefinedDependentTier {
        label,
        content,
        span,
    });

    utterance
        .dependent_tiers
        .push(DependentTierEntry::with_separator(tier, separator));
    true
}

/// Handle unsupported dependent tiers (%custom, %foo, etc.) caught by the grammar catch-all.
/// These are stored as UserDefined tiers so the file can still be parsed.
///
/// The PREFIX is driven by the generated typed visitor:
/// `extract_unsupported_dependent_tier`'s `child_0` (`unsupported_tier_prefix`, a
/// real named node) replaces the removed
/// `find_child_by_kind(tier_node, UNSUPPORTED_TIER_PREFIX)` scan.
///
/// The CONTENT canNOT be driven by the visitor: the `unsupported_dependent_tier`
/// body is the ANONYMOUS regex `/[^\n\r]*/`, which tree-sitter does not surface
/// as a named child (ground-truth verified via `tree-sitter parse`, see the
/// inline comment on the content read below), so the generated `child_2` slot is
/// never populated at runtime. The body is therefore read from the tier's source
/// text exactly as the pre-migration code did, which is behavior-preserving. The
/// generator-side gap (modeling a bare anonymous seq-token as a leaf) is flagged
/// for 4a / conformance.
fn apply_unsupported_tier(
    utterance: &mut Utterance,
    tier_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> bool {
    let children = extract_unsupported_dependent_tier(UnsupportedDependentTierNode(tier_node));
    let separator = super::helpers::dependent_tier_separator(&children.child_1.slot);
    surface_unexpected(&children.unexpected, input, errors);

    // Extract the tier prefix (e.g. "%custom") from the unsupported_tier_prefix child.
    let label_text = match found_node(&children.child_0.slot) {
        Some(n) => match n.utf8_text(input.as_bytes()) {
            Ok(text) => {
                // Strip the leading '%'
                text.strip_prefix('%').unwrap_or(text)
            }
            Err(_) => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(n.start_byte(), n.end_byte()),
                    ErrorContext::new(input, n.start_byte()..n.end_byte(), UNSUPPORTED_TIER_PREFIX),
                    "Unsupported tier prefix is not valid UTF-8",
                ));
                return true;
            }
        },
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                "Missing tier prefix in unsupported dependent tier",
            ));
            return true;
        }
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
    // fail; `.get` keeps this panic-free regardless of a malformed range.
    let content_str = input
        .get(children.child_2.range.clone())
        .map(str::trim)
        .unwrap_or("");

    let content = match NonEmptyString::new(content_str) {
        Some(c) => c,
        None => {
            // Empty unsupported tier, skip it
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                format!("Empty unsupported dependent tier %{}", label_text),
            ));
            return true;
        }
    };

    let label = NonEmptyString::new_unchecked(label_text);
    let span = crate::error::Span::new(tier_node.start_byte() as u32, tier_node.end_byte() as u32);
    let tier = DependentTier::Unsupported(crate::model::UserDefinedDependentTier {
        label,
        content,
        span,
    });

    utterance
        .dependent_tiers
        .push(DependentTierEntry::with_separator(tier, separator));
    true
}
