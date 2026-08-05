//! Generate Rust test files from specifications
//!
//! Reads construct and error specs and generates Rust test files
//! directly into this repository's test tree.

use clap::Parser;
use generators::output::rust_test;
use generators::spec::{ConstructSpec, ErrorSpec};
use std::path::PathBuf;

/// CLI arguments: input spec directories, output directory for generated `.rs` files, and test error type path.
#[derive(Parser)]
#[command(name = "gen_rust_tests")]
#[command(about = "Generate Rust test files")]
struct Args {
    /// Root directory containing construct specs
    #[arg(long, default_value = "spec/constructs")]
    construct_dir: PathBuf,

    /// Root directory containing error specs
    #[arg(long, default_value = "spec/errors")]
    error_dir: PathBuf,

    /// Output directory for generated test files (e.g., crates/talkbank-parser-tests/tests/generated)
    /// WARNING: Generated test files in this directory will be removed before regenerating
    /// to ensure no stale tests remain when specs are deleted
    #[arg(short, long)]
    output_dir: PathBuf,

    /// Fully-qualified path to the TestError type used in generated tests
    #[arg(long, default_value = "talkbank_parser_tests::test_error::TestError")]
    test_error_path: String,
}

/// Generates Rust test files from construct and error specs for the parser test suite.
fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("Loading specifications...");

    let construct_specs = ConstructSpec::load_all(&args.construct_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load construct specs: {}", e))?;

    let error_specs = ErrorSpec::load_all(&args.error_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load error specs: {}", e))?;

    println!(
        "Loaded {} construct specs, {} error specs",
        construct_specs.len(),
        error_specs.len()
    );
    println!("Output directory: {}", args.output_dir.display());

    println!("Cleaning old generated test files...");
    let written = rust_test::write_generated_tests(
        &args.output_dir,
        &construct_specs,
        &error_specs,
        &args.test_error_path,
    )?;

    for path in &written {
        println!("✓ Generated: {}", path.display());
    }
    println!(
        "\n✓ Generated {} test files to {}",
        written.len(),
        args.output_dir.display()
    );

    Ok(())
}
