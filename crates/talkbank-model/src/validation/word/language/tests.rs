//! The one test of word-language resolution that builds nothing: the
//! mapping from a resolution to its validation tag.
//!
//! Every resolution CASE (which marker under which tier and header resolves
//! to what, with which diagnostic) is a parse-backed row in
//! `talkbank-parser-tests/tests/integration/word_language_from_source.rs`,
//! where the parser builds the word from the document that names it. The
//! tests that lived here resolved hand-built words with a marker set beside
//! the raw text and a tier language beside a declared list, the input stated
//! four times, and moved out on 2026-09-09: thirteen of them, one row each.

use super::LanguageResolution;
use crate::model::{LanguageCode, ValidationTag, ValidationTagged};

/// Builds `LanguageCode` values for test fixtures.
fn codes(list: &[&str]) -> Vec<LanguageCode> {
    list.iter()
        .map(|code| LanguageCode::new(*code).expect("test fixture codes are non-empty"))
        .collect()
}

/// Short helper for constructing one `LanguageCode`.
fn lc(code: &str) -> LanguageCode {
    LanguageCode::new(code).expect("test fixture codes are non-empty")
}

/// Language-resolution variants map to expected validation tags.
///
/// Resolved variants are clean, while unresolved state is treated as an error.
#[test]
fn test_language_resolution_validation_tags() {
    assert_eq!(
        LanguageResolution::Single(lc("eng")).validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        LanguageResolution::Multiple(codes(&["eng", "spa"])).validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        LanguageResolution::Ambiguous(codes(&["eng", "spa"])).validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        LanguageResolution::Unresolved.validation_tag(),
        ValidationTag::Error
    );
}
