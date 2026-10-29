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

//! Categorize semantic divergences between Re2cParser and TreeSitterParser.
//!
//! For each divergent file from the full corpus run, this test:
//! 1. Parses with both parsers
//! 2. Serializes both ChatFile to JSON
//! 3. Finds the first point of JSON divergence
//! 4. Extracts the CHAT source context around that point
//! 5. Classifies the divergence by category
//!
//! Run:
//! ```bash
//! cargo test -p talkbank-parser-re2c --test integration categorize_divergences::categorize_corpus_divergences --release -- --ignored --exact --nocapture
//! ```

use crate::diagnostic_json::{JsonPair, JsonReport, collect_cha_files};
use crate::parser_attempt::{ComparisonModels, CompletedParserPair, ParserPairFailure};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use talkbank_model::SemanticEq;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

/// Category of divergence between the two parsers.
///
/// Some variants (e.g. `WordDiff`) are not currently constructed by the
/// categorizer but are kept so the categorization taxonomy is complete and
/// future categorizer rules don't have to revisit the enum shape.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Category {
    /// Different number of lines in ChatFile
    LineLengthMismatch,
    /// Header content differs
    HeaderDiff { header_type: String },
    /// Main tier content differs (word parsing, annotations, etc.)
    MainTierContent { detail: String },
    /// Main tier speaker differs
    MainTierSpeaker,
    /// Dependent tier count differs
    DependentTierCount,
    /// Dependent tier content differs
    DependentTierContent { tier_kind: String },
    /// Word-level difference
    WordDiff { detail: String },
    /// JSON diff at unknown path
    Other { path: String },
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::LineLengthMismatch => write!(f, "line_count_mismatch"),
            Category::HeaderDiff { header_type } => write!(f, "header/{header_type}"),
            Category::MainTierContent { detail } => write!(f, "main_tier/{detail}"),
            Category::MainTierSpeaker => write!(f, "main_tier/speaker"),
            Category::DependentTierCount => write!(f, "dep_tier_count"),
            Category::DependentTierContent { tier_kind } => {
                write!(f, "dep_tier/{tier_kind}")
            }
            Category::WordDiff { detail } => write!(f, "word/{detail}"),
            Category::Other { path } => write!(f, "other/{path}"),
        }
    }
}

/// Classify a JSON diff path into a Category.
fn classify_diff_path(path: &str) -> Category {
    // Examples of paths:
    // lines[3].utterance.main.content[2].word.raw_text
    // lines[0].header.participants[1].role
    // lines[5].utterance.dependent_tiers[0].mor.items[1]

    if path.contains("lines[]") {
        return Category::LineLengthMismatch;
    }

    if path.contains(".header.") || (path.contains(".header") && !path.contains(".utterance")) {
        let header_type = if path.contains("participants") {
            "participants"
        } else if path.contains("id_header") || path.contains("id.") {
            "id"
        } else if path.contains("languages") {
            "languages"
        } else if path.contains("media") {
            "media"
        } else {
            "other"
        };
        return Category::HeaderDiff {
            header_type: header_type.to_string(),
        };
    }

    if path.contains(".dependent_tiers[]")
        || path.contains(".dependent_tiers[") && path.ends_with(']') && !path.contains('.')
    {
        return Category::DependentTierCount;
    }

    if path.contains(".dependent_tiers[") {
        let tier_kind = if path.contains(".mor") {
            "mor"
        } else if path.contains(".gra") {
            "gra"
        } else if path.contains(".pho") {
            "pho"
        } else if path.contains(".sin") {
            "sin"
        } else if path.contains(".cod") {
            "cod"
        } else if path.contains(".act") {
            "act"
        } else if path.contains(".com") {
            "com"
        } else if path.contains(".user_defined") || path.contains(".unsupported") {
            "user_defined"
        } else {
            "other"
        };
        return Category::DependentTierContent {
            tier_kind: tier_kind.to_string(),
        };
    }

    if path.contains(".main.speaker") {
        return Category::MainTierSpeaker;
    }

    if path.contains(".main.content[") || path.contains(".main.content[]") {
        let detail = if path.contains(".raw_text") {
            "raw_text"
        } else if path.contains(".word.content") || path.contains(".word_content") {
            "word_content"
        } else if path.contains(".category") {
            "word_category"
        } else if path.contains(".form_type") {
            "form_type"
        } else if path.contains(".lang") {
            "lang"
        } else if path.contains("content[]") {
            "content_length"
        } else if path.contains("annotation") {
            "annotations"
        } else if path.contains("terminator") || path.contains("utterance_terminator") {
            "terminator"
        } else {
            "other"
        };
        return Category::MainTierContent {
            detail: detail.to_string(),
        };
    }

    if path.contains(".main.") {
        let detail = if path.contains("terminator") {
            "terminator"
        } else {
            "other"
        };
        return Category::MainTierContent {
            detail: detail.to_string(),
        };
    }

    Category::Other {
        path: path.chars().take(60).collect(),
    }
}

