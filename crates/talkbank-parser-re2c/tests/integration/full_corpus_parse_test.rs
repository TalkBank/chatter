// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Full-corpus parse comparison: Re2cParser vs TreeSitterParser.
//!
//! Parses every .cha file in the wild-corpus tree at `$TALKBANK_DATA`
//! with both parsers and compares:
//! 1. Whether both parsers succeed (produce a ChatFile)
//! 2. Whether the ChatFile outputs are semantically equivalent
//!
//! Memory-efficient: streams files via iterator (no upfront collection),
//! recreates parsers periodically to release tree-sitter's internal memory
//! pool. Runs comfortably on a 64 GB machine (~200 MB peak).
//!
//! # Why this is `#[ignore]`d, and what that costs
//!
//! It needs a corpus that no CI runner has, and it reads roughly 100,000
//! files with two parsers, so it cannot sit in the inner loop. It is
//! therefore a NAMED target rather than folklore in a doc comment:
//!
//! ```bash
//! just corpus-parse-equivalence
//! ```
//!
//! What that costs is real and worth stating: this is the only test in the
//! workspace that runs both parsers over real data, so between deliberate
//! runs nothing here is watching. The gate that watches continuously is the
//! corpus differential's cross-backend axis, which compares VERDICTS; this
//! compares parsed MODELS, which is strictly more, and is why it still
//! exists.
//!
//! # It could pass by doing nothing, and on most machines it did
//!
//! Its default corpus path was `$HOME/talkbank/data`, the retired split
//! layout. On the maintainer's machine that path still resolves, through a
//! legacy symlink to the same directory `~/0tb/data` points at, which is
//! exactly why the staleness went unnoticed: it worked in the one place
//! anybody ran it. Anywhere without that symlink, including every other fleet
//! host and CI, it found no directory, RETURNED EARLY, and reported success.
//!
//! An absent corpus and a clean corpus produced the same verdict, which is the
//! rule this workspace states as "a gate that can skip itself is not a gate".
//! The default is now `~/0tb/data`, the only supported layout, and a missing
//! corpus fails.

use crate::corpus_root::CorpusRoot;
use talkbank_model::errors::ErrorCollector;
use talkbank_model::{ChatParser, ParseOutcome, SemanticEq};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

/// Record of a divergence between the two parsers.
///
/// Stores only the relative path (compact) and a category tag.
#[derive(Debug)]
struct Divergence {
    path: String,
    kind: DivergenceKind,
}

// `TreeSitterFailed` is kept so the taxonomy enumerates every reason a corpus
// entry can show up as a divergence; tree-sitter currently never reports a
// hard failure, so the variant is unused but its slot is load-bearing for
// future categorizer changes.
#[allow(dead_code)]
#[derive(Debug)]
enum DivergenceKind {
    Re2cRejected,
    TreeSitterFailed {
        error: String,
    },
    /// The models differ, and where.
    ///
    /// `semantic_eq` answers yes or no, which was all this test recorded, so
    /// 348 divergences over 20 corpora were 348 identical strings and the only
    /// way to learn anything was to open files by hand. A location turns the
    /// population into something you can group and count.
    SemanticMismatch(FirstDifference),
    /// `semantic_eq` and `SemanticDiff` reached opposite conclusions.
    InstrumentsDisagree,
    Re2cPanic {
        message: String,
    },
}

/// The result of comparing one file's two parses.
///
/// # Why this is a type and not two function calls
///
/// Locating a difference only makes sense once you know there IS one, and that
/// ordering used to be a sentence in a doc comment: "this is only ever called
/// after `semantic_eq` has said they differ". A sentence is not enforcement.
/// Here the location is reachable only THROUGH [`Self::Diverged`], so asking
/// where two agreeing parses differ is not a question the type lets you ask,
/// and [`FirstDifference`] has no other constructor.
enum ParseComparison {
    /// `semantic_eq` accepted the pair.
    Equivalent,
    /// They differ, and here is the first place.
    Diverged(FirstDifference),
    /// `semantic_eq` says they differ and `SemanticDiff` finds nothing.
    ///
    /// A named state rather than a placeholder string, because the two are
    /// separate derives with nothing forcing them to agree, so this is a defect
    /// in the pair rather than a property of the file, and it should be
    /// countable as such.
    InstrumentsDisagree,
}

/// Where two parses first differ.
///
/// Structured rather than a formatted string. The persisted JSON report is read
/// by tooling that would otherwise have to split an English sentence back
/// apart, and an earlier cut of this did exactly that: a hand-joined string,
/// wrapped again in a `Debug` rendering, in a file whose consumer then string
/// split on `"at: "`. Two layers of encoding over data that was structured to
/// begin with.
#[derive(Debug)]
struct FirstDifference {
    path: String,
    kind: String,
    left: String,
    right: String,
}

impl FirstDifference {
    /// One line for a human reading the terminal listing.
    fn summary(&self) -> String {
        format!(
            "{} [{}] {} versus {}",
            self.path, self.kind, self.left, self.right
        )
    }
}

