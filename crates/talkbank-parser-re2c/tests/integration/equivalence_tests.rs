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

//! Parser equivalence tests: Re2cParser vs TreeSitterParser.
//!
//! Parse the same input with both parsers and compare output using
//! `SemanticEq` for accepted valid inputs. Recovered models and diagnostic
//! identity are not cross-backend contracts; see the backend compatibility mandate.

use talkbank_model::errors::ErrorCollector;
use talkbank_model::{ChatParser, ParseOutcome, SemanticEq};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

mod comparison;
mod language_precodes;
mod marker_chain;
mod phonological_pauses;
mod reference_fragments;
mod reference_leaf_fragments;
mod reference_typed_fragments;
mod reference_utterances;

use crate::fixture_utils::LineEnding;
use comparison::{ComparisonBatch, compare_parses};

fn both_parsers() -> (TreeSitterParser, Re2cParser) {
    (
        TreeSitterParser::new().expect("tree-sitter grammar loads"),
        Re2cParser::new(),
    )
}

// ═══════════════════════════════════════════════════════════════
// Reference corpus equivalence
// ═══════════════════════════════════════════════════════════════

#[test]
fn equivalence_reference_corpus() -> Result<(), String> {
    let corpus = talkbank_parser_tests::chat_corpus::ChatCorpus::reference()
        .expect("complete reference corpus");

    let (ts, re2c) = both_parsers();

    let mut batch = ComparisonBatch::default();

    for fixture in corpus.fixtures() {
        for ending in LineEnding::ALL {
            let variant = ending.apply(fixture.source());
            let content = variant.as_ref();
            let filename = fixture
                .path()
                .file_name()
                .expect("fixture filename")
                .to_string_lossy()
                .to_string();
            let filename = format!("{filename} ({ending:?})");

            batch.record(
                filename,
                compare_parses(
                    |errors| ChatParser::parse_chat_file(&ts, content, 0, errors),
                    |errors| re2c.parse_chat_file(content, 0, errors),
                ),
            );
        }
    }

    batch.finish()
}

#[test]
fn internal_warning_refuses_reference_model() {
    use talkbank_model::{ErrorCode, ErrorSink, ParseError, Severity};

    let corpus = talkbank_parser_tests::chat_corpus::ChatCorpus::reference()
        .expect("complete reference corpus");
    let fixture = corpus
        .fixtures()
        .first()
        .expect("nonempty reference corpus");
    let (ts, re2c) = both_parsers();
    for failed_backend in ["tree-sitter", "re2c"] {
        let plant_failure = |errors: &ErrorCollector| {
            errors.report(ParseError::at_location(
                ErrorCode::InternalError,
                Severity::Warning,
                talkbank_model::Location::Unlocated,
                "planted internal failure",
            ));
        };
        let comparison = compare_parses(
            |errors| {
                let outcome = ChatParser::parse_chat_file(&ts, fixture.source(), 0, errors);
                if failed_backend == "tree-sitter" {
                    plant_failure(errors);
                }
                outcome
            },
            |errors| {
                let outcome = re2c.parse_chat_file(fixture.source(), 0, errors);
                if failed_backend == "re2c" {
                    plant_failure(errors);
                }
                outcome
            },
        );
        let refusal = comparison.expect_err("internal failure must refuse reference comparison");
        assert!(refusal.contains(failed_backend), "{refusal}");
        assert!(refusal.contains("planted internal failure"), "{refusal}");
    }
}

// ═══════════════════════════════════════════════════════════════
// Per-tier equivalence
// ═══════════════════════════════════════════════════════════════

