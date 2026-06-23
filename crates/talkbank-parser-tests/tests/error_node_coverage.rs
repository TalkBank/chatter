//! Foolproof ERROR-node coverage oracle.
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
//! Two complementary tests guard the class:
//!
//! - [`every_error_node_is_reported`] (curated CHECK-parity fixtures): the
//!   STRONG, precise check. Each curated fixture's single defect IS a recovery
//!   node, so every such node must be span-covered by a parser diagnostic. This
//!   guarantees the diagnostic lands AT the defect, not merely somewhere.
//!
//! - [`no_recovery_node_in_accepted_file`] (the whole error corpus): the BROAD,
//!   robust check. For every fixture across all error corpora that contains any
//!   tree-sitter recovery node, chatter must reject the file (a parse- or
//!   validation-stage error). This is the actual swallow invariant: a recovery
//!   node may never appear in a file chatter calls valid. It avoids the
//!   false-positive trap of demanding per-node parse spans corpus-wide (a
//!   recovery node is often legitimately reported by an adjacent parse
//!   diagnostic, or caught at the validation stage with a precise code), while
//!   still failing loudly if any recovery-bearing file is ever accepted.

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

/// Which kind of tree-sitter recovery a node represents.
enum RecoveryKind {
    /// An `ERROR` node: an unparseable region.
    Error,
    /// A `MISSING` node: a required element tree-sitter inserted during recovery.
    Missing,
}

impl std::fmt::Display for RecoveryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Error => "ERROR",
            Self::Missing => "MISSING",
        })
    }
}

/// A tree-sitter recovery node (`ERROR` or `MISSING`) the grammar produced.
struct RecoveryNode {
    start: usize,
    end: usize,
    kind: RecoveryKind,
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

/// Collect every `ERROR`/`MISSING` node. The error subtree is one diagnostic
/// unit, so we do not descend past a recovery node.
fn recovery_nodes(tree: &tree_sitter::Tree) -> Vec<RecoveryNode> {
    let mut found = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        let is_missing = node.is_missing();
        if node.is_error() || is_missing {
            found.push(RecoveryNode {
                start: node.start_byte(),
                end: node.end_byte(),
                kind: if is_missing {
                    RecoveryKind::Missing
                } else {
                    RecoveryKind::Error
                },
            });
            continue;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    found
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

/// The curated CHECK-parity fixtures (each fixture's single defect is a recovery
/// node, so per-node span coverage is exact for them).
fn check_parity_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/check_parity/fixtures")
}

/// Every directory holding fixtures that legitimately contain tree-sitter
/// recovery nodes: the CHECK-parity fixtures, the spec-generated validation
/// corpus, and the hand-maintained parse-error corpus (at the repo root).
fn error_corpus_dirs() -> Vec<PathBuf> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_dir.join("..").join("..");
    vec![
        check_parity_dir(),
        crate_dir.join("tests/error_corpus/validation_errors"),
        repo_root.join("tests/error_corpus/parse_errors"),
    ]
}

// ---------------------------------------------------------------------------
// Strong check (curated): every recovery node is span-covered by a parse
// diagnostic.
// ---------------------------------------------------------------------------

/// Byte ranges of every diagnostic the production parser reported on `source`.
fn chatter_diagnostic_ranges(
    parser: &TreeSitterParser,
    source: &str,
) -> Vec<std::ops::Range<usize>> {
    let errors = ErrorCollector::new();
    let _ = parser.parse_chat_file_fragment(source, 0, &errors);
    errors
        .to_vec()
        .iter()
        .map(|err| {
            let span = err.location.span;
            (span.start as usize)..(span.end as usize)
        })
        .collect()
}

/// A recovery node is covered if some reported diagnostic's range (inclusive of
/// its endpoints, so a zero-length `MISSING` node is covered by a diagnostic
/// reported at that offset) contains the node's start offset.
fn is_covered(node: &RecoveryNode, diagnostics: &[std::ops::Range<usize>]) -> bool {
    diagnostics
        .iter()
        .any(|range| range.start <= node.start && node.start <= range.end)
}

/// Every ERROR/MISSING node the grammar produces for a curated CHECK-parity
/// fixture must be span-covered by a parser diagnostic. Guards the swallow class
/// precisely: the diagnostic must land AT the defect.
#[test]
fn every_error_node_is_reported() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let fixtures = collect_cha_fixtures(&[check_parity_dir()])?;
    if fixtures.is_empty() {
        return Err(TestError::Failure(
            "no CHECK-parity fixtures found".to_owned(),
        ));
    }

