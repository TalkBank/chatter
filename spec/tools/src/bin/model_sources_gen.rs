//! Bootstrap model sources without compiling the production model or parser.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use generators::artifacts::BOOTSTRAP_ARTIFACTS;
use generators::repo_paths::RepoRoot;

#[derive(Parser)]
#[command(about = "Generate model sources before compiling their consumers")]
struct Args {
    /// Repository root; defaults to this checkout.
    #[arg(long)]
    repo_root: Option<PathBuf>,
}

fn main() -> Result<()> {
    let root = RepoRoot::resolve(Args::parse().repo_root)?;
    for artifact in BOOTSTRAP_ARTIFACTS {
        let written = artifact.write(root.as_path())?;
        println!("wrote {written:4} file(s)  {}", artifact.what);
    }
    Ok(())
}
