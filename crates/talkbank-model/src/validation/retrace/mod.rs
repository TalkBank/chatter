//! Retrace validation for main-tier content streams.
//!
//! This pass enforces a structural invariant: retrace markers (`[/]`, `[//]`,
//! `[///]`, `[/-]`) must be followed by substantive content (the repeated or
//! corrected material) in leaf traversal order; a bare terminator does not
//! satisfy them.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `audit_content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]
mod collection;
mod marker_on_marker;
mod rendering;
mod types;
mod visit;
mod without_words;

use crate::model::MainTier;
use crate::{ErrorCode, ErrorSink, ParseError, Severity, Span};
use collection::collect_retrace_checks;
use marker_on_marker::report_if_marker_on_marker;
use rendering::render_with_spans;
use types::LeafKind;
use visit::visit_every_retrace;
use without_words::report_if_no_words_retraced;

/// Build the `ParseError` shape every retrace rule reports.
///
/// The three rules here were each assembling this by hand, and `rg 'help_url =
/// None'` matched exactly those three lines and nothing else in the crate, so
/// the shape is local to this directory and belongs in one place in it.
///
/// Uses [`ParseError::at_span`], which the error module documents as "the
/// preferred constructor when source text is not available (e.g., during
/// validation passes that only have the AST)" and which ~39 sites already use.
/// The hand-rolled versions passed `ErrorContext::new("", span, "")`, an empty
/// context standing in for no context; `at_span` passes no context, which is
/// what is true.
///
/// The label is suppressed on a dummy span. Under `--parser re2c` that backend
/// discards the lexer's offsets, so a label would point at byte 0.
pub(super) fn retrace_error(
    code: ErrorCode,
    span: Span,
    message: impl Into<String>,
    suggestion: &str,
    label: &str,
) -> ParseError {
    let mut error = ParseError::at_span(code, Severity::Error, span, message)
        .with_suggestion(suggestion.to_owned());
    if !span.is_dummy() {
        error = error.with_label(crate::ErrorLabel::new(span, label));
    }
    // Retrace diagnostics carry their explanation inline; there is no hosted
    // page for them, and a stale help URL is worse than none.
    error.help_url = None;
    error
}

/// Run every retrace check over ONE traversal of the tier.
///
/// The two checks used to walk separately: E370 opened with a
/// `contains_retrace_marker` gate (itself a full walk when there is no
/// retrace) and E377 then walked again, ungated. On the roughly 95% of
/// utterances with no retrace at all, both walks found nothing, twice.
///
/// Sharing became possible only once both were expressed over the same
/// `visit_every_retrace`: the single walk reports E377 as it goes AND answers
/// the question E370's gate was asking, so the expensive half of E370
/// (collection plus span-rendering) still runs only when a retrace exists.
///
/// # A rewrite deliberately declined
///
/// E378's `contains_word` re-walks a retrace's content, and the outer traversal
/// has already descended into every nested retrace, so a nested retrace's
/// content is examined more than once. A bottom-up formulation would compute
/// "contains a word" in one pass.
///
/// Measured against the corpus, it is not worth building. 98.4% of the
/// 2,650,099 retraces put a word at index 0, where `.any()` exits after one
/// classification; the deepest attested enclosure is 2; and the only nodes
/// whose subtree is walked in full are the handful with no words at all. The
/// whole re-walk costs on the order of 10-25 ms across a 107,376-file run.
///
/// The cost of the rewrite is real by comparison: the visitor stops being
/// `FnMut(&Retrace)` and starts carrying a bit only one of the three rules
/// wants, and reporting order flips from outermost-first to innermost-first,
/// which is OBSERVABLE, since E378's spec documents nested wordless retraces
/// each reporting. Recorded here so the question is not re-opened from the
/// shape of the code alone.
pub(crate) fn check_retraces(main_tier: &MainTier, errors: &impl ErrorSink) {
    let mut saw_retrace = false;
    visit_every_retrace(main_tier, &mut |retrace| {
        saw_retrace = true;
        report_if_marker_on_marker(retrace, errors);
        report_if_no_words_retraced(retrace, errors);
    });
    if saw_retrace {
        check_retraces_have_content(main_tier, errors);
    }
}

/// Validate that retrace markers are followed by real content.
///
/// Retrace markers (`[/]`, `[//]`, `[///]`, `[/-]`) must be followed by real
/// content in in-order leaf traversal. Annotations are ignored for traversal. A bare
/// terminator does NOT satisfy a retrace marker: per the CHAT manual the marker
/// is necessarily followed by the repeated or corrected material (this is also
/// CLAN CHECK error 119).
///
/// The implementation short-circuits when no retrace marker exists, then runs:
/// retrace collection, suffix-acceptability computation, and span mapping for
/// precise diagnostics.
///
/// Example violations:
/// - `<the> [/] , .` - ERROR: no real content after the retrace
/// - `<the> [/] .` - ERROR: only a terminator follows the retrace
///
/// Valid:
/// - `<I want> [/] I need cookie .` - OK: next leaf is "I"
///
/// Callers reach this through [`check_retraces`], which has already established
/// that the tier contains a retrace; there is deliberately no gate here, so the
/// walk that answers that question happens once rather than twice.
fn check_retraces_have_content(main_tier: &MainTier, errors: &impl ErrorSink) {
    let (leaf_kinds, retrace_checks) = collect_retrace_checks(main_tier);
    let suffix_has_ok = build_suffix_ok(&leaf_kinds);
    let violations: Vec<_> = retrace_checks
        .iter()
        .filter(|check| !has_ok_after(&suffix_has_ok, check.after_leaf_index))
        .collect();

    if violations.is_empty() {
        return;
    }

    let rendered = render_with_spans(main_tier);

    for check in violations {
        let retrace_span = match rendered.retrace_spans.get(check.retrace_index).copied() {
            Some(span) => span,
            None => Span::from_usize(0, 0),
        };
        let absolute_span = if !main_tier.span.is_dummy() {
            let start = main_tier.span.start.saturating_add(retrace_span.start);
            let end = main_tier.span.start.saturating_add(retrace_span.end);
            Span::new(start, end)
        } else {
            retrace_span
        };

        errors.report(retrace_error(
            ErrorCode::StructuralOrderError,
            absolute_span,
            "Retrace marker ([/], [//], [///], or [/-]) must be followed by the repeated or corrected material",
            "Add content after the retrace marker, or remove the retrace if it's not needed",
            "Retrace marker",
        ));
    }
}

/// Build a suffix-acceptability table over collected leaf kinds.
///
/// Each slot answers whether there is any `RealContent` at or after that
/// position. A retrace marker is satisfied only by real content, not by a bare
/// terminator: per the CHAT manual the marker is necessarily followed by the
/// material it retraces (this is also CLAN CHECK error 119).
fn build_suffix_ok(leaf_kinds: &[LeafKind]) -> Vec<bool> {
    let mut suffix = Vec::with_capacity(leaf_kinds.len());
    let mut has_ok = false;
    for kind in leaf_kinds.iter().rev() {
        if matches!(kind, LeafKind::RealContent) {
            has_ok = true;
        }
        suffix.push(has_ok);
    }
    suffix.reverse();
    suffix
}

/// Return whether acceptable content exists after a given leaf index.
///
/// The `after_index` value is taken from retrace checkpoints captured during
/// collection traversal.
fn has_ok_after(suffix_ok: &[bool], after_index: usize) -> bool {
    matches!(suffix_ok.get(after_index), Some(true))
}
