//! Shared helpers for `talkbank-parser` integration tests.
//!
//! Cargo treats `tests/common/mod.rs` as a regular module (not its own
//! test binary), so each test file in `tests/` can `mod common;` and
//! pull these in without paying the per-binary parse cost.
//!
//! Each integration-test binary compiles this module INDEPENDENTLY, so a
//! helper used by one binary registers as dead code in every binary that
//! does not call it; the allow below silences that structural false
//! positive (it is not a license to keep genuinely unused helpers: a
//! helper no test file references at all should be deleted).
#![allow(dead_code)]

use talkbank_model::model::{Line, Utterance};
use talkbank_model::{ErrorCollector, ParseError, ParseOutcome};
use talkbank_parser::TreeSitterParser;

/// One diagnostic as surfaced at the streaming boundary:
/// `(code, span start, span end, message)`.
pub type DiagRecord = (String, u32, u32, String);

/// Parse `input` through `TreeSitterParser::parse_chat_file_streaming`
/// and return every collected diagnostic.
pub fn parse_and_collect_errors(input: &str) -> Vec<ParseError> {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let _ = parser.parse_chat_file_streaming(input, &errors);
    errors.into_vec()
}

/// Parse `input` as a CHAT file fragment and then run typed-model validation,
/// returning every (error code, message) pair from both phases.
///
/// Uses `parse_chat_file_fragment` with offset 0 (equivalent to streaming).
/// On `ParseOutcome::Rejected`, panics with a diagnostic that includes the
/// input, so callers can pin "must not regress to Rejected" via the panic.
///
/// The `corpus_tag` is a label attached to validation diagnostics; it does not
/// affect which errors are emitted.
pub fn parse_validate_and_collect_diagnostics(
    input: &str,
    corpus_tag: Option<&str>,
) -> Vec<(String, String)> {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let parse_errors = ErrorCollector::new();
    let outcome = parser.parse_chat_file_fragment(input, 0, &parse_errors);
    let mut collected = parse_errors.into_vec();
    match outcome {
        ParseOutcome::Parsed(mut chat_file) => {
            let validation_errors = ErrorCollector::new();
            chat_file.validate_with_alignment(&validation_errors, corpus_tag);
            collected.extend(validation_errors.into_vec());
        }
        ParseOutcome::Rejected => {
            panic!("input must parse into a structured node, but the parser rejected it: {input}");
        }
    }
    collected
        .iter()
        .map(|e| (e.code.as_str().to_string(), e.message.clone()))
        .collect()
}

/// Parse `input` at the real streaming boundary, returning the parsed utterances
/// (cloned out of the line list) and every collected diagnostic as
/// `(code, start, end, message)` tuples.
pub fn parse_utterances_and_diags(input: &str) -> (Vec<Utterance>, Vec<DiagRecord>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &errors);
    let utterances = file
        .lines
        .0
        .iter()
        .filter_map(|line| match line {
            Line::Utterance(utt) => Some(utt.as_ref().clone()),
            Line::Header { .. } => None,
        })
        .collect();
    let diags = errors
        .into_vec()
        .into_iter()
        .map(|d| {
            (
                d.code.as_str().to_string(),
                d.location.span.start,
                d.location.span.end,
                d.message,
            )
        })
        .collect();
    (utterances, diags)
}
