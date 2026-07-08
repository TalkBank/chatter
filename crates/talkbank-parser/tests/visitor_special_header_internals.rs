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

//! Characterization tests for the special-header per-kind functions' INTERNAL
//! child-access (Task 2d, the second LEVEL-2 header family).
//!
//! Task 2b moved the header-KIND dispatch onto the generated `classify_header`
//! classifier; Task 2c migrated the simple-scalar family's content access onto
//! the typed `extract_<kind>(node).child_2` slot. Task 2d migrates the SPECIAL
//! family's INTERNAL content access off the `node.kind()`-based
//! `find_child_by_kind` / `get_required_content_by_kind` / `parse_options_flags`
//! scan onto the generated, typed, POSITIONAL `extract_<kind>(node).child_N`
//! slots (reached through the `HeaderTraversal` ZST seam established by 2b). The
//! special family is more varied than the simple scalars:
//!
//! - `comment`            : `@Comment` -> `parse_bullet_content` on the
//!   `text_with_bullets_and_pics` content child (`child_2`).
//! - `number` /
//!   `recording_quality` /
//!   `transcription`      : single option content child (`child_2`) -> `from_text`.
//! - `birth_of` /
//!   `birthplace_of` /
//!   `l1_of`              : TWO content children (`speaker` at `child_2` + the
//!   value at `child_4`) -> a dual-field `Header`.
//! - `options`            : the `options_contents` content child (`child_2`),
//!   then an inner `option_name` walk -> `ChatOptionFlag`.
//!
//! It is BEHAVIOUR-PRESERVING: the produced `Header` payloads and every
//! diagnostic must stay byte-identical. These tests pin the OBSERVABLE behaviour
//! at the real parser boundary (`parse_chat_file_streaming` -> `ChatFile` +
//! collected diagnostics). All asserted values were captured by RUNNING the
//! pre-migration parser on the existing reference fixtures (NOT hand-authored,
//! NOT guessed). The tests PASS on the current code and MUST STAY GREEN after the
//! child-access migration.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line};
use talkbank_parser::TreeSitterParser;

// Existing reference-corpus fixtures (NOT hand-authored) covering every special
// header shape.
const HEADERS_SPEAKER_INFO: &str =
    include_str!("../../../corpus/reference/core/headers-speaker-info.cha");
const HEADERS_EPISODES: &str = include_str!("../../../corpus/reference/core/headers-episodes.cha");
const HEADERS_RECORDING: &str =
    include_str!("../../../corpus/reference/core/headers-recording.cha");
const HEADERS_METADATA: &str = include_str!("../../../corpus/reference/core/headers-metadata.cha");
const OPTIONS_LONG_FEATURES: &str =
    include_str!("../../../corpus/reference/annotation/long-features.cha");

/// Whether `h` is one of the 8 special header variants migrated in Task 2d.
fn is_special(h: &Header) -> bool {
    matches!(
        h,
        Header::Comment { .. }
            | Header::Number { .. }
            | Header::RecordingQuality { .. }
            | Header::Transcription { .. }
            | Header::Birth { .. }
            | Header::Birthplace { .. }
            | Header::L1Of { .. }
            | Header::Options { .. }
    )
}

