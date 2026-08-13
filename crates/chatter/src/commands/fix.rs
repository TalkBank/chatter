//! `chatter fix`: apply catalog fixes to CHAT file(s) at exact byte spans.
//!
//! This is the successor to the deleted `chatter lint`: a batch fixer built
//! on the span-splicing engine (`talkbank_transform::splice`) rather than a
//! bespoke three-code rewriter. It covers the whole fix catalog, applies
//! fixes at exact byte spans validated against the source text, and, because
//! [`admit_edits`](talkbank_transform::splice::admit_edits) gates every edit
//! on the health of its enclosing utterance, repairs a clean utterance even
//! in a file whose other regions did not parse.
//!
//! # Why the default write set is narrow
//!
//! On 2026-05-06 a batch rewriter in this codebase damaged 440 files and 679
//! utterances; only 5 of those were even detectable by re-validating
//! afterwards, because the rest were structurally valid and semantically
//! wrong. That incident is why every catalog entry carries a
//! [`BatchSafety`] tier and why this command enforces it rather than
//! trusting the caller: bare `--apply` writes only
//! [`BatchSafety::Mechanical`] fixes, a [`BatchSafety::Semantic`] fix is
//! written only when its code is named with `--code`, and a
//! [`BatchSafety::Ambiguous`] fix is never written by this command at all,
//! regardless of `--code`.
//!
//! # Parsing choice
//!
//! Every file is parsed with
//! [`TreeSitterParser::parse_chat_file_streaming`], never the `ParseProduct`
//! constructor: streaming parsing always hands back a `ChatFile`, including
//! for a file whose other regions needed recovery, which is exactly what
//! lets a broken region elsewhere in a file fail to block a fix in a clean
//! utterance.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use talkbank_model::{ErrorCode, ErrorCollector, ParseError, Span};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::splice::{
    BatchSafety, EditProvenance, FixKind, SkipReason, SpliceEdit, SpliceError, admit_edits,
    apply_edits_verified, catalog_fix, mapped_edit_sites,
};

use super::debug::{collect_cha_files, die};
use super::error_codes::resolve_error_codes;
use talkbank_model::model::TranscriptName;

/// Apply catalog fixes to CHAT file(s) at exact byte spans.
///
/// Implements `chatter fix`. Every `.cha` file under `paths` is parsed and
/// validated, each diagnostic is resolved against the fix catalog, and the
/// resulting edits are admitted (health-gated) and spliced. `write`
/// determines whether the spliced result is written to disk or only
/// reported; see the module docs for exactly which fixes are eligible to be
/// written under a bare `--apply` versus a named `--code`.
///
/// `codes` (the `--code` CLI flag) is resolved to a [`CodeSelection`]
/// before any file is opened: naming nothing considers every diagnostic
/// ([`CodeSelection::All`]); naming one or more real codes narrows to
/// exactly those ([`CodeSelection::Only`]), the same way `validate
/// --suppress` narrows its own working set; naming even one value that
/// resolves to no real code aborts the whole run rather than silently
/// falling back to "every code" (see [`resolve_requested_codes`]). Naming a
/// [`BatchSafety::Semantic`] code is how a caller opts into writing it;
/// leaving it unnamed leaves the semantic tier reported but unwritten.
///
/// A file with nothing to report prints nothing; a run over a large corpus
/// should not have to scroll past every already-clean file.
pub fn run_fix(
    paths: &[PathBuf],
    apply: bool,
    dry_run: bool,
    codes: &[String],
    skip_alignment: bool,
) {
    let files = collect_cha_files(paths);
    if files.is_empty() {
        die("no .cha files found in the provided paths");
    }

    let requested_codes = resolve_requested_codes(codes);
    let write = apply && !dry_run;

    let parser = TreeSitterParser::new()
        .unwrap_or_else(|e| die(&format!("parser initialization failed: {e:?}")));

    let mut total_skipped = 0usize;
    let mut files_with_changes = 0usize;
    let mut summaries: Vec<FileFixSummary> = Vec::new();

    for path in files {
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("ERROR: cannot read {}: {err}", path.display());
                continue;
            }
        };

        let outcome = fix_one_file(&parser, &source, &requested_codes, skip_alignment, write);
        if outcome.report_lines.is_empty() {
            continue;
        }

        println!("{}", path.display());
        for line in &outcome.report_lines {
            println!("  {line}");
        }

        total_skipped += outcome.skipped_count;

        let Some(spliced) = outcome.spliced else {
            continue;
        };
        files_with_changes += 1;

        let write_outcome = if write {
            match std::fs::write(&path, &spliced) {
                Ok(()) => WriteOutcome::Written,
                Err(err) => {
                    eprintln!("ERROR: cannot write {}: {err}", path.display());
                    WriteOutcome::Failed
                }
            }
        } else {
            WriteOutcome::NotAttempted
        };

        summaries.push(FileFixSummary {
            selected_count: outcome.selected_count,
            write_outcome,
        });
    }

    // Derived from the summaries rather than accumulated alongside the
    // loop above: a fix that was SELECTED but never actually WRITTEN
    // (the file's write failed) must not count toward "applied", which a
    // running total incremented before the write was attempted cannot
    // express. See `WriteOutcome::counts_toward_applied`.
    let total_selected: usize = summaries
        .iter()
        .filter(|summary| summary.write_outcome.counts_toward_applied())
        .map(|summary| summary.selected_count)
        .sum();
    let files_written = summaries
        .iter()
        .filter(|summary| matches!(summary.write_outcome, WriteOutcome::Written))
        .count();

    if write {
        println!(
            "\n{total_selected} fix(es) applied across {files_written} file(s); {total_skipped} diagnostic(s) skipped."
        );
    } else {
        println!(
            "\n{total_selected} fix(es) would be applied across {files_with_changes} file(s) (pass --apply to write); {total_skipped} diagnostic(s) skipped."
        );
    }
}

