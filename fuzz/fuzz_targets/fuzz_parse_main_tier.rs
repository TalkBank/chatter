//! Fuzz target for parse_main_tier
//!
//! Tests that main tier parsing never panics on arbitrary input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use talkbank_parser::TreeSitterParser;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        let parser = match TreeSitterParser::new() {
            Ok(parser) => parser,
            Err(err) => {
                eprintln!("Parser init failed: {}", err);
                return;
            }
        };

        // Should never panic
        let _ = parser.parse_main_tier(input);
    }
});
