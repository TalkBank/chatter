//! Characterization tests for the @Situation and @Types header parsers'
//! INTERNAL child-access (Task 2f, the first STRUCTURED LEVEL-2 family).
//!
//! `parse_situation_header` and `parse_types_header` live in
//! `tree_parsing/header/metadata/` and are SHARED by both the line path
//! (`header_parser/dispatch/structured.rs`) and the single-line
//! `header_dispatch/parse.rs` API. Task 2f migrates each BODY off the
//! `node.kind()`-based `find_child_by_kind` / `find_child_text` scan onto the
//! generated, typed, POSITIONAL `extract_<kind>(node).child_N` slots (reached
//! through a shared `HeaderTraversal` ZST seam in `tree_parsing/header/typed.rs`).
//! It is BEHAVIOUR-PRESERVING: the produced `Header` payloads and every
//! diagnostic must stay byte-identical.
//!
//! These tests pin the OBSERVABLE behaviour at the real parser boundary
//! (`parse_chat_file_streaming` -> `ChatFile` + collected diagnostics) on the
//! existing reference fixtures (NOT hand-authored, NOT guessed):
//!
//! - `@Situation` (`free_text` direct at `child_2`)   : `headers-metadata.cha`
//! - `@Types` (3 flat fields `child_2/4/6`)           : `headers-time-and-types.cha`
//!
//! All asserted values were captured by RUNNING the pre-migration parser. The
//! tests PASS on the current code and MUST STAY GREEN after the child-access
//! migration.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line};
use talkbank_parser::TreeSitterParser;

// Existing reference-corpus fixtures (NOT hand-authored).
const HEADERS_METADATA: &str = include_str!("../../../corpus/reference/core/headers-metadata.cha");
const HEADERS_TIME_AND_TYPES: &str =
    include_str!("../../../corpus/reference/core/headers-time-and-types.cha");

/// Whether `h` is a `@Situation` or `@Types` header (the two structured families
/// migrated in Task 2f).
fn is_situation_or_types(h: &Header) -> bool {
    matches!(h, Header::Situation { .. } | Header::Types(_))
}

/// Parse `input` at the real streaming boundary and return the `Debug` string of
/// every `@Situation` / `@Types` header (in document order) plus every collected
/// diagnostic as `(code, message)`.
fn situation_types_headers_and_diags(input: &str) -> (Vec<String>, Vec<(String, String)>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);
    let headers = chat
        .lines
        .0
        .iter()
        .filter_map(|l| match l {
            Line::Header { header, .. } if is_situation_or_types(header) => {
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

/// The valid `@Situation` (free-text direct at `child_2`) decodes to its exact
/// typed payload with zero diagnostics.
#[test]
fn situation_header_decodes_to_exact_payload() {
    let (headers, diags) = situation_types_headers_and_diags(HEADERS_METADATA);
    assert_eq!(
        headers,
        vec![
            r#"Situation { text: SituationDescription("Free play in the living room with blocks and dolls") }"#
                .to_string(),
        ],
        "@Situation content access must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}

/// The valid 3-field `@Types` (`types_design` / `types_activity` / `types_group`
/// at `child_2/4/6`) decodes to its exact typed payload with zero diagnostics.
#[test]
fn types_header_decodes_to_exact_payload() {
    let (headers, diags) = situation_types_headers_and_diags(HEADERS_TIME_AND_TYPES);
    assert_eq!(
        headers,
        vec![
            r#"Types(TypesHeader { design: DesignType("long"), activity: ActivityType("toyplay"), group: GroupType("TD") })"#
                .to_string(),
        ],
        "@Types content access must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}
