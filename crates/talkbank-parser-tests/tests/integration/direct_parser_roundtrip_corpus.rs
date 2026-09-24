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

//! Direct Parser Roundtrip Test on Reference Corpus
//!
//! This test verifies that the TreeSitterParser can roundtrip all reference corpus
//! files: parse → serialize → re-parse → compare with SemanticEq.
//!
//! The complete committed corpus is required; missing evidence is a failure.
//!
//! ## Usage
//!
//! ```bash
//! # Run the roundtrip test
//! cargo test -p talkbank-parser-tests --test integration direct_parser_roundtrip_corpus
//!
//! # Show detailed output for failures
//! cargo test -p talkbank-parser-tests --test integration direct_parser_roundtrip_corpus -- --nocapture
//! ```

use std::path::PathBuf;
use talkbank_model::model::{SemanticEq, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::{ChatCorpus, ChatFixture};

/// Parse a file with TreeSitterParser and roundtrip it.
///
/// Returns Ok(()) if the file roundtrips successfully, or an error message.
fn roundtrip_file(fixture: &ChatFixture, parser: &TreeSitterParser) -> Result<(), String> {
    let path = fixture.path();
    let content = fixture.source();

    // Parse with TreeSitterParser. `strict_parse` reproduces the
    // pre-`ParseProduct` fail-on-any-diagnostic contract: the reference
    // corpus is expected to be clean.
    let chat_file =
        talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(content))
            .map_err(|e| format!("TreeSitterParser failed to parse {}: {}", path.display(), e))?;

    // Serialize back to CHAT
    let serialized = chat_file.to_chat_string();

    // Re-parse the serialized CHAT
    let reparsed =
        talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(&serialized))
            .map_err(|e| {
                format!(
                    "TreeSitterParser failed to re-parse serialized {}: {}",
                    path.display(),
                    e
                )
            })?;

    // Semantic comparison
    if !chat_file.semantic_eq(&reparsed) {
        return Err(format!(
            "Semantic mismatch after roundtrip for {}",
            path.display()
        ));
    }

    Ok(())
}

/// Verifies TreeSitterParser round-trip stability on the reference corpus.
#[test]
fn direct_parser_roundtrip_reference_corpus() {
    let corpus = ChatCorpus::reference().expect("complete reference corpus");
    let files = corpus.fixtures();

    let parser = TreeSitterParser::new().expect("Failed to create TreeSitterParser");

    let mut passed = 0;
    let mut failures: Vec<(PathBuf, String)> = Vec::new();

    for (i, file) in files.iter().enumerate() {
        if (i + 1) % 50 == 0 {
            println!("Progress: {}/{}", i + 1, files.len());
        }

        match roundtrip_file(file, &parser) {
            Ok(()) => passed += 1,
            Err(msg) => {
                eprintln!("✗ {}", msg);
                failures.push((file.path().to_owned(), msg));
            }
        }
    }

    println!();
    println!("=== Direct Parser Roundtrip Summary ===");
    println!("Total files: {}", files.len());
    println!("✓ Passed:    {}", passed);
    println!("✗ Failed:    {}", failures.len());
    println!("=======================================");

    if !failures.is_empty() {
        eprintln!();
        eprintln!("Failed files:");
        for (path, reason) in &failures {
            eprintln!("  {}: {}", path.display(), reason);
        }
        panic!(
            "FAILED: {} of {} files did not pass direct parser roundtrip",
            failures.len(),
            files.len()
        );
    }
}
