// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! `TreeSitterParser` must implement the shared `ChatParser` trait.
//!
//! The trait (`talkbank_model::ChatParser`) is the parser-agnostic CHAT
//! parsing API: downstream consumers select a backend (tree-sitter
//! natively, re2c on wasm) behind a single generic bound. `Re2cParser`
//! has implemented it from the start; this test pins the contract that
//! the canonical tree-sitter parser is usable through the SAME trait,
//! so no consumer ever needs a cfg-gated facade to switch backends.
//!
//! Every call below goes through a generic `P: ChatParser` bound, never
//! through `TreeSitterParser`'s inherent methods: the point under test
//! is the trait surface itself.

use talkbank_model::{ChatParser, ErrorCollector, ParseOutcome};
use talkbank_parser::TreeSitterParser;

/// A minimal valid CHAT document exercising the file-level entry point.
const VALID_CHAT_FILE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n@End\n";

/// Parse a fragment through the trait and assert it produced semantic
/// output with no reported errors. Returns the parsed value.
fn parse_clean<T>(what: &str, outcome: ParseOutcome<T>, errors: ErrorCollector) -> T {
    let reported = errors.into_vec();
    assert!(
        reported.is_empty(),
        "{what}: expected no diagnostics, got {reported:?}"
    );
    match outcome {
        ParseOutcome::Parsed(value) => value,
        ParseOutcome::Rejected => panic!("{what}: expected Parsed, got Rejected"),
    }
}

/// The generic battery: exercises the trait's main granularities on
/// valid inputs. Instantiated with `TreeSitterParser`; the re2c parser
/// runs the same trait in its own crate's tests and the equivalence
/// suite cross-checks semantics corpus-wide.
fn exercise_chat_parser<P: ChatParser>(parser: &P) {
    // File level.
    let errors = ErrorCollector::new();
    let file = parse_clean(
        "parse_chat_file",
        parser.parse_chat_file(VALID_CHAT_FILE, 0, &errors),
        errors,
    );
    assert_eq!(
        file.utterances().count(),
        1,
        "parse_chat_file: expected exactly one utterance"
    );

    // Header level.
    let errors = ErrorCollector::new();
    parse_clean(
        "parse_header",
        parser.parse_header("@Languages:\teng\n", 0, &errors),
        errors,
    );
    let errors = ErrorCollector::new();
    parse_clean(
        "parse_id_header",
        parser.parse_id_header("@ID:\teng|corpus|CHI|||||Target_Child|||\n", 0, &errors),
        errors,
    );
    let errors = ErrorCollector::new();
    parse_clean(
        "parse_participant_entry",
        parser.parse_participant_entry("CHI Target_Child", 0, &errors),
        errors,
    );

    // Utterance / tier level.
    let errors = ErrorCollector::new();
    parse_clean(
        "parse_main_tier",
        parser.parse_main_tier("*CHI:\thello .\n", 0, &errors),
        errors,
    );
    let errors = ErrorCollector::new();
    parse_clean(
        "parse_utterance",
        parser.parse_utterance("*CHI:\thello .\n", 0, &errors),
        errors,
    );

    // Token level.
    let errors = ErrorCollector::new();
    parse_clean("parse_word", parser.parse_word("hello", 0, &errors), errors);

    // Morphology tiers (content WITHOUT the `%tier:` prefix).
    let errors = ErrorCollector::new();
    let mor = parse_clean(
        "parse_mor_tier",
        parser.parse_mor_tier("co|hello .", 0, &errors),
        errors,
    );
    assert!(
        !mor.items().is_empty(),
        "parse_mor_tier: expected at least one item"
    );
    let errors = ErrorCollector::new();
    parse_clean(
        "parse_mor_word",
        parser.parse_mor_word("co|hello", 0, &errors),
        errors,
    );
    let errors = ErrorCollector::new();
    parse_clean(
        "parse_gra_tier",
        parser.parse_gra_tier("1|0|ROOT", 0, &errors),
        errors,
    );
    let errors = ErrorCollector::new();
    parse_clean(
        "parse_gra_relation",
        parser.parse_gra_relation("1|0|ROOT", 0, &errors),
        errors,
    );

    // A free-text dependent tier, plus the prefixed dispatcher.
    let errors = ErrorCollector::new();
    parse_clean(
        "parse_com_tier",
        parser.parse_com_tier("a comment .", 0, &errors),
        errors,
    );
    let errors = ErrorCollector::new();
    parse_clean(
        "parse_dependent_tier",
        parser.parse_dependent_tier("%com:\ta comment .\n", 0, &errors),
        errors,
    );
}

#[test]
fn tree_sitter_parser_implements_chat_parser_trait() {
    let parser = TreeSitterParser::new().expect("grammar failed to load");
    exercise_chat_parser(&parser);
}

/// Regression: `parse_gra_relation` on a bare valid relation must parse
/// cleanly. Before 2026-07-24 the fragment wrapper appended a scaffold
/// terminator with index 0 (`0|0|PUNCT`), which tripped E709 inside the
/// wrapper: that both leaked a scaffold diagnostic to the caller's sink
/// and rejected EVERY single-relation fragment outright, while the re2c
/// backend parsed the same input cleanly. The scaffold must stay inert
/// for relations at any index, not just index 1.
#[test]
fn gra_relation_fragment_parses_bare_relations_without_scaffold_leak() {
    let parser = TreeSitterParser::new().expect("grammar failed to load");
    for (relation, index, head) in [("1|0|ROOT", 1, 0), ("5|3|OBJ", 5, 3)] {
        let errors = ErrorCollector::new();
        let outcome = ChatParser::parse_gra_relation(&parser, relation, 0, &errors);
        let parsed = parse_clean(
            &format!("parse_gra_relation({relation:?})"),
            outcome,
            errors,
        );
        assert_eq!(
            (parsed.index, parsed.head),
            (index, head),
            "returned relation should be the caller's, not the scaffold's"
        );
    }
}

#[test]
fn tree_sitter_parser_reports_its_name_via_trait() {
    let parser = TreeSitterParser::new().expect("grammar failed to load");
    let name = ChatParser::parser_name(&parser);
    assert!(
        name.to_ascii_lowercase().contains("tree"),
        "parser_name should identify the tree-sitter backend, got {name:?}"
    );
}
