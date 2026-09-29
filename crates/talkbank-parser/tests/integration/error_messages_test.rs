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

//! Tests to ensure error messages are user-friendly and don't expose parser internals
//!
//! **CRITICAL**: No error message should contain "node 'ERROR'" or expose tree-sitter internals.

use crate::common::{parse_and_collect_errors, parse_validate_and_collect_diagnostics};
use talkbank_model::model::TranscriptName;

/// Tests no error node keyword in messages.
#[test]
fn test_no_error_node_keyword_in_messages() {
    // Test that error messages don't contain "node 'ERROR'"
    // This validates the unexpected_node_error() function works correctly

    // Use various inputs (may or may not produce errors, but if they do, messages must be clean)
    let test_inputs = vec![
        "@UTF8\n@Begin\n*CHI:\thello @ world .\n@End\n", // Lone @
        "@UTF8\n@Begin\n*CHI:\thello & world .\n@End\n", // Lone &
        "@UTF8\n@Begin\n*CHI:\thello [: world .\n@End\n", // Unclosed bracket
    ];

    for input in test_inputs {
        let errors = parse_and_collect_errors(input);

        // Check all errors (whether parse or validation)
        for error in &errors {
            // CRITICAL: No error message should expose "ERROR" node type
            assert!(
                !error.message.contains("node 'ERROR'"),
                "Error message contains 'node 'ERROR'': {}",
                error.message
            );

            // Also check for old-style messages
            assert!(
                !error.message.contains("Unexpected node 'ERROR'"),
                "Error message contains 'Unexpected node 'ERROR'': {}",
                error.message
            );

            // Verify error span is within input bounds
            let span = &error.location.span;
            assert!(
                span.start <= input.len() as u32,
                "Error span start {} exceeds input length {}. Input: {}",
                span.start,
                input.len(),
                input.escape_debug()
            );
            assert!(
                span.end <= input.len() as u32,
                "Error span end {} exceeds input length {}. Input: {}",
                span.end,
                input.len(),
                input.escape_debug()
            );
        }
    }
}

/// Tests error messages dont expose internals.
#[test]
fn test_error_messages_dont_expose_internals() {
    // Test that error messages don't expose tree-sitter/CST internals

    let test_inputs = vec![
        "@UTF8\n@Begin\n*CHI:\thello @ .\n@End\n", // Lone @
        "@UTF8\n@Begin\n*CHI:\thello & .\n@End\n", // Lone &
    ];

    for input in test_inputs {
        let errors = parse_and_collect_errors(input);

        for error in &errors {
            let msg = &error.message;

            // Should not expose tree-sitter ERROR keyword
            assert!(
                !msg.contains("ERROR"),
                "Error message exposes 'ERROR' keyword: {} (input: {})",
                msg,
                input.escape_debug()
            );

            // Messages should not be empty
            assert!(
                !msg.is_empty(),
                "Error message is empty (input: {})",
                input.escape_debug()
            );

            // Should not expose "node" terminology (CST internal)
            assert!(
                !msg.contains("node '"),
                "Error message exposes CST 'node' terminology: {}",
                msg
            );

            // Verify error span is within input bounds
            let span = &error.location.span;
            assert!(
                span.start <= input.len() as u32,
                "Error span start {} exceeds input length {}. Input: {}",
                span.start,
                input.len(),
                input.escape_debug()
            );
            assert!(
                span.end <= input.len() as u32,
                "Error span end {} exceeds input length {}. Input: {}",
                span.end,
                input.len(),
                input.escape_debug()
            );
        }
    }
}

/// Tests error spans point to problem locations.
#[test]
fn test_error_spans_point_to_problem_locations() {
    // Precision comes from actual CST recovery nodes, not a second text scan
    // narrowing an ERROR to the visually suspicious character inside it.
    for input in [
        "@UTF8\n@Begin\n*CHI:\thello @ world .\n@End\n",
        "@UTF8\n@Begin\n*CHI:\thello [: world .\n@End\n",
    ] {
        let parser = talkbank_parser::TreeSitterParser::new().unwrap();
        let parsed = parser.parse_source_incremental(input, None).unwrap();
        let mut pending = vec![parsed.root_node()];
        let mut recovery = Vec::new();
        while let Some(node) = pending.pop() {
            if node.is_error() {
                recovery.push(node.byte_range());
            }
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
        }
        let errors = parse_and_collect_errors(input);
        assert!(!errors.is_empty(), "malformed input must not pass silently");
        assert!(
            errors.iter().any(|error| {
                let range = error.location.span.start as usize..error.location.span.end as usize;
                error.code == talkbank_model::ErrorCode::UnparsableContent
                    && range.contains(&25)
                    && recovery.contains(&range)
            }),
            "diagnostic must retain the producer's recovery span: {errors:?}"
        );
        for error in errors {
            let span = error.location.span;
            assert!(
                input.get(span.start as usize..span.end as usize).is_some(),
                "diagnostic must stay within readable source coordinates"
            );
        }
    }
}