/// Which diagnostics `chatter fix` narrows its work to.
///
/// Replaces the earlier untyped behavior of an empty `HashSet<ErrorCode>`
/// standing in for "no narrowing": that sentinel meant an argument list
/// that resolved to nothing (every value a typo) was indistinguishable
/// from no `--code` at all, so a wholly unrecognized `--code` silently
/// widened the run to every code in the catalog. `resolve_requested_codes`
/// never returns `Only` with an empty set (see its doc), so an argument
/// list that named codes cannot collapse to `All`.
enum CodeSelection {
    /// No `--code` was given: every diagnostic is a candidate for
    /// selection, subject to `BatchSafety` as always.
    All,
    /// `--code` named these codes, and only these codes, are considered.
    Only(HashSet<ErrorCode>),
}

impl CodeSelection {
    /// Whether a diagnostic with `code` passes `--code` narrowing at all:
    /// with [`Self::All`] every code passes (nothing was named to narrow
    /// away); with [`Self::Only`] only a named code does.
    fn admits(&self, code: ErrorCode) -> bool {
        match self {
            CodeSelection::All => true,
            CodeSelection::Only(codes) => codes.contains(&code),
        }
    }

    /// Whether `code` was EXPLICITLY named with `--code`.
    ///
    /// Distinct from [`Self::admits`]: `All` admits every code (nothing
    /// narrowed it away) but names none of them. That distinction is
    /// exactly what keeps a `BatchSafety::Semantic` fix from writing
    /// under a bare `--apply`, which only opts in a diagnostic whose code
    /// was actually typed on the command line.
    fn names(&self, code: ErrorCode) -> bool {
        matches!(self, CodeSelection::Only(codes) if codes.contains(&code))
    }
}

/// Resolve `codes` (raw `--code` strings) into a [`CodeSelection`], or
/// abort the whole run naming every value that resolved to nothing.
///
/// Fails closed: on 2026-05-06 a batch rewriter in this codebase damaged
/// 440 files because a mechanism much like this one treated "nothing
/// recognized" the same as "nothing narrowed". A `--code` value that
/// names no real error code is a typo, and a typo must never widen a
/// batch run to every code in the catalog; it must stop the run before
/// any file is even opened.
fn resolve_requested_codes(codes: &[String]) -> CodeSelection {
    if codes.is_empty() {
        return CodeSelection::All;
    }
    let resolved = resolve_error_codes(codes);
    if !resolved.unrecognized.is_empty() {
        die(&format!(
            "--code named an unrecognized error code: {}",
            resolved.unrecognized.join(", ")
        ));
    }
    CodeSelection::Only(resolved.codes)
}

