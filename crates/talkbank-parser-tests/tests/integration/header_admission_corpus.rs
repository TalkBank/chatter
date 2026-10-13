//! Strict header lexemes win equal-length ties; longer malformed values stay whole.

use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::{
    DateContentsChoice, DateContentsChoiceBoundView, TimeDurationContentsChoice,
    TimeDurationContentsChoiceBoundView,
};
use talkbank_parser_tests::repo_paths::workspace_root;

#[derive(Debug, PartialEq, Eq)]
enum HeaderToken {
    StrictDate,
    GenericDate,
    StrictTime,
    GenericTime,
}

#[test]
fn reference_and_spec_header_tokens_preserve_whole_value_selection() {
    use HeaderToken::{GenericDate, GenericTime, StrictDate, StrictTime};
    let parser = TreeSitterParser::new().expect("parser");
    for (path, expected) in [
        (
            "corpus/reference/core/headers-metadata.cha",
            vec![(StrictDate, "28-JUL-2001")],
        ),
        (
            "corpus/reference/core/headers-time-and-types.cha",
            vec![(StrictTime, "8:30:31"), (StrictTime, "17:30-18:00")],
        ),
        (
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E541_1.cha",
            vec![(GenericTime, "01:23:45.678")],
        ),
        (
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E518_16.cha",
            vec![(GenericDate, "01-JAN-2024x")],
        ),
    ] {
        let source = std::fs::read_to_string(workspace_root().join(path)).expect("canonical input");
        let parsed = parser
            .parse_source_incremental(&source, None)
            .expect("source owner");
        assert!(!parsed.root_node().has_error(), "{path}");
        let mut pending = vec![parsed.root_node()];
        let mut observed = Vec::new();
        while let Some(node) = pending.pop() {
            let (kind, text) = match node.kind() {
                "date_contents" => {
                    let child = node.named_child(0).expect("date alternative");
                    assert_eq!(child.byte_range(), node.byte_range(), "whole date: {path}");
                    let choice = parsed
                        .bind(child)
                        .expect("source binding")
                        .typed::<DateContentsChoice>()
                        .expect("date choice");
                    match choice.view() {
                        DateContentsChoiceBoundView::StrictDate(value) => {
                            (StrictDate, value.text())
                        }
                        DateContentsChoiceBoundView::GenericDate(value) => {
                            (GenericDate, value.text())
                        }
                    }
                }
                "time_duration_contents" => {
                    let child = node.named_child(0).expect("time alternative");
                    assert_eq!(child.byte_range(), node.byte_range(), "whole time: {path}");
                    let choice = parsed
                        .bind(child)
                        .expect("source binding")
                        .typed::<TimeDurationContentsChoice>()
                        .expect("time choice");
                    match choice.view() {
                        TimeDurationContentsChoiceBoundView::StrictTime(value) => {
                            (StrictTime, value.text())
                        }
                        TimeDurationContentsChoiceBoundView::GenericTime(value) => {
                            (GenericTime, value.text())
                        }
                    }
                }
                _ => {
                    let start = pending.len();
                    let mut cursor = node.walk();
                    pending.extend(node.named_children(&mut cursor));
                    pending[start..].reverse();
                    continue;
                }
            };
            observed.push((kind, text));
        }
        assert_eq!(observed, expected, "{path}");
    }
}

#[test]
fn reference_id_languages_retain_structured_list_selection() {
    use talkbank_model::ErrorCollector;
    use talkbank_parser::generated_traversal::{IdLanguagesNode, LanguageCodeNode};
    let parser = TreeSitterParser::new().expect("parser");
    for (reference, expected, expected_ids) in [
        (
            "corpus/reference/core/basic-conversation.cha",
            vec!["eng"],
            2,
        ),
        (
            "corpus/reference/edge-cases/multi-language.cha",
            vec!["eng", "spa"],
            2,
        ),
        (
            "corpus/reference/languages/spa-conversation.cha",
            vec!["spa", "eng"],
            2,
        ),
        (
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E519_15.cha",
            vec!["eng", "spa"],
            1,
        ),
        (
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E519_17.cha",
            vec!["eng", "spa"],
            1,
        ),
    ] {
        let source =
            std::fs::read_to_string(workspace_root().join(reference)).expect("reference source");
        let errors = ErrorCollector::new();
        let (_, parsed) = parser.parse_chat_file_with_source(&source, &errors);
        let parsed = parsed.expect("source owner");
        let mut ids = 0;
        for node in parsed.root().expect("root").descendants() {
            let node = node.expect("reference source range");
            let Some(languages) = node.typed::<IdLanguagesNode>() else {
                continue;
            };
            let codes: Vec<_> = languages
                .descendants()
                .map(|node| node.expect("language source range"))
                .filter_map(|node| node.typed::<LanguageCodeNode>())
                .map(|code| code.text())
                .collect();
            assert_eq!(
                codes, expected,
                "{reference}: preserve grammar-selected code order, not an opaque fallback"
            );
            ids += 1;
        }
        assert_eq!(
            ids, expected_ids,
            "{reference}: every participant ID must be witnessed"
        );
    }
}

