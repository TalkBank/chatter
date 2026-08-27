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

//! Per-file parse check over the reference corpus.
//!
//! Each `.cha` file in the reference corpus becomes its own `#[test]`,
//! asserting that the tree-sitter parser accepts it with no diagnostics.
//!
//! # This is NOT an equivalence test, and it was called one until 2026-08-26
//!
//! It ran, and still runs, exactly ONE parser. The name `parser_equivalence`
//! was repeated as fact in the crate's own module table ("Per-file equivalence
//! on the 74-file reference corpus"), in `CLAUDE.md`'s list of MANDATORY gates
//! for parser work, and in three book pages. The consequence was not cosmetic:
//! the gate a parser change was required to run did not compare the backends
//! at all, so a re2c divergence could pass every mandated check.
//!
//! One did. `&*SPK:word@i` lexed without its form marker for as long as the
//! rule existed, and was found on real transcripts outside the repository rather
//! than by any gate. THE PARITY ORACLE IS
//! `cargo test -p talkbank-parser-re2c --test integration equivalence_reference_corpus`,
//! which parses each file with both backends and compares the models with
//! `SemanticEq`. Run that one when you change a parser.
//!
//! ## Usage
//!
//! ```bash
//! cargo test -p talkbank-parser-tests reference_corpus_parses
//! ```

use std::path::PathBuf;

use rstest::rstest;
use talkbank_parser::TreeSitterParser;

/// Test that the tree-sitter parser accepts each reference corpus file
/// with no diagnostics. Compares nothing; see the module docs.
#[rstest]
fn reference_corpus_parses(#[files("../../corpus/reference/**/*.cha")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));

    let parser = TreeSitterParser::new().expect("TreeSitterParser init");
    // `strict_parse` reproduces the pre-`ParseProduct` fail-on-any-diagnostic
    // contract: the reference corpus is expected to be clean.
    let result = talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(&content));

    assert!(
        result.is_ok(),
        "Parser failed for {}: {:?}",
        path.display(),
        result.err()
    );
}