/// What happened to the spliced text of one file that had at least one
/// selected edit.
///
/// A three-way enum rather than a `bool`: "the write was never
/// attempted" (dry-run/report mode) and "the write was attempted and
/// failed" both look like "did not write" to a boolean, but only the
/// second means edits were SELECTED and then LOST, which is exactly the
/// case the write-count bug in this module needed to distinguish and a
/// `bool` cannot.
enum WriteOutcome {
    /// `--apply` was not requested, or `--dry-run` suppressed the write:
    /// nothing was ever attempted.
    NotAttempted,
    /// The write reached disk.
    Written,
    /// The write was attempted and failed; nothing was persisted.
    Failed,
}

impl WriteOutcome {
    /// Whether this file's `selected_count` belongs in the run's final
    /// "applied" total: true for everything except a failed write, since
    /// a failed write means nothing about this file was actually
    /// applied, no matter how many edits were selected for it.
    fn counts_toward_applied(&self) -> bool {
        match self {
            WriteOutcome::NotAttempted | WriteOutcome::Written => true,
            WriteOutcome::Failed => false,
        }
    }
}

/// Per-file bookkeeping [`run_fix`] needs to compute its final totals:
/// what was selected, and whether it actually reached disk.
struct FileFixSummary {
    /// Diagnostics selected to apply in this file (or that would have
    /// applied, in dry-run/report mode).
    selected_count: usize,
    /// What happened to the spliced result for this file.
    write_outcome: WriteOutcome,
}

/// Everything learned about one file: what would (or did) change, and why
/// everything else did not.
struct FileFixOutcome {
    /// Human-readable lines, one per diagnostic that had something to say:
    /// applied, or skipped with a reason. Diagnostics excluded entirely by
    /// `--code` narrowing produce no line, matching how `--suppress` narrows
    /// `validate` silently.
    report_lines: Vec<String>,
    /// Count of diagnostics selected to apply (written or, in report/dry-run
    /// mode, that would have been written).
    selected_count: usize,
    /// Count of diagnostics considered and not selected, for any reason.
    skipped_count: usize,
    /// The spliced text, present exactly when at least one edit was
    /// selected and the splice engine accepted the whole edit set.
    spliced: Option<String>,
}