impl ParseComparison {
    /// Compare two parses of the same source.
    ///
    /// `semantic_eq` remains the verdict. `SemanticDiff` is asked only for the
    /// location, and only after the verdict is "differ", which is deliberate:
    /// the two are independent derives, so using the diff's emptiness as the
    /// verdict would silently retire the cross-check that
    /// [`Self::InstrumentsDisagree`] exists to report.
    fn of(left: &talkbank_model::ChatFile, right: &talkbank_model::ChatFile) -> Self {
        use talkbank_model::{SemanticDiff, SemanticDiffContext, SemanticDiffReport, SemanticPath};

        if left.semantic_eq(right) {
            return Self::Equivalent;
        }

        // Capped at one. The derive checks `is_truncated()` before descending
        // into each field, so a limit of 1 stops the walk at the first
        // difference instead of hunting the default 20 across the rest of a
        // document this caller reads exactly one entry from.
        let mut report = SemanticDiffReport::new(1);
        let mut path = SemanticPath::new();
        let mut ctx = SemanticDiffContext::new();
        left.semantic_diff_into(right, &mut path, &mut report, &mut ctx);

        match report.differences().first() {
            Some(difference) => Self::Diverged(FirstDifference {
                path: difference.path.clone(),
                kind: format!("{:?}", difference.kind),
                left: truncate(&difference.left),
                right: truncate(&difference.right),
            }),
            None => Self::InstrumentsDisagree,
        }
    }
}

/// A value short enough to group by.
///
/// Local rather than shared: `talkbank-model` has a `truncate_value` for its
/// own diff rendering, and it is private. Widening the CHAT core's public
/// surface to save five lines in a test is the wrong trade the month before a
/// 1.0 freezes that surface.
fn truncate(value: &str) -> String {
    if value.chars().count() <= 40 {
        return value.to_string();
    }
    let head: String = value.chars().take(40).collect();
    format!("{head}...")
}

/// How often to recreate parsers to release tree-sitter's internal memory pool.
const PARSER_RESET_INTERVAL: usize = 5_000;

