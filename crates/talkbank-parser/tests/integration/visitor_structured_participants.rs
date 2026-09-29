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

//! Characterization and source-range admission tests for @Participants.
//!
//! `parse_participants_header` lives in
//! `tree_parsing/header/participants.rs` and is SHARED by both the line path
//! (`header_parser/dispatch/structured.rs`) and the single-line
//! `header_dispatch/parse.rs` API. Both list and entry traversal use generated
//! positional slots. Entry lowering now admits selected ranges before decoding;
//! it retains typed recovery states and never rematches raw token text.
//!
//! This migration is BEHAVIOUR-PRESERVING: the produced `Header` payloads and
//! every diagnostic must stay byte-identical.
//!
//! These tests pin the OBSERVABLE behaviour at the real parser boundary
//! (`parse_chat_file_streaming` -> `ChatFile` + collected diagnostics) on the
//! existing reference fixtures (NOT hand-authored, NOT guessed):
//!
//! - `@Participants` with three entries (`MOT Mother, CHI Target_Child, FAT Father`):
//!   `heb-conversation.cha` -- exercises the outer repeat loop multiple times.
//! - `@Participants` with two entries, one with a multi-word name
//!   (`NAR Narrator Investigator, CHI Target_Child`): `phon-intervals.cha`
//!   -- exercises the name-word path.
//! - `@Participants` with a single entry (`CHI Child`):
//!   `empty-and-minimal.cha` -- no inner repeat iterations.
//!
//! All asserted values were captured by RUNNING the pre-migration parser. The
//! tests PASS on the current code and MUST STAY GREEN after the outer child-access
//! migration.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line};
use talkbank_parser::TreeSitterParser;

// Existing reference-corpus fixtures (NOT hand-authored).
const HEB_CONVERSATION: &str =
    include_str!("../../../../corpus/reference/languages/heb-conversation.cha");
const PHON_INTERVALS: &str = include_str!("../../../../corpus/reference/tiers/phon-intervals.cha");
const EMPTY_AND_MINIMAL: &str =
    include_str!("../../../../corpus/reference/edge-cases/empty-and-minimal.cha");

/// Admission checks every selected range once; retained reads add no checks.
/// Storage figures are layout evidence, not a throughput or peak-memory claim.
#[test]
fn participant_reference_range_admission_preserves_fields_and_counts_work() {
    use talkbank_parser::generated_traversal::{
        AsRawNode, ParsedSource, ParticipantNode, ReadableSlot,
    };
    let mut entries = 0;
    for source in [HEB_CONVERSATION, PHON_INTERVALS, EMPTY_AND_MINIMAL] {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_talkbank::LANGUAGE.into())
            .unwrap();
        let parsed = ParsedSource::parse(&mut parser, source, None).unwrap();
        let foreign = ParsedSource::parse(&mut parser, source, None).unwrap();
        for slice in parsed.root().unwrap().descendants() {
            let Some(entry) = slice.unwrap().typed::<ParticipantNode>() else {
                continue;
            };
            entries += 1;
            assert!(
                foreign.bind(entry.raw_node()).is_err(),
                "equal text is not the same parse owner"
            );
            let selected = entry.extract().unwrap();
            assert!(selected.children().unexpected.is_empty());
            let raw_inline = std::mem::size_of_val(selected.children());
            let raw_groups = selected.children().child_1.slot();
            let raw_heap = std::mem::size_of_val(raw_groups.as_slice());
            let admitted = selected.admit_ranges().unwrap();
            let fields = admitted.children();
            let expected_checks =
                1 + 2 * fields.child_1.slot.len() + usize::from(fields.child_2.slot.is_some());
            assert_eq!(admitted.checked_range_count(), expected_checks);
            let ReadableSlot::Present(speaker) = &fields.code.slot else {
                panic!("speaker")
            };
            for _ in 0..100 {
                assert!(!speaker.text().is_empty());
                assert_eq!(speaker.source(), source);
                for position in &fields.child_1.slot {
                    let ReadableSlot::Present(group) = &position.slot else {
                        panic!("group")
                    };
                    let ReadableSlot::Present(word) = &group.child_1.slot else {
                        panic!("name/role")
                    };
                    assert!(!word.text().is_empty());
                    assert_eq!(word.source(), source);
                }
            }
            assert_eq!(admitted.checked_range_count(), expected_checks);
            eprintln!(
                "PARTICIPANT RANGE COST checks={expected_checks} raw_inline={raw_inline} raw_heap={raw_heap} admitted_inline={} admitted_heap={} admitted_owner={}",
                std::mem::size_of_val(fields),
                std::mem::size_of_val(fields.child_1.slot.as_slice()),
                std::mem::size_of_val(&admitted),
            );
        }
    }
    assert_eq!(
        entries, 6,
        "all authored participant entries must be witnessed"
    );
}

