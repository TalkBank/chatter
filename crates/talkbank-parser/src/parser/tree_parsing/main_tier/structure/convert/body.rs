//! Tier-body extraction for `main_tier` conversion.
//!
//! Driven by the generated typed visitor: one `extract_tier_body` call yields the
//! optional `linkers`, the optional `langcode`, the required `contents`, and the
//! required `utterance_end` as typed slots. This UNIFIES what used to be a
//! separate body walk (linkers / langcode / contents) and a separate end re-walk
//! (terminator / postcodes / bullet) into a single pass over [`AdmittedTierBodyChildren`].
//! Each slot is matched EXHAUSTIVELY over [`ReadableSlot`] so recovery nodes are
//! handled explicitly rather than silently dropped, and the recovery diagnostics
//! ("Malformed language code", "Missing terminator in tier_body", the tier-body
//! "unexpected child" message) retain their codes and wording while their
//! contexts use the owning parse source. The `contents`
//! internals are still parsed by the existing `parse_main_tier_contents` (migrated
//! in task 3c). The `utterance_end` internals are now decoded off the generated
//! visitor by [`super::ending::parse_utterance_end`] (task 3d): terminator subtype,
//! postcodes, and trailing bullet come from `extract_utterance_end`'s typed slots.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{
    AdmittedTierBodyChildren, AsRawNode, NoChild, RangeAdmittedChildren, Ranged, ReadableSlot,
    SlotView, SourceBound, TierBodyNode,
};
use crate::parser::typed_cst::{ExtractCanonical, ReportedCstFailure, report_admission};

use super::super::super::content::{
    MainTierRegion, classify_main_tier_recovery, surface_main_tier_sink,
};
use super::super::contents::parse_main_tier_contents;
use super::ending::{UtteranceEndTail, parse_utterance_end};
use super::linkers::parse_linkers;
use super::{TierBodyData, report_missing_child};
use crate::parser::node_span::span_of;

/// Parse the typed `tier_body` slots into the unified [`TierBodyData`].
///
/// The source-bound node owns the extraction, carrier coordinates, and text.
/// Its children contain `linkers` (optional),
/// `language_code` (the optional NESTED `[langcode, whitespaces]` group),
/// `content_2` (the required `contents` block; field accessor `content()`),
/// and `ending` (the required `utterance_end` block). The valid path emits no
/// diagnostics; the remaining slot states reproduce the prior recovery behavior.
pub(super) fn parse_tier_body<'tree>(
    typed: SourceBound<'tree, '_, TierBodyNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<TierBodyData, ReportedCstFailure> {
    let source = typed.source();
    let associated = report_admission(
        typed
            .extract_canonical()
            .and_then(|children| children.admit_ranges().map_err(Into::into)),
        typed.raw_node(),
        source,
        errors,
    )?;
    let body = associated.children();
    // Linkers (optional). The slot is `Positioned<Option<NodeSlot<LinkersNode>>>`
    // (shape UNCHANGED from OLD: `linkers` is a single-symbol optional, no
    // interstitial whitespace to widen it into a group). Exhaustive over the
    // outer `Option` and the admitted inner slot: only `Present` contributes
    // (decoded by the shared linker parser); every other state maps to an empty
    // linker list, matching the pre-migration absent-linkers behavior with no new
    // diagnostic.
    let linkers = match body.linkers.slot.as_ref().map(ReadableSlot::view) {
        Some(SlotView::Present(linkers_node)) => report_admission(
            parse_linkers(*linkers_node, errors),
            linkers_node.raw_node(),
            source,
            errors,
        ),
        Some(SlotView::Error(_)) | None => Ok(Vec::new()),
    };

    // Optional language-switch token (the `[- code]` precode), carrying its
    // own source location (opening `[` at `.start`) for E758.
    let language_code = parse_optional_langcode(&associated, errors);

    // Compiled admission excludes MISSING contents. ERROR, absence and
    // source-read refusal stay separate; only present contents are decoded.
    let content = match body.content_2.slot.view() {
        SlotView::Present(contents) => report_admission(
            parse_main_tier_contents(*contents, errors),
            contents.raw_node(),
            source,
            errors,
        ),
        // Invalid input can leave ERROR at this position. Surface the node (the whole-tree
        // backstop also covers ERROR nodes) and yield empty content rather than
        // fabricating model values.
        SlotView::Error(node) => {
            errors.report(classify_main_tier_recovery(
                node.raw_node(),
                source,
                MainTierRegion::Body,
            ));
            Ok(Vec::new())
        }
        SlotView::Absent(NoChild) => Ok(Vec::new()),
    };

    // Ending: the `utterance_end` block (terminator, postcodes, trailing bullet).
    // Compiled admission excludes its own MISSING state; nested terminator
    // recovery remains the responsibility of the ending parser and validation.
    let ending = match body.ending.slot.view() {
        SlotView::Present(ending) => report_admission(
            parse_utterance_end(*ending, errors),
            ending.raw_node(),
            source,
            errors,
        ),
        // A stray ERROR node landed at the `utterance_end` slot position. The
        // previous tier-body walk surfaced such an ERROR via the shared
        // word-error analyzer (and then re-found the real `utterance_end`); route
        // it to the same analyzer here. No terminator is recovered; the whole-tree
        // backstop covers the surviving ERROR / MISSING nodes. Malformed-only path.
        SlotView::Error(error_node) => {
            errors.report(classify_main_tier_recovery(
                error_node.raw_node(),
                source,
                MainTierRegion::Body,
            ));
            Ok(UtteranceEndTail::default())
        }
        // No usable `utterance_end` at this position: the old end-parser reported
        // `MissingTerminator` "in tier_body" when no `utterance_end` child was
        // found inside `tier_body`. A retained observation for a
        // terminator-less-but-otherwise-well-formed line still yields a
        // `Present` `utterance_end` (its OWN inner terminator slot is merely
        // absent) rather than reaching this arm.
        SlotView::Absent(NoChild) => {
            report_missing_child(
                typed.source_slice(),
                errors,
                ErrorCode::MissingTerminator,
                "Missing terminator in tier_body",
            );
            Ok(UtteranceEndTail::default())
        }
    };

    // Surface the carrier's own `unexpected` sink (R2).
    //
    // This is no longer the empty set it was assumed to be. A malformed
    // fragment BETWEEN `contents` and `ending` (`*CHI:\thello [: world .`)
    // parses as an ERROR that is a direct child of `tier_body`, filling no
    // grammar position, so it arrives here rather than inside `contents`.
    //
    // It must be classified by the MAIN-TIER word classifier, not by the
    // generic backstop. The two answer the same question at different
    // resolutions: the region classifier can retain a narrower finding when
    // native delimiter evidence proves it; otherwise E316 honestly reports
    // unparsed content. It does not infer a replacement or marker identity
    // merely from recovery text. Structured construct decoders own their
    // specific diagnostics independently.
    //
    // Routing the sink to the backstop silently degraded six error codes to
    // E316 with every gate green, because each of those codes still had a
    // PASSING test: the same construct at utterance start takes a different
    // route and was the only position any spec example covered. A node is not a
    // different kind of problem because recovery placed it elsewhere, so the
    // sink asks the classifier that matches the REGION.
    surface_main_tier_sink(body, source, errors);

    // Every selected sibling and displaced recovery is visited before a
    // reported producer fault prevents construction of the tier body.
    let language_code = match language_code {
        ParsedLangcode::Parsed(precode) => Some(precode),
        ParsedLangcode::Absent | ParsedLangcode::Malformed => None,
    };
    let linkers = linkers?;
    let content = content?;
    let UtteranceEndTail {
        terminator,
        postcodes,
        bullet,
    } = ending?;

    Ok(TierBodyData {
        linkers,
        language_code,
        content,
        terminator,
        postcodes,
        bullet,
    })
}

