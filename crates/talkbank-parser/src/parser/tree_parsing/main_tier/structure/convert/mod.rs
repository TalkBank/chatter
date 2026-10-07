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
    AdmittedMainTierChildren, AsRawNode, MainTierNode, NoChild, NodeSlot, SlotView,
    SourceBindingError, SourceBound, SourceChildren, SourceSlotView, TierBodyNode,
};
use crate::model::{
    Bullet, LanguageCode, Linker, MainTier, Postcode, Terminator, TierSeparator, UtteranceContent,
};
use tree_sitter::Node;

use super::super::content::{MainTierRegion, classify_main_tier_recovery};
use crate::parser::tree_parsing::parser_helpers::surface_displaced;

mod body;
mod ending;
mod linkers;
mod prefix;

/// Evidence that conversion already emitted its rejection diagnostic.
/// Only the reporting transition constructs this value; a failed conversion
/// cannot return a diagnostic-free rejection to its consumers.
pub struct ReportedMainTierError {
    _private: (),
}

impl ReportedMainTierError {
    fn report(error: ParseError, errors: &impl ErrorSink) -> Self {
        errors.report(error);
        Self { _private: () }
    }
}

impl From<crate::parser::typed_cst::ReportedCstFailure> for ReportedMainTierError {
    fn from(_reported: crate::parser::typed_cst::ReportedCstFailure) -> Self {
        Self { _private: () }
    }
}

/// Report the terminator genuinely missing, when the tier has no body anywhere.
///
/// Reached only once [`TierBodyLocation`] has looked in BOTH places, the slot and
/// the sink. Claiming a missing terminator without asking the sink is how
/// chatter told users an utterance had none on a line ending in " .".
///
/// This was `parse_displaced_or_report_missing`, which took the displaced body
/// as a parameter and branched on it. The branch moved to the caller when the
/// location became a value, so what is left is the one arm that reports.
fn report_missing_tier_body(
    node: tree_sitter::Node<'_>,
    original_input: &str,
    errors: &impl ErrorSink,
) -> TierBodyData {
    report_missing_child(
        node.byte_range(),
        original_input,
        errors,
        ErrorCode::MissingTerminator,
        "Missing terminator in main tier",
    );
    TierBodyData::empty()
}

/// Where a main tier's `tier_body` actually is, and what its `unexpected` sink
/// holds once that body is taken out.
///
/// # Possession of `leftover` IS the proof the sink entry was removed
///
/// An ERROR at the `tier_body` slot position CAN displace the body rather than
/// replacing it: the traversal puts the real node in the carrier's `unexpected`
/// sink, which spec Section 7 guarantees never drops a child. The body must then
/// be parsed from the sink AND must not also be surfaced as an unexplained
/// leftover. (Measured unreached today; see [`TierBodyLocation::locate`].)
///
/// That used to be three separate steps holding one invariant by convention:
/// `main.child_5.slot()` was matched once to decide whether to search the sink
/// and again to decide the diagnostic (two matches that had to partition
/// identically, with nothing saying so), and the sink was filtered afterwards by
/// comparing tree-sitter node IDs:
///
/// ```text
/// .filter(|candidate| Some(candidate.id()) != consumed_tier_body.map(|n| n.raw_node().id()))
/// ```
///
/// Identity arithmetic under a comment asserting the relation is the tell for a
/// missing type. Here there is no way to obtain the body without also obtaining
/// the remainder, so "parsed the displaced body and also reported it as
/// unexplained" is not a state a caller can construct.
struct TierBodyLocation<'tree, 'source> {
    /// A located body's source admission, or nothing if this tier has none.
    /// Refusal is not absence and cannot trigger a missing-terminator fallback.
    body: Option<Result<SourceBound<'tree, 'source, TierBodyNode<'tree>>, SourceBindingError>>,
    /// The `unexpected` sink with the displaced body, if there was one, removed.
    leftover: Vec<tree_sitter::Node<'tree>>,
}