#[test]
fn equivalence_mor_tier() -> Result<(), String> {
    let (ts, re2c) = both_parsers();

    let entries = crate::fixture_utils::load_fixture("tier_mor");
    // No empty guard: `load_fixture` refuses a fixture that yields no
    // entries, so this loop cannot run zero times and report a pass.

    let mut batch = ComparisonBatch::default();
    for entry in &entries {
        let body = entry.strip_prefix("%mor:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };

        batch.record(
            format!("MOR {body:?}"),
            compare_parses(
                |errors| ChatParser::parse_mor_tier(&ts, &input, 0, errors),
                |errors| ChatParser::parse_mor_tier(&re2c, &input, 0, errors),
            ),
        );
    }
    batch.finish()
}

#[test]
fn equivalence_gra_tier() -> Result<(), String> {
    let (ts, re2c) = both_parsers();

    let entries = crate::fixture_utils::load_fixture("tier_gra");
    // No empty guard: `load_fixture` refuses a fixture that yields no
    // entries, so this loop cannot run zero times and report a pass.

    let mut batch = ComparisonBatch::default();
    for entry in &entries {
        let body = entry.strip_prefix("%gra:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };

        batch.record(
            format!("GRA {body:?}"),
            compare_parses(
                |errors| ChatParser::parse_gra_tier(&ts, &input, 0, errors),
                |errors| ChatParser::parse_gra_tier(&re2c, &input, 0, errors),
            ),
        );
    }
    batch.finish()
}

#[test]
fn equivalence_word() -> Result<(), String> {
    let (ts, re2c) = both_parsers();

    let words = ["hello", "ice+cream", "mama@f", "no::", "&-um", "(be)cause"];
    let mut batch = ComparisonBatch::default();
    for w in &words {
        batch.record(
            format!("word {w:?}"),
            compare_parses(
                |errors| ChatParser::parse_word(&ts, w, 0, errors),
                |errors| ChatParser::parse_word(&re2c, w, 0, errors),
            ),
        );
    }
    batch.finish()
}

// ═══════════════════════════════════════════════════════════════
// Offset parameter wiring tests
// ═══════════════════════════════════════════════════════════════
//
// These tests verify that non-zero offsets don't cause panics or errors,
// with semantic comparison for the file case. They do not check source-span
// identity; the source-provenance and fragment-rebasing suites own that contract.

/// Verify that parse_chat_file at non-zero offset produces identical
/// semantic content. Source locations are outside this smoke check's scope.
#[test]
fn offset_wiring_chat_file_no_panic() {
    let re2c = Re2cParser::new();
    let re2c_errors = ErrorCollector::new();
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI||||Target_Child|||\n*CHI:\thello world .\n@End\n";

    // Parsing at offset 0 and offset 200 should both succeed
    let zero = re2c.parse_chat_file(input, 0, &re2c_errors);
    let shifted = re2c.parse_chat_file(input, 200, &re2c_errors);

    assert!(matches!(zero, ParseOutcome::Parsed(_)));
    assert!(matches!(shifted, ParseOutcome::Parsed(_)));

    // Both should produce semantically equivalent output
    let (ParseOutcome::Parsed(zero_file), ParseOutcome::Parsed(shifted_file)) = (zero, shifted)
    else {
        unreachable!();
    };
    assert!(zero_file.semantic_eq(&shifted_file));
}

/// Verify that parse_word at non-zero offset succeeds.
#[test]
fn offset_wiring_word_no_panic() {
    let re2c = Re2cParser::new();
    let re2c_errors = ErrorCollector::new();

    let zero = re2c.parse_word("hello", 0, &re2c_errors);
    let shifted = re2c.parse_word("hello", 100, &re2c_errors);

    assert!(matches!(zero, ParseOutcome::Parsed(_)));
    assert!(matches!(shifted, ParseOutcome::Parsed(_)));
}

/// Verify that parse_main_tier at non-zero offset succeeds.
#[test]
fn offset_wiring_main_tier_no_panic() {
    let re2c = Re2cParser::new();
    let re2c_errors = ErrorCollector::new();

    let zero = re2c.parse_main_tier("*CHI:\thello .\n", 0, &re2c_errors);
    let shifted = re2c.parse_main_tier("*CHI:\thello .\n", 500, &re2c_errors);

    assert!(matches!(zero, ParseOutcome::Parsed(_)));
    assert!(matches!(shifted, ParseOutcome::Parsed(_)));
}

/// Verify that parse_mor_tier at non-zero offset succeeds.
#[test]
fn offset_wiring_mor_tier_no_panic() {
    let re2c = Re2cParser::new();
    let re2c_errors = ErrorCollector::new();

    let zero = re2c.parse_mor_tier("pro|I v|want .\n", 0, &re2c_errors);
    let shifted = re2c.parse_mor_tier("pro|I v|want .\n", 300, &re2c_errors);

    assert!(matches!(zero, ParseOutcome::Parsed(_)));
    assert!(matches!(shifted, ParseOutcome::Parsed(_)));
}

// ═══════════════════════════════════════════════════════════════
// Error reporting tests
// ═══════════════════════════════════════════════════════════════

/// Verify that parse_chat_file reports errors for malformed input
/// via the ErrorSink (not silently swallowed).
#[test]
fn error_reporting_unhandled_tokens() {
    let re2c = Re2cParser::new();
    let errors = ErrorCollector::new();

    // Input with an unrecognizable line (not @, *, or %)
    let input = "@UTF8\n@Begin\nGARBAGE LINE\n@End\n";
    let result = re2c.parse_chat_file(input, 0, &errors);
    assert!(matches!(result, ParseOutcome::Parsed(_)));

    let error_vec = errors.to_vec();
    assert!(
        !error_vec.is_empty(),
        "malformed input should produce at least one diagnostic, got none"
    );
}

/// Valid marker scope is specified independently of backend recovery shape.
/// Invalid chains are owned by E377/E378 and the one-way error comparison.
#[test]
fn equivalence_marker_chain() -> Result<(), String> {
    marker_chain::valid_reference_scopes()
}

#[test]
fn sin_fragment_preserves_single_token_groups_and_rejects_unclosed_groups() {
    use talkbank_model::model::SinItem;
    let parser = Re2cParser::new();
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(tier) = parser.parse_sin_tier("〔g:toy:hold〕", 0, &errors) else {
        panic!("complete gesture group must parse");
    };
    assert_eq!(tier.items.len(), 1, "the group must not disappear");
    assert!(matches!(&tier.items[0], SinItem::SinGroup(group) if group.len() == 1));
    assert!(
        parser
            .parse_sin_tier("〔g:toy:hold", 0, &errors)
            .is_rejected()
    );
    assert!(
        !errors.into_vec().is_empty(),
        "rejection must retain a diagnostic"
    );
}

/// Both backends place every container, wrapper and word identically.
#[test]
fn equivalence_container_locations() {
    marker_chain::container_locations_agree();
}
