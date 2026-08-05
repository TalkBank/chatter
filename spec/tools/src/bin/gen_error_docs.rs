//! Generate error documentation in Markdown format
//!
//! Reads error specs and generates publishable documentation.

use clap::Parser;
use generators::output::markdown;
use generators::owned_output::clear_owned;
use generators::spec::ErrorSpec;
use std::path::PathBuf;

/// CLI arguments: input error spec directory and output directory for generated Markdown docs.
#[derive(Parser)]
#[command(name = "gen_error_docs")]
#[command(about = "Generate error documentation")]
struct Args {
    /// Root directory containing error specs
    #[arg(short, long, default_value = "spec/errors")]
    error_dir: PathBuf,

    /// Output directory for generated documentation
    #[arg(short, long, default_value = "docs/errors")]
    output_dir: PathBuf,
}

/// Generates publishable Markdown documentation (index + per-error pages) from error specs.
fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!(
        "Loading error specifications from: {}",
        args.error_dir.display()
    );

    let specs = ErrorSpec::load_all(&args.error_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load error specs: {}", e))?;

    println!("Loaded {} error specifications", specs.len());

    // Clear stale docs. Previously this swept `*.md` with `let _ =`, so a
    // hand-written note in the output directory was destroyed silently and a
    // failed delete was not reported at all.
    clear_owned(&args.output_dir)?;

    // Generate index page
    let index = markdown::generate_error_index(&specs);
    let index_path = args.output_dir.join("index.md");
    std::fs::write(&index_path, index)?;
    println!("✓ Generated: {}", index_path.display());

    // Generate individual error pages
    let mut page_count = 0;
    for spec in &specs {
        for error in &spec.errors {
            // Category-level status applies to every error in the spec;
            // see ErrorMetadata::status and generate_error_page docs.
            let page = markdown::generate_error_page(error, &spec.metadata.status);
            let page_path = args.output_dir.join(format!("{}.md", error.code));
            std::fs::write(&page_path, page)?;
            println!("✓ Generated: {}", page_path.display());
            page_count += 1;
        }
    }

    println!("\n✓ Generated {} error documentation pages", page_count + 1);

    Ok(())
}
