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
    DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, Span,
};
use crate::generated_traversal::{
    AsRawNode, KindAdmitted, MainTierChildren, MainTierNode, NodeSlot, Raw, RequiredPresence,
    SlotView, SourceBound, SourceSlice, SourceSlotView,
};
use crate::model::{
    Bullet, Linker, MainTier, Postcode, Terminator, TierSeparator, UtteranceContent,
};

use super::super::content::{MainTierRegion, classify_main_tier_recovery};
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;

mod body;
mod ending;
mod linkers;
mod prefix;

pub(super) type RequiredMainTierChildren<'tree> =
    MainTierChildren<'tree, KindAdmitted, Raw, RequiredPresence>;

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

/// Every recovery node under one `main_tier`, reportable AT MOST ONCE each.
///
/// Direct ERROR children can also occur in the unexpected sink. Collect them
/// once and consume the set for reporting; required-position admission does
/// not exclude recovery nodes outside those positions.
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

    /// Consume the deduplicated recovery set for reporting.
    fn into_nodes(self) -> Vec<tree_sitter::Node<'tree>> {
        self.unreported
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
/// The source-bound input owns text and diagnostic coordinates together. Fragment
/// presentation is handled at the fragment boundary, not by a second source.
pub fn convert_main_tier_node<'tree>(
    typed: SourceBound<'tree, '_, MainTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<MainTier, ReportedMainTierError> {
    let node = typed.raw_node();
    let source = typed.source();
    // Speaker prefix slots (`star`, `speaker`, `colon`, `tab`), the optional
    // `sep_trailing_space` (E758 provenance), and the `tier_body` slot, read
    // from the generated typed visitor. Every field is `Positioned<..>`: read
    // `.slot`.
    let extraction = crate::parser::typed_cst::main_tier_rule()
        .and_then(|rule| typed.extract_required(rule).map_err(Into::into));
    let associated = crate::parser::typed_cst::report_admission(extraction, node, source, errors)
        .map_err(ReportedMainTierError::from)?;
    let main = associated.children();

    // Speaker prefix (`* speaker : tab`).
    let prefix = prefix::parse_prefix(&associated, errors);

    // The optional trailing separator space after the tab, before tier_body
    // (E758 provenance): `main.child_4.slot` is `Option<NodeSlot<..>>`. Only
    // `Present` carries a real span; every other outer/inner state (grammar
    // omits the node entirely, or it recovers as Missing/Error/Absent) means
    // no illegal trailing space was captured, mirroring how
    // `body.linkers.slot` is read for the other optional single-symbol slot.
    let separator = sep_from_slot(main);

    // Robustness for a recovery ERROR produced by malformed content right after
    // the tab (a bare `&`, a retrace/bracket code at tier start,
    // an italic control byte, ...). Before the `sep_trailing_space` slot existed
    // the grammar was `seq(star, speaker, colon, tab, tier_body)`, so that ERROR
    // landed in the `tier_body` slot and was classified by `analyze_word_error`.
    // With `optional(sep_trailing_space)` now between tab and tier_body, the ERROR
    // can land in EITHER position depending on the shape: a bare `&` (no trailing
    // space) fills the `sep_trailing_space` slot itself as `NodeSlot::Error`,
    // while a retrace/bracket followed by a space (`[/] world`) leaves the real
    // `sep_trailing_space` in its slot and the ERROR as a SEPARATE sibling node.
    // Checking only one slot missed the second shape and downgraded the specific
    // region-specific classification (the "never silently drop a recovery node"
    // rule). So classify EVERY direct ERROR child of the main
    // tier here, deduplicated against the unexpected sink. The whole-tree backstop still
    // retains unhandled recovery; specific diagnostics require evidence from
    // their owning construct, not a guess from unparsed marker text.
    let recovery = MainTierRecovery::collect(node, &main.unexpected);
    for child in recovery.into_nodes() {
        errors.report(classify_main_tier_recovery(
            child,
            source,
            MainTierRegion::OutsideBody,
        ));
    }

    // The per-rule proof supplies the composite body at its required position.
    // Reading its bytes is a separate, fallible source-ownership boundary.
    let SourceSlotView::Present(body) = associated.field_child_5().slot().view();
    let tier = match body.read() {
        Ok(tier_body) => body::parse_tier_body(tier_body, errors).map_err(|fault| {
            ReportedMainTierError::report(
                crate::parser::typed_cst::cst_failure_diagnostic(node, source, fault),
                errors,
            )
        }),
        Err(error) => Err(ReportedMainTierError::report(
            ParseError::new(
                ErrorCode::InternalError,
                Severity::Error,
                DiagnosticLocation::new(span_of(node)),
                ErrorContext::new(source, span_of(node), "tier_body"),
                error.to_string(),
            ),
            errors,
        )),
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
    // The sink splits by OWNER, not by bookkeeping. Its ERROR nodes are
    // `MainTierRecovery`'s and were classified above with their region; what is
    // left is content that filled no grammar position, which is
    // `surface_displaced`'s. Partitioning on `is_error`, a property of the
    // node, is not the identity arithmetic this refactor removed: nothing here
    // has to know what another walk did.
    let unexpected_content: Vec<tree_sitter::Node<'_>> = main
        .unexpected
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

    let span = span_of(node);

    // Content span: from after the colon to the end of the main_tier line.
    // A lexical MISSING colon still owns its zero-width boundary.
    let colon_end = match associated.field_child_2().slot().view() {
        SourceSlotView::Present(colon) | SourceSlotView::Missing(colon) => {
            colon.raw_node().end_byte()
        }
    };
    let content_span = Span::new(colon_end, node.end_byte());

    let mut main_tier = MainTier::new(
        talkbank_model::MainTierLocations {
            line: span.into(),
            speaker: speaker_span.into(),
            content: content_span.into(),
        },
        speaker,
        tier.content,
        tier.terminator,
    )
    .with_linkers(tier.linkers)
    .with_postcodes(tier.postcodes)
    .with_separator(separator);

    if let Some(precode) = tier.language_code {
        main_tier = main_tier.with_language_code(precode);
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
    /// The `[- code]` precode with its location, when one parsed.
    pub language_code: Option<talkbank_model::model::LanguagePrecode>,
    pub content: Vec<UtteranceContent>,
    pub terminator: Option<Terminator>,
    pub postcodes: Vec<Postcode>,
    pub bullet: Option<Bullet>,
}

/// Byte offset just past the main tier's `tab`, when the tab actually parsed.
///
/// `None` means the tab is missing or recovered, in which case nothing can be
/// proven adjacent to it and no adjacency-dependent claim is asserted.
fn tab_end(main: &RequiredMainTierChildren<'_>) -> Option<usize> {
    match main.child_3.slot().view() {
        SlotView::Present(tab_node) => Some(tab_node.raw_node().end_byte()),
        SlotView::Missing(_) => None,
    }
}

/// Decode the optional `sep_trailing_space` slot into a [`TierSeparator`]
/// (E758 provenance). Mirrors the `body.linkers.slot` read pattern for the
/// other optional single-symbol slot (see `body.rs`): only `Present` carries
/// a real span; the outer `None` and every inner non-`Present` state
/// (Missing/Error/Absent) mean no illegal trailing space was
/// captured, and map to a clean separator with no diagnostic (the E758 check
/// itself is a later validation pass over this provenance, not parse-time).
fn sep_from_slot(main: &RequiredMainTierChildren<'_>) -> TierSeparator {
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
        // together by the caller's care. Holding `RequiredMainTierChildren` is itself
        // the proof that both slots came from the same `main_tier`.
        //
        // The tab is read INSIDE this arm, so a well-formed utterance (no
        // separator span at all, which is the overwhelming majority of a corpus)
        // never touches it.
        Some(SlotView::Present(sep_node))
            if tab_end(main) == Some(sep_node.raw_node().start_byte()) =>
        {
            let node = sep_node.raw_node();
            TierSeparator::with_trailing_space(span_of(node))
        }
        Some(SlotView::Present(_)) => TierSeparator::CLEAN,
        Some(SlotView::Missing(_) | SlotView::Error(_)) | None => TierSeparator::CLEAN,
    }
}

/// Report a required-child omission, located on the carrier node.
///
/// The admitted carrier owns both the node range and its complete parse source.
/// A caller cannot pair a document-absolute range with a local tier substring.
/// This is a reporting boundary, not proof that a required child can be absent.
pub(super) fn report_missing_child(
    carrier: SourceSlice<'_, '_>,
    errors: &impl ErrorSink,
    code: ErrorCode,
    message: &str,
) -> ReportedMainTierError {
    let range = carrier.raw_node().byte_range();
    ReportedMainTierError::report(
        ParseError::new(
            code,
            Severity::Error,
            DiagnosticLocation::from_offsets(range.start, range.end),
            ErrorContext::new(
                carrier.source(),
                talkbank_model::Span::new(range.start, range.end),
                "",
            ),
            message,
        ),
        errors,
    )
}

#[cfg(test)]
mod diagnostic_source_tests {
    use super::*;
    use crate::generated_traversal::TierBodyNode;
    use crate::{TreeSitterParser, error::ErrorCollector};

    /// Exercise the reporter boundary with real owned carriers, not a claim
    /// that valid reference syntax reaches missing-child recovery.
    #[test]
    fn diagnostic_source_owner_retains_document_coordinates() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/content/terminators-standard.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser.parse_source_incremental(source, None).expect("tree");
        let root = parsed.bind(parsed.root_node()).expect("owned root");
        let mut checked = 0;
        for carrier in root.descendants() {
            let carrier = carrier.expect("owned descendant");
            if carrier.typed::<MainTierNode>().is_none()
                && carrier.typed::<TierBodyNode>().is_none()
            {
                continue;
            }
            let errors = ErrorCollector::new();
            report_missing_child(
                carrier,
                &errors,
                ErrorCode::MissingTerminator,
                "boundary control",
            );
            let diagnostics = errors.into_vec();
            assert_eq!(diagnostics.len(), 1);
            let error = &diagnostics[0];
            let range = carrier.raw_node().byte_range();
            assert!(range.start > 0);
            let context = error.context.as_ref().expect("context");
            assert_eq!(context.source_text, source);
            assert_eq!(context.span, Span::new(range.start, range.end));
            assert_eq!(
                error.location.span().expect("located diagnostic"),
                context.span
            );
            assert_eq!(context.source_text.get(range), Some(carrier.text()));
            checked += 1;
        }
        assert_eq!(checked, 8, "four main-tier and four body carriers");
    }

    #[test]
    fn diagnostic_source_owner_preserves_fragment_extent() {
        let valid = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/content/terminators-standard.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let mut checked = 0;
        for input in valid.lines().filter(|line| line.starts_with('*')) {
            let tier = parser.parse_main_tier(input).expect("reference fragment");
            assert_eq!(tier.location.span().expect("located").end(), input.len());
            assert!(tier.content.location.span().expect("located").end() <= input.len());
            checked += 1;
        }
        assert_eq!(checked, 4);
        let invalid = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../talkbank-parser-tests/tests/error_corpus/validation_errors/E312_2.cha"
        ));
        let input = invalid
            .lines()
            .find(|line| line.starts_with('*'))
            .expect("main tier");
        let errors = parser
            .parse_main_tier(input)
            .expect_err("authored invalid fragment");
        assert!(!errors.is_empty());
        for error in &errors.errors {
            assert!(error.location.span().expect("located diagnostic").end() <= input.len());
            if let Some(context) = &error.context {
                assert_eq!(context.source_text, input);
                assert!(context.span.end() <= input.len());
            }
        }
    }
}