    let mut swallowed = Vec::new();
    let mut checked_with_error_nodes = 0usize;

    for fixture in &fixtures {
        let source = std::fs::read_to_string(fixture)
            .map_err(|err| TestError::Failure(format!("read {}: {err}", fixture.display())))?;
        let nodes = recovery_nodes(&tree_sitter_parse(&source)?);
        if nodes.is_empty() {
            // Validation-layer fixture (syntactically valid CHAT); nothing for
            // this precise check to guard.
            continue;
        }
        checked_with_error_nodes += 1;

        let diagnostics = chatter_diagnostic_ranges(&parser, &source);
        for node in &nodes {
            if !is_covered(node, &diagnostics) {
                let name = fixture.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                swallowed.push(format!(
                    "{name}: {} node at bytes {}..{} not reported by the parser \
                     (diagnostics covered: {:?})",
                    node.kind, node.start, node.end, diagnostics
                ));
            }
        }
    }

    if !swallowed.is_empty() {
        return Err(TestError::Failure(format!(
            "{} tree-sitter recovery node(s) were silently swallowed by the parser \
             (each ERROR/MISSING node must produce a diagnostic):\n  {}",
            swallowed.len(),
            swallowed.join("\n  ")
        )));
    }

    // Guard against a vacuous pass: at least one fixture must actually contain
    // recovery nodes, or this check is exercising nothing. The CHECK 100
    // trailing-comma fixture guarantees this.
    if checked_with_error_nodes == 0 {
        return Err(TestError::Failure(
            "no CHECK-parity fixture produced any ERROR/MISSING node; the swallow \
             oracle is vacuous (expected at least the CHECK 100 trailing-comma fixture)"
                .to_owned(),
        ));
    }

    println!(
        "✓ every ERROR/MISSING node span-covered across {checked_with_error_nodes} \
         curated parity fixture(s)"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Broad check (whole corpus): no recovery node in an accepted file.
// ---------------------------------------------------------------------------

/// Whether chatter surfaces ANY diagnostic (error OR warning) for `source`,
/// considering both the parse stage and the validation stage. "Surfaced" is the
/// not-swallowed signal: a recovery node may be reported as a parse error, or as
/// a validation error/warning with a precise code (e.g. E603/E605 surface as
/// warnings). A swallow is the absence of any diagnostic at all, which is when a
/// recovery node sits silently in a file chatter would otherwise call valid.
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
/// backlog: chatter is known not to catch them yet, so the broad swallow oracle
/// skips them rather than flagging a known gap as a regression. The same
/// manifest status drives `validation_errors_detected`'s skip logic.
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

/// Across the WHOLE error corpus, any fixture that tree-sitter parses with one
/// or more recovery nodes, and that we intend to reject, must surface at least
/// one chatter diagnostic. This is the broad swallow invariant: a recovery node
/// may never sit silently inside a file chatter accepts with no diagnostic at
/// all.
///
/// We assert "surfaces a diagnostic", not per-node parse spans, because
/// corpus-wide a recovery node is often reported by an adjacent diagnostic or at
/// the validation stage; demanding exact per-node spans there flags those benign
/// cases (the precise per-node guarantee is kept for the curated set in
/// `every_error_node_is_reported`). Validation fixtures whose rule is not yet
/// implemented are a tracked backlog (skipped); the parse-error and CHECK-parity
/// corpora are always-reject and always checked.
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
        let nodes = recovery_nodes(&tree_sitter_parse(&source)?);
        if nodes.is_empty() {
            continue;
        }
        checked += 1;

        let stem = fixture.file_stem().and_then(|s| s.to_str());
        if !chatter_surfaces_diagnostic(&parser, &source, stem) {
            swallowed.push(format!(
                "{name}: {} tree-sitter recovery node(s) but chatter surfaced NO diagnostic \
                 (swallowed, file silently accepted)",
                nodes.len()
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
            "no error-corpus fixture produced any tree-sitter recovery node; the broad \
             swallow oracle is vacuous"
                .to_owned(),
        ));
    }

    println!(
        "✓ all {checked} recovery-bearing error-corpus fixture(s) surfaced a diagnostic \
         ({skipped_backlog} not-yet-implemented validation fixture(s) skipped as tracked backlog)"
    );
    Ok(())
}
