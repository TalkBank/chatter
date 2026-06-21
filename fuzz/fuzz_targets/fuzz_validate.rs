//! Fuzz target for validation
//!
//! Tests that parsing + validation never panics.

#![no_main]

use libfuzzer_sys::fuzz_target;
use talkbank_parser::TreeSitterParser;
use talkbank_model::ErrorCollector;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        let parser = match TreeSitterParser::new() {
            Ok(parser) => parser,
            Err(err) => {
                eprintln!("Parser init failed: {}", err);
                return;
            }
        };

        // Parse first
        if let Ok(chat_file) = parser.parse_chat_file(input) {
            // Then validate - should never panic
            let errors = ErrorCollector::new();
            chat_file.validate(&errors, None);
        }
    }
});
