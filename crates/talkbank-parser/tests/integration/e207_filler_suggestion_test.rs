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

//! Source recovery for incomplete ampersand markers follows the canonical
//! E207 spec's E316 subsumption policy. No raw-text classifier may manufacture
//! a marker kind or require specialized repair suggestions from an ERROR.

use crate::common::parse_and_collect_errors;

/// Retained legacy input: it stays invalid, without its retired E207 classifier.
const CHAT_WITH_BARE_AMP: &str = "@UTF8\n\
    @Begin\n\
    @Languages:\teng\n\
    @Participants:\tCHI Target_Child\n\
    @ID:\teng|corpus|CHI|||||Target_Child|||\n\
    *CHI:\t&um something .\n\
    @End\n";

#[test]
fn incomplete_ampersand_retains_recovery_and_valid_filler_control() {
    let control = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../talkbank-parser-tests/tests/error_corpus/validation_errors/E207_5.cha"
    ));
    let truncated = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../talkbank-parser-tests/tests/error_corpus/validation_errors/E207_6.cha"
    ));
    assert!(parse_and_collect_errors(control).is_empty());
    for source in [truncated, CHAT_WITH_BARE_AMP] {
        let diagnostics = parse_and_collect_errors(source);
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == talkbank_model::ErrorCode::UnparsableContent),
            "incomplete marker remains invalid: {diagnostics:?}"
        );
        assert!(
            diagnostics
                .iter()
                .all(|d| d.code != talkbank_model::ErrorCode::UnknownAnnotation),
            "an ERROR cannot fabricate typed marker identity: {diagnostics:?}"
        );
    }
}
