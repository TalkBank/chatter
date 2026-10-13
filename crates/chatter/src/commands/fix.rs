//! `chatter fix`: apply catalog fixes to CHAT file(s) at exact byte spans.
//!
//! This is the successor to the deleted `chatter lint`: a batch fixer built
//! on the span-splicing engine (`talkbank_transform::splice`) rather than a
//! bespoke three-code rewriter. It covers the whole fix catalog, applies
//! fixes at exact byte spans validated against the source text, and, because
//! [`SelectedRepairs::admit`](talkbank_transform::splice::SelectedRepairs::admit) gates every edit
//! on the health of its enclosing utterance, repairs a clean utterance even
//! in a file whose other regions did not parse.
//!
//! # Why the default write set is narrow
//!
//! On 2026-05-06 a batch rewriter in this codebase damaged 440 files and 679
//! utterances; only 5 of those were even detectable by re-validating
//! afterwards, because the rest were structurally valid and semantically
//! wrong. That incident is why every catalog entry carries a
//! [`BatchSafety`](talkbank_transform::splice::BatchSafety) tier and why this command enforces it rather than
//! trusting the caller: bare `--apply` writes only
//! [`Mechanical`](talkbank_transform::splice::BatchSafety::Mechanical) fixes, a [`Semantic`](talkbank_transform::splice::BatchSafety::Semantic) fix is
//! written only when its code is named with `--code`, and a
//! [`Ambiguous`](talkbank_transform::splice::BatchSafety::Ambiguous) fix is never written by this command at all,
//! regardless of `--code`.
//!
//! # Parsing choice
//!
//! Every file is parsed with
//! [`TreeSitterParser::parse_chat_file_with_source`], never the `ParseProduct`
//! constructor: streaming parsing always hands back a `ChatFile`, including
//! for a file whose other regions needed recovery, which is exactly what
//! lets a broken region elsewhere in a file fail to block a fix in a clean
//! utterance.
//! The retained source-bound CST is reused by fix planning; no catalog entry
//! reparses the unchanged input. Changed output is still parsed for verification.

use std::path::PathBuf;

use talkbank_model::ErrorCode;
use talkbank_parser::TreeSitterParser;
use talkbank_transform::splice::{
    EditProvenance, FixCodeSelection as CodeSelection, FixDiagnosis, SkipReason,
};

mod postcheck;
#[cfg(test)]
use postcheck::{FixVerificationError, check_fix_result, code_counts};
use postcheck::{PostCheckedFix, verify_fix_result};

use super::debug::die;
use crate::cli::FixMode;
use talkbank_model::model::TranscriptName;