impl<'tree, 'source> TierBodyLocation<'tree, 'source> {
    /// Locate the body: its own slot first, then the sink.
    ///
    /// Reports NOTHING. What to say about the slot state is a separate question
    /// with its own match at the call site, and keeping it separate is what lets
    /// the two stop having to agree.
    ///
    /// # The sink half is UNREACHED today, and that was measured, not assumed
    ///
    /// Instrumenting this branch and running it over 2,136 sampled corpus files,
    /// all 363 error-corpus fixtures, all 107 reference-corpus files and eight
    /// hand-built adversarial main tiers fired it ZERO times (2026-08-26). The
    /// reconstruction's tail-aware splitting now places `tier_body` at its own
    /// slot even when an ERROR precedes it, so nothing is displaced into the
    /// sink; `*CHI:\t[: closed] .`, the input the displacement bug was found on,
    /// parses to `main_tier(star speaker colon tab ERROR sep_trailing_space
    /// tier_body)` with the body exactly where it belongs.
    ///
    /// KEPT ANYWAY, and deliberately. "Does not fire on everything I have" is
    /// not "cannot fire": this is recovery code, its absence once told users an
    /// utterance had no terminator on a line ending in " .", and the generator
    /// change that made it unnecessary is the same class of change that made it
    /// necessary. What is NOT justified is reading the paragraph below as a
    /// description of current behaviour, which is why this says so.
    ///
    /// The sink search projects the traversal's source-associated `unexpected`
    /// fields. `read_typed::<TierBodyNode>()` uses the generated kind classifier
    /// once, retaining both the wrapper and its range-admission result. No
    /// independent kind-name test or second classification is needed. This is
    /// the ONLY way to use the sink at all, and it is not the banned hand-walk,
    /// which is driving the parse by scanning `node.kind()` instead of the
    /// generated traversal.
    fn locate(
        associated: &SourceChildren<'tree, 'source, AdmittedMainTierChildren<'tree>>,
    ) -> Self {
        let main = associated.children();
        // The question is not "which slot state is it?" but "is the content
        // actually here?", which is the only one whose answer is a fact about
        // the user's file rather than about our recovery. The generated API
        // permits an absent required position and retains an unexpected sink;
        // it does not certify that the sink cannot contain the body.
        if let SourceSlotView::Present(body) = associated.field_child_5().slot().view() {
            return Self {
                body: Some(body.read()),
                leftover: main.unexpected.clone(),
            };
        }

        let mut leftover = Vec::with_capacity(main.unexpected.len());
        let mut body = None;
        for candidate in associated.field_unexpected().iter() {
            let raw = candidate.raw_node();
            if body.is_none()
                && let Some(admitted) = candidate.read_typed::<TierBodyNode>()
            {
                // Source admission cannot turn a located-but-unreadable body
                // into absence. Retain the refusal separately from no body.
                body = Some(admitted);
            } else {
                leftover.push(raw);
            }
        }
        Self { body, leftover }
    }
}

/// Every recovery node under one `main_tier`, reportable AT MOST ONCE each.
///
/// # "Already reported" was arithmetic in three places, and they disagreed
///
/// Three walks report these nodes: the direct-children walk below, the
/// `tier_body` slot's own `Error` arm, and the `unexpected` sink. Each carried
/// its own bookkeeping for what the others had done. The direct-children walk
/// compared SPANS against the `tier_body` slot's error
/// (`Some((child.start_byte(), child.end_byte())) != tier_body_error_span`); the
/// sink was filtered by node ID; and the whole-tree backstop one layer up dedups
/// by span OVERLAP. None knew about all the others, so on
/// `*CHI:\t[: closed] .` one ERROR, which is BOTH a direct child and a sink
/// entry, was reported twice at the identical span.
///
/// Here a node is TAKEN to be reported, and taking removes it, so a second
/// report is not something a caller can express. The three walks keep their
/// distinct regions and diagnostics; what they no longer keep is a private
/// theory of what the others did.
struct MainTierRecovery<'tree> {
    /// Recovery nodes not yet reported, in document order.
    unreported: Vec<tree_sitter::Node<'tree>>,
}

