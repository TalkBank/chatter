//! Tier-body extraction for `main_tier` conversion.
//!
//! Driven by the generated typed visitor: one `extract_tier_body` call yields the
//! optional `linkers`, the optional `langcode`, the required `contents`, and the
//! required `utterance_end` as typed slots. This UNIFIES what used to be a
//! separate body walk (linkers / langcode / contents) and a separate end re-walk
//! (terminator / postcodes / bullet) into a single pass over [`AdmittedTierBodyChildren`].
//! Each slot is matched EXHAUSTIVELY over [`NodeSlot`] so recovery nodes are
//! handled explicitly rather than silently dropped, and the recovery diagnostics
//! ("Malformed language code", "Missing terminator in tier_body", the tier-body
//! "unexpected child" message) are reproduced byte-identically. The `contents`
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

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{
    AdmittedTierBodyChildren, AsRawNode, NoChild, NodeSlot, SlotView, SourceBound, SourceSlotView,
    TierBodyNode,
};

use super::super::super::content::{
    MainTierRegion, classify_main_tier_recovery, surface_main_tier_sink,
};
use super::super::contents::parse_main_tier_contents;
use super::ending::{UtteranceEndTail, parse_utterance_end};
use super::linkers::parse_linkers;
use super::{TierBodyData, report_missing_child};

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
    original_input: &str,
    errors: &impl ErrorSink,
) -> Result<TierBodyData, crate::CstFailure> {
    let carrier = typed.raw_node().byte_range();
    let source = typed.source();
    let associated = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    let body = associated.children();
    // Linkers (optional). The slot is `Positioned<Option<NodeSlot<LinkersNode>>>`
    // (shape UNCHANGED from OLD: `linkers` is a single-symbol optional, no
    // interstitial whitespace to widen it into a group). Exhaustive over the
    // outer `Option` and the admitted inner slot: only `Present` contributes
    // (decoded by the shared linker parser); every other state maps to an empty
    // linker list, matching the pre-migration absent-linkers behavior with no new
    // diagnostic.
    let linkers = match body.linkers.slot().as_ref().map(NodeSlot::view) {
        Some(SlotView::Present(linkers_node)) => parse_linkers(*linkers_node, source, errors)?,
        Some(SlotView::Error(_)) | None => Vec::new(),
    };

    // Optional language-switch token (the `[- code]` precode) plus its source
    // span (opening `[` at `.start`), for source-spacing validation (E758).
    let ParsedLangcode {
        code: language_code,
        span: language_code_span,
    } = parse_optional_langcode(body, source, errors);

    // The admitted producer excludes Missing for the contents nonterminal,
    // not ERROR, absence or failures reading its source-associated range.
    let content = match associated.field_content_2().slot().view() {
        SourceSlotView::Present(contents) => parse_main_tier_contents(contents.read()?, errors)?,
        // Invalid input can leave ERROR at this position. Surface the node (the whole-tree
        // backstop also covers ERROR nodes) and yield empty content rather than
        // fabricating model values.
        SourceSlotView::Error(node) => {
            errors.report(classify_main_tier_recovery(
                node.raw_node(),
                source,
                MainTierRegion::Body,
            ));
            Vec::new()
        }
        SourceSlotView::Absent(NoChild) => Vec::new(),
    };

    // Ending: the `utterance_end` block (terminator, postcodes, trailing bullet).
    // Its own Missing state is producer-impossible; nested terminator recovery
    // remains the responsibility of the ending parser and validation.
    let UtteranceEndTail {
        terminator,
        postcodes,
        bullet,
    } = match associated.field_ending().slot().view() {
        SourceSlotView::Present(ending) => parse_utterance_end(ending.read()?, errors)?,
        // A stray ERROR node landed at the `utterance_end` slot position. The
        // previous tier-body walk surfaced such an ERROR via the shared
        // word-error analyzer (and then re-found the real `utterance_end`); route
        // it to the same analyzer here. No terminator is recovered; the whole-tree
        // backstop covers the surviving ERROR / MISSING nodes. Malformed-only path.
        SourceSlotView::Error(error_node) => {
            errors.report(classify_main_tier_recovery(
                error_node.raw_node(),
                source,
                MainTierRegion::Body,
            ));
            UtteranceEndTail::default()
        }
        // No usable `utterance_end` at this position: the old end-parser reported
        // `MissingTerminator` "in tier_body" when no `utterance_end` child was
        // found inside `tier_body`. A retained observation for a
        // terminator-less-but-otherwise-well-formed line still yields a
        // `Present` `utterance_end` (its OWN inner terminator slot is merely
        // absent) rather than reaching this arm.
        SourceSlotView::Absent(NoChild) => {
            report_missing_child(
                carrier.clone(),
                original_input,
                errors,
                ErrorCode::MissingTerminator,
                "Missing terminator in tier_body",
            );
            UtteranceEndTail::default()
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
    // resolutions: [`analyze_word_error`] knows main-tier word syntax and names
    // the construct (an unclosed replacement, a bare `&`, a misplaced form
    // marker), while the backstop knows only that something did not parse and
    // so answers E316, the generic "unparsable content" catch-all.
    //
    // Routing the sink to the backstop silently degraded six error codes to
    // E316 with every gate green, because each of those codes still had a
    // PASSING test: the same construct at utterance start takes a different
    // route and was the only position any spec example covered. A node is not a
    // different kind of problem because recovery placed it elsewhere, so the
    // sink asks the classifier that matches the REGION.
    surface_main_tier_sink(body, source, errors);

    Ok(TierBodyData {
        linkers,
        language_code,
        language_code_span,
        content,
        terminator,
        postcodes,
        bullet,
    })
}

/// The outcome of decoding a tier's optional `[- code]` precode: the parsed
/// language `code` (absent when there is no precode, OR a precode is present
/// but malformed) and the precode token's source `span` (present whenever the
/// token node exists, independent of whether its code parsed, since E758 needs
/// only its position). The two fields are deliberately independent:
/// `{ code: None, span: Some(_) }` means "a precode token is present but did
/// not parse", which is why this is a named struct rather than a tuple.
struct ParsedLangcode {
    code: Option<talkbank_model::model::LanguageCode>,
    span: Option<Span>,
}

/// Decode the optional `langcode` slot into a language-code string.
///
/// Reproduces the prior `LANGCODE` arm byte-identically for a `Present` token:
/// parse the token text via the shared language-code parser; if no valid code is
/// produced, report a `Malformed language code` structural diagnostic at the
/// token span. The NEW backend groups `tier_body`'s
/// `optional(seq(langcode, whitespaces))` into a NESTED carrier
/// (`TierBodyLanguageCodeChildren`, since the pair together is what is
/// grammar-optional, not `langcode` alone: the R2/B2 nested-group precedent),
/// so this descends one level (`group.child_0`) to reach the langcode itself.
/// The outer `None` and every non-`Present` slot state, at either nesting
/// level, map to no language code and no diagnostic, matching the
/// pre-migration absent-langcode behavior.
fn parse_optional_langcode(
    body: &AdmittedTierBodyChildren,
    source: &str,
    errors: &impl ErrorSink,
) -> ParsedLangcode {
    let group = match body.language_code.slot().as_ref().map(NodeSlot::view) {
        Some(SlotView::Present(group)) => group,
        Some(SlotView::Missing(_) | SlotView::Error(_)) | None => {
            return ParsedLangcode {
                code: None,
                span: None,
            };
        }
    };
    surface_main_tier_sink(group, source, errors);

    // Only a `Present` langcode token proceeds to decode, matching the OLD
    // `.ok()` collapse (which yielded `Some` for `Present` ONLY): a zero-width
    // MISSING langcode placeholder maps to no code and no diagnostic, the same
    // as Error/Unexpected/Absent, exactly like the pre-migration behavior.
    let node = match group.child_0.slot().view() {
        SlotView::Present(langcode_node) => langcode_node.raw_node(),
        SlotView::Missing(_) | SlotView::Error(_) | SlotView::Absent(NoChild) => {
            return ParsedLangcode {
                code: None,
                span: None,
            };
        }
    };

    // The `langcode` node spans the whole `[- code]` precode (opening `[` at
    // `.start`); record it for E758 whenever the token is present, independent
    // of whether the inner code parses, since E758 needs only its position.
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

    // Valid token: return the parsed code directly (already typed, no
    // String round-trip). Anything else falls through to the "Malformed
    // language code" diagnostic and no code, byte-identical to the prior
    // flag-then-check.
    let talkbank_model::ParseOutcome::Parsed(raw) =
        crate::parser::tree_parsing::parser_helpers::extract_utf8_text(
            node,
            source,
            errors,
            "language precode",
        )
    else {
        return ParsedLangcode {
            code: None,
            span: Some(span),
        };
    };
    if let Some(lc) = crate::tokens::parse_langcode_token(raw) {
        return ParsedLangcode {
            code: Some(lc),
            span: Some(span),
        };
    }

    errors.report(ParseError::new(
        ErrorCode::StructuralOrderError,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
        "Malformed language code".to_string(),
    ));
    ParsedLangcode {
        code: None,
        span: Some(span),
    }
}
