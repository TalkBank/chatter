//! Fuzz target for parse_chat_file
//!
//! Tests that the parser never panics on arbitrary input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use talkbank_parser::TreeSitterParser;

fuzz_target!(|data: &[u8]| {
    // Try to interpret as UTF-8, skip if invalid
    if let Ok(input) = std::str::from_utf8(data) {
        let parser = match TreeSitterParser::new() {
            Ok(parser) => parser,
            Err(err) => {
                eprintln!("Parser init failed: {}", err);
                return;
            }
        };

        // Should never panic - may return Ok or Err
        let _ = parser.parse_chat_file(input);
    }
});