/// The outcome of decoding a tier's optional `[- code]` precode. A parsed
/// precode carries its code and its location together; a malformed one has
/// already been reported at its token and yields no code. There is no state
/// in which a code exists without the token that carried it.
enum ParsedLangcode {
    Absent,
    Malformed,
    Parsed(talkbank_model::model::LanguagePrecode),
}

/// Decode the generated optional precode group without detaching its source.
/// Only actual optional absence is Absent. Recovered or malformed syntax
/// yields no precode; unreadable source is a producer fault, not either case.
fn parse_optional_langcode<'tree, 'source>(
    body: &RangeAdmittedChildren<'tree, 'source, AdmittedTierBodyChildren<'tree, Ranged<'source>>>,
    errors: &impl ErrorSink,
) -> ParsedLangcode {
    let group = match body.children().language_code.slot.as_ref() {
        None => return ParsedLangcode::Absent,
        Some(slot) => match slot.view() {
            SlotView::Present(group) => group,
            SlotView::Error(_) => return ParsedLangcode::Malformed,
            SlotView::Absent(never) => match never {},
        },
    };
    surface_main_tier_sink(group, body.parent().source(), errors);

    // Only a `Present` langcode token proceeds to decode, matching the OLD
    // `.ok()` collapse (which yielded `Some` for `Present` ONLY). Compiled
    // admission excludes a MISSING langcode; Error/Absent still yield no code.
    let token = match group.child_0.slot.view() {
        SlotView::Present(langcode) => langcode,
        SlotView::Error(_) => {
            return ParsedLangcode::Malformed;
        }
        SlotView::Absent(never) => match never {},
    };

    // The `langcode` node spans the whole `[- code]` precode (opening `[` at
    // `.start`); a parsed precode carries it for E758.
    let node = token.raw_node();
    let span = span_of(node);

    // Valid token: return the parsed code directly (already typed, no
    // String round-trip). Anything else falls through to the "Malformed
    // language code" diagnostic and no code, byte-identical to the prior
    // flag-then-check.
    if let Some(lc) = crate::tokens::parse_langcode_token(token.text()) {
        return ParsedLangcode::Parsed(talkbank_model::model::LanguagePrecode::new(span, lc));
    }

    errors.report(ParseError::new(
        ErrorCode::StructuralOrderError,
        Severity::Error,
        DiagnosticLocation::new(span_of(node)),
        ErrorContext::new(token.source(), span, ""),
        "Malformed language code".to_string(),
    ));
    ParsedLangcode::Malformed
}
