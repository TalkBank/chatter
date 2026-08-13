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
    AsRawNode, MainTierChildren, MainTierNode, NodeSlot, TierBodyNode, extract_main_tier,
    extract_tier_body,
};
use crate::model::{
    Bullet, LanguageCode, Linker, MainTier, Postcode, Terminator, TierSeparator, UtteranceContent,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::content::{MainTierRegion, classify_main_tier_recovery, surface_main_tier_sink};

mod body;
mod ending;
mod linkers;
mod prefix;

/// Parse a `tier_body` the traversal displaced into its sink, or report the
/// terminator genuinely missing when there is none.
///
/// Shared by every non-`Present` slot state, because any of them can occur with
/// the real body sitting in the sink. Claiming a missing terminator without
/// asking is how chatter told users an utterance had none on a line ending in
/// " .".
fn parse_displaced_or_report_missing<'tree>(
    displaced: Option<tree_sitter::Node<'tree>>,
    node: tree_sitter::Node<'tree>,
    source: &str,
    original_input: &str,
    errors: &impl ErrorSink,
) -> TierBodyData {
    match displaced {
        Some(raw) => {
            let tier_body_children = extract_tier_body(TierBodyNode(raw));
            body::parse_tier_body(
                &tier_body_children,
                raw.byte_range(),
                source,
                original_input,
                errors,
            )
        }
        None => {
            report_missing_child(
                node.byte_range(),
                original_input,
                errors,
                ErrorCode::MissingTerminator,
                "Missing terminator in main tier",
            );
            TierBodyData::empty()
        }
    }
}

/// The `tier_body` an ERROR displaced from its positional slot, if there is one.
///
/// Reads the traversal's own `unexpected` sink, which is deliberately a
/// `Vec<tree_sitter::Node>` of children that filled no grammar position. Routing
/// one back into its typed wrapper needs its kind, and this is the ONLY way to
/// use the sink at all; it is not the banned hand-walk, which is driving the
/// parse by scanning `node.kind()` instead of the generated traversal, or
/// classifying the text of an ERROR node.
fn displaced_tier_body<'tree>(
    unexpected: &[tree_sitter::Node<'tree>],
) -> Option<tree_sitter::Node<'tree>> {
    unexpected
        .iter()
        .copied()
        .find(|candidate| candidate.kind() == "tier_body")
}

