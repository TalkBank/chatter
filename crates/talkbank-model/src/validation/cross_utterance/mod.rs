//! Cross-utterance validation for quotation patterns and completion linkers
//!
//! This module validates relationships between utterances that require looking
//! at sequences of utterances, particularly:
//! - Quotation patterns (Pattern A: +"/. and Pattern B: +". )
//! - Completion linkers (+, and ++)
//!
//! ## These checks are OPT-IN, not disabled
//!
//! The quotation and completion checks in this module run only when
//! `RuleSelection::with_strict_linkers` is set, which the CLI spells
//! `--strict-linkers`. They are implemented, they fire, and they are
//! demonstrated by spec examples that declare `rules = 'strict_linkers'`.
//!
//! This section said "**DISABLED** (2025-12-28)" until 2026-09-08, and that
//! sentence had propagated: eight codes were marked `not_implemented` in the
//! registry, eight tests in this module's own suite were `#[ignore]`d with
//! "E34x validation disabled" as the reason, and one of those tests had been
//! narrowed to assert half of what its name promised. None of it was true.
//! The rules were made opt-in, which is a different fact, and the word
//! "disabled" was never corrected.
//!
//! ### Why they are off by default
//!
//! The rationale below is real and is why the default is what it is. Legacy
//! CHAT tooling never checked these cross-utterance patterns; they were
//! written fresh here to enforce strict conventions for quoted passages and
//! completion sequences, and real corpora show the strict sequential patterns
//! do not match natural conversational flow:
//!
//! - Quotation follows (`+"/. `) requires the next same-speaker utterance to
//!   carry a `+"` linker, and fails when a speaker does not continue with
//!   quoted content or another speaker interjects.
//! - Quotation precedes (`+".`) requires preceding same-speaker utterances
//!   with `+"` linkers, and fails when quoted content appears without them.
//! - The quoted-utterance linker (`+"`) requires a surrounding same-speaker
//!   utterance ending `+"/. ` or `+".`, and fails when a speaker interrupts or
//!   continues a quoted passage non-canonically.
//! - The self-completion linker (`+,`) requires a preceding same-speaker
//!   utterance ending `+/.`, and fails when speakers resume without an
//!   interruption marker.
//!
//! The open question is what these should BE: warnings rather than errors,
//! context-sensitive by corpus type, relaxed in their sequential matching, or
//! dropped. Until it is answered they are available on request, which is
//! strictly more than nothing and honest about the default.
//!
//! ### Which codes the option turns on
//!
//! Deliberately not listed here. Four hand-written lists of them existed and
//! three were wrong at once: this section named `E349/E350`, which are not
//! codes at all, and named E352 for a function that emits E351 and E352; the
//! `RuleSelection` field doc and the CLI's own `--strict-linkers` help both
//! said "E351-E355", omitting E341, E344 and E346. What cannot drift is
//! DERIVED from each code's registry entry: `chatter validate --list-checks`
//! prints such a code as `[Opt-in]` rather than `[Active]`, the generated
//! error index marks its row, and its own generated page names the flag.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OtherCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SelfCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

mod completion;
mod file_utterances;
use file_utterances::FileUtterances;
mod helpers;
mod quotation_follows;
mod quotation_precedes;
mod quoted_linker;
mod scoped_markers;

use crate::model::{OverlapPointKind, Terminator, UtteranceContent};
use crate::{ErrorCollector, ErrorSink, ParseError};
use helpers::has_quoted_linker;

/// Validates cross-utterance constraints and returns collected diagnostics.
///
/// This is the allocation-friendly convenience entrypoint used by callers that
/// do not need to reuse a custom error sink. It always runs scoped-marker and
/// overlap checks, while quotation-specific checks depend on runtime context flags.
pub fn check_cross_utterance_patterns(
    file: &crate::model::ChatFile,
    context: &crate::validation::ValidationContext,
) -> Vec<ParseError> {
    let errors = ErrorCollector::new();
    check_cross_utterance_patterns_with_sink(file, context, &errors);
    errors.into_vec()
}

