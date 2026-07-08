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

//! Characterization tests for `parse_header_node`'s header-KIND dispatch
//! migrated onto the generated `classify_header` typed classifier (Task 2b).
//!
//! These tests pin the OBSERVABLE behaviour of header dispatch at the real
//! parser boundary (`parse_chat_file_streaming` -> `ChatFile` + collected
//! diagnostics). The migration replaces `parse_header_node`'s 5-step
//! `node.kind()` string pipeline with one exhaustive typed
//! `match classify_header(node)` over the 34 `HeaderChoice` variants, but it is
//! BEHAVIOUR-PRESERVING (LEVEL 1, the kind-dispatch mechanism only): the per
//! header internal parsing stays byte-identical, so the parsed model and the
//! diagnostics must not change.
//!
//! The single fixture `corpus/reference/core/headers-episodes.cha` is an
//! existing reference-corpus file (NOT hand-authored) that exercises one header
//! of every dispatch family that flows through `parse_header_node`:
//!
//! - marker:     `@New Episode` (new_episode_header), `@Blank` (blank_header)
//! - structured: `@Languages`, `@Participants`, `@ID`
//! - special:    `@Comment` (comment_header), `@Number` (number_header)
//! - GEM:        `@G` (g_header), `@Bg` (bg_header), `@Eg` (eg_header)
//! - simple:     `@Bck` (bck_header), `@Page` (page_header), `@T` (t_header)
//!
//! All asserted values were captured by RUNNING the pre-migration parser on the
//! fixture (not guessed). The test PASSES on the current code and MUST STAY
//! GREEN after the dispatch rewrite.
//!
//! Note on the `thumbnail_header` variant: it is the one `HeaderChoice` variant
//! with no per-kind model logic (the old `ends_with("_header")` fall-through
//! reported `UnknownHeader`). No `.cha` fixture in the corpus produces a
//! `thumbnail_header` node, so it cannot be exercised through this streaming
//! boundary; its explicit arm reproduces the old fall-through diagnostic
//! verbatim and is guarded by the exhaustive `HeaderChoice` match plus the
//! whole-corpus gate suites.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line};
use talkbank_parser::TreeSitterParser;

/// One diagnostic: (code, span start, span end, message).
type Diag = (String, u32, u32, String);

/// Existing reference-corpus fixture covering every header dispatch family.
const HEADERS_EPISODES: &str = include_str!("../../../corpus/reference/core/headers-episodes.cha");

/// A stable tag for each `Header` variant present in the fixture, used to assert
/// line structure without depending on header-payload internals.
fn header_tag(h: &Header) -> &'static str {
    match h {
        Header::Utf8 => "Utf8",
        Header::Begin => "Begin",
        Header::End => "End",
        Header::Languages { .. } => "Languages",
        Header::Participants { .. } => "Participants",
        Header::ID(_) => "ID",
        Header::Comment { .. } => "Comment",
        Header::Number { .. } => "Number",
        Header::Page { .. } => "Page",
        Header::Bck { .. } => "Bck",
        Header::T { .. } => "T",
        Header::NewEpisode => "NewEpisode",
        Header::Blank => "Blank",
        Header::BeginGem { .. } => "BeginGem",
        Header::EndGem { .. } => "EndGem",
        Header::LazyGem { .. } => "LazyGem",
        _ => "Other",
    }
}

/// Render each line as a stable structural tag for order-sensitive assertions.
fn line_tags(chat_lines: &[Line]) -> Vec<String> {
    chat_lines
        .iter()
        .map(|l| match l {
            Line::Header { header, .. } => format!("Header({})", header_tag(header)),
            Line::Utterance(_) => "Utterance".to_string(),
        })
        .collect()
}

/// Parse `input` at the real streaming boundary and return the line tags plus
/// every collected diagnostic as `(code, start, end, message)` tuples.
fn parse_lines_and_diags(input: &str) -> (Vec<String>, Vec<Diag>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);
    let diags = errors
        .into_vec()
        .into_iter()
        .map(|d| {
            (
                d.code.as_str().to_string(),
                d.location.span.start,
                d.location.span.end,
                d.message,
            )
        })
        .collect();
    (line_tags(&chat.lines.0), diags)
}

/// The exact line structure produced by the pre-migration parser for the
/// fixture, captured by RUNNING it. Every entry is a header that flows through
/// `parse_header_node` (plus the three utterances interleaved in the file). The
/// `parse_header_node` -> `classify_header` migration must reproduce this
/// sequence byte-for-byte.
fn expected_line_tags() -> Vec<String> {
    [
        "Header(Utf8)",
        "Header(Begin)",
        "Header(Languages)",    // structured
        "Header(Participants)", // structured
        "Header(ID)",           // structured
        "Header(ID)",           // structured
        "Header(Comment)",      // special
        "Header(Comment)",      // special
        "Header(Bck)",          // simple
        "Header(LazyGem)",      // GEM: @G
        "Utterance",
        "Utterance",
        "Header(T)",          // simple
        "Header(NewEpisode)", // marker
        "Header(Number)",     // special
        "Header(Page)",       // simple
        "Header(BeginGem)",   // GEM: @Bg <label>
        "Utterance",
        "Header(Blank)", // marker
        "Utterance",
        "Header(EndGem)",   // GEM: @Eg <label>
        "Header(BeginGem)", // GEM: @Bg (no label)
        "Utterance",
        "Header(EndGem)", // GEM: @Eg (no label)
        "Header(End)",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

/// The reference fixture parses to the exact pre-migration line structure with
/// ZERO diagnostics. This single assertion pins the dispatch of one header of
/// every family routed through `parse_header_node`: marker (NewEpisode, Blank),
/// structured (Languages, Participants, ID), special (Comment, Number), GEM
/// (LazyGem/@G, BeginGem/@Bg, EndGem/@Eg), and simple (Bck, Page, T). A change
/// in any header's dispatch (wrong `Header` variant, dropped line, or new
/// diagnostic) fails here.
#[test]
fn reference_headers_dispatch_to_expected_structure_with_zero_diagnostics() {
    let (tags, diags) = parse_lines_and_diags(HEADERS_EPISODES);

    assert_eq!(
        tags,
        expected_line_tags(),
        "header dispatch must reproduce the exact pre-migration line structure"
    );
    assert!(
        diags.is_empty(),
        "valid reference fixture must produce zero diagnostics, got: {diags:?}"
    );
}

/// Guard the family coverage explicitly so a future fixture edit that drops a
/// family is caught: each dispatch family must contribute at least its
/// representative header tag. This is redundant with the structural assertion
/// above but documents the intent (one header per dispatch family).
#[test]
fn every_dispatch_family_is_represented() {
    let (tags, _diags) = parse_lines_and_diags(HEADERS_EPISODES);
    for required in [
        "Header(NewEpisode)",   // marker
        "Header(Blank)",        // marker
        "Header(Languages)",    // structured
        "Header(Participants)", // structured
        "Header(ID)",           // structured
        "Header(Comment)",      // special
        "Header(Number)",       // special
        "Header(LazyGem)",      // GEM
        "Header(BeginGem)",     // GEM
        "Header(EndGem)",       // GEM
        "Header(Bck)",          // simple
        "Header(Page)",         // simple
        "Header(T)",            // simple
    ] {
        assert!(
            tags.iter().any(|t| t == required),
            "dispatch-family representative {required} missing from {tags:?}"
        );
    }
}