/// Apply catalog fixes to CHAT file(s) at exact byte spans.
///
/// Implements `chatter fix`. Every `.cha` file under `paths` is parsed and
/// validated, each diagnostic is resolved against the fix catalog, and the
/// resulting edits are admitted (health-gated) and spliced. `mode`
/// ([`FixMode::Apply`] or [`FixMode::Report`]) determines whether the
/// spliced result is written to disk or only reported; see the module docs
/// for exactly which fixes are eligible to be written under a bare `--apply`
/// versus a named `--code`.
///
/// `codes` (the `--code` CLI flag, each value already a real code parsed
/// by clap) becomes a [`CodeSelection`] before any file is opened: naming
/// nothing considers every diagnostic ([`CodeSelection::All`]); naming one
/// or more codes narrows to exactly those ([`CodeSelection::Only`]), the
/// same way `validate --suppress` narrows its own working set. Naming a
/// [`Semantic`](talkbank_transform::splice::BatchSafety::Semantic) code is how a caller opts into writing it;
/// leaving it unnamed leaves the semantic tier reported but unwritten.
///
/// A file with nothing to report prints nothing; a run over a large corpus
/// should not have to scroll past every already-clean file.
///
/// Returns how the run ended; the caller exits unsuccessfully when
/// [`FixOutcome::failed`] says a file could not be read or written.
pub fn run_fix(
    paths: &[PathBuf],
    mode: FixMode,
    codes: &[ErrorCode],
    alignment: talkbank_model::validation::AlignmentValidation,
) -> FixOutcome {
    let files = super::inputs::readable_transcripts(paths)
        .unwrap_or_else(|refusal| die(&refusal.to_string()));

    let requested_codes = requested_codes(codes);

    let parser = TreeSitterParser::new()
        .unwrap_or_else(|e| die(&format!("parser initialization failed: {e:?}")));

    let mut total_skipped = 0usize;
    let mut unreadable = 0usize;
    let mut summaries: Vec<FileFixSummary> = Vec::new();

    for stored in files {
        let path = stored.path();
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("ERROR: cannot read {}: {err}", path.display());
                unreadable += 1;
                continue;
            }
        };

        let outcome = fix_one_file(
            &parser,
            &source,
            &requested_codes,
            alignment,
            mode,
            stored.name(),
        );
        if outcome.report_lines.is_empty() {
            continue;
        }

        outln!("{}", path.display());
        for line in &outcome.report_lines {
            outln!("  {line}");
        }

        total_skipped += outcome.skipped_count;

        let Some(spliced) = outcome.spliced else {
            continue;
        };

        let write_outcome = match mode {
            FixMode::Apply => match std::fs::write(path, spliced.text()) {
                Ok(()) => WriteOutcome::Written,
                Err(err) => {
                    eprintln!("ERROR: cannot write {}: {err}", path.display());
                    WriteOutcome::Failed
                }
            },
            FixMode::Report => WriteOutcome::NotAttempted,
        };

        summaries.push(FileFixSummary {
            selected_count: spliced.edit_count(),
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

    match mode {
        FixMode::Apply => outln!(
            "\n{total_selected} fix(es) applied across {files_written} file(s); {total_skipped} diagnostic(s) skipped."
        ),
        FixMode::Report => outln!(
            "\n{total_selected} fix(es) would be applied across {} file(s) (pass --apply to write); {total_skipped} diagnostic(s) skipped.",
            // One summary per file with a spliced result.
            summaries.len()
        ),
    }

    let outcome = FixOutcome {
        unreadable,
        unwritten: summaries
            .iter()
            .filter(|summary| matches!(summary.write_outcome, WriteOutcome::Failed))
            .count(),
    };
    if outcome.failed() {
        eprintln!(
            "ERROR: {} file(s) could not be read; {} file(s) could not be written.",
            outcome.unreadable, outcome.unwritten
        );
    }
    outcome
}

/// How a `chatter fix` run ended: how many files it could not read, and how
/// many changed files `--apply` could not write. Each one is a file the
/// caller asked about that this run left unexamined or unchanged.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixOutcome {
    unreadable: usize,
    unwritten: usize,
}

impl FixOutcome {
    /// Whether the command must exit unsuccessfully: a file could not be
    /// read, or a fix could not be written.
    pub fn failed(&self) -> bool {
        self.unreadable > 0 || self.unwritten > 0
    }
}

/// The [`CodeSelection`] `--code` names: every diagnostic when it named
/// nothing, otherwise exactly the named codes. Each value was already parsed
/// by clap into a real [`ErrorCode`], so a typo stopped the run as a usage
/// error before any file was opened and can never widen it to every code.
fn requested_codes(codes: &[ErrorCode]) -> CodeSelection {
    match codes {
        [] => CodeSelection::All,
        named => CodeSelection::Only(named.iter().copied().collect()),
    }
}

/// What happened to the spliced text of one file that had at least one
/// selected edit.
///
/// A three-way enum rather than a `bool`: "the write was never
/// attempted" (report mode) and "the write was attempted and
/// failed" both look like "did not write" to a boolean, but only the
/// second means edits were SELECTED and then LOST, which is exactly the
/// case the write-count bug in this module needed to distinguish and a
/// `bool` cannot.
enum WriteOutcome {
    /// The run was [`FixMode::Report`]: nothing was ever attempted.
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
    /// applied, in report mode).
    selected_count: usize,
    /// What happened to the spliced result for this file.
    write_outcome: WriteOutcome,
}

/// Everything learned about one file: what would (or did) change, and why
/// everything else did not.
struct FileFixOutcome<'source> {
    /// Human-readable lines, one per diagnostic that had something to say:
    /// applied, or skipped with a reason. Diagnostics excluded entirely by
    /// `--code` narrowing produce no line, matching how `--suppress` narrows
    /// `validate` silently.
    report_lines: Vec<String>,
    /// Count of diagnostics considered and not selected, for any reason.
    skipped_count: usize,
    /// An actual byte-verified and post-checked result, never an arbitrary string.
    spliced: Option<PostCheckedFix<'source>>,
}

