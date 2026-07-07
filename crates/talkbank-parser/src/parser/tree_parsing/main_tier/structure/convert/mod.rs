//! Convert `main_tier` CST nodes into `MainTier` model values.
//!
//! Driven by the generated typed visitor. `extract_main_tier` yields the speaker
//! prefix slots (`star`, `speaker`, `colon`, `tab`) plus the `tier_body` slot;
//! `extract_tier_body` then yields the body/end slots (linkers, langcode,
//! contents, utterance_end) in a single pass. This replaces the previous
//! positional `idx`-cursor + `node.kind()` hand-walk and unifies what were
//! separate body and end re-walks. The `utterance_end` internals are decoded off
//! the generated visitor by `ending::parse_utterance_end` (task 3d, via
//! `extract_utterance_end`); the `contents` internals are still handed to the
//! existing `parse_main_tier_contents` (task 3c).
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{
    AsRawNode, MainTierNode, NodeSlot, TierBodyNode, extract_main_tier, extract_tier_body,
};
use crate::model::{
    Bullet, LanguageCode, Linker, MainTier, Postcode, Terminator, UtteranceContent,
};
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::content::analyze_word_error;

mod body;
mod ending;
mod linkers;
mod prefix;

/// Positional label for the `tier_body` slot, used by the unreachable
/// no-`tier_body` recovery arm's `StructuralOrderError` diagnostic. Mirrors the
/// child cursor the previous positional walk reached after the four prefix
/// positions (star=0, speaker=1, colon=2, tab=3, tier_body=4).
const TIER_BODY_POSITION: usize = 4;