impl<'tree> MainTierRecovery<'tree> {
    /// Every ERROR directly under the main tier, plus every ERROR the traversal
    /// swept into its `unexpected` sink. A node in BOTH appears once, which is
    /// the duplicate this type exists to make unwritable.
    fn collect(node: tree_sitter::Node<'tree>, sink: &[tree_sitter::Node<'tree>]) -> Self {
        let mut unreported: Vec<tree_sitter::Node<'tree>> = Vec::new();
        let mut cursor = node.walk();
        let direct = node
            .children(&mut cursor)
            .filter(tree_sitter::Node::is_error);
        for candidate in direct.chain(sink.iter().copied().filter(tree_sitter::Node::is_error)) {
            if !unreported.contains(&candidate) {
                unreported.push(candidate);
            }
        }
        Self { unreported }
    }

    /// Take `node` for reporting, or `None` if it is not ours to report: either
    /// something already took it, or it is not a recovery node under this tier.
    fn take(&mut self, node: tree_sitter::Node<'tree>) -> Option<tree_sitter::Node<'tree>> {
        let at = self.unreported.iter().position(|held| *held == node)?;
        Some(self.unreported.remove(at))
    }

    /// Take everything still unreported, in document order.
    fn take_rest(&mut self) -> Vec<tree_sitter::Node<'tree>> {
        std::mem::take(&mut self.unreported)
    }
}