/// A diagnostic caused by a defect on one line must be LOCATED on that line.
///
/// Regression for the IISRP-residue finding (2026-07-30): an unparsable main
/// tier (`&- um and .`, the split filler prefix) produced an E305 whose span
/// was `0..line_len`, i.e. fragment-local coordinates never rebased onto the
/// file, so the diagnostic rendered on line 1 over the header block. The
/// `report_missing_child` recovery used `0..original_input.len()` as its span;
/// every span it reports must lie inside the offending line instead.
#[test]
fn test_recovery_diagnostics_are_located_on_the_offending_line() {
    let header = "@UTF8
@Begin
@Languages:	eng
@Participants:	INV Investigator
@ID:	eng|test|INV|||||Investigator|||
";
    let bad_line = "*INV:	&- um and .
";
    let input = format!(
        "{header}{bad_line}@End
"
    );
    let line_start = header.len() as u32;
    let line_end = (header.len() + bad_line.len()) as u32;

    let errors = parse_and_collect_errors(&input);
    assert!(
        !errors.is_empty(),
        "the split filler prefix must produce diagnostics"
    );
    for error in &errors {
        let span = error.location.span;
        assert!(
            span.start >= line_start && span.end <= line_end,
            "diagnostic located outside the offending line              (span {}..{}, line is {line_start}..{line_end}): {:?}",
            span.start,
            span.end,
            error
        );
    }
}

/// A parse-recovered main tier must not cascade into structural claims
/// about content the parser never saw.
///
/// Regression for the IISRP-residue findings 2 and 4 (2026-07-30):
/// `*INV:\t&- um and .` (the split filler prefix) reported, besides the
/// correct E316, both "Utterance is empty (no content after speaker)"
/// (E306, false: `um and .` is right there) and the model-side E305
/// "Expected terminator not found" (unknowable: the terminator was in
/// the unparsed region). Both are consequences of recovery emptying the
/// content list, so both are gated on the main tier's parse-taint; the
/// E316 alone marks the file invalid.
#[test]
fn test_recovered_main_tier_does_not_cascade_e306_or_model_e305() {
    let header = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tINV Investigator\n@ID:\teng|test|INV|||||Investigator|||\n";
    let input = format!("{header}*INV:\t&- um and .\n@End\n");

    let diagnostics = parse_validate_and_collect_diagnostics(&input, TranscriptName::Anonymous);
    let codes: Vec<&str> = diagnostics.iter().map(|(code, _)| code.as_str()).collect();
    assert!(
        codes.contains(&"E316"),
        "the unparsable content must stay flagged: {codes:?}"
    );
    assert!(
        !codes.contains(&"E306"),
        "E306 'utterance is empty' is a recovery cascade, content exists: {codes:?}"
    );
    let model_e305 = diagnostics
        .iter()
        .filter(|(code, message)| code == "E305" && message.contains("Expected terminator"))
        .count();
    assert_eq!(
        model_e305, 0,
        "the model-side E305 is unknowable on a recovered tier: {diagnostics:?}"
    );
}

/// E747 "blank lines are not allowed" must fire only on lines that are
/// actually blank.
///
/// Regression for IISRP-residue finding 3 (2026-07-30): a speaker-less
/// main tier (`*:` alone, or `*:` with a bullet) parses under recovery
/// with a `blank_line` node covering just its trailing newline, so the
/// file was told it contains a blank line it does not have. The real
/// finding on such lines is the empty speaker code (E301), which must
/// keep firing; a genuinely blank line must also keep its E747.
#[test]
fn test_e747_fires_only_on_genuinely_blank_lines() {
    let header = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tINV Investigator\n@ID:\teng|test|INV|||||Investigator|||\n";

    // A speaker-less tier: NOT a blank line.
    let input = format!("{header}*INV:\thello .\n*:\n@End\n");
    let codes: Vec<String> = parse_and_collect_errors(&input)
        .into_iter()
        .map(|error| format!("{:?}", error.code))
        .collect();
    assert!(
        codes.iter().any(|code| code == "UnparsableContent"),
        "the malformed tier must stay flagged: {codes:?}"
    );
    assert!(
        !codes.iter().any(|code| code.contains("BlankLine")),
        "no line here is blank: {codes:?}"
    );

    // A genuinely blank line keeps its E747.
    let blank_input = format!("{header}*INV:\thello .\n\n*INV:\tagain .\n@End\n");
    let blank_codes: Vec<String> = parse_and_collect_errors(&blank_input)
        .into_iter()
        .map(|error| format!("{:?}", error.code))
        .collect();
    assert!(
        blank_codes.iter().any(|code| code.contains("BlankLine")),
        "the genuine blank line must keep E747: {blank_codes:?}"
    );
}