/// Validates cross-utterance constraints using a caller-provided error sink.
///
/// This function centralizes the full rule dispatch order, including feature-
/// gated quotation checks and always-on scoped-marker balancing. The ordering
/// is intentionally deterministic so diagnostics remain stable across runs.
pub(crate) fn check_cross_utterance_patterns_with_sink(
    file: &crate::model::ChatFile,
    context: &crate::validation::ValidationContext,
    errors: &impl ErrorSink,
) {
    let utterances = &FileUtterances::of(file);
    for (idx, utterance) in utterances.iter().enumerate() {
        // Quotation follows pattern (Pattern A - E341)
        // Opt-in: runs under `RuleSelection::with_strict_linkers`.
        // See module-level documentation for rationale
        if context.shared.enable_quotation_validation
            && let Some(ref term) = utterance.main.content.terminator
            && matches!(term, Terminator::QuotedNewLine { .. })
        {
            errors.report_all(quotation_follows::check_quotation_follows(utterances, idx));
        }

        // Quotation precedes pattern (Pattern B - E344)
        // Opt-in: runs under `RuleSelection::with_strict_linkers`.
        // See module-level documentation for rationale
        if context.shared.enable_quotation_validation
            && let Some(ref term) = utterance.main.content.terminator
            && matches!(term, Terminator::QuotedPeriodSimple { .. })
        {
            errors.report_all(quotation_precedes::check_quotation_precedes(
                utterances, idx,
            ));
        }

        // Quoted utterance linker (E346)
        // Opt-in: runs under `RuleSelection::with_strict_linkers`.
        // See module-level documentation for rationale
        if context.shared.enable_quotation_validation && has_quoted_linker(utterance) {
            errors.report_all(quoted_linker::check_quoted_linker(utterances, idx));
        }

        // Other-completion linker (++), gated behind runtime flag.
        // See the module docs for why this one is off by default.
        if context.shared.enable_quotation_validation
            && helpers::has_other_completion_linker(utterance)
        {
            errors.report_all(completion::check_other_completion(utterances, idx));
        }
    }

    // Self-completion linker (E352) - O(n) batch validation
    // Opt-in: runs under `RuleSelection::with_strict_linkers`.
    // See module-level documentation for rationale
    if context.shared.enable_quotation_validation {
        completion::check_self_completion_all(utterances, errors);
    }

    // Validate scoped markers that can span across utterances
    scoped_markers::check_long_feature_balance(utterances, errors);
    scoped_markers::check_nonvocal_balance(utterances, errors);

    // E704: Same speaker must not encode top+bottom overlap pair with self.
    check_self_overlap_markers(utterances, errors);

    // E347: Cross-utterance overlap balance, top regions should have
    // matching bottom regions on a different speaker.
    check_cross_utterance_overlap_balance(file, errors);
}

/// Rejects overlap pairs that imply a speaker overlapping with themself.
///
/// Adjacent utterances by the same speaker must not encode a top-overlap pair
/// followed by a bottom-overlap pair, which semantically implies self-overlap.
/// This catches annotation slips that are easy to miss when overlap brackets
/// are edited manually across turn boundaries.
fn check_self_overlap_markers(utterances: &FileUtterances<'_>, errors: &impl ErrorSink) {
    use crate::{ErrorCode, ErrorContext, Severity, SourceLocation};

    for (first, second) in utterances.consecutive_pairs() {
        if first.main.speaker != second.main.speaker {
            continue;
        }

        let first_has_top = has_overlap_kind(
            &first.main.content.content,
            OverlapPointKind::TopOverlapBegin,
            OverlapPointKind::TopOverlapEnd,
        );
        let second_has_bottom = has_overlap_kind(
            &second.main.content.content,
            OverlapPointKind::BottomOverlapBegin,
            OverlapPointKind::BottomOverlapEnd,
        );

        if first_has_top && second_has_bottom {
            let span = second.main.span;
            let speaker = second.main.speaker.as_str();
            errors.report(
                crate::ParseError::new(
                    ErrorCode::SpeakerSelfOverlap,
                    Severity::Error,
                    SourceLocation::new(span),
                    ErrorContext::new(speaker, span, speaker),
                    format!(
                        "Speaker '{}' has overlapping top/bottom overlap markers across adjacent utterances",
                        speaker
                    ),
                )
                .with_suggestion(
                    "Overlap markers should represent overlap between different speakers",
                ),
            );
        }
    }
}