/// Positional label for the `tier_body` slot, used by the unreachable
/// no-`tier_body` recovery arm's `StructuralOrderError` diagnostic. Mirrors the
/// child cursor the positional walk reaches after the five prefix positions
/// (star=0, speaker=1, colon=2, tab=3, sep_trailing_space=4, tier_body=5).
const TIER_BODY_POSITION: usize = 5;

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
    // Speaker prefix slots (`star`, `speaker`, `colon`, `tab`), the optional
    // `sep_trailing_space` (E758 provenance), and the `tier_body` slot, read
    // from the generated typed visitor. Every field is `Positioned<..>`: read
    // `.slot`.
    let main = extract_main_tier(MainTierNode(node));

    // Speaker prefix (`* speaker : tab`).
    let prefix = prefix::parse_prefix(&main, node.byte_range(), source, original_input, errors);

    // The optional trailing separator space after the tab, before tier_body
    // (E758 provenance): `main.child_4.slot` is `Option<NodeSlot<..>>`. Only
    // `Present` carries a real span; every other outer/inner state (grammar
    // omits the node entirely, or it recovers as Missing/Error/Unexpected/
    // Absent) means no illegal trailing space was captured, mirroring how
    // `body.linkers.slot` is read for the other optional single-symbol slot.
    let separator = sep_from_slot(&main);

    // Robustness for a recovery ERROR produced by malformed content right after
    // the tab (a bare `&` -> E207, a retrace/bracket code at tier start -> E747,
    // an italic control byte, ...). Before the `sep_trailing_space` slot existed
    // the grammar was `seq(star, speaker, colon, tab, tier_body)`, so that ERROR
    // landed in the `tier_body` slot and was classified by `analyze_word_error`.
    // With `optional(sep_trailing_space)` now between tab and tier_body, the ERROR
    // can land in EITHER position depending on the shape: a bare `&` (no trailing
    // space) fills the `sep_trailing_space` slot itself as `NodeSlot::Error`,
    // while a retrace/bracket followed by a space (`[/] world`) leaves the real
    // `sep_trailing_space` in its slot and the ERROR as a SEPARATE sibling node.
    // Checking only one slot missed the second shape and downgraded the specific
    // diagnostic to a generic whole-tree-backstop E316 (the "never silently drop
    // a recovery node" rule). So classify EVERY direct ERROR child of the main
    // tier here, skipping the one the `tier_body` (child_5) Error arm below
    // classifies itself, to avoid a duplicate. The whole-tree backstop still
    // emits E316 for coverage; the richer specific code coexists with it.
    let tier_body_error_span = match main.child_5.slot() {
        NodeSlot::Error(error_node) => Some((error_node.start_byte(), error_node.end_byte())),
        _ => None,
    };
    let mut error_cursor = node.walk();
    for child in node.children(&mut error_cursor) {
        if child.is_error() && Some((child.start_byte(), child.end_byte())) != tier_body_error_span
        {
            errors.report(classify_main_tier_recovery(
                child,
                source,
                MainTierRegion::OutsideBody,
            ));
        }
    }

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
    // The tier_body may be DISPLACED into the sink under any non-Present slot
    // state, not only `Error`. The traversal absorbs an ERROR child at whatever
    // position its cursor is at, so a single stray ERROR shifts every later
    // position: the tsgu session's minimal case reports two REQUIRED positions
    // as `Absent` while their content sits, correctly typed, in the sink.
    //
    // So the question this asks is not "which slot state is it?" but "is the
    // content actually here?", which is the only question whose answer is a
    // fact about the user's file rather than about our recovery.
    let consumed_tier_body = match main.child_5.slot() {
        NodeSlot::Present(_) | NodeSlot::Missing(_) => None,
        NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent => {
            displaced_tier_body(&main.unexpected)
        }
    };
    let tier = match main.child_5.slot() {
        NodeSlot::Present(tier_body) => {
            let raw = tier_body.raw_node();
            let tier_body_children = extract_tier_body(TierBodyNode(raw));
            body::parse_tier_body(
                &tier_body_children,
                raw.byte_range(),
                source,
                original_input,
                errors,
            )
        }
        NodeSlot::Missing(tier_body_node) => {
            let tier_body_children = extract_tier_body(TierBodyNode(*tier_body_node));
            body::parse_tier_body(
                &tier_body_children,
                tier_body_node.byte_range(),
                source,
                original_input,
                errors,
            )
        }
        NodeSlot::Error(error_node) => {
            errors.report(classify_main_tier_recovery(
                *error_node,
                source,
                MainTierRegion::Body,
            ));
            // An ERROR in this slot DISPLACES `tier_body` rather than replacing
            // it: the traversal puts the displaced node in its `unexpected`
            // sink, which spec Section 7 guarantees never drops a child. Parse
            // it from there.
            //
            // Without this, an utterance opening with an annotation
            // (`*CHI:\t[: closed] .`) was told its terminator was missing while
            // the terminator sat in the tree, in the very `tier_body` this arm
            // had thrown away: the lowering discarded the node holding the
            // answer and then reported the absence as a fact about the user's
            // file. Real CLAN CHECK reports one thing for that input, and so
            // should chatter.
            parse_displaced_or_report_missing(
                consumed_tier_body,
                node,
                source,
                original_input,
                errors,
            )
        }
        NodeSlot::Unexpected(unexpected_node) => {
            report_unexpected_child(
                *unexpected_node,
                source,
                errors,
                "tier_body",
                TIER_BODY_POSITION,
            );
            parse_displaced_or_report_missing(
                consumed_tier_body,
                node,
                source,
                original_input,
                errors,
            )
        }
        NodeSlot::Absent => parse_displaced_or_report_missing(
            consumed_tier_body,
            node,
            source,
            original_input,
            errors,
        ),
    };

    // Surface the carrier's own `unexpected` sink (R2), classified by region.
    //
    // This is NOT the empty set the comment here used to claim. The same
    // sentence stood over `tier_body`'s sink until a generator fix started
    // absorbing ERROR nodes at the cursor position, which put real content in
    // it and degraded six error codes to E316. "Empty on every fixture probed
    // so far" was a statement about our fixtures, not about the grammar, and it
    // was read as the latter for months.
    // Placed BEFORE the speaker-check early return below, preserving the prior
    // "diagnostics emitted before reject" ordering the doc comment states.
    // The displaced `tier_body`, if this tier had one, was CONSUMED above and
    // must not also be surfaced here as an unexplained leftover.
    let leftover: Vec<tree_sitter::Node<'_>> = main
        .unexpected
        .iter()
        .copied()
        .filter(|candidate| Some(candidate.id()) != consumed_tier_body.map(|n| n.id()))
        .collect();
    surface_main_tier_sink(&leftover, MainTierRegion::OutsideBody, source, errors);

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
        .slot()
        .raw_node()
        .map(|colon| Span::new(colon.end_byte() as u32, node.end_byte() as u32));

    let mut main_tier = MainTier::new(speaker, tier.content, tier.terminator)
        .with_span(span)
        .with_speaker_span(prefix.speaker_span)
        .with_linkers(tier.linkers)
        .with_postcodes(tier.postcodes)
        .with_separator(separator);

    // Extract a terminal bullet that the greedy contents rule left in content.
    main_tier.content.extract_terminal_bullet();

    if let Some(span) = content_span {
        main_tier = main_tier.with_content_span(span);
    }

    if let Some(lang_code) = tier.language_code {
        main_tier = main_tier.with_language_code(lang_code);
    }

    if let Some(lang_span) = tier.language_code_span {
        main_tier = main_tier.with_language_code_span(lang_span);
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
    /// Source span of the `[- code]` precode token (opening `[` at `.start`),
    /// when present. Provenance for source-spacing validation (E758).
    pub language_code_span: Option<Span>,
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
            language_code_span: None,
            content: Vec::new(),
            terminator: None,
            postcodes: Vec::new(),
            bullet: None,
        }
    }
}

