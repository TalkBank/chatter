//! Characterization tests for the @Media header parser's INTERNAL child-access
//! (Task 2g, Level-2 structured migration).
//!
//! `parse_media_header` lives in
//! `tree_parsing/header/metadata/media.rs` and is SHARED by both the line path
//! (`header_parser/dispatch/structured.rs`) and the single-line
//! `header_dispatch/parse.rs` API. Task 2g migrates the body off the
//! `node.kind()`-based `find_child_by_kind` / raw positional `child(N)` scan
//! onto the generated, typed, positional `extract_<kind>(node).child_N` slots
//! (reached through the shared `HeaderTraversal` ZST seam in
//! `tree_parsing/header/typed.rs`). It is BEHAVIOUR-PRESERVING: the produced
//! `Header` payloads and every diagnostic must stay byte-identical.
//!
//! These tests pin the OBSERVABLE behaviour at the real parser boundary
//! (`parse_chat_file_streaming` -> `ChatFile` + collected diagnostics) on the
//! existing reference fixtures (NOT hand-authored, NOT guessed):
//!
//! - `@Media` with filename + type only (no status): `media-bullets.cha`
//! - `@Media` with filename + type + status field:   `headers-media.cha`
//!
//! All asserted values were captured by RUNNING the pre-migration parser. The
//! tests PASS on the current code and MUST STAY GREEN after the child-access
//! migration.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line};
use talkbank_parser::TreeSitterParser;

// Existing reference-corpus fixtures (NOT hand-authored).
const MEDIA_BULLETS: &str = include_str!("../../../corpus/reference/content/media-bullets.cha");
const HEADERS_MEDIA: &str = include_str!("../../../corpus/reference/core/headers-media.cha");

/// Whether `h` is a `Header::Media` variant.
fn is_media(h: &Header) -> bool {
    matches!(h, Header::Media(_))
}

/// Parse `input` at the real streaming boundary and return the `Debug` string of
/// every `Header::Media` (in document order) plus every collected diagnostic as
/// `(code, message)`.
fn media_headers_and_diags(input: &str) -> (Vec<String>, Vec<(String, String)>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);
    let headers = chat
        .lines
        .0
        .iter()
        .filter_map(|l| match l {
            Line::Header { header, .. } if is_media(header) => Some(format!("{header:?}")),
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

/// `@Media` with filename + type only (no status field) decodes to its exact
/// typed payload with zero diagnostics.
#[test]
fn media_header_no_status_decodes_to_exact_payload() {
    let (headers, diags) = media_headers_and_diags(MEDIA_BULLETS);
    assert_eq!(
        headers,
        vec![
            r#"Media(MediaHeader { filename: MediaFilename("media-bullets"), media_type: Video, status: None })"#
                .to_string(),
        ],
        "@Media filename+type-only must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}

/// `@Media` with filename + type + status field decodes to its exact typed
/// payload with zero diagnostics.
#[test]
fn media_header_with_status_decodes_to_exact_payload() {
    let (headers, diags) = media_headers_and_diags(HEADERS_MEDIA);
    assert_eq!(
        headers,
        vec![
            r#"Media(MediaHeader { filename: MediaFilename("headers-media"), media_type: Video, status: Some(Unlinked) })"#
                .to_string(),
        ],
        "@Media filename+type+status must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}