/// Parse, validate, resolve every diagnostic against the fix catalog, and
/// splice the selected edits for one file. Never writes; the caller decides
/// whether to persist [`FileFixOutcome::spliced`].
fn fix_one_file(
    parser: &TreeSitterParser,
    source: &str,
    requested_codes: &CodeSelection,
    skip_alignment: bool,
    write: bool,
) -> FileFixOutcome {
    let sink = ErrorCollector::new();
    let mut chat_file = parser.parse_chat_file_streaming(source, &sink);

    if skip_alignment {
        chat_file.validate(&sink, TranscriptName::Anonymous);
    } else {
        chat_file.validate_with_alignment(&sink, TranscriptName::Anonymous);
    }
    let diagnostics = sink.into_vec();

    let mut report_lines = Vec::new();
    let mut skipped_count = 0usize;
    let mut candidate_edits = Vec::new();

    for diagnostic in &diagnostics {
        if !requested_codes.admits(diagnostic.code) {
            continue;
        }
        classify_diagnostic(
            diagnostic,
            source,
            requested_codes,
            &mut candidate_edits,
            &mut report_lines,
            &mut skipped_count,
        );
    }

    if candidate_edits.is_empty() {
        return FileFixOutcome {
            report_lines,
            selected_count: 0,
            skipped_count,
            spliced: None,
        };
    }

    let admission = admit_edits(&chat_file, candidate_edits);
    for skipped in &admission.skipped {
        report_lines.push(format!(
            "skip {}: {}",
            provenance_label(&skipped.provenance),
            describe_skip_reason(&skipped.reason)
        ));
        skipped_count += 1;
    }

    if admission.admitted.is_empty() {
        return FileFixOutcome {
            report_lines,
            selected_count: 0,
            skipped_count,
            spliced: None,
        };
    }

    let admitted_labels: Vec<String> = admission
        .admitted
        .iter()
        .map(|edit| provenance_label(edit.provenance()))
        .collect();
    let selected_count = admitted_labels.len();
    let action_verb = if write { "apply" } else { "would apply" };

    let codes_before = code_counts(&diagnostics);

    // `admission.admitted` is only ever borrowed from here: once by
    // `apply_edits_verified` to splice and byte-gate the result, and again
    // by `verify_fix_result`'s post-parse re-check. Neither call needs to
    // own the edits, so there is nothing to clone.
    match apply_edits_verified(source, &admission.admitted) {
        Ok(spliced) => {
            if let Err(err) = verify_fix_result(
                parser,
                source,
                &spliced,
                &admission.admitted,
                &codes_before,
                skip_alignment,
            ) {
                for label in &admitted_labels {
                    report_lines.push(format!(
                        "skip {label}: the post-fix safety check refused the result: {err}"
                    ));
                }
                return FileFixOutcome {
                    report_lines,
                    selected_count: 0,
                    skipped_count: skipped_count + selected_count,
                    spliced: None,
                };
            }

            for label in &admitted_labels {
                report_lines.push(format!("{action_verb} {label}"));
            }
            FileFixOutcome {
                report_lines,
                selected_count,
                skipped_count,
                spliced: Some(spliced),
            }
        }
        Err(err) => {
            for label in &admitted_labels {
                report_lines.push(format!(
                    "skip {label}: the splice engine refused the edit set: {err}"
                ));
            }
            FileFixOutcome {
                report_lines,
                selected_count: 0,
                skipped_count: skipped_count + selected_count,
                spliced: None,
            }
        }
    }
}

/// Why the post-splice re-parse safety check refused to let a spliced
/// result reach disk, once the byte-identity write gate
/// (`apply_edits_verified`, called before this function ever runs) has
/// already passed.
///
/// This command's OWN additional tier, layered on top of that gate: having
/// passed byte identity, the spliced text is re-parsed and re-validated,
/// and the result must show every targeted diagnostic gone from its fix
/// site and no diagnostic code firing MORE often than it did before
/// (counting a code absent before as firing zero times). Neither tier
/// proves the fix was semantically correct; see the
/// `talkbank_transform::splice::gate` module docs on why `BatchSafety`
/// carries that weight instead.
#[derive(Debug, thiserror::Error)]
enum FixVerificationError {
    /// Re-mapping `admitted` onto the spliced text failed. Unreachable in
    /// practice here (the exact same edits against the exact same source
    /// already succeeded moments earlier, inside `apply_edits_verified`),
    /// but the possibility is typed rather than assumed away.
    #[error("could not map the applied edits onto the spliced text: {0}")]
    EditMapping(#[from] SpliceError),
    /// A diagnostic this fix targeted still fires at (an overlap with) the
    /// new-text span its edit produced.
    #[error("{code} still fires at its fix site after re-parsing the result")]
    TargetedCodeStillFires {
        /// The code that should have cleared but did not.
        code: ErrorCode,
    },
    /// Re-parsing the spliced text produced MORE instances of a code than
    /// the original file had; `before` is zero for a code the original
    /// never had at all. One variant covers both a brand-new
    /// code and a multiplied existing one: a "new" code is not a
    /// different KIND of regression, only the `before == 0` case of the
    /// same one, and splitting them into two variants was itself the
    /// review finding that produced this type. A fix that shifts tier
    /// alignment can multiply an existing code exactly like this;
    /// comparing only the SET of codes before and after (as this check
    /// once did) cannot see it, because the set already contained the
    /// code.
    #[error(
        "re-parsing the result found {code} {after} time(s), up from {before} time(s) in the original file"
    )]
    CodeCountIncreased {
        /// The code whose count increased. Zero when the code is
        /// entirely new to the file.
        code: ErrorCode,
        /// How many times it fired in the original file.
        before: usize,
        /// How many times it fires after re-parsing the spliced result.
        after: usize,
    },
}