#[test]
#[ignore = "needs the wild corpus; a report generator that emits a taxonomy \
           rather than a gate that can fail. Run with --ignored --nocapture."]
fn categorize_corpus_divergences() -> Result<(), Box<dyn std::error::Error>> {
    let base = crate::corpus_root::CorpusRoot::resolve().require();

    eprintln!("Collecting .cha files from {}...", base.display());
    let files = collect_cha_files(&base)?;
    eprintln!("Found {} .cha files", files.len());

    let ts = TreeSitterParser::new().expect("tree-sitter grammar loads");
    let re2c = Re2cParser::new();

    write_report(
        &base,
        &files,
        Path::new("/tmp/re2c_divergence_categories.json"),
        |path, source| CompletedParserPair::parse(path, source, &ts, &re2c),
    )
}

fn write_report(
    base: &Path,
    files: &[PathBuf],
    report_path: &Path,
    mut parse: impl FnMut(&Path, &str) -> Result<CompletedParserPair, ParserPairFailure>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut category_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut category_examples: BTreeMap<String, Vec<(String, String, String, String)>> =
        BTreeMap::new();
    let mut native_rejected = 0;
    let mut unclassified = 0;
    let mut total = 0;
    let mut divergent = 0;

    for (i, path) in files.iter().enumerate() {
        if i > 0 && i % 10000 == 0 {
            eprintln!("  Progress: {}/{} ({divergent} divergent)", i, files.len());
        }

        let content = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        total += 1;

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

        divergent += 1;

        // Serialize both to JSON for structural comparison.
        // Strip span fields (skipped by SemanticEq) to avoid false positives.
        let comparison = JsonPair::serialize(ts_file, re2c_file)?;
        if let Some(difference) = comparison.first_difference(20)? {
            let diff_path = difference.path;
            let ts_val = difference.left;
            let re2c_val = difference.right;
            let category = classify_diff_path(&diff_path);
            let cat_str = category.to_string();
            *category_counts.entry(cat_str.clone()).or_insert(0) += 1;

            let examples = category_examples.entry(cat_str).or_default();
            if examples.len() < 3 {
                let file_str = path
                    .strip_prefix(base)
                    .unwrap_or(path)
                    .display()
                    .to_string();
                examples.push((file_str, diff_path, ts_val, re2c_val));
            }
        } else {
            unclassified += 1;
        }
    }

    // Report
    eprintln!("\n=== DIVERGENCE CATEGORIZATION ===");
    eprintln!("Total files: {total}");
    eprintln!("Divergent: {divergent}");
    eprintln!("Native parser rejected: {native_rejected}");
    eprintln!("Semantic divergences without a JSON difference: {unclassified}");
    eprintln!("\nCategories (sorted by count):\n");

    let mut sorted: Vec<_> = category_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));

    for (cat, count) in &sorted {
        let pct = **count as f64 / divergent as f64 * 100.0;
        eprintln!("  {cat:45} {count:6} ({pct:5.1}%)");
        if let Some(examples) = category_examples.get(*cat) {
            for (file, path, ts_val, re2c_val) in examples {
                eprintln!("    file: {file}");
                eprintln!("    path: {path}");
                eprintln!("    ts:   {}", ts_val.chars().take(100).collect::<String>());
                eprintln!(
                    "    re2c: {}",
                    re2c_val.chars().take(100).collect::<String>()
                );
                eprintln!();
            }
        }
    }

    // Write JSON report
    let report: Vec<serde_json::Value> = sorted
        .iter()
        .map(|(cat, count)| {
            let examples = category_examples
                .get(*cat)
                .map(|ex| {
                    ex.iter()
                        .map(|(f, p, t, r)| {
                            serde_json::json!({
                                "file": f,
                                "diff_path": p,
                                "tree_sitter": t,
                                "re2c": r,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            serde_json::json!({
                "category": cat,
                "count": count,
                "percentage": format!("{:.1}%", **count as f64 / divergent as f64 * 100.0),
                "examples": examples,
            })
        })
        .collect();

    JsonReport::serialize(&report)?.write(report_path)?;
    eprintln!(
        "\nFull categorized report written to {}",
        report_path.display()
    );
    Ok(())
}

#[test]
fn internal_parser_faults_refuse_report_publication() -> Result<(), Box<dyn std::error::Error>> {
    crate::parser_attempt::assert_report_admission(|base, files, destination, parse| {
        crate::categorize_divergences::write_report(base, files, destination, parse)
    })
}
