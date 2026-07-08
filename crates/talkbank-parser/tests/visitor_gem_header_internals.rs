// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
)]

//! Characterization tests for the GEM-header per-kind functions' INTERNAL
//! child-access (Task 2e, the third LEVEL-2 header family).
//!
//! Task 2b moved the header-KIND dispatch onto the generated `classify_header`
//! classifier. Task 2e migrates the GEM family's INTERNAL content access off the
//! local `find_child_by_kind(header_actual, FREE_TEXT)` scan onto the generated,
//! typed, POSITIONAL `extract_<kind>(node).child_2` slot (reached through the
//! `HeaderTraversal` ZST seam established by 2b). The GEM family is:
//!
//! - `@Bg` (`bg_header`)  : optional `free_text` child (`BgHeaderChildren.child_2:
//!   Option<FreeTextNode>`), parsed via
//!   `parse_optional_gem_label`. Special bespoke logic:
//!   label-absent AND `header_contains_colon` -> `LazyGem`,
//!   else `BeginGem`.
//! - `@Eg` (`eg_header`)  : optional `free_text` child (`EgHeaderChildren.child_2:
//!   Option<FreeTextNode>`), parsed via
//!   `parse_optional_gem_label` -> `EndGem`.
//! - `@G`  (`g_header`)   : required `free_text` child (`GHeaderChildren.child_2:
//!   NodeSlot<FreeTextNode>`), parsed via
//!   `parse_optional_gem_label` -> `LazyGem`.
//!
//! This is BEHAVIOUR-PRESERVING: the produced `Header` payloads and every
//! diagnostic must stay byte-identical. These tests pin the OBSERVABLE behaviour
//! at the real parser boundary (`parse_chat_file_streaming` -> `ChatFile` +
//! collected diagnostics). All asserted values were captured by RUNNING the
//! pre-migration parser on the existing reference fixtures (NOT hand-authored,
//! NOT guessed). The tests PASS on the current code and MUST STAY GREEN after the
//! child-access migration.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line};
use talkbank_parser::TreeSitterParser;

// Existing reference-corpus fixtures (NOT hand-authored) covering every GEM
// header shape needed: `@Bg` WITH a label, `@Eg` WITH a label, `@Bg`/`@Eg`
// WITHOUT a label (optional-absent path), and `@G` WITH a label.
const POSTCODES_AND_GEMS: &str =
    include_str!("../../../corpus/reference/edge-cases/postcodes-and-gems.cha");
const HEADERS_EPISODES: &str = include_str!("../../../corpus/reference/core/headers-episodes.cha");

/// Whether `h` is one of the 3 GEM header variants migrated in Task 2e.
fn is_gem(h: &Header) -> bool {
    matches!(
        h,
        Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
    )
}

/// Parse `input` at the real streaming boundary and return the `Debug` string of
/// every GEM header (in document order) plus every collected diagnostic as
/// `(code, message)`.
fn gem_headers_and_diags(input: &str) -> (Vec<String>, Vec<(String, String)>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);
    let headers = chat
        .lines
        .0
        .iter()
        .filter_map(|l| match l {
            Line::Header { header, .. } if is_gem(header) => Some(format!("{header:?}")),
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

/// `@Bg` WITH a label and `@Eg` WITH a label: the optional `free_text` child is
/// present, so `parse_optional_gem_label` extracts the `rest_of_line` text and
/// returns it as the `GemLabel`.
#[test]
fn bg_and_eg_with_labels_decode_to_exact_payloads() {
    let (headers, diags) = gem_headers_and_diags(POSTCODES_AND_GEMS);
    assert_eq!(
        headers,
        vec![
            r#"BeginGem { label: Some(GemLabel("morning play")) }"#.to_string(),
            r#"EndGem { label: Some(GemLabel("morning play")) }"#.to_string(),
            r#"BeginGem { label: Some(GemLabel("reading time")) }"#.to_string(),
            r#"EndGem { label: Some(GemLabel("reading time")) }"#.to_string(),
        ],
        "gem-header content access must reproduce the pre-migration payloads"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}

/// `@G` WITH a label (`LazyGem`), `@Bg`/`@Eg` WITH labels, and bare `@Bg`/`@Eg`
/// WITHOUT labels (the optional-absent path -> `label: None`). This covers
/// every GEM variant and the two optional-absent paths.
#[test]
fn g_and_bare_bg_eg_decode_to_exact_payloads() {
    let (headers, diags) = gem_headers_and_diags(HEADERS_EPISODES);
    assert_eq!(
        headers,
        vec![
            r#"LazyGem { label: Some(GemLabel("morning routine")) }"#.to_string(),
            r#"BeginGem { label: Some(GemLabel("afternoon play")) }"#.to_string(),
            r#"EndGem { label: Some(GemLabel("afternoon play")) }"#.to_string(),
            r#"BeginGem { label: None }"#.to_string(),
            r#"EndGem { label: None }"#.to_string(),
        ],
        "gem-header content access must reproduce the pre-migration payloads"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}
