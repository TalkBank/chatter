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
use talkbank_model::{ChatParser, ParseOutcome, SemanticDiff, SemanticEq, Severity};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

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
fn equivalence_reference_corpus() {
    let corpus = talkbank_parser_tests::chat_corpus::ChatCorpus::reference()
        .expect("complete reference corpus");

    let (ts, re2c) = both_parsers();

    let mut total = 0;
    let mut passed = 0;
    let mut failed_files = Vec::new();

    for fixture in corpus.fixtures() {
        total += 1;
        let content = fixture.source();
        let filename = fixture
            .path()
            .file_name()
            .expect("fixture filename")
            .to_string_lossy()
            .to_string();

        // Inspect both outcomes and both sinks. A recovered model with an
        // error is not a valid semantic pass, even if both models agree.
        // Keep every fixture in the inventory; no recovery-output equality
        // or exact diagnostic-set requirement is imposed on either backend.
        let ts_errors = ErrorCollector::new();
        let ts_result = ChatParser::parse_chat_file(&ts, content, 0, &ts_errors);
        let re2c_errors = ErrorCollector::new();
        let re2c_result = re2c.parse_chat_file(content, 0, &re2c_errors);
        let ts_faults = error_codes_of(&ts_errors);
        let re2c_faults = error_codes_of(&re2c_errors);
        if !ts_faults.is_empty() || !re2c_faults.is_empty() {
            failed_files.push(format!(
                "{filename}: valid reference has parse errors: ts {ts_faults:?}, re2c {re2c_faults:?}"
            ));
            continue;
        }

        match (ts_result, re2c_result) {
            (ParseOutcome::Parsed(ts_file), ParseOutcome::Parsed(re2c_file)) => {
                if ts_file.semantic_eq(&re2c_file) {
                    passed += 1;
                } else {
                    failed_files.push(format!(
                        "{filename}: semantic mismatch\n{}",
                        ts_file.semantic_diff(&re2c_file).render()
                    ));
                }
            }
            (ParseOutcome::Rejected, _) => failed_files.push(format!(
                "{filename}: tree-sitter rejected the valid reference"
            )),
            (_, ParseOutcome::Rejected) => {
                failed_files.push(format!("{filename}: re2c rejected the valid reference"))
            }
        }
    }

    eprintln!("\n=== Reference corpus equivalence ===");
    eprintln!("Total: {total}");
    eprintln!("Passed: {passed}");
    eprintln!("Failed: {}", failed_files.len());
    for f in &failed_files {
        eprintln!("  FAIL: {f}");
    }

    assert!(
        failed_files.is_empty(),
        "{} valid reference-corpus file(s) fail error-free acceptance or \
         semantic equality between the backends; full list above.",
        failed_files.len()
    );
    assert_eq!(passed, total, "every admitted reference must be compared");
}

/// Error codes are failure detail, not a cross-backend equality requirement.
fn error_codes_of(errors: &ErrorCollector) -> Vec<String> {
    let mut codes: Vec<String> = errors
        .to_vec()
        .iter()
        .filter(|error| error.severity == Severity::Error)
        .map(|error| error.code.as_str().to_owned())
        .collect();
    codes.sort_unstable();
    codes.dedup();
    codes
}

// ═══════════════════════════════════════════════════════════════
// Per-tier equivalence
// ═══════════════════════════════════════════════════════════════