/// The post-splice safety net, run AFTER `apply_edits_verified` has already
/// byte-gated `spliced`: re-parse and re-validate `spliced` and require
/// that every targeted diagnostic cleared at its fix site, that no
/// diagnostic code appears which `codes_before` did not already contain,
/// and that no code that DID already appear now appears MORE often.
///
/// `codes_before` is deliberately the FULL multiset of codes the original
/// parse produced, not just the codes this run selected to fix: an
/// untouched pre-existing diagnostic elsewhere in the file is expected to
/// survive unchanged, and only a diagnostic code whose count went up
/// (including from zero) counts as something re-parsing introduced.
fn verify_fix_result(
    parser: &TreeSitterParser,
    source: &str,
    spliced: &str,
    admitted: &[SpliceEdit],
    codes_before: &HashMap<ErrorCode, usize>,
    skip_alignment: bool,
) -> Result<(), FixVerificationError> {
    let sink = ErrorCollector::new();
    let mut reparsed = parser.parse_chat_file_streaming(spliced, &sink);
    if skip_alignment {
        reparsed.validate(&sink, TranscriptName::Anonymous);
    } else {
        reparsed.validate_with_alignment(&sink, TranscriptName::Anonymous);
    }
    let diagnostics_after = sink.into_vec();

    let codes_after = code_counts(&diagnostics_after);
    for (code, after_count) in &codes_after {
        let before_count = codes_before.get(code).copied().unwrap_or(0);
        if *after_count > before_count {
            return Err(FixVerificationError::CodeCountIncreased {
                code: *code,
                before: before_count,
                after: *after_count,
            });
        }
    }

    for (code, fix_site) in mapped_fix_sites(source, admitted)? {
        let still_fires = diagnostics_after
            .iter()
            .any(|d| d.code == code && d.location.span.overlaps(fix_site));
        if still_fires {
            return Err(FixVerificationError::TargetedCodeStillFires { code });
        }
    }

    Ok(())
}

/// Count how many times each error code appears in `diagnostics`.
///
/// A `HashMap` rather than a `HashSet`: the post-splice safety net
/// (`verify_fix_result`) needs to notice a fix that MULTIPLIES an
/// already-present code, not only one that introduces a brand-new one. A
/// set can answer "did this code ever appear" but not "how many times",
/// so it cannot see the difference between one occurrence and twenty.
fn code_counts(diagnostics: &[ParseError]) -> HashMap<ErrorCode, usize> {
    let mut counts = HashMap::new();
    for diagnostic in diagnostics {
        *counts.entry(diagnostic.code).or_insert(0) += 1;
    }
    counts
}

/// For every diagnostic-provenance edit in `admitted`, the byte span its
/// replacement text occupies in the SPLICED text, paired with the code it
/// targeted.
///
/// Delegates the validate/sort/overlap/cumulative-delta mapping entirely to
/// [`mapped_edit_sites`]: the same computation `apply_edits` builds its
/// output from and the write gate (`verify_splice`) walks to check it, so
/// this fold exists in exactly one place rather than a second private copy
/// here. A non-diagnostic edit (`EditProvenance::Transform`) still occupies
/// room in the mapping (it shifts every later edit's spliced offset) but
/// produces no entry: there is no code to check it against. A deletion
/// (empty replacement) maps to a zero-width span, which
/// [`talkbank_model::Span::overlaps`] defines to overlap nothing, so a
/// diagnostic can never "still fire" at content that was removed outright:
/// the right outcome, since there is no byte left there to re-flag.
fn mapped_fix_sites(
    source: &str,
    admitted: &[SpliceEdit],
) -> Result<Vec<(ErrorCode, Span)>, SpliceError> {
    let mapped = mapped_edit_sites(source, admitted)?;
    Ok(mapped
        .iter()
        .filter_map(|edit| match edit.provenance() {
            EditProvenance::Diagnostic(code) => Some((*code, edit.spliced())),
            EditProvenance::Transform(_) => None,
        })
        .collect())
}