/// Byte offset just past the main tier's `tab`, when the tab actually parsed.
///
/// `None` means the tab is missing or recovered, in which case nothing can be
/// proven adjacent to it and no adjacency-dependent claim is asserted.
fn tab_end(main: &MainTierChildren<'_>) -> Option<usize> {
    match main.child_3.slot() {
        NodeSlot::Present(tab_node) => Some(tab_node.raw_node().end_byte()),
        NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent => {
            None
        }
    }
}

/// Decode the optional `sep_trailing_space` slot into a [`TierSeparator`]
/// (E758 provenance). Mirrors the `body.linkers.slot` read pattern for the
/// other optional single-symbol slot (see `body.rs`): only `Present` carries
/// a real span; the outer `None` and every inner non-`Present` state
/// (Missing/Error/Unexpected/Absent) mean no illegal trailing space was
/// captured, and map to a clean separator with no diagnostic (the E758 check
/// itself is a later validation pass over this provenance, not parse-time).
fn sep_from_slot(main: &MainTierChildren<'_>) -> TierSeparator {
    match main.child_4.slot() {
        // E758 says "extra whitespace BETWEEN THE TAB AND the tier content", so
        // the span only carries that meaning while it is genuinely adjacent to
        // the tab. Filling this slot does not establish that: when a recovery
        // node sits between the tab and the whitespace (`*CHI:\t[/] we go .`),
        // the slot is filled by ordinary space between two words, and reporting
        // it as a leading-space violation is a diagnostic about a tab the user
        // cannot see near it.
        //
        // The adjacency is a relationship between two values, so it is checked
        // rather than assumed from position. This takes the CARRIER rather than
        // the two values: a `tab_end: usize` parameter type-checks against any
        // node's end byte in the crate, so the pairing would have been held
        // together by the caller's care. Holding `MainTierChildren` is itself
        // the proof that both slots came from the same `main_tier`.
        //
        // The tab is read INSIDE this arm, so a well-formed utterance (no
        // separator span at all, which is the overwhelming majority of a corpus)
        // never touches it.
        Some(NodeSlot::Present(sep_node))
            if tab_end(main) == Some(sep_node.raw_node().start_byte()) =>
        {
            let node = sep_node.raw_node();
            TierSeparator::with_trailing_space(Span::new(
                node.start_byte() as u32,
                node.end_byte() as u32,
            ))
        }
        Some(NodeSlot::Present(_)) => TierSeparator::CLEAN,
        Some(
            NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent,
        )
        | None => TierSeparator::CLEAN,
    }
}

/// Report a required-child omission, located on the carrier node.
///
/// `carrier` is the byte range of the node whose child is missing (the
/// `main_tier` or `tier_body` node), in the PARSE's coordinate space, so the
/// diagnostic lands on the offending line. Until 2026-07-30 this helper used
/// `0..original_input.len()`, a fragment-local span: correct for the
/// standalone fragment entry points, but in whole-file parsing it rendered
/// every such diagnostic at line 1 over the header block (the IISRP-residue
/// finding; same family as the annotated-word wrapper span).
pub(super) fn report_missing_child(
    carrier: std::ops::Range<usize>,
    original_input: &str,
    errors: &impl ErrorSink,
    code: ErrorCode,
    message: &str,
) {
    errors.report(ParseError::new(
        code,
        Severity::Error,
        SourceLocation::from_offsets(carrier.start, carrier.end),
        ErrorContext::new(original_input, carrier, ""),
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
