//! Error corpus generation infrastructure.
//!
//! Programmatically generates test files for all error codes to ensure 100% coverage.
//! Uses [`ChatFileBuilder`](crate::ChatFileBuilder) to create valid CHAT files with
//! specific errors for validation testing.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

// Validation-error generators (E2xx word, E4xx/E6xx tier, E5xx header, E7xx
// alignment) were retired once `spec/errors/` became the single source of truth
// for validation tests (see `tests/error_corpus/validation_errors/README.md`).
// Only the parse-error and warning generators remain, feeding
// `tests/error_corpus/parse_errors/` and `tests/error_corpus/warnings/`.
pub mod internal_errors;
pub mod parser_errors;
pub mod warnings;

use std::fs;
use std::path::Path;

pub use internal_errors::generate_e0_e1xx_internal_errors;
pub use parser_errors::generate_e3xx_parser_errors;
pub use warnings::generate_wxxx_warnings;

/// Convenience type alias used by all generator functions.
pub type GenResult = Result<usize, Box<dyn std::error::Error>>;

/// Updates file.
pub fn write_file(path: &Path, content: String) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(path, content)?;
    let fallback = path.to_string_lossy();
    let name = match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => fallback.as_ref(),
    };
    println!("  Generated: {}", name);
    Ok(())
}
