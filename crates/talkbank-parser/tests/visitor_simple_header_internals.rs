//! Characterization tests for the simple-scalar header per-kind functions'
//! INTERNAL child-access (Task 2c, the first LEVEL-2 header family).
//!
//! Task 2b moved the header-KIND dispatch onto the generated `classify_header`
//! classifier; each simple-scalar header (`@Date`, `@Warning`, `@Page`,
//! `@Tape Location`, ...) now has its own per-kind function in
//! `header_parser/dispatch/simple.rs`. Task 2c migrates those functions'
//! INTERNAL content access off the `node.kind()`-based
//! `get_required_content_by_kind` / `find_child_by_kind` scan onto the generated,
//! typed, POSITIONAL `extract_<kind>(node).child_2` slot (reached through the
//! `HeaderTraversal` ZST seam established by 2b). It is BEHAVIOUR-PRESERVING: the
//! produced `Header` payloads and every diagnostic must stay byte-identical.
//!
//! These tests pin the OBSERVABLE behaviour at the real parser boundary
//! (`parse_chat_file_streaming` -> `ChatFile` + collected diagnostics). They
//! cover one valid header for every distinct content node type that the 13
//! standard simple headers decode:
//!
//! - `date_contents`           : `@Date`
//! - `free_text`               : `@Location`, `@Activities`, `@Transcriber`,
//!   `@Room Layout`, `@Tape Location`, `@Warning`,
//!   `@Bck`, `@T`, `@Videos`
//! - `page_number`             : `@Page`
//! - `time_duration_contents`  : `@Time Start`, `@Time Duration`
//!
//! All asserted values were captured by RUNNING the pre-migration parser on the
//! existing reference fixtures (NOT hand-authored, NOT guessed). The tests PASS
//! on the current code and MUST STAY GREEN after the child-access migration.
//!
//! Note on the malformed "missing content" path. The pre-migration
//! `get_required_content_by_kind` returned `None` (and each per-kind function
//! then built `Header::Unknown`) only when the positional content child was
//! absent. Empirically this arm is NOT reachable through the streaming boundary:
//! an empty `@Date:` yields a `Present` but empty `date_contents` node (so the
//! happy path fires and produces an empty-valued header), and a more broken
//! simple header is rejected upstream at file level (E316) before its per-kind
//! function runs, so no real input drives the non-`Present` content slot. Like
//! the `thumbnail_header` gap in the 2b characterization, the non-`Present` arm
//! is therefore guarded by the exhaustive `NodeSlot` match plus the whole-corpus
//! gate suites rather than by a streaming-boundary fixture here.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line};
use talkbank_parser::TreeSitterParser;

// Existing reference-corpus fixtures (NOT hand-authored) covering the simple
// scalar headers across every content node type they decode.
const HEADERS_METADATA: &str = include_str!("../../../corpus/reference/core/headers-metadata.cha");
const HEADERS_RECORDING: &str =
    include_str!("../../../corpus/reference/core/headers-recording.cha");
const HEADERS_COMMENTS: &str = include_str!("../../../corpus/reference/core/headers-comments.cha");
const HEADERS_EPISODES: &str = include_str!("../../../corpus/reference/core/headers-episodes.cha");
const HEADERS_TIME_AND_TYPES: &str =
    include_str!("../../../corpus/reference/core/headers-time-and-types.cha");
const HEADERS_MEDIA: &str = include_str!("../../../corpus/reference/core/headers-media.cha");

/// Whether `h` is one of the 13 standard simple-scalar header variants migrated
/// in Task 2c (each parsed through a `dispatch/simple.rs` per-kind function).
/// `@unsupported` (`Header::Unknown`) is intentionally excluded: its body reads
/// the whole node, not a content child, so Task 2c does not migrate it.
fn is_simple_scalar(h: &Header) -> bool {
    matches!(
        h,
        Header::Date { .. }
            | Header::Location { .. }
            | Header::Activities { .. }
            | Header::Transcriber { .. }
            | Header::RoomLayout { .. }
            | Header::TapeLocation { .. }
            | Header::Warning { .. }
            | Header::Page { .. }
            | Header::Bck { .. }
            | Header::T { .. }
            | Header::TimeDuration { .. }
            | Header::TimeStart { .. }
            | Header::Videos { .. }
    )
}