/// Returns whether the utterance has a well-paired overlap region of the given kind.
///
/// Uses `extract_overlap_info` to check all content levels (including intra-word
/// markers). Requires both begin and end markers to be present.
fn has_overlap_kind(
    content: &[UtteranceContent],
    begin_kind: OverlapPointKind,
    _end_kind: OverlapPointKind,
) -> bool {
    use crate::alignment::helpers::overlap::{OverlapRegionKind, extract_overlap_info};

    let target_kind = match begin_kind {
        OverlapPointKind::TopOverlapBegin => OverlapRegionKind::Top,
        OverlapPointKind::BottomOverlapBegin => OverlapRegionKind::Bottom,
        _ => return false,
    };

    let info = extract_overlap_info(content);
    info.regions
        .iter()
        .any(|r| r.kind == target_kind && r.is_well_paired())
}

/// Validate cross-utterance overlap balance (E347).
///
/// Uses `analyze_file_overlaps` for proper 1:N matching, one top region
/// from speaker A can be matched by bottom regions from speakers B, C, etc.
/// Only orphaned tops (no matching bottom from any speaker) and orphaned
/// bottoms (no matching top from any speaker) are reported.
fn check_cross_utterance_overlap_balance(file: &crate::model::ChatFile, errors: &impl ErrorSink) {
    let utterances = FileUtterances::of(file);
    use crate::{ErrorCode, ErrorContext, Severity, SourceLocation};

    // A near-copy of `alignment::helpers::overlap_groups::analyze_file_overlaps`.
    //
    // It said three times that it could not call the shared function because it
    // "only had `&[Utterance]`", which stopped being true when this module
    // started taking the file: `analyze_file_overlaps` wants `&[Line]` and the
    // file has them. Deduplicating is a separate change with its own risk,
    // because the two have already DIVERGED (this copy carries the
    // vacant-sibling distribution guard below and the shared one does not), so
    // merging them is an adjudication rather than a move.
    // ONE owner. This was a ~90-line near-copy of `analyze_file_overlaps`,
    // justified by a comment saying it "only had `&[Utterance]`"; the module
    // takes the file now, and the shared function wants exactly its lines. The
    // copy had also DIVERGED, carrying a distribution guard the shared one
    // lacked, so the validator and the alignment analysis disagreed about the
    // same transcript. The guard moved to the shared function, where the 23
    // existing tests still pass, and the copy is deleted.
    let analysis = crate::alignment::helpers::overlap_groups::analyze_file_overlaps(&file.lines);

    // Report orphaned tops, only for indexed markers.
    // Unindexed multi-party overlaps are inherently ambiguous; see
    // docs/overlap-validation-audit.md (2026-03-19).
    for orphan in &analysis.orphaned_tops {
        if orphan.region.index.is_none() {
            continue;
        }
        let Some(utt) = utterances.get(orphan.utterance_index) else {
            continue;
        };
        // Control-flow invariant: `is_none()` short-circuits the
        // continue above; reaching this line guarantees `Some(...)`.
        #[allow(clippy::unwrap_used)]
        let index_label = format!(" (index {})", orphan.region.index.unwrap().get());
        errors.report(
            ParseError::new(
                ErrorCode::UnbalancedOverlap,
                Severity::Error,
                SourceLocation::new(utt.main.span),
                ErrorContext::new(
                    orphan.speaker.as_str(),
                    utt.main.span,
                    orphan.speaker.as_str(),
                ),
                format!(
                    "Top overlap ⌈{index_label} on speaker '{}' has no matching \
                     bottom overlap ⌊ from a different speaker",
                    orphan.speaker
                ),
            )
            .with_suggestion(
                "Check that the overlapping speaker's utterance has a matching ⌊ marker \
                 with the same index",
            ),
        );
    }

    // Report orphaned bottoms, only for indexed markers.
    for orphan in &analysis.orphaned_bottoms {
        if orphan.region.index.is_none() {
            continue;
        }
        let Some(utt) = utterances.get(orphan.utterance_index) else {
            continue;
        };
        // Same control-flow invariant as the orphaned_tops loop above.
        #[allow(clippy::unwrap_used)]
        let index_label = format!(" (index {})", orphan.region.index.unwrap().get());
        errors.report(
            ParseError::new(
                ErrorCode::UnbalancedOverlap,
                Severity::Error,
                SourceLocation::new(utt.main.span),
                ErrorContext::new(
                    orphan.speaker.as_str(),
                    utt.main.span,
                    orphan.speaker.as_str(),
                ),
                format!(
                    "Bottom overlap ⌊{index_label} on speaker '{}' has no matching \
                     top overlap ⌈ from a different speaker",
                    orphan.speaker
                ),
            )
            .with_suggestion(
                "Check that the other speaker's utterance has a matching ⌈ marker \
                 with the same index",
            ),
        );
    }
}
