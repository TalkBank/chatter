//! Validate error specifications against actual parser + validator behaviour.
//!
//! A renderer. The logic is `spec_runtime_tools::error_spec_validation`, and
//! the CI gate is `tests/error_spec_codes.rs`, which calls it directly. This
//! binary existed alone for a long time, and CI runs `cargo test`, never
//! `cargo run`, so the check named as THE validation step in ten `spec/`
//! documents had never actually run in CI.

use clap::Parser;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::ExitCode;

use spec_runtime_tools::error_spec_validation::{
    CodeCheck, CodeFilter, Request, SkippedSpecs, default_spec_dir, run,
};

#[derive(Parser)]
#[command(name = "validate_error_specs")]
#[command(about = "Validate error specifications against actual parser + validator behavior")]
struct Args {
    /// Root directory containing error specs.
    #[arg(short, long)]
    spec_dir: Option<PathBuf>,

    /// Verify that each example produces the claimed error code.
    #[arg(long)]
    check_codes: bool,

    /// Include not_implemented/deprecated specs (normally skipped).
    #[arg(long)]
    include_skipped: bool,

    /// Comma-separated list of error codes to check (e.g., "E248,E249").
    /// If omitted, checks all specs.
    #[arg(long, value_delimiter = ',')]
    filter: Option<Vec<String>>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let request = Request {
        spec_dir: args.spec_dir.unwrap_or_else(default_spec_dir),
        code_check: if args.check_codes {
            CodeCheck::Verify
        } else {
            CodeCheck::ParseOnly
        },
        skipped: if args.include_skipped {
            SkippedSpecs::Include
        } else {
            SkippedSpecs::Omit
        },
        filter: args.filter.map_or(CodeFilter::All, |codes| {
            CodeFilter::Only(codes.into_iter().collect::<BTreeSet<_>>())
        }),
    };

    let report = match run(&request) {
        Ok(report) => report,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::FAILURE;
        }
    };

    // The SAME value the gate asserts on, so this binary and CI cannot report
    // different things about the same corpus.
    match report.outcome() {
        Ok(summary) => {
            eprintln!("{summary}");
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
