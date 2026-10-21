//! `spec_gen`: regenerate every artifact derived from `spec/`, or check that
//! the committed copies are current.
//!
//! ```bash
//! just spec-gen      # rewrite every generated artifact from the specs
//! just spec-check    # report staleness, writing nothing (this is the gate)
//! ```
//!
//! # Why one binary
//!
//! There used to be one binary per artifact, each taking the destination as a
//! `--output-dir` argument, invoked by three long `cargo run --manifest-path`
//! commands that existed only on a book page. Nothing checked that they had
//! been run, and the generated files' own headers named `make test-gen`, for a
//! repository that has no `Makefile`.
//!
//! Every destination is now a constant in the registry, so a generator cannot
//! be aimed at the wrong directory, and the same list drives writing, checking
//! and the gate.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Parser;
use generators::artifacts::BOOTSTRAP_ARTIFACTS;
use generators::repo_paths::RepoRoot;
use spec_runtime_tools::artifacts::{after_bootstrap, all};

/// CLI arguments.
#[derive(Parser)]
#[command(name = "spec_gen")]
#[command(about = "Regenerate, or check, every artifact generated from spec/")]
struct Args {
    /// Report staleness and write nothing. Exits 1 if anything is stale.
    #[arg(long)]
    check: bool,

    /// Repository root. Defaults to the chatter checkout this crate is in.
    #[arg(long)]
    repo_root: Option<PathBuf>,

    /// Regenerate only the artifact owning this repository-relative root.
    /// Omit to regenerate all artifacts in dependency order.
    #[arg(long)]
    artifact_root: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    // `RepoRoot`, not `PathBuf`: which kind of path this is is the whole reason
    // that newtype exists, and an `unwrap_or_else` here used to flatten both
    // branches back to a bare path to make their types agree.
    let root = RepoRoot::resolve(args.repo_root)?;
    // Updating a source file cannot update code already linked into this process.
    // Refuse before observing or writing any downstream artifact.
    if !args.check {
        for artifact in BOOTSTRAP_ARTIFACTS {
            if !artifact.check(root.as_path())?.is_empty() {
                bail!(
                    "bootstrap sources are stale; run `just spec-gen` to regenerate them before rebuilding the runtime generator"
                );
            }
        }
    }
    let artifacts: Vec<_> = match args.artifact_root.as_deref() {
        Some(selected) => {
            if !args.check && BOOTSTRAP_ARTIFACTS.iter().any(|a| a.root == selected) {
                bail!(
                    "bootstrap sources are owned by `just model-sources-gen`, not the runtime writer"
                );
            }
            let artifact = all()
                .find(|artifact| artifact.root == selected)
                .ok_or_else(|| anyhow::anyhow!("no artifact owns root {selected:?}"))?;
            vec![artifact]
        }
        None if args.check => all().collect(),
        None => after_bootstrap().collect(),
    };

    if args.check {
        let mut stale = 0usize;
        for artifact in &artifacts {
            let differences = artifact.check(root.as_path())?;
            if differences.is_empty() {
                println!("current  {}", artifact.what);
            } else {
                stale += 1;
                println!(
                    "STALE    {} ({} file(s) differ, under {})",
                    artifact.what,
                    differences.len(),
                    artifact.root
                );
                for difference in differences.iter().take(20) {
                    println!("           {difference}");
                }
                if differences.len() > 20 {
                    println!("           ... and {} more", differences.len() - 20);
                }
            }
        }
        if stale > 0 {
            bail!("{stale} artifact(s) are stale. Run `just spec-gen` and commit the result.");
        }
        println!("\nEvery selected generated artifact is current.");
        return Ok(());
    }

    for artifact in artifacts {
        let written = artifact.write(root.as_path())?;
        println!("wrote {written:4} file(s)  {}", artifact.what);
    }
    println!("\nRegenerated. Review the diff before committing.");
    Ok(())
}