/// Resolve one diagnostic against the fix catalog and the batch-safety
/// gate, either queuing its edits in `candidate_edits` or recording a
/// report line explaining why it was not queued.
///
/// Exhaustive over [`FixKind`] and [`BatchSafety`]: there is no catch-all
/// arm that could silently drop a new combination as it is added to the
/// catalog.
fn classify_diagnostic(
    diagnostic: &ParseError,
    source: &str,
    requested_codes: &CodeSelection,
    candidate_edits: &mut Vec<SpliceEdit>,
    report_lines: &mut Vec<String>,
    skipped_count: &mut usize,
) {
    // The four skip sites below differ only in their reason text; this
    // closure is the one place that turns a reason into the "skip <code>:
    // <reason>" report line and the count increment, so a fifth site
    // added later cannot drift from the other four's format.
    let skip = |report_lines: &mut Vec<String>, skipped_count: &mut usize, reason: &str| {
        report_lines.push(format!("skip {}: {reason}", diagnostic.code));
        *skipped_count += 1;
    };

    let Some(fix) = catalog_fix(diagnostic, source) else {
        skip(report_lines, skipped_count, "no catalog entry");
        return;
    };

    match fix.kind {
        FixKind::Alternatives(alternatives) => {
            let labels: Vec<&str> = alternatives.iter().map(|a| a.label.as_str()).collect();
            skip(
                report_lines,
                skipped_count,
                &format!("ambiguous, several valid fixes ({})", labels.join("; ")),
            );
        }
        FixKind::Deterministic(edits) => match fix.safety {
            BatchSafety::Ambiguous => {
                skip(
                    report_lines,
                    skipped_count,
                    "ambiguous, never batch-applied",
                );
            }
            BatchSafety::Mechanical => candidate_edits.extend(edits),
            BatchSafety::Semantic => {
                if requested_codes.names(diagnostic.code) {
                    candidate_edits.extend(edits);
                } else {
                    skip(
                        report_lines,
                        skipped_count,
                        "semantic fix, name it with --code to apply",
                    );
                }
            }
        },
    }
}

/// Human-readable label for an edit's provenance, for report lines.
///
/// The catalog only ever produces [`EditProvenance::Diagnostic`], but the
/// splice engine's [`SkipReason`]/error types are typed over the full
/// [`EditProvenance`] enum (it also serves non-diagnostic transforms), so
/// this stays total rather than assuming the variant this caller happens
/// to produce today.
fn provenance_label(provenance: &EditProvenance) -> String {
    match provenance {
        EditProvenance::Diagnostic(code) => code.to_string(),
        EditProvenance::Transform(name) => format!("transform {}", name.as_str()),
    }
}

/// Human-readable reason one admitted-or-not edit was refused by the
/// health gate.
fn describe_skip_reason(reason: &SkipReason) -> &'static str {
    match reason {
        SkipReason::TaintedUtterance => "the enclosing utterance needed parser recovery",
        SkipReason::UnknownHealth => "the enclosing utterance carries no parse provenance",
        SkipReason::OutsideAnyUtterance => "the fix site lies outside every utterance",
    }
}

// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the workspace
// [lints.clippy] table holds production code to deny.
#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_transform::splice::{EditTarget, Replacement, TransformName};

    fn parser() -> TreeSitterParser {
        TreeSitterParser::new().expect("parser initializes")
    }

    /// Parse and validate `source`, returning its diagnostics; the same
    /// pipeline `fix_one_file` runs, extracted so a unit test can build a
    /// `codes_before` multiset from a REAL parse rather than a fabricated
    /// one.
    fn diagnostics_for(parser: &TreeSitterParser, source: &str) -> Vec<ParseError> {
        let sink = ErrorCollector::new();
        let mut chat_file = parser.parse_chat_file_streaming(source, &sink);
        chat_file.validate_with_alignment(&sink, TranscriptName::Anonymous);
        sink.into_vec()
    }

    /// Regression for the review finding that `verify_fix_result` compared
    /// SETS of codes before/after, so a fix that MULTIPLIES an already
    /// present code (rather than introducing a brand-new one) passed
    /// unnoticed. `source` carries exactly one E241 `IllegalUntranscribed`
    /// (`"xx"` is untranscribed material); `spliced` stands in for a fix
    /// result that produced a SECOND "xx" (same code, no new code at all).
    /// The admitted edit is a non-diagnostic `Transform`, which
    /// `mapped_fix_sites` never turns into a fix site, so this isolates the
    /// multiset comparison from the separate `TargetedCodeStillFires`
    /// check.
    #[test]
    fn verify_fix_result_rejects_a_multiplied_existing_code() {
        let parser = parser();
        let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                       @ID:\teng|test|CHI|||||Child|||\n*CHI:\txx .\n@End\n";
        let codes_before = code_counts(&diagnostics_for(&parser, source));
        assert_eq!(
            codes_before.get(&ErrorCode::IllegalUntranscribed).copied(),
            Some(1),
            "fixture must start with exactly one E241"
        );

        let spliced = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                        @ID:\teng|test|CHI|||||Child|||\n*CHI:\txx and xx .\n@End\n";
        // An identity replace of the `*CHI:` speaker code itself: a valid,
        // in-bounds span against `source` whose exact location does not
        // matter, since a `Transform`-provenance edit never becomes a fix
        // site (`mapped_fix_sites` only keeps `Diagnostic` provenance), so
        // this test isolates the multiset comparison from the separate
        // `TargetedCodeStillFires` check.
        let admitted = vec![SpliceEdit::new(
            EditTarget::Replace(Span::new(0, 5)),
            Replacement::new("*CHI:"),
            EditProvenance::Transform(TransformName::new("test")),
        )];

        let result = verify_fix_result(&parser, source, spliced, &admitted, &codes_before, false);
        match result {
            Err(FixVerificationError::CodeCountIncreased {
                code,
                before,
                after,
            }) => {
                assert_eq!(code, ErrorCode::IllegalUntranscribed);
                assert_eq!(before, 1);
                assert_eq!(after, 2);
            }
            other => panic!("expected CodeCountIncreased, got {other:?}"),
        }
    }

    /// The companion case: a code with zero prior occurrences still reports
    /// as `CodeCountIncreased` with `before: 0`, the same variant the
    /// multiplied-code case above uses. `source` is the full-header
    /// fixture with only E241 firing (the same shape `fix_tests.rs`'s
    /// `MECHANICAL_AND_SEMANTIC_FIXTURE` uses, minus its leading comma);
    /// `spliced` reintroduces that comma, which `fix_tests.rs` independently
    /// verified fires E259 `CommaAfterNonSpokenContent`, a code the
    /// original never had at all.
    #[test]
    fn verify_fix_result_reports_a_genuinely_new_code_as_before_zero() {
        let parser = parser();
        let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                       @ID:\teng|test|CHI|||||Child|||\n*CHI:\txx .\n@End\n";
        let codes_before = code_counts(&diagnostics_for(&parser, source));
        assert_eq!(
            codes_before.get(&ErrorCode::IllegalUntranscribed).copied(),
            Some(1),
            "fixture must start with exactly one E241"
        );

        let spliced = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                        @ID:\teng|test|CHI|||||Child|||\n*CHI:\t, xx .\n@End\n";
        let admitted: Vec<SpliceEdit> = Vec::new();

        let result = verify_fix_result(&parser, source, spliced, &admitted, &codes_before, false);
        match result {
            Err(FixVerificationError::CodeCountIncreased {
                code,
                before,
                after,
            }) => {
                assert_eq!(code, ErrorCode::CommaAfterNonSpokenContent);
                assert_eq!(before, 0);
                assert_eq!(after, 1);
            }
            other => panic!("expected CodeCountIncreased with before: 0, got {other:?}"),
        }
    }
}
