//! CI gate: the reference corpus exercises every concrete grammar node type it
//! is supposed to, and none that it must not.
//!
//! `corpus_node_coverage` ended in `std::process::exit(1)`, and CI runs
//! `cargo test`, never `cargo run`, so that exit code had never been observed
//! by anything while `book/src/contributing/reference-corpus.md` cited the tool
//! as the coverage check.

use generators::node_coverage::{Request, run};
use generators::repo_paths::RepoRoot;

/// SURVIVES: policy. WHICH node types a curated corpus must exercise, and which
/// are excused, is a judgement with real alternatives; no type holds it. What
/// the types hold is that an exclusion's KIND decides which reverse check
/// applies, and that the verdict and its text are one value the renderer
/// shares, so `cargo run` and CI cannot disagree.
#[test]
fn the_reference_corpus_covers_the_grammar() -> Result<(), String> {
    let root = RepoRoot::resolve(None).map_err(|why| why.to_string())?;
    run(&Request::for_repo(&root))?
        .outcome()
        .map(|summary| println!("{summary}"))
}