/// Convert a `main_tier` CST node into the typed `MainTier` domain model.
///
/// Mirrors the specification in the CHAT manual’s Main Tier chapter by parsing the speaker prefix, body,
/// terminator/postcode tail, and optional media bullet. Diagnostics are reported when optional sections
/// deviate from the expected layout, keeping the eventual `MainTier` instance aligned with the published
/// utterance structure (speaker, colon, content, terminator).
///
/// Shared by the production utterance path and the single-main-tier parser API,
/// so migrating this one function drives both off the generated visitor.
/// Rejection carries evidence of an already-emitted speaker diagnostic; body
/// recovery still runs before returning it. Success does not imply validity.
/// The source-bound input owns all node-text reads; `original_input` is retained
/// only as the caller's diagnostic context, including fragment presentation.
pub fn convert_main_tier_node<'tree>(
    typed: SourceBound<'tree, '_, MainTierNode<'tree>>,
    original_input: &str,
    errors: &impl ErrorSink,
) -> Result<MainTier, ReportedMainTierError> {
    let node = typed.raw_node();
    let source = typed.source();
    // Speaker prefix slots (`star`, `speaker`, `colon`, `tab`), the optional
    // `sep_trailing_space` (E758 provenance), and the `tier_body` slot, read
    // from the generated typed visitor. Every field is `Positioned<..>`: read
    // `.slot`.
    let extraction = crate::parser::typed_cst::canonical_grammar()
        .and_then(|grammar| typed.extract_admitted(grammar));
    let associated =
        crate::parser::typed_cst::report_reconstruction(extraction, node, source, errors)
            .map_err(ReportedMainTierError::from)?;
    let main = associated.children();

    // Speaker prefix (`* speaker : tab`).
    let prefix = prefix::parse_prefix(&associated, node.byte_range(), original_input, errors);

    // The optional trailing separator space after the tab, before tier_body
    // (E758 provenance): `main.child_4.slot` is `Option<NodeSlot<..>>`. Only
    // `Present` carries a real span; every other outer/inner state (grammar
    // omits the node entirely, or it recovers as Missing/Error/Absent) means
    // no illegal trailing space was captured, mirroring how
    // `body.linkers.slot` is read for the other optional single-symbol slot.
    let separator = sep_from_slot(main);

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
    let mut recovery = MainTierRecovery::collect(node, &main.unexpected);
    // The `tier_body` slot's own ERROR is classified by its arm below with the
    // richer Body region, so take it out of this walk's reach FIRST rather than
    // comparing spans against it afterwards.
    if let SlotView::Error(in_slot) = main.child_5.slot().view() {
        recovery.take(in_slot);
    }
    for child in recovery.take_rest() {
        errors.report(classify_main_tier_recovery(
            child,
            source,
            MainTierRegion::OutsideBody,
        ));
    }

    // Compiled grammar admission proves that a matched tier_body is not
    // Missing. It does not prove presence or rule out ERROR recovery. Preserve
    // location admission and the unexpected sink independently of this fact.
    // The tier_body may be DISPLACED into the sink under any non-Present slot
    // state, not only `Error`. Earlier traversal implementations displaced
    // required children after an ERROR. The current shared match plan avoids
    // independent rematching, but its public result still permits absent slots
    // and unexpected children; it does not prove displacement impossible.
    //
    // So the question this asks is not "which slot state is it?" but "is the
    // content actually here?", which is the only question whose answer is a
    // fact about the user's file rather than about our recovery.
    let located = TierBodyLocation::locate(&associated);

    // What to SAY about the slot state, which is a different question from where
    // the body is. An ERROR here displaces the body rather than replacing it, so
    // reporting the ERROR and still parsing the body are both correct; without
    // that, an utterance opening with an annotation (`*CHI:\t[: closed] .`) was
    // told its terminator was missing while the terminator sat in the tree, in
    // the very `tier_body` the arm had thrown away.
    match main.child_5.slot().view() {
        SlotView::Present(_) => {}
        SlotView::Error(error_node) => errors.report(classify_main_tier_recovery(
            error_node,
            source,
            MainTierRegion::Body,
        )),
        SlotView::Absent(NoChild) => {}
    }

    let tier = match located.body {
        Some(Ok(tier_body)) => {
            body::parse_tier_body(tier_body, original_input, errors).map_err(|fault| {
                ReportedMainTierError::report(
                    crate::parser::typed_cst::cst_failure_diagnostic(node, source, fault),
                    errors,
                )
            })
        }
        Some(Err(error)) => Err(ReportedMainTierError::report(
            ParseError::new(
                ErrorCode::InternalError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.byte_range(), "tier_body"),
                error.to_string(),
            ),
            errors,
        )),
        None => Ok(report_missing_tier_body(node, original_input, errors)),
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
    // `located.leftover` is the sink with the displaced body already out of it;
    // there is no way to hold it without that having happened.
    // The sink splits by OWNER, not by bookkeeping. Its ERROR nodes are
    // `MainTierRecovery`'s and were classified above with their region; what is
    // left is content that filled no grammar position, which is
    // `surface_displaced`'s. Partitioning on `is_error`, a property of the
    // node, is not the identity arithmetic this refactor removed: nothing here
    // has to know what another walk did.
    let unexpected_content: Vec<tree_sitter::Node<'_>> = located
        .leftover
        .iter()
        .copied()
        .filter(|candidate| !candidate.is_error())
        .collect();
    surface_displaced(&unexpected_content, "main_tier", source, errors);

    // No fabricated speaker fallback: if speaker could not be parsed, skip
    // main-tier construction. (All diagnostics above are still emitted first,
    // preserving the prior emit-then-reject ordering.)
    let (speaker, speaker_span) = prefix.speaker?.into_parts();
    let tier = tier?;

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
        .with_speaker_span(speaker_span)
        .with_linkers(tier.linkers)
        .with_postcodes(tier.postcodes)
        .with_separator(separator);

    if let Some(span) = content_span {
        main_tier = main_tier.with_content_span(span);
    }

    if let Some(lang_code) = tier.language_code {
        main_tier = main_tier.with_language_code(lang_code);
    }

    if let Some(lang_span) = tier.language_code_span {
        main_tier = main_tier.with_language_code_span(lang_span);
    }

    // Install the grammar-owned terminal slot before considering a content tail.
    if let Some(b) = tier.bullet {
        main_tier = main_tier.with_bullet(b);
    }
    main_tier.content.extract_terminal_bullet();

    Ok(main_tier)
}