/// Convert a `main_tier` CST node into the typed `MainTier` domain model.
///
/// Mirrors the specification in the CHAT manual’s Main Tier chapter by parsing the speaker prefix, body,
/// terminator/postcode tail, and optional media bullet. Diagnostics are reported when optional sections
/// deviate from the expected layout, keeping the eventual `MainTier` instance aligned with the published
/// utterance structure (speaker, colon, content, terminator).
///
/// Shared by the production utterance path and the single-main-tier parser API,
/// so migrating this one function drives both off the generated visitor.
pub fn convert_main_tier_node(
    node: Node,
    source: &str,
    original_input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<MainTier> {
    // Speaker prefix slots (`star`, `speaker`, `colon`, `tab`) and the
    // `tier_body` slot, read from the generated typed visitor. Every field is
    // `Positioned<..>`: read `.slot`.
    let main = extract_main_tier(MainTierNode(node));

    // Speaker prefix (`* speaker : tab`).
    let prefix = prefix::parse_prefix(&main, source, original_input, errors);

    // tier_body (linkers / langcode / contents / utterance_end). `Present`
    // carries a typed `TierBodyNode`; `Missing` carries a bare `Node` directly
    // under the NEW closed `NodeSlot`, so the two are split into separate arms
    // (both still descend through `extract_tier_body`); a MISSING tier_body is
    // childless, so its inner slots are absent and the "Missing terminator in
    // tier_body" recovery fires, exactly as the previous re-walk did. The
    // remaining slot states are unreachable in the real grammar (tier_body is a
    // required child that recovers as Present/MISSING) and route to the
    // missing-main-tier recovery, surfacing any stray node. Matched
    // EXHAUSTIVELY so no recovery node is silently dropped.
    let tier = match &main.child_4.slot {
        NodeSlot::Present(tier_body) => {
            let tier_body_children = extract_tier_body(TierBodyNode(tier_body.raw_node()));
            body::parse_tier_body(&tier_body_children, source, original_input, errors)
        }
        NodeSlot::Missing(tier_body_node) => {
            let tier_body_children = extract_tier_body(TierBodyNode(*tier_body_node));
            body::parse_tier_body(&tier_body_children, source, original_input, errors)
        }
        NodeSlot::Error(error_node) => {
            errors.report(analyze_word_error(*error_node, source));
            report_missing_child(
                original_input,
                errors,
                ErrorCode::MissingTerminator,
                "Missing terminator in main tier",
            );
            TierBodyData::empty()
        }
        NodeSlot::Unexpected(unexpected_node) => {
            report_unexpected_child(
                *unexpected_node,
                source,
                errors,
                "tier_body",
                TIER_BODY_POSITION,
            );
            report_missing_child(
                original_input,
                errors,
                ErrorCode::MissingTerminator,
                "Missing terminator in main tier",
            );
            TierBodyData::empty()
        }
        NodeSlot::Absent => {
            report_missing_child(
                original_input,
                errors,
                ErrorCode::MissingTerminator,
                "Missing terminator in main tier",
            );
            TierBodyData::empty()
        }
    };

    // Surface the carrier's own `unexpected` sink (R2). Empty on every fixture
    // probed so far; load-bearing once the whole-tree backstop is deleted.
    // Placed BEFORE the speaker-check early return below, preserving the prior
    // "diagnostics emitted before reject" ordering the doc comment states.
    surface_unexpected(&main.unexpected, source, errors);

    // No fabricated speaker fallback: if speaker could not be parsed, skip
    // main-tier construction. (All diagnostics above are still emitted first,
    // preserving the prior emit-then-reject ordering.)
    let speaker = match prefix.speaker.filter(|speaker| !speaker.is_empty()) {
        Some(speaker) => speaker,
        None => return ParseOutcome::rejected(),
    };

    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

    // Content span: from after the colon to the end of the main_tier line.
    // Grammar: main_tier: seq($.star, $.speaker, $.colon, $.tab, $.tier_body).
    // The colon slot's raw node gives the same byte boundary the prior positional
    // `node.child(2)` read (on the valid path the colon is always at raw child 2,
    // and `raw_node()` returns `None` only when the colon slot is `Absent`, exactly
    // like the old `if let Some(colon_node) = node.child(2)` guard).
    let content_span = main
        .child_2
        .slot
        .raw_node()
        .map(|colon| Span::new(colon.end_byte() as u32, node.end_byte() as u32));

    let mut main_tier = MainTier::new(speaker, tier.content, tier.terminator)
        .with_span(span)
        .with_speaker_span(prefix.speaker_span)
        .with_linkers(tier.linkers)
        .with_postcodes(tier.postcodes);

    // Extract a terminal bullet that the greedy contents rule left in content.
    main_tier.content.extract_terminal_bullet();

    if let Some(span) = content_span {
        main_tier = main_tier.with_content_span(span);
    }

    if let Some(lang_code) = tier.language_code {
        main_tier = main_tier.with_language_code(lang_code);
    }

    // Bullet: grammar-routed bullet from utterance_end takes priority.
    if let Some(b) = tier.bullet {
        main_tier = main_tier.with_bullet(b);
    }

    ParseOutcome::parsed(main_tier)
}

/// Parsed prefix slice (`*`, speaker, `:`, tab).
pub(super) struct PrefixData {
    pub speaker: Option<String>,
    pub speaker_span: Span,
}

/// Parsed `tier_body` payload: linkers, optional language code, content, and the
/// terminator / postcode / bullet tail.
///
/// Unifies what were previously separate `BodyData` (linkers / langcode /
/// content) and `EndData` (terminator / postcodes / bullet) values, now that a
/// single `extract_tier_body` call yields every tier-body slot.
pub(super) struct TierBodyData {
    pub linkers: Vec<Linker>,
    pub language_code: Option<LanguageCode>,
    pub content: Vec<UtteranceContent>,
    pub terminator: Option<Terminator>,
    pub postcodes: Vec<Postcode>,
    pub bullet: Option<Bullet>,
}

impl TierBodyData {
    /// Empty tier-body payload, used by the unreachable no-`tier_body` recovery
    /// arms (the model carries no linkers/content/terminator in that case).
    fn empty() -> Self {
        Self {
            linkers: Vec::new(),
            language_code: None,
            content: Vec::new(),
            terminator: None,
            postcodes: Vec::new(),
            bullet: None,
        }
    }
}

/// Report a required-child omission in a user-facing input slice.
pub(super) fn report_missing_child(
    original_input: &str,
    errors: &impl ErrorSink,
    code: ErrorCode,
    message: &str,
) {
    errors.report(ParseError::new(
        code,
        Severity::Error,
        SourceLocation::from_offsets(0, original_input.len()),
        ErrorContext::new(original_input, 0..original_input.len(), ""),
        message,
    ));
}

/// Report an unexpected node kind at a positional slot in `main_tier`.
pub(super) fn report_unexpected_child(
    child: Node,
    source: &str,
    errors: &impl ErrorSink,
    expected: &str,
    position: usize,
) {
    errors.report(ParseError::new(
        ErrorCode::StructuralOrderError,
        Severity::Error,
        SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
        ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
        format!(
            "Expected '{}' at position {} of main_tier, found '{}'",
            expected,
            position,
            child.kind()
        ),
    ));
}
