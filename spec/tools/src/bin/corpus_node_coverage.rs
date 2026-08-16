//! Report grammar node type coverage for a corpus.
//!
//! A renderer. The computation is `generators::node_coverage`, and the CI gate
//! is `tests/node_coverage.rs`, which calls it directly. This binary was the
//! only caller for a long time, and it ended in `std::process::exit(1)` that
//! nothing ever observed, because CI runs `cargo test`, never `cargo run`.
//!
//! Usage:
//!   cargo run --manifest-path spec/Cargo.toml --bin corpus_node_coverage
//!   cargo run ... -- --corpus-dir DIR --node-types FILE --json

use clap::Parser as ClapParser;
use serde::Serialize;
use std::path::PathBuf;
use std::process::ExitCode;

use generators::node_coverage::{Request, run};
use generators::repo_paths::RepoRoot;

#[derive(ClapParser)]
#[command(name = "corpus_node_coverage")]
#[command(about = "Report grammar node type coverage for a corpus")]
struct Args {
    /// Directory containing .cha files to analyze.
    #[arg(long)]
    corpus_dir: Option<PathBuf>,

    /// Path to tree-sitter node-types.json.
    #[arg(long)]
    node_types: Option<PathBuf>,

    /// Output as JSON.
    #[arg(long)]
    json: bool,
}

/// The machine-readable shape. Derived from the report rather than accumulated
/// alongside it, so the two cannot disagree.
#[derive(Serialize)]
struct JsonReport {
    required: usize,
    exercised: usize,
    missing_count: usize,
    coverage_pct: f64,
    supertype_count: usize,
    missing: Vec<String>,
    invalid_present: Vec<String>,
    stale_exclusions: Vec<String>,
    files_parsed: usize,
    files_with_errors: usize,
}

fn main() -> ExitCode {
    let args = Args::parse();

    // Each path is the operator's if they named one, and this repository's
    // otherwise. Written as a match rather than `unwrap_or_else(default_...)`
    // for two reasons: the old form resolved the repository root EAGERLY, on
    // every run, including runs that named both paths and needed no checkout at
    // all; and the root resolution can now fail, which an `unwrap_or_else`
    // closure has nowhere to report.
    //
    // These are configuration defaults, not invented measurements: each is
    // derived from a `RepoRoot` that proved itself a chatter checkout.
    let request = match (args.corpus_dir, args.node_types) {
        (Some(corpus_dir), Some(node_types)) => Request {
            corpus_dir,
            node_types,
        },
        (given_corpus_dir, given_node_types) => {
            let derived = match RepoRoot::resolve(None).map(|root| Request::for_repo(&root)) {
                Ok(request) => request,
                Err(why) => {
                    eprintln!("{why}");
                    return ExitCode::FAILURE;
                }
            };
            Request {
                corpus_dir: match given_corpus_dir {
                    Some(given) => given,
                    None => derived.corpus_dir,
                },
                node_types: match given_node_types {
                    Some(given) => given,
                    None => derived.node_types,
                },
            }
        }
    };

    let report = match run(&request) {
        Ok(report) => report,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::FAILURE;
        }
    };

    if args.json {
        let payload = JsonReport {
            required: report.required,
            exercised: report.exercised(),
            missing_count: report.missing.len(),
            coverage_pct: report.coverage_pct(),
            supertype_count: report.supertype_count,
            missing: report.missing.clone(),
            invalid_present: report
                .invalid_present
                .iter()
                .map(|found| {
                    format!(
                        "{} ({}) in: {}",
                        found.kind,
                        found.code,
                        found.files.join(", ")
                    )
                })
                .collect(),
            stale_exclusions: report
                .stale_exclusions
                .iter()
                .map(|k| (*k).to_owned())
                .collect(),
            files_parsed: report.files_parsed,
            files_with_errors: report.files_with_errors,
        };
        match serde_json::to_string_pretty(&payload) {
            Ok(text) => println!("{text}"),
            Err(err) => {
                eprintln!("failed to serialize report: {err}");
                return ExitCode::FAILURE;
            }
        }
    }

    // The SAME value the gate asserts on, so this binary and CI cannot report
    // different things about the same corpus.
    match report.outcome() {
        Ok(summary) => {
            if !args.json {
                println!("{summary}");
            }
            ExitCode::SUCCESS
        }
        Err(why) => {
            if !args.json {
                eprintln!("{why}");
            }
            ExitCode::FAILURE
        }
    }
}
