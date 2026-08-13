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

//! Tests for `ChatFile::validate_headers_only`, the LSP's entry point.
//!
//! This path existed with NO test coverage, which is how E767 shipped
//! invisible in the editor for a day: it was implemented as a file-level sweep
//! that `validate_headers_only` never runs, so a rule that fired correctly in
//! `chatter validate` produced nothing in a VS Code buffer, and no gate said
//! so. Header-payload rules now live on `check_header`, which both entry
//! points call, and these tests hold that.
//!
//! The property is deliberately stated as "a header-payload rule fires from
//! BOTH entry points", not "E767 fires", so the next header rule that gets
//! written as a file-level sweep fails here rather than in an editor.

use talkbank_model::model::TranscriptName;
use talkbank_model::{ErrorCode, ErrorCollector};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// `@Media` with a space before the comma: invalid, and E767 names it.
const WHITESPACE_BEFORE_COMMA: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
    @Participants:\tCHI Target_Child\n\
    @ID:\teng|corpus|CHI|||||Target_Child|||\n\
    @Media:\trecording , audio\n\
    *CHI:\thello .\n@End\n";

/// Parses, keeping the model regardless of diagnostics.
fn parse(source: &str) -> Result<talkbank_model::model::ChatFile, TestError> {
    Ok(TreeSitterParser::new()?
        .parse_chat_file(source)
        .expect_built())
}

/// Collects the codes a validation closure reports.
fn codes_from(run: impl FnOnce(&ErrorCollector)) -> Vec<ErrorCode> {
    let errors = ErrorCollector::new();
    run(&errors);
    errors.into_vec().into_iter().map(|e| e.code).collect()
}

/// A header-payload rule must fire from the headers-only entry point, which is
/// what the LSP calls, and not only from full file validation.
#[test]
fn header_payload_rules_fire_from_validate_headers_only() -> Result<(), TestError> {
    let file = parse(WHITESPACE_BEFORE_COMMA)?;

    let headers_only = codes_from(|errors| {
        file.validate_headers_only(errors, TranscriptName::Anonymous);
    });
    assert!(
        headers_only.contains(&ErrorCode::MediaWhitespaceBeforeComma),
        "E767 must reach the editor: validate_headers_only reported {headers_only:?}"
    );
    Ok(())
}

/// The two entry points must agree about a header-payload rule. If they
/// diverge, one of the two audiences (CLI users, editor users) silently loses
/// the rule, which is exactly the defect this file exists to prevent.
#[test]
fn both_entry_points_report_the_same_header_rule() -> Result<(), TestError> {
    let file = parse(WHITESPACE_BEFORE_COMMA)?;

    let headers_only = codes_from(|errors| {
        file.validate_headers_only(errors, TranscriptName::Anonymous);
    });
    let full = codes_from(|errors| {
        file.validate(errors, TranscriptName::Anonymous);
    });

    assert!(
        headers_only.contains(&ErrorCode::MediaWhitespaceBeforeComma)
            && full.contains(&ErrorCode::MediaWhitespaceBeforeComma),
        "both paths must name the rule: headers_only={headers_only:?} full={full:?}"
    );
    Ok(())
}

/// Moving the rules onto the per-header dispatcher must not make full
/// validation report them twice: `run_validation_checks` loops `check_header`
/// AND used to run its own media sweep.
#[test]
fn full_validation_reports_the_rule_exactly_once() -> Result<(), TestError> {
    let file = parse(WHITESPACE_BEFORE_COMMA)?;
    let full = codes_from(|errors| {
        file.validate(errors, TranscriptName::Anonymous);
    });

    let count = full
        .iter()
        .filter(|c| **c == ErrorCode::MediaWhitespaceBeforeComma)
        .count();
    assert_eq!(count, 1, "expected exactly one E767, got {full:?}");
    Ok(())
}
