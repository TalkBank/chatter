//! Tier-body extraction for `main_tier` conversion.
//!
//! Driven by the generated typed visitor: one `extract_tier_body` call yields the
//! optional `linkers`, the optional `langcode`, the required `contents`, and the
//! required `utterance_end` as typed slots. This UNIFIES what used to be a
//! separate body walk (linkers / langcode / contents) and a separate end re-walk
//! (terminator / postcodes / bullet) into a single pass over [`TierBodyChildren`].
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

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use crate::generated_traversal::{AsRawNode, NodeSlot, TierBodyChildren};
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use tree_sitter::Node;

use super::super::super::content::analyze_word_error;
use super::super::contents::parse_main_tier_contents;
use super::ending::{UtteranceEndTail, parse_utterance_end};
use super::linkers::parse_linkers;
use super::{TierBodyData, report_missing_child};

/// Parse the typed `tier_body` slots into the unified [`TierBodyData`].
///
/// `body` is the result of `extract_tier_body`: `linkers` (optional),
/// `language_code` (the optional NESTED `[langcode, whitespaces]` group),
/// `content_2` (the required `contents` block; field accessor `content()`),
/// and `ending` (the required `utterance_end` block). The valid path emits no
/// diagnostics; the remaining slot states reproduce the prior recovery behavior.
pub(super) fn parse_tier_body(
    body: &TierBodyChildren,
    source: &str,
    original_input: &str,
    errors: &impl ErrorSink,
) -> TierBodyData {
    // Linkers (optional). The slot is `Positioned<Option<NodeSlot<LinkersNode>>>`
    // (shape UNCHANGED from OLD: `linkers` is a single-symbol optional, no
    // interstitial whitespace to widen it into a group). Exhaustive over the
    // outer `Option` and the inner 5-state `NodeSlot`: only `Present` contributes
    // (decoded by the shared linker parser); every other state maps to an empty
    // linker list, matching the pre-migration absent-linkers behavior with no new
    // diagnostic.
    let linkers = match &body.linkers.slot {
        Some(NodeSlot::Present(linkers_node)) => {
            parse_linkers(linkers_node.raw_node(), source, errors)
        }
        Some(
            NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent,
        )
        | None => Vec::new(),
    };

    // Optional language-switch token (the `[- code]` precode) plus its source
    // span (opening `[` at `.start`), for source-spacing validation (E758).
    let ParsedLangcode {
        code: language_code,
        span: language_code_span,
    } = parse_optional_langcode(body, source, errors);

    // Content: the `contents` block. `Present` carries a typed `ContentsNode`;
    // `Missing` carries a bare `Node` directly under the NEW closed `NodeSlot`
    // (a MISSING node is childless, so the walk yields an empty vec), matching
    // the old behavior of running the contents walk over whatever node sat at
    // this position. The contents internals are migrated separately (this
    // cluster's `contents.rs`, below).
    let content = match &body.content_2.slot {
        NodeSlot::Present(contents_node) => {
            parse_main_tier_contents(contents_node.raw_node(), source, errors)
        }
        NodeSlot::Missing(node) => parse_main_tier_contents(*node, source, errors),
        // Unreachable on valid input: a required slot recovers as Present/MISSING,
        // never as an ERROR or a wrong kind. Surface the node (the whole-tree
        // backstop also covers ERROR nodes) and yield empty content rather than
        // fabricating model values.
        NodeSlot::Error(node) => {
            errors.report(analyze_word_error(*node, source));
            Vec::new()
        }
        NodeSlot::Unexpected(node) => {
            report_unexpected_tier_body_child(*node, source, errors);
            Vec::new()
        }
        NodeSlot::Absent => Vec::new(),
    };

    // Ending: the `utterance_end` block (terminator, postcodes, trailing bullet).
    // `Present`/`Missing` both descend through the visitor-driven
    // `parse_utterance_end` decode (which itself calls `extract_utterance_end`
    // and matches its five slots exhaustively), exactly as the old re-walk did
    // when it found a (possibly MISSING) `utterance_end` child inside
    // `tier_body`. A MISSING (childless) `utterance_end` yields no terminator
    // and no error; the `MissingTerminator` (E305) diagnostic comes from
    // validation, not here.
    let UtteranceEndTail {
        terminator,
        postcodes,
        bullet,
    } = match &body.ending.slot {
        NodeSlot::Present(ending_node) => {
            parse_utterance_end(ending_node.raw_node(), source, errors)
        }
        NodeSlot::Missing(node) => parse_utterance_end(*node, source, errors),
        // A stray ERROR node landed at the `utterance_end` slot position. The
        // previous tier-body walk surfaced such an ERROR via the shared
        // word-error analyzer (and then re-found the real `utterance_end`); route
        // it to the same analyzer here. No terminator is recovered; the whole-tree
        // backstop covers the surviving ERROR / MISSING nodes. Malformed-only path.
        NodeSlot::Error(error_node) => {
            errors.report(analyze_word_error(*error_node, source));
            UtteranceEndTail::default()
        }
        // No usable `utterance_end` at this position: the old end-parser reported
        // `MissingTerminator` "in tier_body" when no `utterance_end` child was
        // found inside `tier_body`. Unreachable on valid input (the slot recovers
        // as Present/MISSING): confirmed empirically (see the B3 report) for a
        // terminator-less-but-otherwise-well-formed line, which still yields a
        // `Present` `utterance_end` (its OWN inner terminator slot is merely
        // absent) rather than reaching this arm.
        NodeSlot::Unexpected(_) | NodeSlot::Absent => {
            report_missing_child(
                original_input,
                errors,
                ErrorCode::MissingTerminator,
                "Missing terminator in tier_body",
            );
            UtteranceEndTail::default()
        }
    };

    // Surface the carrier's own `unexpected` sink (R2). Empty on every fixture
    // probed so far; load-bearing once the whole-tree backstop is deleted.
    surface_unexpected(&body.unexpected, source, errors);

    TierBodyData {
        linkers,
        language_code,
        language_code_span,
        content,
        terminator,
        postcodes,
        bullet,
    }
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
    body: &TierBodyChildren,
    source: &str,
    errors: &impl ErrorSink,
) -> ParsedLangcode {
    let group = match &body.language_code.slot {
        Some(NodeSlot::Present(group)) => group,
        Some(
            NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent,
        )
        | None => return ParsedLangcode { code: None, span: None },
    };
    surface_unexpected(&group.unexpected, source, errors);

    // Only a `Present` langcode token proceeds to decode, matching the OLD
    // `.ok()` collapse (which yielded `Some` for `Present` ONLY): a zero-width
    // MISSING langcode placeholder maps to no code and no diagnostic, the same
    // as Error/Unexpected/Absent, exactly like the pre-migration behavior.
    let node = match &group.child_0.slot {
        NodeSlot::Present(langcode_node) => langcode_node.raw_node(),
        NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent => {
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
    if let Ok(raw) = node.utf8_text(source.as_bytes())
        && let Some(lc) = crate::tokens::parse_langcode_token(raw)
    {
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

/// Report the tier-body `StructuralOrderError` "unexpected child" diagnostic.
///
/// Reproduces the previous catch-all arm of the tier-body walk byte-identically.
fn report_unexpected_tier_body_child(node: Node, source: &str, errors: &impl ErrorSink) {
    errors.report(ParseError::new(
        ErrorCode::StructuralOrderError,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
        format!("Unexpected child '{}' in tier_body", node.kind()),
    ));
}
