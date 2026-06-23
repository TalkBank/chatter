//! Foolproof ERROR-node swallow oracle.
//!
//! tree-sitter represents every unparseable region as an `ERROR` node and every
//! tree-sitter-inserted required element as a `MISSING` node. These are exactly
//! the `NodeSlot::Error` / `NodeSlot::Missing` cases the generated typed
//! traversal (`generated_traversal.rs`) models. The production parser must never
//! silently drop one: a "swallow" is when an invalid file is accepted as valid
//! because the only evidence of invalidity (the recovery node) was never
//! reported. The `@Participants` trailing comma (CLAN CHECK 100, chatter E550)
//! was exactly this, and `@Languages` had the identical bug.
//!
//! The invariant: across the whole error corpus, any fixture that tree-sitter
//! parses with one or more recovery nodes, and that we intend to reject, must
//! surface at least one chatter diagnostic (a parse- OR validation-stage error
//! or warning). A recovery node may never sit silently inside a file chatter
//! accepts with no diagnostic at all.
//!
//! We assert "surfaces a diagnostic", not per-node parse spans: corpus-wide a
//! recovery node is routinely reported by an ADJACENT diagnostic (e.g. the
//! `@Media` space-before-comma whose ERROR sits one byte from the reported
//! token) or caught at the validation stage with a precise code. Demanding exact
//! per-node parse spans flags those benign cases as failures. The exact code for
//! a given CLAN-CHECK fixture is pinned separately by `chatter_matches_check` in
//! the behavioral parity suite; this oracle's job is solely to make a silently
//! swallowed recovery node loud. Validation fixtures whose rule is not yet
//! implemented are a tracked backlog (skipped via the manifest status); the
//! parse-error and CHECK-parity corpora are always-reject and always checked.

use std::collections::HashSet;
use std::path::PathBuf;

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// Minimal view of the validation corpus manifest: just enough to learn which
/// fixtures correspond to validation rules that are not yet implemented.
#[derive(serde::Deserialize)]
struct ManifestEntry {
    fixture: String,
    status: String,
}

#[derive(serde::Deserialize)]
struct Manifest {
    fixtures: Vec<ManifestEntry>,
}

/// Raw tree-sitter parse: the grammar's own view of error recovery, independent
/// of how the production parser walks it.
fn tree_sitter_parse(source: &str) -> Result<tree_sitter::Tree, TestError> {
    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser
        .set_language(&lang)
        .map_err(|err| TestError::ParserInit(err.to_string()))?;
    parser
        .parse(source, None)
        .ok_or_else(|| TestError::Failure("tree-sitter returned no tree".to_owned()))
}

/// How many `ERROR`/`MISSING` recovery nodes the grammar produced. The error
/// subtree is one diagnostic unit, so we do not descend past a recovery node.
fn recovery_node_count(tree: &tree_sitter::Tree) -> usize {
    let mut count = 0usize;
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            count += 1;
            continue;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    count
}

/// `.cha` fixtures under each directory, sorted for deterministic output.
fn collect_cha_fixtures(dirs: &[PathBuf]) -> Result<Vec<PathBuf>, TestError> {
    let mut fixtures = Vec::new();
    for dir in dirs {
        let entries = std::fs::read_dir(dir)
            .map_err(|err| TestError::Failure(format!("read {}: {err}", dir.display())))?;
        for entry in entries {
            let path = entry
                .map_err(|err| {
                    TestError::Failure(format!("dir entry in {}: {err}", dir.display()))
                })?
                .path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("cha") {
                fixtures.push(path);
            }
        }
    }
    fixtures.sort();
    Ok(fixtures)
}

/// Every directory holding fixtures that legitimately contain tree-sitter
/// recovery nodes: the CHECK-parity fixtures, the spec-generated validation
/// corpus, and the hand-maintained parse-error corpus (at the repo root).
fn error_corpus_dirs() -> Vec<PathBuf> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_dir.join("..").join("..");
    vec![
        crate_dir.join("tests/check_parity/fixtures"),
        crate_dir.join("tests/error_corpus/validation_errors"),
        repo_root.join("tests/error_corpus/parse_errors"),
    ]
}