/// Parse, validate, resolve every diagnostic against the fix catalog, and
/// splice the selected edits for one file. Never writes; the caller decides
/// whether to persist [`FileFixOutcome::spliced`].
fn fix_one_file<'source>(
    parser: &TreeSitterParser,
    source: &'source str,
    requested_codes: &CodeSelection,
    alignment: talkbank_model::validation::AlignmentValidation,
    mode: FixMode,
    name: TranscriptName<'source>,
) -> FileFixOutcome<'source> {
    let diagnosis = FixDiagnosis::observe(
        parser,
        source,
        talkbank_model::validation::ValidationPolicy::new(
            talkbank_model::RuleSelection::new(),
            alignment,
        ),
        name,
    );
    let selected = diagnosis.select(requested_codes);
    let mut report_lines: Vec<String> = selected
        .skipped()
        .iter()
        .map(|skip| format!("skip {}: {}", skip.code, skip.reason))
        .collect();
    let mut skipped_count = selected.skipped().len();
    if selected.is_empty() {
        return FileFixOutcome {
            report_lines,
            skipped_count,
            spliced: None,
        };
    }
    let admission = selected.admit();
    for skipped in admission.skipped() {
        report_lines.push(format!(
            "skip {}: {}",
            provenance_label(&skipped.provenance),
            describe_skip_reason(&skipped.reason)
        ));
        skipped_count += 1;
    }

    if admission.admitted().is_empty() {
        return FileFixOutcome {
            report_lines,
            skipped_count,
            spliced: None,
        };
    }

    let admitted_labels: Vec<String> = admission
        .admitted()
        .iter()
        .map(|edit| provenance_label(edit.provenance()))
        .collect();
    let selected_count = admitted_labels.len();
    let action_verb = match mode {
        FixMode::Apply => "apply",
        FixMode::Report => "would apply",
    };

    // Admission and mapping obtain their source/model from the consumed owner.
    match admission.apply_verified() {
        Ok(spliced) => {
            let checked = match verify_fix_result(parser, spliced) {
                Ok(checked) => checked,
                Err(err) => {
                    for label in &admitted_labels {
                        report_lines.push(format!(
                            "skip {label}: the post-fix safety check refused the result: {err}"
                        ));
                    }
                    return FileFixOutcome {
                        report_lines,
                        skipped_count: skipped_count + selected_count,
                        spliced: None,
                    };
                }
            };

            for label in &admitted_labels {
                report_lines.push(format!("{action_verb} {label}"));
            }
            FileFixOutcome {
                report_lines,
                skipped_count,
                spliced: Some(checked),
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
                skipped_count: skipped_count + selected_count,
                spliced: None,
            }
        }
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
        SkipReason::OutsideAnyUtterance => "the full fix target is not enclosed by one utterance",
    }
}

// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the workspace
// [lints.clippy] table holds production code to deny.
#[cfg(test)]
mod tests {
    use super::*;

    use talkbank_transform::splice::{
        EditTarget, Replacement, SpliceEdit, TransformName, mapped_edit_sites,
    };

    fn parser() -> TreeSitterParser {
        TreeSitterParser::new().expect("parser initializes")
    }