#[test]
fn equivalence_mor_tier() {
    let (ts, re2c) = both_parsers();
    let re2c_errors = ErrorCollector::new();

    let entries = talkbank_parser_re2c::tests_support::load_fixture("tier_mor");
    // No empty guard: `load_fixture` refuses a fixture that yields no
    // entries, so this loop cannot run zero times and report a pass.

    let mut passed = 0;
    let mut failed = 0;
    for entry in &entries {
        let body = entry.strip_prefix("%mor:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };

        // TreeSitterParser fragment API
        let ts_result = ts.parse_mor_tier_fragment(&input, 0, &re2c_errors);
        let re2c_result = re2c.parse_mor_tier(&input, 0, &re2c_errors);

        match (ts_result, re2c_result) {
            (ParseOutcome::Parsed(ts_tier), ParseOutcome::Parsed(re2c_tier)) => {
                if ts_tier.semantic_eq(&re2c_tier) {
                    passed += 1;
                } else {
                    failed += 1;
                    if failed <= 3 {
                        eprintln!(
                            "MOR MISMATCH: {}",
                            body.chars().take(60).collect::<String>()
                        );
                    }
                }
            }
            _ => {
                failed += 1;
            }
        }
    }
    eprintln!(
        "  %mor equivalence: {passed}/{} passed, {failed} failed",
        entries.len()
    );
}

#[test]
fn equivalence_gra_tier() {
    let (ts, re2c) = both_parsers();
    let re2c_errors = ErrorCollector::new();

    let entries = talkbank_parser_re2c::tests_support::load_fixture("tier_gra");
    // No empty guard: `load_fixture` refuses a fixture that yields no
    // entries, so this loop cannot run zero times and report a pass.

    let mut passed = 0;
    let mut failed = 0;
    for entry in &entries {
        let body = entry.strip_prefix("%gra:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };

        let ts_result = ts.parse_gra_tier_fragment(&input, 0, &re2c_errors);
        let re2c_result = re2c.parse_gra_tier(&input, 0, &re2c_errors);

        match (ts_result, re2c_result) {
            (ParseOutcome::Parsed(ts_tier), ParseOutcome::Parsed(re2c_tier)) => {
                if ts_tier.semantic_eq(&re2c_tier) {
                    passed += 1;
                } else {
                    failed += 1;
                    if failed <= 3 {
                        eprintln!(
                            "GRA MISMATCH: {}",
                            body.chars().take(60).collect::<String>()
                        );
                    }
                }
            }
            _ => {
                failed += 1;
            }
        }
    }
    eprintln!(
        "  %gra equivalence: {passed}/{} passed, {failed} failed",
        entries.len()
    );
}

#[test]
fn equivalence_word() {
    let (ts, re2c) = both_parsers();
    let re2c_errors = ErrorCollector::new();

    let words = ["hello", "ice+cream", "mama@f", "no::", "&-um", "(be)cause"];
    let mut passed = 0;
    let mut failed = 0;
    for w in &words {
        let ts_result = ts.parse_word(w);
        let re2c_result = re2c.parse_word(w, 0, &re2c_errors);

        match (ts_result, re2c_result) {
            (Ok(ts_word), ParseOutcome::Parsed(re2c_word)) => {
                if ts_word.semantic_eq(&re2c_word) {
                    passed += 1;
                } else {
                    failed += 1;
                    eprintln!("WORD MISMATCH: {w}");
                    eprintln!("  ts:   {:?}", ts_word.raw_text());
                    eprintln!("  re2c: {:?}", re2c_word.raw_text());
                }
            }
            (Err(e), ParseOutcome::Parsed(re2c_word)) => {
                failed += 1;
                eprintln!("WORD: ts failed ({e:?}), re2c: {:?}", re2c_word.raw_text());
            }
            (Ok(ts_word), ParseOutcome::Rejected) => {
                failed += 1;
                eprintln!("WORD: ts: {:?}, re2c rejected", ts_word.raw_text());
            }
            _ => {
                failed += 1;
            }
        }
    }
    eprintln!(
        "  word equivalence: {passed}/{} passed, {failed} failed",
        words.len()
    );
}

// ═══════════════════════════════════════════════════════════════
// Offset parameter wiring tests
// ═══════════════════════════════════════════════════════════════
//
// The re2c parser currently produces Span::DUMMY (0,0) for all spans.
// SpanShift::shift_spans_after skips DUMMY spans (by design). So the
// offset parameter is wired through but has no visible effect until real
// byte-offset spans are added to the re2c parser's AST→model conversion.
//
// These tests verify that non-zero offsets don't cause panics or errors,
// and that the parsing results are semantically identical regardless of
// offset (since all spans are currently DUMMY).

/// Verify that parse_chat_file at non-zero offset produces identical
/// semantic content (spans are DUMMY, so shifting is a no-op).
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

/// Both backends must lower a whole MARKER RUN, including the illegal shapes.
///
/// A content item followed by scoped markers is a left-associative chain: each
/// marker scopes over everything to its left, so `dog [* p:w] [/]` and
/// `dog [/] [* p:w]` are different claims. The tree-sitter side folds the chain
/// (`content/marker_chain.rs`); this test is what keeps the oracle honest about
/// whether re2c does the same.
///
/// # Why inline sources rather than a reference-corpus fixture
///
/// The parity oracle runs over `corpus/reference`, which is VALID CHAT by
/// construction. The fold's whole premise is "lower illegal shapes faithfully
/// and let validation judge them", which places every illegal shape
/// permanently outside that corpus's reach. `a [//] [/] a` is exactly such a
/// shape: it round-trips byte for byte and validation rejects it (E377). It
/// cannot live in the reference corpus, and without a case like it here the
/// oracle reports green while the two backends disagree.
#[test]
fn equivalence_marker_chain() {
    let (ts, re2c) = both_parsers();

    // Each is one main tier, wrapped in the minimum legal file.
    let tiers = [
        // Two markers with nothing between them. Ruled an error, and the
        // parsers must still agree on the tree so ONE validation rule can
        // judge it for both.
        "a [//] [/] a .",
        // An annotation before the marker annotates the retraced material.
        "dog [* p:w] [/] dog .",
        // ...and after it, the retrace. Different claim, different tree.
        "dog [/] [* p:w] dog .",
        // The group form of the same distinction.
        "<the dog> [* s:r] [/] the dog .",
        // A retrace over a bare event, six occurrences corpus-wide.
        "I know it &=laughs [//] you were good .",
        // TWO markers after a group. The existing group case below carries one
        // marker, which is exactly the gap that let the group path stay on the
        // old split-at-first-marker algorithm after the word path was folded.
        "<a b> [//] [/] c .",
        // A marker and then an annotation on an event: the interleaving the
        // fold exists to preserve, on the path that partitions markers out.
        "&=laughs [/] [* p:w] a .",
        // A replacement alongside an error code, the dominant attested shape
        // of annotation-before-marker. `classify.rs` hoists replacements out
        // of the run before folding, so this pins that the hoist does not
        // reorder what survives.
        "dog [: cat] [* p:w] [/] dog .",
        // The BRACKETED spelling of a retrace over an event, which is the form
        // that actually dominates in the corpora and reaches the group seed
        // rather than the event seed. Ruled illegal (E378), so like the
        // adjacent-marker case above it can never appear in the reference
        // corpus and only a case here can see the backends disagree.
        "<&=sigh> [/] &=sigh ok .",
        // A pause beside the event, so the group holds two non-word leaves
        // rather than one. Attested verbatim in slabank-data.
        "<(.) &=laughs> [//] ok .",
    ];

    let mut failed = Vec::new();
    for tier in &tiers {
        let content = format!(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
             @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t{tier}\n@End\n"
        );
        let ts_errors = ErrorCollector::new();
        let ts_file = ts.parse_chat_file_streaming(&content, &ts_errors);
        let re2c_errors = ErrorCollector::new();
        match re2c.parse_chat_file(&content, 0, &re2c_errors) {
            ParseOutcome::Parsed(re2c_file) => {
                if !ts_file.semantic_eq(&re2c_file) {
                    failed.push(format!("semantic mismatch: {tier}"));
                }
            }
            ParseOutcome::Rejected => failed.push(format!("re2c rejected, ts parsed: {tier}")),
        }
    }

    assert!(
        failed.is_empty(),
        "the two backends disagree on {} of {} marker chains:\n  {}",
        failed.len(),
        tiers.len(),
        failed.join("\n  ")
    );
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