/// A readable carrier can still contain recovery: range admission is not validity.
#[test]
fn participant_recovery_repeat_preserves_source_and_valid_siblings() {
    use talkbank_model::{ErrorCode, ErrorCollector, model::Header};
    use talkbank_parser::generated_traversal::{AsRawNode, ParticipantsContentsNode, ReadableSlot};
    use talkbank_parser_tests::test_error::strict_parse;

    let parser = TreeSitterParser::new().expect("parser");
    for (example, expected_faults) in [(2, 0), (3, 1)] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E506_{example}.cha"
        )))
        .expect("authored participant list spec");
        let errors = ErrorCollector::new();
        let (file, parsed) = parser.parse_chat_file_with_source(&source, &errors);
        let parsed = parsed.expect("source owner survives recovery");
        let mut contents_count = 0;
        let mut recovery_ranges = Vec::new();
        for node in parsed.root().expect("root").descendants() {
            let node = node.expect("source range");
            let Some(contents) = node.typed::<ParticipantsContentsNode>() else {
                continue;
            };
            contents_count += 1;
            let children = contents.extract();
            // Consuming range admission must retain the ERROR item rather than
            // either reject readable source or relabel it as a valid group.
            let admitted = children.admit_ranges().expect("readable recovery ranges");
            assert_eq!(admitted.parent().source(), source);
            for item in &admitted.children().child_1.slot {
                if let ReadableSlot::Error(bad) = &item.slot {
                    assert_eq!(bad.text(), ",");
                    recovery_ranges.push(bad.raw_node().byte_range());
                }
            }
        }
        assert_eq!(contents_count, 1, "E506_{example}: real list carrier");
        assert_eq!(recovery_ranges.len(), expected_faults);
        let actual: Vec<_> = file
            .headers()
            .filter_map(|header| match header {
                Header::Participants { entries } => Some(entries),
                _ => None,
            })
            .flatten()
            .map(|entry| (entry.speaker_code.as_str(), entry.role.as_str()))
            .collect();
        let expected = if example == 2 {
            vec![("CHI", "Target_Child")]
        } else {
            vec![("CHI", "Target_Child"), ("MOT", "Mother")]
        };
        assert_eq!(actual, expected, "readable siblings are not dropped");
        let diagnostics = errors.into_vec();
        if expected_faults == 0 {
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            assert!(strict_parse(parser.parse_chat_file(&source)).is_ok());
        } else {
            assert!(strict_parse(parser.parse_chat_file(&source)).is_err());
            for range in recovery_ranges {
                assert!(
                    diagnostics.iter().any(|error| {
                        error.code == ErrorCode::EmptyParticipantsHeader
                            && error.location.span.start as usize == range.start
                            && error.location.span.end as usize == range.end
                    }),
                    "list recovery reports its actual source range: {diagnostics:?}"
                );
            }
        }
    }
}

/// Empty source is an authored syntax fault, not permission to invent a declaration.
#[test]
fn participant_recovery_empty_header_retains_without_invented_entries() {
    use talkbank_model::{ErrorCode, ErrorCollector, model::Header};
    use talkbank_parser::generated_traversal::{AsRawNode, ParticipantNode, ReadableSlot};
    use talkbank_parser_tests::test_error::strict_parse;

    let source = std::fs::read_to_string(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E506_1.cha"),
    )
    .expect("authored empty participant header");
    let parser = TreeSitterParser::new().expect("parser");
    let errors = ErrorCollector::new();
    let (file, parsed) = parser.parse_chat_file_with_source(&source, &errors);
    let parsed = parsed.expect("source owner");
    assert!(parsed.root_node().has_error());
    assert!(strict_parse(parser.parse_chat_file(&source)).is_err());
    assert!(file.headers().all(|header| match header {
        Header::Participants { entries } => entries.is_empty(),
        _ => true,
    }));
    let diagnostics = errors.into_vec();
    let mut missing_speakers = 0;
    for node in parsed.root().expect("root").descendants() {
        let node = node.expect("source range");
        let Some(participant) = node.typed::<ParticipantNode>() else {
            continue;
        };
        let admitted = participant
            .extract()
            .admit_ranges()
            .expect("readable recovery");
        let ReadableSlot::Missing(missing) = &admitted.children().code.slot else {
            panic!("the authored empty header carries a MISSING speaker");
        };
        let range = missing.raw_node().byte_range();
        assert!(range.is_empty(), "missing token is a source point");
        assert!(
            diagnostics.iter().any(|error| {
                error.code == ErrorCode::MissingRequiredElement
                    && error.location.span.start as usize == range.start
                    && error.location.span.end as usize == range.end
            }),
            "missing speaker retains its diagnostic point: {diagnostics:?}"
        );
        missing_speakers += 1;
    }
    assert_eq!(
        missing_speakers, 1,
        "real participant recovery must execute"
    );
    assert!(
        diagnostics
            .iter()
            .all(|error| error.code != ErrorCode::EmptyParticipantCode)
    );
    for error in &diagnostics {
        let span = error.location.span;
        assert!(
            source.get(span.start as usize..span.end as usize).is_some(),
            "diagnostic stays inside its real source: {error:?}"
        );
    }
}
