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
const MEDIA_BULLETS: &str = include_str!("../../../../corpus/reference/content/media-bullets.cha");
const HEADERS_MEDIA: &str = include_str!("../../../../corpus/reference/core/headers-media.cha");

/// Count admission work and retained storage on real reference headers, rather
/// than assuming that eager range admission is a free performance improvement.
#[test]
fn media_reference_range_admission_preserves_payload_and_counts_work() {
    use talkbank_parser::generated_traversal::{MediaContentsNode, ParsedSource, ReadableSlot};
    for (source, expected_checks) in [(MEDIA_BULLETS, 4), (HEADERS_MEDIA, 7)] {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_talkbank::LANGUAGE.into())
            .unwrap();
        let parsed = ParsedSource::parse(&mut parser, source, None).unwrap();
        let mut seen = 0;
        for slice in parsed.root().unwrap().descendants() {
            let Some(contents) = slice.unwrap().typed::<MediaContentsNode>() else {
                continue;
            };
            seen += 1;
            let selected = contents.extract().unwrap();
            let raw_bytes = std::mem::size_of_val(selected.children());
            let admitted = selected.admit_ranges().unwrap();
            assert_eq!(admitted.checked_range_count(), expected_checks);
            let ReadableSlot::Present(filename) = &admitted.children().child_0.slot else {
                panic!("reference filename")
            };
            for _ in 0..100 {
                assert!(!filename.text().is_empty());
                assert_eq!(filename.source(), source);
            }
            assert_eq!(admitted.checked_range_count(), expected_checks);
            eprintln!(
                "MEDIA RANGE COST checks={expected_checks} raw_inline={raw_bytes} admitted_inline={} admitted_owner={}",
                std::mem::size_of_val(admitted.children()),
                std::mem::size_of_val(&admitted)
            );
        }
        assert_eq!(seen, 1, "one media body in each reference fixture");
    }
}

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
        .as_slice()
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
            r#"Media(MediaHeader { filename: MediaFilename("media-bullets"), media_type: Video, status: None, whitespace_before_comma: None })"#
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
            r#"Media(MediaHeader { filename: MediaFilename("headers-media"), media_type: Video, status: Some(Unlinked), whitespace_before_comma: None })"#
                .to_string(),
        ],
        "@Media filename+type+status must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}
