//! The editor's incremental entry points must preserve cold-parse semantics.
//! Sources come from the admitted reference and diagnostic corpora. These are
//! API/wire-boundary comparisons, not fabricated ASTs or validity assertions
//! about intentionally invalid error fixtures.

#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::repo_paths::workspace_root;
use tree_sitter::{InputEdit, Point};

/// Successive admitted specimens stand for arbitrary skipped editor revisions.
/// The producer must derive edits itself and preserve cold-parse semantics,
/// including invalid input, Unicode, a complete deletion, and restoration.
#[test]
fn owned_revisions_preserve_corpus_models_and_diagnostics() {
    let parser = TreeSitterParser::new().expect("parser initialization");
    let mut previous = None;
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("admit corpus");
        for fixture in corpus.fixtures() {
            let source = fixture.source();
            let warm_errors = ErrorCollector::new();
            let (warm, revision) =
                parser.parse_chat_file_revision(source.into(), previous.as_ref(), &warm_errors);
            let cold_errors = ErrorCollector::new();
            let (cold, _) =
                parser.parse_chat_file_streaming_incremental(source, None, &cold_errors);
            assert_eq!(warm, cold, "revision model: {}", fixture.path().display());
            assert_eq!(
                warm_errors.to_vec(),
                cold_errors.to_vec(),
                "revision diagnostics: {}",
                fixture.path().display()
            );
            assert_eq!(revision.source(), source);
            previous = Some(revision);
        }
    }
    assert!(previous.is_some(), "nonempty corpus witness");
    const RESTORED: &str =
        include_str!("../../../../corpus/reference/languages/eng-conversation.cha");
    for source in ["", RESTORED, RESTORED] {
        let warm_errors = ErrorCollector::new();
        let (warm, revision) =
            parser.parse_chat_file_revision(source.into(), previous.as_ref(), &warm_errors);
        let cold_errors = ErrorCollector::new();
        let (cold, _) = parser.parse_chat_file_streaming_incremental(source, None, &cold_errors);
        assert_eq!(warm, cold, "delete/restore model");
        assert_eq!(
            warm_errors.to_vec(),
            cold_errors.to_vec(),
            "delete/restore diagnostics"
        );
        // Structural-query callers may edit their detached copy, but cannot
        // alter the owner's next transition, including unchanged-source reuse.
        if let Some(mut detached) = revision.tree() {
            detached.edit(&InputEdit {
                start_byte: 0,
                old_end_byte: 0,
                new_end_byte: 1,
                start_position: Point::new(0, 0),
                old_end_position: Point::new(0, 0),
                new_end_position: Point::new(0, 1),
            });
        }
        previous = Some(revision);
    }
}

/// A raw incremental tree is a caller contract, not evidence of readable ranges.
/// Deliberately violating that contract must not turn source association into
/// unchecked indexing. The old source is a retained reference specimen.
#[test]
fn stale_incremental_tree_does_not_certify_readable_source_ranges() {
    use talkbank_parser::generated_traversal::SourceBindingError;

    let parser = TreeSitterParser::new().expect("parser initialization");
    let source = include_str!("../../../../corpus/reference/languages/eng-conversation.cha");
    let old = parser
        .parse_tree_incremental(source, None)
        .expect("reference tree");
    // Simulate a caller deleting all input but forgetting Tree::edit. This is
    // intentionally not a supported incremental edit; it probes the admission
    // boundary rather than asserting CHAT semantics for the resulting tree.
    let stale = parser
        .parse_source_incremental("", Some(&old))
        .expect("runtime completes");
    assert!(matches!(
        stale.root(),
        Err(SourceBindingError::InvalidRange)
    ));

    let fresh = parser
        .parse_source_incremental("", None)
        .expect("cold empty parse");
    assert_eq!(fresh.root().expect("cold range is readable").text(), "");
}

#[test]
fn incremental_entry_points_preserve_corpus_models_and_diagnostics() {
    let parser = TreeSitterParser::new().expect("parser initialization");
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("admit corpus");
        for fixture in corpus.fixtures() {
            let source = fixture.source();
            let path = fixture.path().display();
            let (cold, tree) = parser.parse_chat_file_incremental(source, None);
            let tree = tree.expect("tree-sitter completes fixture parse");
            let (warm, reused) = parser.parse_chat_file_incremental(source, Some(&tree));
            match (&cold, &warm) {
                (Ok(cold), Ok(warm)) => assert_eq!(cold, warm, "strict model mismatch: {path}"),
                (Err(cold), Err(warm)) => assert_eq!(
                    cold.to_error_vec(),
                    warm.to_error_vec(),
                    "strict diagnostics mismatch: {path}"
                ),
                _ => panic!("strict incremental acceptance mismatch: {path}"),
            }
            assert!(reused.is_some(), "incremental tree missing: {path}");

            let cold_errors = ErrorCollector::new();
            let (cold_file, _) =
                parser.parse_chat_file_streaming_incremental(source, None, &cold_errors);
            let warm_errors = ErrorCollector::new();
            let (warm_file, _) =
                parser.parse_chat_file_streaming_incremental(source, Some(&tree), &warm_errors);
            assert_eq!(cold_file, warm_file, "streaming model mismatch: {path}");
            assert_eq!(
                cold_errors.to_vec(),
                warm_errors.to_vec(),
                "streaming diagnostics mismatch: {path}"
            );
            if let Ok(strict_file) = cold {
                assert_eq!(strict_file, cold_file, "strict/streaming mismatch: {path}");
            }

            // Model an editor newline insertion. The old tree must be edited
            // before reuse; compare with a cold parse of exactly those bytes,
            // including source spans and diagnostic multiplicity/order.
            let row = source.bytes().filter(|byte| *byte == b'\n').count();
            let column = source.rsplit('\n').next().expect("last line").len();
            let mut edited_tree = tree;
            edited_tree.edit(&InputEdit {
                start_byte: source.len(),
                old_end_byte: source.len(),
                new_end_byte: source.len() + 1,
                start_position: Point::new(row, column),
                old_end_position: Point::new(row, column),
                new_end_position: Point::new(row + 1, 0),
            });
            let edited_source = format!("{source}\n");
            let fresh_errors = ErrorCollector::new();
            let (fresh, _) =
                parser.parse_chat_file_streaming_incremental(&edited_source, None, &fresh_errors);
            let edited_errors = ErrorCollector::new();
            let (edited, _) = parser.parse_chat_file_streaming_incremental(
                &edited_source,
                Some(&edited_tree),
                &edited_errors,
            );
            assert_eq!(fresh, edited, "edited model mismatch: {path}");
            assert_eq!(
                fresh_errors.to_vec(),
                edited_errors.to_vec(),
                "edited diagnostics mismatch: {path}"
            );
        }
    }
}