/// Parse `input` at the real streaming boundary and return the `Debug` string of
/// every special header (in document order) plus every collected diagnostic as
/// `(code, message)`.
fn special_headers_and_diags(input: &str) -> (Vec<String>, Vec<(String, String)>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);
    let headers = chat
        .lines
        .0
        .iter()
        .filter_map(|l| match l {
            Line::Header { header, .. } if is_special(header) => Some(format!("{header:?}")),
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

/// `@Birth of` (dual `speaker` + `date_contents`), `@Birthplace of` (dual
/// `speaker` + `free_text`), `@L1 of` (dual `speaker` + `language_code`) and
/// `@Comment` (bullet content) decode to their exact typed payloads with zero
/// diagnostics.
#[test]
fn speaker_info_special_headers_decode_to_exact_payloads() {
    let (headers, diags) = special_headers_and_diags(HEADERS_SPEAKER_INFO);
    assert_eq!(
        headers,
        vec![
            r#"Birth { participant: SpeakerCode("CHI"), date: Valid { day: 28, month: Jun, year: 2001, raw: "28-JUN-2001" } }"#.to_string(),
            r#"Birth { participant: SpeakerCode("MOT"), date: Valid { day: 15, month: Mar, year: 1975, raw: "15-MAR-1975" } }"#.to_string(),
            r#"Birthplace { participant: SpeakerCode("MOT"), place: BirthplaceDescription("Taipei, Taiwan") }"#.to_string(),
            r#"L1Of { participant: SpeakerCode("F_A_T"), language: LanguageName("ara") }"#.to_string(),
            r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Speaker info headers: @Birth of, @Birthplace of, @L1 of" })]) } }"#.to_string(),
            r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Constructs: birth_of_header, birthplace_of_header, l1_of_header," }), Continuation, Text(BulletContentText { text: "age_format, multiple @Languages codes, participant with special chars" })]) } }"#.to_string(),
        ],
        "special-header content access must reproduce the pre-migration payloads"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}

/// `@Number` (single `number_option` -> `Number::from_text`) decodes to its exact
/// typed payload with zero diagnostics.
#[test]
fn number_header_decodes_to_exact_payload() {
    let (headers, diags) = special_headers_and_diags(HEADERS_EPISODES);
    assert_eq!(
        headers,
        vec![
            r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Episode and inline headers: @New Episode, @G, @Bg, @Eg, @Bck, @Blank, @Number, @Page, @T" })]) } }"#.to_string(),
            r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Constructs: new_episode_header, g_header, bg_header, eg_header," }), Continuation, Text(BulletContentText { text: "bck_header, blank_header, number_header, page_header, t_header" })]) } }"#.to_string(),
            r#"Number { number: Number2 }"#.to_string(),
        ],
        "special-header content access must reproduce the pre-migration payloads"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}

/// `@Recording Quality` (single `recording_quality_option` ->
/// `RecordingQuality::from_text`) decodes to its exact typed payload with zero
/// diagnostics.
#[test]
fn recording_quality_header_decodes_to_exact_payload() {
    let (headers, diags) = special_headers_and_diags(HEADERS_RECORDING);
    assert_eq!(
        headers,
        vec![
            r#"RecordingQuality { quality: Quality4 }"#.to_string(),
            r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Recording environment headers" })]) } }"#.to_string(),
            r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Constructs: recording_quality_header, recording_quality_option," }), Continuation, Text(BulletContentText { text: "room_layout_header, tape_location_header" })]) } }"#.to_string(),
        ],
        "special-header content access must reproduce the pre-migration payloads"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}

/// `@Transcription` (single `transcription_option` -> `Transcription::from_text`)
/// decodes to its exact typed payload with zero diagnostics.
#[test]
fn transcription_header_decodes_to_exact_payload() {
    let (headers, diags) = special_headers_and_diags(HEADERS_METADATA);
    assert_eq!(
        headers,
        vec![
            r#"Transcription { transcription: EyeDialect }"#.to_string(),
            r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Metadata headers: @Date, @Location, @Situation, @Activities," }), Continuation, Text(BulletContentText { text: "@Transcriber, @Transcription" })]) } }"#.to_string(),
            r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Constructs: date_header, location_header, situation_header," }), Continuation, Text(BulletContentText { text: "activities_header, transcriber_header, transcription_header" })]) } }"#.to_string(),
        ],
        "special-header content access must reproduce the pre-migration payloads"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}

/// `@Options` (the `options_contents` content child + inner `option_name` walk ->
/// `ChatOptionFlag`) decodes to its exact typed payload with zero diagnostics.
#[test]
fn options_header_decodes_to_exact_payload() {
    let (headers, diags) = special_headers_and_diags(OPTIONS_LONG_FEATURES);
    assert_eq!(
        headers,
        vec![
            r#"Options { options: ChatOptionFlags([Ca]) }"#.to_string(),
            r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Long feature spans and nonvocal begin/end markers" })]) } }"#.to_string(),
            r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Constructs: long_feature, long_feature_begin, long_feature_begin_marker," }), Continuation, Text(BulletContentText { text: "long_feature_end, long_feature_end_marker, long_feature_label," }), Continuation, Text(BulletContentText { text: "nonvocal, nonvocal_begin, nonvocal_begin_marker, nonvocal_end," }), Continuation, Text(BulletContentText { text: "nonvocal_end_marker, nonvocal_simple" })]) } }"#.to_string(),
        ],
        "special-header content access must reproduce the pre-migration payloads"
    );
    assert!(
        diags.is_empty(),
        "valid fixture must have zero diags: {diags:?}"
    );
}