/// Parsed prefix slice (`*`, speaker, `:`, tab).
pub(super) struct PrefixData {
    speaker: Result<prefix::ParsedSpeakerPrefix, ReportedMainTierError>,
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
fn tab_end(main: &AdmittedMainTierChildren<'_>) -> Option<usize> {
    match main.child_3.slot().view() {
        SlotView::Present(tab_node) => Some(tab_node.raw_node().end_byte()),
        SlotView::Missing(_) | SlotView::Error(_) | SlotView::Absent(NoChild) => None,
    }
}

/// Decode the optional `sep_trailing_space` slot into a [`TierSeparator`]
/// (E758 provenance). Mirrors the `body.linkers.slot` read pattern for the
/// other optional single-symbol slot (see `body.rs`): only `Present` carries
/// a real span; the outer `None` and every inner non-`Present` state
/// (Missing/Error/Absent) mean no illegal trailing space was
/// captured, and map to a clean separator with no diagnostic (the E758 check
/// itself is a later validation pass over this provenance, not parse-time).
fn sep_from_slot(main: &AdmittedMainTierChildren<'_>) -> TierSeparator {
    match main.child_4.slot().as_ref().map(NodeSlot::view) {
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
        // together by the caller's care. Holding `AdmittedMainTierChildren` is itself
        // the proof that both slots came from the same `main_tier`.
        //
        // The tab is read INSIDE this arm, so a well-formed utterance (no
        // separator span at all, which is the overwhelming majority of a corpus)
        // never touches it.
        Some(SlotView::Present(sep_node))
            if tab_end(main) == Some(sep_node.raw_node().start_byte()) =>
        {
            let node = sep_node.raw_node();
            TierSeparator::with_trailing_space(Span::new(
                node.start_byte() as u32,
                node.end_byte() as u32,
            ))
        }
        Some(SlotView::Present(_)) => TierSeparator::CLEAN,
        Some(SlotView::Missing(_) | SlotView::Error(_)) | None => TierSeparator::CLEAN,
    }
}

/// Report a required-child omission, located on the carrier node.
///
/// `carrier` is the byte range of the node whose child is missing (the
/// `main_tier` or `tier_body` node), in the PARSE's coordinate space, so the
/// diagnostic lands on the offending line. Until 2026-07-30 this helper used
/// `0..original_input.len()`, a fragment-local span: correct for the
/// standalone fragment entry points, but in whole-file parsing it rendered
/// every such diagnostic at line 1 over the header block (found on a real
/// transcript; same family as the annotated-word wrapper span).
pub(super) fn report_missing_child(
    carrier: std::ops::Range<usize>,
    original_input: &str,
    errors: &impl ErrorSink,
    code: ErrorCode,
    message: &str,
) -> ReportedMainTierError {
    ReportedMainTierError::report(
        ParseError::new(
            code,
            Severity::Error,
            SourceLocation::from_offsets(carrier.start, carrier.end),
            ErrorContext::new(original_input, carrier, ""),
            message,
        ),
        errors,
    )
}

/// Report an unexpected node kind at a positional slot in `main_tier`.
pub(super) fn report_unexpected_child(
    child: Node,
    source: &str,
    errors: &impl ErrorSink,
    expected: &str,
    position: usize,
) -> ReportedMainTierError {
    ReportedMainTierError::report(
        ParseError::new(
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
        ),
        errors,
    )
}
