//! Emit source-bound inline-test ranges for the Rust files named on stdin.
//! Input is a JSON array of repository-relative paths; output is keyed by path.

use std::collections::BTreeMap;
use std::io::{self, Read};
use talkbank_parser_tests::coverage_source::test_source_ranges;
use talkbank_parser_tests::repo_paths::workspace_root;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let files: Vec<String> = serde_json::from_str(&input)?;
    let root = workspace_root();
    let mut ranges = BTreeMap::new();
    for file in files {
        let source = std::fs::read_to_string(root.join(&file))?;
        ranges.insert(file, test_source_ranges(&source)?);
    }
    serde_json::to_writer_pretty(io::stdout().lock(), &ranges)?;
    Ok(())
}