/// Parse `input` at the real streaming boundary and return the `Debug` string of
/// every simple-scalar header (in document order) plus every collected
/// diagnostic as `(code, message)`.
fn simple_headers_and_diags(input: &str) -> (Vec<String>, Vec<(String, String)>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);
    let headers = chat
        .lines
        .0
        .iter()
        .filter_map(|l| match l {
            Line::Header { header, .. } if is_simple_scalar(header) => Some(format!("{header:?}")),
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

/// `@Date` (`date_contents`) and the free-text `@Location` / `@Activities` /
/// `@Transcriber` decode to their exact typed payloads with zero diagnostics.
#[test]
fn metadata_simple_headers_decode_to_exact_payloads() {
    let (headers, diags) = simple_headers_and_diags(HEADERS_METADATA);
    assert_eq!(
        headers,
        vec![
            r#"Date { date: Valid { day: 28, month: Jul, year: 2001, raw: "28-JUL-2001" } }"#
                .to_string(),
            r#"Location { location: LocationDescription("Pittsburgh, PA") }"#.to_string(),
            r#"Activities { activities: ActivitiesDescription("block play, doll play") }"#
                .to_string(),
            r#"Transcriber { transcriber: TranscriberName("chris harris, mike jackson") }"#
                .to_string(),
        ],
        "simple-header content access must reproduce the pre-migration payloads"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}

/// The free-text `@Room Layout` and `@Tape Location` decode to their exact typed
/// payloads with zero diagnostics.
#[test]
fn recording_simple_headers_decode_to_exact_payloads() {
    let (headers, diags) = simple_headers_and_diags(HEADERS_RECORDING);
    assert_eq!(
        headers,
        vec![
            r#"RoomLayout { layout: RoomLayoutDescription("mother and child seated at table, toys on floor") }"#
                .to_string(),
            r#"TapeLocation { location: TapeLocationDescription("tape 3, side A, counter 145") }"#
                .to_string(),
        ],
        "simple-header content access must reproduce the pre-migration payloads"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}

/// The free-text `@Warning` decodes to its exact typed payload with zero
/// diagnostics.
#[test]
fn warning_header_decodes_to_exact_payload() {
    let (headers, diags) = simple_headers_and_diags(HEADERS_COMMENTS);
    assert_eq!(
        headers,
        vec![
            r#"Warning { text: WarningText("audio quality degrades after minute 15") }"#
                .to_string(),
        ],
        "simple-header content access must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}

/// `@Page` (`page_number`) plus the free-text `@Bck` / `@T` decode to their exact
/// typed payloads with zero diagnostics.
#[test]
fn episode_simple_headers_decode_to_exact_payloads() {
    let (headers, diags) = simple_headers_and_diags(HEADERS_EPISODES);
    assert_eq!(
        headers,
        vec![
            r#"Bck { bck: BackgroundDescription("background noise from traffic outside") }"#
                .to_string(),
            r#"T { text: TDescription("cereal preparation") }"#.to_string(),
            r#"Page { page: PageNumber("42") }"#.to_string(),
        ],
        "simple-header content access must reproduce the pre-migration payloads"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}

/// `@Time Start` and `@Time Duration` (both `time_duration_contents`) decode to
/// their exact typed payloads with zero diagnostics.
#[test]
fn time_simple_headers_decode_to_exact_payloads() {
    let (headers, diags) = simple_headers_and_diags(HEADERS_TIME_AND_TYPES);
    assert_eq!(
        headers,
        vec![
            r#"TimeStart { start: Parsed { hours: 8, minutes: 30, seconds: 31, millis: None, raw: "8:30:31" } }"#
                .to_string(),
            r#"TimeDuration { duration: Parsed { segments: [Range { start: TimeValue { hours: 0, minutes: 17, seconds: 30, millis: None }, end: TimeValue { hours: 0, minutes: 18, seconds: 0, millis: None } }], raw: "17:30-18:00" } }"#
                .to_string(),
        ],
        "simple-header content access must reproduce the pre-migration payloads"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}

/// The free-text `@Videos` decodes to its exact typed payload with zero
/// diagnostics.
#[test]
fn videos_header_decodes_to_exact_payload() {
    let (headers, diags) = simple_headers_and_diags(HEADERS_MEDIA);
    assert_eq!(
        headers,
        vec![r#"Videos { videos: VideoSpec("1a, 1b, 1c") }"#.to_string()],
        "simple-header content access must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}
