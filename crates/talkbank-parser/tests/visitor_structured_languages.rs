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

//! Characterization tests for the @Languages header parser's INTERNAL child-access
//! (Task 2h, Level-2 PARTIAL structured migration).
//!
//! `parse_languages_header` lives in
//! `tree_parsing/header/metadata/languages.rs` and is SHARED by both the line path
//! (`header_parser/dispatch/structured.rs`) and the single-line
//! `header_dispatch/parse.rs` API. Task 2h migrates the OUTER
//! `find_child_by_kind(node, LANGUAGES_CONTENTS)` call off the raw
//! `node.kind()` scan onto the generated, typed `extract_languages_header(node).child_2`
//! slot (reached through the shared `HeaderTraversal` ZST seam in
//! `tree_parsing/header/typed.rs`). The INNER loop over language codes stays as a
//! `node.kind()` walk (Repeat(Seq) generator limit, documented in `languages.rs`).
//! This migration is BEHAVIOUR-PRESERVING: the produced `Header` payloads and every
//! diagnostic must stay byte-identical.
//!
//! These tests pin the OBSERVABLE behaviour at the real parser boundary
//! (`parse_chat_file_streaming` -> `ChatFile` + collected diagnostics) on the
//! existing reference fixtures (NOT hand-authored, NOT guessed):
//!
//! - `@Languages` with two codes (`eng, spa`):      `multi-language.cha`
//! - `@Languages` with three codes (`eng, hrv, spa`): `linkers-multiple.cha`
//! - `@Languages` with a single code (`eng`):        `empty-and-minimal.cha`
//!
//! All asserted values were captured by RUNNING the pre-migration parser. The
//! tests PASS on the current code and MUST STAY GREEN after the outer child-access
//! migration.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line};
use talkbank_parser::TreeSitterParser;

// Existing reference-corpus fixtures (NOT hand-authored).
const MULTI_LANGUAGE: &str =
    include_str!("../../../corpus/reference/edge-cases/multi-language.cha");
const LINKERS_MULTIPLE: &str =
    include_str!("../../../corpus/reference/content/linkers-multiple.cha");
const EMPTY_AND_MINIMAL: &str =
    include_str!("../../../corpus/reference/edge-cases/empty-and-minimal.cha");

/// Parse `input` at the real streaming boundary and return the `Debug` string of
/// every `Header::Languages` (in document order) plus every collected diagnostic as
/// `(code, message)`.
fn languages_headers_and_diags(input: &str) -> (Vec<String>, Vec<(String, String)>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);
    let headers = chat
        .lines
        .0
        .iter()
        .filter_map(|l| match l {
            Line::Header { header, .. } if matches!(&**header, Header::Languages { .. }) => {
                Some(format!("{header:?}"))
            }
            _ => None,
        })
        .collect();
    let diags = errors
        .into_vec()
        .into_iter()
        .map(|d| (d.code.as_str().to_string(), d.message))
        .collect();
    (headers, diags)
}

/// `@Languages` with two codes exercises the inner repeat loop (the Repeat(Seq)
/// limit path) and must produce the exact pre-migration payload with zero diagnostics.
#[test]
fn languages_header_two_codes_decodes_to_exact_payload() {
    let (headers, diags) = languages_headers_and_diags(MULTI_LANGUAGE);
    // Assert this fixture has exactly one @Languages header.
    assert_eq!(
        headers.len(),
        1,
        "multi-language.cha must have exactly one @Languages header: {headers:?}"
    );
    // Capture PRE-MIGRATION: Header::Languages with codes ["eng", "spa"].
    // If this assertion ever fails before the migration is applied, STOP (BLOCKED).
    // Note: the `codes` field uses the `LanguageCodes` newtype wrapper in Debug output.
    assert_eq!(
        headers[0],
        r#"Languages { codes: LanguageCodes([LanguageCode("eng"), LanguageCode("spa")]) }"#,
        "@Languages eng+spa must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid two-code fixture must have zero diags: {diags:?}"
    );
}

/// `@Languages` with three codes exercises the inner repeat loop (multiple
/// Repeat(Seq) iterations) and must produce the exact pre-migration payload with
/// zero diagnostics.
#[test]
fn languages_header_three_codes_decodes_to_exact_payload() {
    let (headers, diags) = languages_headers_and_diags(LINKERS_MULTIPLE);
    assert_eq!(
        headers.len(),
        1,
        "linkers-multiple.cha must have exactly one @Languages header: {headers:?}"
    );
    // Capture PRE-MIGRATION: Header::Languages with codes ["eng", "hrv", "spa"].
    // Note: the `codes` field uses the `LanguageCodes` newtype wrapper in Debug output.
    assert_eq!(
        headers[0],
        r#"Languages { codes: LanguageCodes([LanguageCode("eng"), LanguageCode("hrv"), LanguageCode("spa")]) }"#,
        "@Languages eng+hrv+spa must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid three-code fixture must have zero diags: {diags:?}"
    );
}

/// `@Languages` with a single code (no inner repeat loop iterations) must produce
/// the exact pre-migration payload with zero diagnostics.
#[test]
fn languages_header_single_code_decodes_to_exact_payload() {
    let (headers, diags) = languages_headers_and_diags(EMPTY_AND_MINIMAL);
    assert_eq!(
        headers.len(),
        1,
        "empty-and-minimal.cha must have exactly one @Languages header: {headers:?}"
    );
    // Capture PRE-MIGRATION: Header::Languages with codes ["eng"].
    // Note: the `codes` field uses the `LanguageCodes` newtype wrapper in Debug output.
    assert_eq!(
        headers[0], r#"Languages { codes: LanguageCodes([LanguageCode("eng")]) }"#,
        "@Languages single code must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid single-code fixture must have zero diags: {diags:?}"
    );
}
