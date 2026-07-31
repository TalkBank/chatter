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

//! Per-file parser validation tests.
//!
//! Each `.cha` file in the reference corpus becomes its own `#[test]`.
//!
//! ## Usage
//!
//! ```bash
//! cargo test -p talkbank-parser-tests parser_equivalence
//! ```

use std::path::PathBuf;

use rstest::rstest;
use talkbank_parser::TreeSitterParser;

/// Test that the parser successfully parses each reference corpus file.
#[rstest]
fn parser_equivalence(#[files("../../corpus/reference/**/*.cha")] path: PathBuf) {
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