/// Parse `input` at the real streaming boundary and return the `Debug` string of
/// every `Header::Participants` (in document order) plus every collected diagnostic
/// as `(code, message)`.
fn participants_headers_and_diags(input: &str) -> (Vec<String>, Vec<(String, String)>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);
    let headers = chat
        .lines
        .as_slice()
        .iter()
        .filter_map(|l| match l {
            Line::Header { header, .. } if matches!(&**header, Header::Participants { .. }) => {
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

/// `@Participants` with three entries exercises the outer repeat loop (multiple
/// comma-separated participants) and must produce the exact pre-migration payload
/// with zero diagnostics.
#[test]
fn participants_header_three_entries_decodes_to_exact_payload() {
    let (headers, diags) = participants_headers_and_diags(HEB_CONVERSATION);
    assert_eq!(
        headers.len(),
        1,
        "heb-conversation.cha must have exactly one @Participants header: {headers:?}"
    );
    // Captured PRE-MIGRATION by running the parser on this fixture.
    // If this assertion fails before the migration is applied, STOP (BLOCKED).
    assert_eq!(
        headers[0],
        concat!(
            r#"Participants { entries: ParticipantEntries(["#,
            r#"ParticipantEntry { speaker_code: SpeakerCode("MOT"), name: None, role: ParticipantRole("Mother") }, "#,
            r#"ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Target_Child") }, "#,
            r#"ParticipantEntry { speaker_code: SpeakerCode("FAT"), name: None, role: ParticipantRole("Father") }"#,
            r#"]) }"#,
        ),
        "@Participants three-entry fixture must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid three-entry fixture must have zero diags: {diags:?}"
    );
}

/// `@Participants` with two entries, one carrying a multi-word name, exercises the
/// participant_word path and the two-entry outer loop. Must produce the exact
/// pre-migration payload with zero diagnostics.
#[test]
fn participants_header_two_entries_with_name_decodes_to_exact_payload() {
    let (headers, diags) = participants_headers_and_diags(PHON_INTERVALS);
    assert_eq!(
        headers.len(),
        1,
        "phon-intervals.cha must have exactly one @Participants header: {headers:?}"
    );
    // Captured PRE-MIGRATION by running the parser on this fixture.
    assert_eq!(
        headers[0],
        concat!(
            r#"Participants { entries: ParticipantEntries(["#,
            r#"ParticipantEntry { speaker_code: SpeakerCode("NAR"), name: Some(ParticipantName("Narrator")), role: ParticipantRole("Investigator") }, "#,
            r#"ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Target_Child") }"#,
            r#"]) }"#,
        ),
        "@Participants two-entry with name must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid two-entry fixture must have zero diags: {diags:?}"
    );
}

/// `@Participants` with a single entry (no outer repeat iterations) must produce
/// the exact pre-migration payload with zero diagnostics.
#[test]
fn participants_header_single_entry_decodes_to_exact_payload() {
    let (headers, diags) = participants_headers_and_diags(EMPTY_AND_MINIMAL);
    assert_eq!(
        headers.len(),
        1,
        "empty-and-minimal.cha must have exactly one @Participants header: {headers:?}"
    );
    // Captured PRE-MIGRATION by running the parser on this fixture.
    assert_eq!(
        headers[0],
        concat!(
            r#"Participants { entries: ParticipantEntries(["#,
            r#"ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }"#,
            r#"]) }"#,
        ),
        "@Participants single-entry must reproduce the pre-migration payload"
    );
    assert!(
        diags.is_empty(),
        "valid single-entry fixture must have zero diags: {diags:?}"
    );
}