    fn diagnosis_for<'source>(
        parser: &TreeSitterParser,
        source: &'source str,
    ) -> FixDiagnosis<'source> {
        FixDiagnosis::observe(
            parser,
            source,
            talkbank_model::validation::ValidationPolicy::new(
                talkbank_model::RuleSelection::new(),
                talkbank_model::validation::AlignmentValidation::IncludeTierAlignment,
            ),
            TranscriptName::Anonymous,
        )
    }

    /// Byte fidelity and unchanged counts cannot license an edit that leaves
    /// its diagnosed defect at the same site. Use the authored recovery input.
    #[test]
    fn verify_fix_result_rejects_a_byte_faithful_nonrepair_at_its_site() {
        let parser = parser();
        let source = include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E241_illegal_untranscribed_marker_3.cha"
        );
        let diagnosis = diagnosis_for(&parser, source);
        let diagnostics = diagnosis.diagnostics();
        let span = diagnostics
            .iter()
            .find(|d| d.code == ErrorCode::IllegalUntranscribed)
            .expect("authored E241")
            .location
            .span;
        let edit = SpliceEdit::new(
            EditTarget::Replace(span),
            Replacement::new(&source[span.to_range()]),
            EditProvenance::Diagnostic(ErrorCode::IllegalUntranscribed),
        );
        let verified = mapped_edit_sites(source, &[edit])
            .expect("mapping")
            .apply_verified()
            .expect("byte-faithful nonrepair");
        assert_eq!(verified.text(), source);
        assert!(matches!(
            check_fix_result(&parser, &verified, &diagnosis),
            Err(FixVerificationError::TargetedCodeStillFires {
                code: ErrorCode::IllegalUntranscribed
            })
        ));
    }

    /// Regression for the review finding that `verify_fix_result` compared
    /// SETS of codes before/after, so a fix that MULTIPLIES an already
    /// present code (rather than introducing a brand-new one) passed
    /// unnoticed. `source` carries exactly one E241 `IllegalUntranscribed`
    /// (`"xx"` is untranscribed material); `spliced` stands in for a fix
    /// result that produced a SECOND "xx" (same code, no new code at all).
    /// The admitted edit is a non-diagnostic `Transform`, which
    /// the retained mapping never turns into a diagnostic fix site, so this isolates the
    /// multiset comparison from the separate `TargetedCodeStillFires`
    /// check.
    #[test]
    fn verify_fix_result_rejects_a_multiplied_existing_code() {
        let parser = parser();
        let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                       @ID:\teng|test|CHI|||||Child|||\n*CHI:\txx .\n@End\n";
        let diagnosis = diagnosis_for(&parser, source);
        let diagnostics_before = diagnosis.diagnostics();
        let codes_before = code_counts(diagnostics_before);
        assert_eq!(
            codes_before.get(&ErrorCode::IllegalUntranscribed).copied(),
            Some(1),
            "fixture must start with exactly one E241"
        );

        let spliced = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                        @ID:\teng|test|CHI|||||Child|||\n*CHI:\txx and xx .\n@End\n";
        // The actual diagnostic owns the original word range. Produce the
        // adversarial candidate faithfully rather than pairing unrelated edits
        // with a prewritten string; it must pass byte verification first.
        let word_span = diagnostics_before
            .iter()
            .find(|d| d.code == ErrorCode::IllegalUntranscribed)
            .expect("actual E241")
            .location
            .span;
        assert_eq!(&source[word_span.to_range()], "xx");
        let admitted = vec![SpliceEdit::new(
            EditTarget::Replace(word_span),
            Replacement::new("xx and xx"),
            EditProvenance::Transform(TransformName::new("test")),
        )];

        let verified = mapped_edit_sites(source, &admitted)
            .expect("source mapping")
            .apply_verified()
            .expect("faithful candidate");
        assert_eq!(verified.text(), spliced);
        let result = check_fix_result(&parser, &verified, &diagnosis);
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
        let diagnosis = diagnosis_for(&parser, source);
        let diagnostics_before = diagnosis.diagnostics();
        let codes_before = code_counts(diagnostics_before);
        assert_eq!(
            codes_before.get(&ErrorCode::IllegalUntranscribed).copied(),
            Some(1),
            "fixture must start with exactly one E241"
        );

        let spliced = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                        @ID:\teng|test|CHI|||||Child|||\n*CHI:\t, xx .\n@End\n";
        let word_span = diagnostics_before
            .iter()
            .find(|d| d.code == ErrorCode::IllegalUntranscribed)
            .expect("actual E241")
            .location
            .span;
        assert_eq!(&source[word_span.to_range()], "xx");
        let admitted = vec![SpliceEdit::new(
            EditTarget::InsertAt(word_span.start),
            Replacement::new(", "),
            EditProvenance::Transform(TransformName::new("test")),
        )];
        let verified = mapped_edit_sites(source, &admitted)
            .expect("source mapping")
            .apply_verified()
            .expect("faithful candidate");
        assert_eq!(verified.text(), spliced);

        let result = check_fix_result(&parser, &verified, &diagnosis);
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
