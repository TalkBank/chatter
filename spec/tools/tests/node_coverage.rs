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

/// E758 is conditional on CA policy; syntax alone cannot report invalidity.
#[test]
fn separator_padding_requires_context_review() -> Result<(), Box<dyn std::error::Error>> {
    let root = RepoRoot::resolve(None)?;
    for fixture in ["E758_6.cha", "E758_1.cha"] {
        let temporary = tempfile::tempdir()?;
        std::fs::copy(
            root.join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors")
                .join(fixture),
            temporary.path().join(fixture),
        )?;
        let report = run(&Request {
            corpus_dir: temporary.path().to_owned(),
            node_types: root.join("grammar/src/node-types.json"),
        })?;
        assert!(
            report.invalid_present.is_empty(),
            "{fixture}: {}",
            report.outcome().unwrap_err()
        );
        assert!(!report.stale_exclusions.contains(&"sep_trailing_space"));
        assert!(
            report.outcome().is_err(),
            "a single fixture is not complete coverage"
        );
    }
    Ok(())
}

/// Real unconditional violations retain their code and every contributing file.
#[test]
fn unconditional_findings_retain_source_evidence() -> Result<(), Box<dyn std::error::Error>> {
    let root = RepoRoot::resolve(None)?;
    for (fixture, expected) in [
        ("E747_1.cha", "blank_line (E747)"),
        ("E256_1.cha", "illegal_curly_quote (E256)"),
        ("E203_1.cha", "repeated_form_marker (E203)"),
    ] {
        let temporary = tempfile::tempdir()?;
        for filename in ["first.cha", "second.cha"] {
            std::fs::copy(
                root.join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors")
                    .join(fixture),
                temporary.path().join(filename),
            )?;
        }
        let report = run(&Request {
            corpus_dir: temporary.path().to_owned(),
            node_types: root.join("grammar/src/node-types.json"),
        })?;
        assert_eq!(report.invalid_present.len(), 1, "{fixture}");
        let finding = report.invalid_present[0].to_string();
        assert!(finding.starts_with(expected), "{finding}");
        assert!(finding.contains("first.cha"), "{finding}");
        assert!(finding.contains("second.cha"), "{finding}");
        assert!(report.outcome().unwrap_err().contains(&finding));
    }
    Ok(())
}
