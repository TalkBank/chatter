// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
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
//! This is an `#[ignore]` test, run manually:
//! ```bash
//! cargo test -p talkbank-parser-re2c --test full_corpus_parse_test --release -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use talkbank_model::errors::ErrorCollector;
use talkbank_model::{ChatParser, ParseOutcome, SemanticEq};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

fn corpus_base() -> PathBuf {
    PathBuf::from(
        std::env::var("TALKBANK_DATA")
            .unwrap_or_else(|_| format!("{}/talkbank/data", std::env::var("HOME").unwrap())),
    )
}

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
    TreeSitterFailed { error: String },
    SemanticMismatch,
    Re2cPanic { message: String },
}

/// How often to recreate parsers to release tree-sitter's internal memory pool.
const PARSER_RESET_INTERVAL: usize = 5_000;

#[test]
#[ignore]
fn full_corpus_parse_equivalence() {
    let base = corpus_base();
    if !base.exists() {
        eprintln!("Skipping: {} not found", base.display());
        return;
    }

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
                    if ts_file.semantic_eq(&re2c_file) {
                        None
                    } else {
                        Some(DivergenceKind::SemanticMismatch)
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
        let mut panics = 0;
        let mut ts_failed = 0;

        for d in &divergences {
            match &d.kind {
                DivergenceKind::Re2cRejected => rejected += 1,
                DivergenceKind::SemanticMismatch => mismatches += 1,
                DivergenceKind::Re2cPanic { .. } => panics += 1,
                DivergenceKind::TreeSitterFailed { .. } => ts_failed += 1,
            }
        }

        eprintln!("\nDivergence breakdown:");
        if rejected > 0 {
            eprintln!("  Re2c rejected: {rejected}");
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
                DivergenceKind::SemanticMismatch => "MISMATCH".to_string(),
                DivergenceKind::Re2cPanic { message } => {
                    format!("PANIC: {}", &message[..message.len().min(80)])
                }
                DivergenceKind::TreeSitterFailed { error } => {
                    format!("TS_FAIL: {}", &error[..error.len().min(80)])
                }
            };
            eprintln!("  {}, {}", d.path, kind_str);
        }

        // Write JSON report
        let report_path = "/tmp/re2c_corpus_divergences.json";
        let report: Vec<serde_json::Value> = divergences
            .iter()
            .map(|d| {
                serde_json::json!({
                    "path": &d.path,
                    "kind": format!("{:?}", d.kind),
                })
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