/// Whether chatter surfaces ANY diagnostic (error OR warning) for `source`,
/// considering both the parse stage and the validation stage. "Surfaced" is the
/// not-swallowed signal: a recovery node may be reported as a parse error, or as
/// a validation error/warning with a precise code (e.g. E603/E605 surface as
/// warnings). A swallow is the absence of any diagnostic at all.
fn chatter_surfaces_diagnostic(
    parser: &TreeSitterParser,
    source: &str,
    stem: Option<&str>,
) -> bool {
    let parse_errors = ErrorCollector::new();
    let outcome = parser.parse_chat_file_fragment(source, 0, &parse_errors);
    if !parse_errors.is_empty() {
        return true;
    }
    match outcome {
        ParseOutcome::Parsed(mut chat_file) => {
            let validation_errors = ErrorCollector::new();
            chat_file.validate_with_alignment(&validation_errors, stem);
            !validation_errors.is_empty()
        }
        ParseOutcome::Rejected => true,
    }
}

/// Fixture filenames whose validation rule is NOT yet implemented (or is
/// deprecated), read from the validation corpus manifest. These are a tracked
/// backlog: chatter is known not to catch them yet, so the oracle skips them
/// rather than flagging a known gap as a regression. The same manifest status
/// drives `validation_errors_detected`'s skip logic.
fn unimplemented_validation_fixtures() -> Result<HashSet<String>, TestError> {
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/error_corpus/validation_errors/manifest.json");
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|err| TestError::Failure(format!("read validation manifest: {err}")))?;
    let manifest: Manifest = serde_json::from_str(&text)
        .map_err(|err| TestError::Failure(format!("parse validation manifest: {err}")))?;
    Ok(manifest
        .fixtures
        .into_iter()
        .filter(|entry| entry.status != "implemented")
        .map(|entry| entry.fixture)
        .collect())
}

/// Across the whole error corpus, any recovery-bearing fixture we intend to
/// reject must surface a chatter diagnostic. Guards the swallow class of bug.
#[test]
fn no_recovery_node_in_accepted_file() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let skip = unimplemented_validation_fixtures()?;
    let fixtures = collect_cha_fixtures(&error_corpus_dirs())?;
    if fixtures.is_empty() {
        return Err(TestError::Failure(
            "no error-corpus fixtures found".to_owned(),
        ));
    }

    let mut swallowed = Vec::new();
    let mut checked = 0usize;
    let mut skipped_backlog = 0usize;

    for fixture in &fixtures {
        let name = fixture.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        if skip.contains(name) {
            skipped_backlog += 1;
            continue;
        }
        let source = std::fs::read_to_string(fixture)
            .map_err(|err| TestError::Failure(format!("read {}: {err}", fixture.display())))?;
        let recovery_nodes = recovery_node_count(&tree_sitter_parse(&source)?);
        if recovery_nodes == 0 {
            continue;
        }
        checked += 1;

        let stem = fixture.file_stem().and_then(|s| s.to_str());
        if !chatter_surfaces_diagnostic(&parser, &source, stem) {
            swallowed.push(format!(
                "{name}: {recovery_nodes} tree-sitter recovery node(s) but chatter surfaced NO \
                 diagnostic (swallowed, file silently accepted)"
            ));
        }
    }

    if !swallowed.is_empty() {
        return Err(TestError::Failure(format!(
            "{} error-corpus fixture(s) contain tree-sitter recovery nodes yet chatter surfaced \
             no diagnostic (a swallowed recovery node):\n  {}",
            swallowed.len(),
            swallowed.join("\n  ")
        )));
    }

    if checked == 0 {
        return Err(TestError::Failure(
            "no error-corpus fixture produced any tree-sitter recovery node; the swallow oracle \
             is vacuous"
                .to_owned(),
        ));
    }

    println!(
        "✓ all {checked} recovery-bearing error-corpus fixture(s) surfaced a diagnostic \
         ({skipped_backlog} not-yet-implemented validation fixture(s) skipped as tracked backlog)"
    );
    Ok(())
}