#[test]
#[ignore]
fn full_corpus_parse_equivalence() {
    let base = CorpusRoot::resolve().require();

    eprintln!("Scanning .cha files from {}...", base.display());

    // Stream files via iterator, no upfront Vec<PathBuf> allocation.
    let file_iter = walkdir::WalkDir::new(&base)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"));

    let mut ts = TreeSitterParser::new().expect("tree-sitter grammar loads");
    let mut re2c = Re2cParser::new();

    let mut total = 0usize;
    let mut passed = 0usize;
    let mut divergences: Vec<Divergence> = Vec::new();
    let mut read_errors = 0usize;

    let base_str = base.to_string_lossy().to_string();

    for entry in file_iter {
        let path = entry.into_path();

        // Periodically recreate parsers to release tree-sitter's growing
        // internal memory pool. Without this, memory climbs to 4+ GB on
        // large corpora. With it, peak stays under ~200 MB.
        if total > 0 && total.is_multiple_of(PARSER_RESET_INTERVAL) {
            ts = TreeSitterParser::new().expect("tree-sitter grammar loads");
            re2c = Re2cParser::new();

            eprintln!(
                "  Progress: {} files ({} divergences), parsers reset",
                total,
                divergences.len()
            );
        } else if total > 0 && total.is_multiple_of(10_000) {
            eprintln!(
                "  Progress: {} files ({} divergences)",
                total,
                divergences.len()
            );
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => {
                read_errors += 1;
                continue;
            }
        };

        total += 1;

        // Relative path for compact storage in divergence records.
        let rel_path = path
            .to_string_lossy()
            .strip_prefix(&base_str)
            .unwrap_or(&path.to_string_lossy())
            .trim_start_matches('/')
            .to_string();

        // Parse with both parsers in a tight scope so ASTs are dropped
        // before the next iteration.
        let divergence = {
            let ts_errors = ErrorCollector::new();
            let ts_file = ts.parse_chat_file_streaming(&content, &ts_errors);

            let re2c_errors = ErrorCollector::new();
            let re2c_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                re2c.parse_chat_file(&content, 0, &re2c_errors)
            }));

            match re2c_result {
                Ok(ParseOutcome::Parsed(re2c_file)) => {
                    match ParseComparison::of(&ts_file, &re2c_file) {
                        ParseComparison::Equivalent => None,
                        ParseComparison::Diverged(first) => {
                            Some(DivergenceKind::SemanticMismatch(first))
                        }
                        ParseComparison::InstrumentsDisagree => {
                            Some(DivergenceKind::InstrumentsDisagree)
                        }
                    }
                }
                Ok(ParseOutcome::Rejected) => Some(DivergenceKind::Re2cRejected),
                Err(panic_info) => {
                    let message = panic_info
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| panic_info.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_else(|| "unknown panic".to_string());
                    Some(DivergenceKind::Re2cPanic { message })
                }
            }
            // ts_file, re2c_file, content all dropped here
        };

        if let Some(kind) = divergence {
            divergences.push(Divergence {
                path: rel_path,
                kind,
            });
        } else {
            passed += 1;
        }
    }

    // Report
    eprintln!("\n=== FULL CORPUS PARSE COMPARISON ===");
    eprintln!("Total files parsed: {total}");
    eprintln!("Read errors (skipped): {read_errors}");
    eprintln!("Passed (semantically equivalent): {passed}");
    eprintln!("Divergences: {}", divergences.len());

    if !divergences.is_empty() {
        let mut rejected = 0;
        let mut mismatches = 0;
        let mut instruments_disagree = 0;
        let mut panics = 0;
        let mut ts_failed = 0;

        for d in &divergences {
            match &d.kind {
                DivergenceKind::Re2cRejected => rejected += 1,
                DivergenceKind::SemanticMismatch(_) => mismatches += 1,
                DivergenceKind::InstrumentsDisagree => instruments_disagree += 1,
                DivergenceKind::Re2cPanic { .. } => panics += 1,
                DivergenceKind::TreeSitterFailed { .. } => ts_failed += 1,
            }
        }

        eprintln!("\nDivergence breakdown:");
        if rejected > 0 {
            eprintln!("  Re2c rejected: {rejected}");
        }
        if instruments_disagree > 0 {
            eprintln!("  semantic_eq and semantic_diff disagree: {instruments_disagree}");
        }
        if mismatches > 0 {
            eprintln!("  Semantic mismatches: {mismatches}");
        }
        if panics > 0 {
            eprintln!("  Re2c panics: {panics}");
        }
        if ts_failed > 0 {
            eprintln!("  TreeSitter failed: {ts_failed}");
        }

        eprintln!("\nDivergent files:");
        for d in &divergences {
            let kind_str = match &d.kind {
                DivergenceKind::Re2cRejected => "REJECTED".to_string(),
                DivergenceKind::SemanticMismatch(first) => {
                    format!("MISMATCH at {}", first.summary())
                }
                DivergenceKind::InstrumentsDisagree => {
                    "INSTRUMENTS-DISAGREE (semantic_eq says differ, semantic_diff finds nothing)"
                        .to_string()
                }
                DivergenceKind::Re2cPanic { message } => {
                    format!("PANIC: {}", &message[..message.len().min(80)])
                }
                DivergenceKind::TreeSitterFailed { error } => {
                    format!("TS_FAIL: {}", &error[..error.len().min(80)])
                }
            };
            eprintln!("  {}, {}", d.path, kind_str);
        }

        // Write JSON report. Not `/tmp`: this workspace treats it as
        // erased-on-reboot scratch, and the report is the artifact the run
        // exists to produce. `target/` travels with the build it describes.
        let report_path = std::env::var("TALKBANK_RE2C_DIVERGENCE_REPORT")
            .unwrap_or_else(|_| "target/re2c_corpus_divergences.json".to_string());
        let report_path = report_path.as_str();
        // Real keys, not a `Debug` rendering. This file is read by tooling, and
        // a `format!("{:?}", kind)` forces every consumer to split a Rust debug
        // string back into the fields it was built from. Grouping the
        // population by `mismatch_path` is the whole reason the location is
        // recorded at all.
        let report: Vec<serde_json::Value> = divergences
            .iter()
            .map(|d| {
                let mut record = serde_json::json!({
                    "file": &d.path,
                    "kind": match &d.kind {
                        DivergenceKind::Re2cRejected => "re2c_rejected",
                        DivergenceKind::TreeSitterFailed { .. } => "tree_sitter_failed",
                        DivergenceKind::SemanticMismatch(_) => "semantic_mismatch",
                        DivergenceKind::InstrumentsDisagree => "instruments_disagree",
                        DivergenceKind::Re2cPanic { .. } => "re2c_panic",
                    },
                });
                match &d.kind {
                    DivergenceKind::SemanticMismatch(first) => {
                        record["mismatch_path"] = serde_json::json!(first.path);
                        record["mismatch_kind"] = serde_json::json!(first.kind);
                        record["left"] = serde_json::json!(first.left);
                        record["right"] = serde_json::json!(first.right);
                    }
                    DivergenceKind::Re2cPanic { message } => {
                        record["message"] = serde_json::json!(message);
                    }
                    DivergenceKind::TreeSitterFailed { error } => {
                        record["error"] = serde_json::json!(error);
                    }
                    DivergenceKind::Re2cRejected | DivergenceKind::InstrumentsDisagree => {}
                }
                record
            })
            .collect();
        if let Ok(json) = serde_json::to_string_pretty(&report) {
            let _ = std::fs::write(report_path, &json);
            eprintln!("\nFull report written to {report_path}");
        }
    }

    eprintln!(
        "\nPass rate: {:.2}%",
        if total > 0 {
            passed as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    );
}
