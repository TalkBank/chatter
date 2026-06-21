//! Shared helpers for `talkbank-parser` integration tests.
//!
//! Cargo treats `tests/common/mod.rs` as a regular module (not its own
//! test binary), so each test file in `tests/` can `mod common;` and
//! pull these in without paying the per-binary parse cost.

use talkbank_model::{ErrorCollector, ParseError};
use talkbank_parser::TreeSitterParser;

/// Parse `input` through `TreeSitterParser::parse_chat_file_streaming`
/// and return every collected diagnostic.
pub fn parse_and_collect_errors(input: &str) -> Vec<ParseError> {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let _ = parser.parse_chat_file_streaming(input, &errors);
    errors.into_vec()
}
