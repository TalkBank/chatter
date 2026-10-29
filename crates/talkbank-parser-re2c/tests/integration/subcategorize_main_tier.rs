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

//! Sub-categorize main_tier divergences between Re2cParser and TreeSitterParser.
//!
//! Samples divergent files and classifies the FIRST non-span diff path
//! into fine-grained categories.
//!
//! Run:
//! ```bash
//! cargo test -p talkbank-parser-re2c --test integration subcategorize_main_tier::subcategorize_main_tier_divergences --release -- --ignored --exact --nocapture
//! ```

use crate::diagnostic_json::{JsonPair, JsonReport, collect_cha_files};
use crate::parser_attempt::{ComparisonModels, CompletedParserPair, ParserPairFailure};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use talkbank_model::SemanticEq;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

/// Extract a sub-category from a main_tier diff path.
fn sub_classify(path: &str, ts_val: &str, re2c_val: &str) -> String {
    // Content length mismatch
    if path.ends_with(".content[]") || path.ends_with(".content.content[]") {
        return "content_length_mismatch".into();
    }

    // Event type
    if path.contains("event_type") {
        return format!("event_type (ts={ts_val}, re2c={re2c_val})");
    }

    // Raw text differences
    if path.contains("raw_text") {
        // Truncate for classification
        let ts_short: String = ts_val.chars().take(30).collect();
        let re2c_short: String = re2c_val.chars().take(30).collect();
        return format!("raw_text (ts={ts_short}, re2c={re2c_short})");
    }

    // Word content items
    if path.contains(".content[") && path.contains("].content") {
        if path.contains("word_type") || path.contains("line_type") {
            return "word_type_mismatch".into();
        }
        if path.contains("category") {
            return "word_category".into();
        }
        if path.contains("form_type") {
            return "form_type".into();
        }
        if path.contains("lang") {
            return "lang_marker".into();
        }
        return "word_content_structure".into();
    }

    // Variant mismatches (different enum type)
    if path.contains("line_type") || path.contains("type") {
        return format!("type_mismatch (ts={ts_val}, re2c={re2c_val})");
    }

    // Annotation
    if path.contains("annotation") || path.contains("scoped") {
        return "annotation".into();
    }

    // Terminator
    if path.contains("terminator") || path.contains("utterance_terminator") {
        return "terminator".into();
    }

    // Catch-all: use last path segment
    let last = path.rsplit('.').next().unwrap_or(path);
    format!(
        "other:{last} (ts={}, re2c={})",
        ts_val.chars().take(40).collect::<String>(),
        re2c_val.chars().take(40).collect::<String>()
    )
}

#[test]
#[ignore = "needs the wild corpus, which no contributor has; a report \
           generator rather than a gate. Run with --ignored --nocapture."]
fn subcategorize_main_tier_divergences() -> Result<(), Box<dyn std::error::Error>> {
    let base = crate::corpus_root::CorpusRoot::resolve().require();

    let files = collect_cha_files(&base)?;
    eprintln!("Found {} .cha files", files.len());

    let ts = TreeSitterParser::new().expect("tree-sitter grammar loads");
    let re2c = Re2cParser::new();

    write_report(
        &base,
        &files,
        Path::new("/tmp/re2c_main_tier_subcategories.json"),
        |path, source| CompletedParserPair::parse(path, source, &ts, &re2c),
    )
}

fn write_report(
    base: &Path,
    files: &[PathBuf],
    report_path: &Path,
    mut parse: impl FnMut(&Path, &str) -> Result<CompletedParserPair, ParserPairFailure>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut subcats: BTreeMap<String, usize> = BTreeMap::new();
    let mut subcat_examples: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut native_rejected = 0;
    let mut unclassified = 0;
    let mut total_divergent = 0;
    let mut main_tier_divergent = 0;

    for (i, path) in files.iter().enumerate() {
        if i > 0 && i % 10000 == 0 {
            eprintln!(
                "  Progress: {}/{} ({main_tier_divergent} main_tier divergences)",
                i,
                files.len()
            );
        }

        let content = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;

        let attempts = parse(path, &content)?;
        let (ts_file, re2c_file) = match attempts.models() {
            ComparisonModels::Built { tree_sitter, re2c } => (tree_sitter, re2c),
            ComparisonModels::NativeRejected => {
                native_rejected += 1;
                continue;
            }
        };

        if ts_file.semantic_eq(re2c_file) {
            continue;
        }
        total_divergent += 1;

        let comparison = JsonPair::serialize(ts_file, re2c_file)?;
        if let Some(difference) = comparison.first_difference(25)? {
            let diff_path = difference.path;
            let ts_val: String = difference.left.chars().take(200).collect();
            let re2c_val: String = difference.right.chars().take(200).collect();
            // Only sub-categorize main tier divergences
            if diff_path.contains(".main.") || diff_path.contains(".main_") {
                main_tier_divergent += 1;
                let subcat = sub_classify(&diff_path, &ts_val, &re2c_val);
                *subcats.entry(subcat.clone()).or_insert(0) += 1;

                let examples = subcat_examples.entry(subcat).or_default();
                if examples.len() < 5 {
                    let file_str = path
                        .strip_prefix(base)
                        .unwrap_or(path)
                        .display()
                        .to_string();
                    examples.push((file_str, diff_path));
                }
            }
        } else {
            unclassified += 1;
        }
    }

    eprintln!("\n=== MAIN TIER SUB-CATEGORIZATION ===");
    eprintln!("Total divergent: {total_divergent}");
    eprintln!("Native parser rejected: {native_rejected}");
    eprintln!("Semantic divergences without a JSON difference: {unclassified}");
    eprintln!("Main tier divergent: {main_tier_divergent}");
    eprintln!();

    let mut sorted: Vec<_> = subcats.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));

    for (subcat, count) in &sorted {
        let pct = **count as f64 / main_tier_divergent.max(1) as f64 * 100.0;
        eprintln!("  {subcat}");
        eprintln!("    count: {count} ({pct:.1}%)");
        if let Some(examples) = subcat_examples.get(*subcat) {
            for (file, path) in examples.iter().take(3) {
                eprintln!("    ex: {file}");
                eprintln!("        {path}");
            }
        }
        eprintln!();
    }

    // Write report
    let report: Vec<serde_json::Value> = sorted
        .iter()
        .map(|(subcat, count)| {
            let examples = subcat_examples
                .get(*subcat)
                .map(|ex| {
                    ex.iter()
                        .map(|(f, p)| serde_json::json!({"file": f, "path": p}))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            serde_json::json!({ "subcategory": subcat, "count": count, "examples": examples })
        })
        .collect();
    JsonReport::serialize(&report)?.write(report_path)?;
    eprintln!("Report: {}", report_path.display());
    Ok(())
}

#[test]
fn internal_parser_faults_refuse_report_publication() -> Result<(), Box<dyn std::error::Error>> {
    crate::parser_attempt::assert_report_admission(|base, files, destination, parse| {
        crate::subcategorize_main_tier::write_report(base, files, destination, parse)
    })
}
