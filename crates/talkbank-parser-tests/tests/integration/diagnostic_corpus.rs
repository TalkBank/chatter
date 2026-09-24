//! User-facing diagnostic rendering over canonical spec inputs.

#![allow(clippy::unwrap_used)] // Fixture assertions may fail loudly; production remains panic-free.

use talkbank_model::model::TranscriptName;
use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{ErrorCollector, RuleSelection, SourceIndex, enhance_errors_with_index};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::repo_paths::workspace_root;

/// Spec controls and single-space deletions retain exact source boundaries at
/// every content depth; serialization restores only the missing separator.
#[test]
fn code_spacing_specs_preserve_nested_sibling_boundaries() {
    use talkbank_model::model::WriteChat;
    use talkbank_model::{ErrorCode, Span};
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (control, following) in [
        (5, "hello"),
        (7, "dat"),
        (9, "again"),
        (11, "there"),
        (13, "hello"),
        (15, "there"),
        (17, "there"),
        (19, "again"),
        (21, "again"),
    ] {
        let clean_source = std::fs::read_to_string(corpus.join(format!("E757_{control}.cha")))
            .expect("canonical spacing control");
        for example in [control, control + 1] {
            let source = std::fs::read_to_string(corpus.join(format!("E757_{example}.cha")))
                .expect("canonical spacing spec");
            let errors = ErrorCollector::new();
            let mut file = parser.parse_chat_file_streaming(&source, &errors);
            assert!(errors.into_vec().is_empty(), "E757_{example}: clean syntax");
            let errors = ErrorCollector::new();
            file.validate_with_alignment(&errors, TranscriptName::Anonymous);
            let diagnostics = errors.into_vec();
            if example == control {
                assert!(diagnostics.is_empty(), "E757_{example}: {diagnostics:?}");
            } else {
                assert_eq!(diagnostics.len(), 1, "E757_{example}: {diagnostics:?}");
                assert_eq!(diagnostics[0].code, ErrorCode::CodeGluedToFollowingContent);
                let start = source
                    .find(&format!("]{following}"))
                    .expect("deleted separator")
                    + 1;
                assert_eq!(diagnostics[0].location.span, Span::from_usize(start, start));
            }
            assert_eq!(
                file.to_chat_string(),
                clean_source,
                "E757_{example}: canonical spacing"
            );
        }
    }
}

/// Role suggestions are advice, never aliases or silent normalization of the
/// participant/ID vocabulary. Canonical source supplies every model here.
#[test]
fn role_specs_preserve_spelling_and_suggest_canonical_labels() {
    use talkbank_model::ErrorCode;
    use talkbank_model::model::WriteChat;
    enum Spelling<'a> {
        Canonical,
        Invalid(&'a [(&'a str, &'a str)]),
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, spelling) in [
        (4, Spelling::Canonical),
        (
            5,
            Spelling::Invalid(&[
                ("Target_child", "Child or Target_Child"),
                ("Mom", "Mother"),
                ("Dad", "Father"),
                ("Adul", "Adult or Target_Adult"),
                ("Teachr", "Teacher"),
                ("Studnt", "Student"),
            ]),
        ),
        (
            6,
            Spelling::Invalid(&[
                ("target_child", "Child or Target_Child"),
                ("mother", "Mother"),
                ("father", "Father"),
                ("adult", "Adult or Target_Adult"),
                ("teacher", "Teacher"),
                ("student", "Student"),
            ]),
        ),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E532_{example}.cha")))
            .expect("canonical role vocabulary spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.into_vec().is_empty(), "E532_{example}: clean syntax");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let diagnostics = errors.into_vec();
        match spelling {
            Spelling::Canonical => assert!(diagnostics.is_empty(), "{diagnostics:?}"),
            Spelling::Invalid(expected) => {
                assert_eq!(
                    diagnostics.len(),
                    2 * expected.len(),
                    "both header occurrences: {diagnostics:?}"
                );
                assert!(
                    diagnostics
                        .iter()
                        .all(|error| error.code == ErrorCode::InvalidParticipantRole)
                );
                for (role, suggested) in expected {
                    let matching: Vec<_> = diagnostics
                        .iter()
                        .filter(|error| {
                            error.message == format!("Invalid participant role: '{role}'")
                        })
                        .collect();
                    assert_eq!(matching.len(), 2, "{role}: both headers");
                    let advice = format!("Use a valid CHAT role such as: {suggested}");
                    assert!(
                        matching
                            .iter()
                            .all(|error| error.suggestion.as_deref() == Some(advice.as_str()))
                    );
                }
            }
        }
        assert_eq!(
            file.to_chat_string(),
            source,
            "validation must preserve original spelling"
        );
    }
}

/// Header policy survives model serialization without treating an empty public
/// constructor value as proof that an empty raw CHAT header is valid.
#[test]
fn duration_specs_preserve_assessment_across_header_and_json_boundaries() {
    use talkbank_model::ErrorCode;
    use talkbank_model::model::{ChatFile, Header, Line, TimeDurationValue, WriteChat};
    use talkbank_parser_tests::test_error::strict_parse;
    let parser = TreeSitterParser::new().expect("parser");
    for example in 1..=9 {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E540_{example}.cha",
        )))
        .expect("canonical duration spec");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("duration spec parses");
        let errors = ErrorCollector::new();
        file.validate_headers_only(&errors, TranscriptName::Anonymous);
        let findings = errors.into_vec();
        assert_eq!(
            findings.len(),
            usize::from(!matches!(example, 6 | 7)),
            "E540_{example}: {findings:?}"
        );
        assert!(
            findings
                .iter()
                .all(|error| error.code == ErrorCode::InvalidTimeDuration)
        );
        assert_eq!(file.to_chat_string(), source);

        // A public model constructor can represent an empty optional value
        // even if raw CHAT parsing rejects the corresponding empty header.
        // Exercise that distinct boundary without inventing a whole AST.
        let mut omitted = file.clone();
        let mut changed = 0;
        for line in omitted.lines.as_mut_slice() {
            if let Line::Header { header, .. } = line
                && let Header::TimeDuration { duration } = header.as_mut()
            {
                *duration = TimeDurationValue::from_text("");
                changed += 1;
            }
        }
        assert_eq!(changed, 1);
        let wire = serde_json::to_string(&omitted).expect("serialize optional value");
        let decoded: ChatFile = serde_json::from_str(&wire).expect("deserialize optional value");
        let errors = ErrorCollector::new();
        decoded.validate_headers_only(&errors, TranscriptName::Anonymous);
        assert!(
            errors.into_vec().is_empty(),
            "existing optional-empty model policy"
        );
        assert_eq!(decoded.to_chat_string(), omitted.to_chat_string());
    }
}

/// Lexical transcription and occupancy of a timed speech interval are distinct.
#[test]
fn untranscribed_timing_specs_constrain_following_speech() {
    use talkbank_model::ErrorCode;
    use talkbank_model::model::WriteChat;
    use talkbank_model::validation::has_transcribed_content;
    use talkbank_parser_tests::test_error::strict_parse;
    let parser = TreeSitterParser::new().expect("parser");
    for example in 1..=4 {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E704_untranscribed_timing_{example}.cha",
        ))).expect("canonical untranscribed timing spec");
        let mut file = strict_parse(parser.parse_chat_file(&source)).expect("timing spec parses");
        let turns: Vec<_> = file.utterances().collect();
        assert_eq!(turns.len(), 3);
        assert_eq!(
            has_transcribed_content(&turns[1].main.content.content),
            example == 4
        );
        let expected_span = turns[2]
            .media_bullet()
            .expect("following timed speech")
            .span;
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let overlaps: Vec<_> = errors
            .into_vec()
            .into_iter()
            .filter(|error| error.code == ErrorCode::SpeakerSelfOverlap)
            .collect();
        assert_eq!(overlaps.len(), 1, "example {example}: {overlaps:?}");
        assert_eq!(overlaps[0].location.span, expected_span);
        assert_eq!(
            file.to_chat_string(),
            source,
            "timing validation never changes transcription"
        );
    }
}

/// Phone interval bounds preserve the rounding policy at both integer limits.
#[test]
fn phone_bounds_specs_preserve_tolerance_and_invalid_interval_evidence() {
    use talkbank_model::ErrorCode;
    use talkbank_model::model::WriteChat;
    use talkbank_parser_tests::test_error::strict_parse;
    let parser = TreeSitterParser::new().expect("parser");
    for example in 2..=7 {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E744_{example}.cha",
        )))
        .expect("canonical phone bounds spec");
        let mut file = strict_parse(parser.parse_chat_file(&source)).expect("phone bounds syntax");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let findings = errors.into_vec();
        let bounds = findings
            .iter()
            .filter(|error| error.code == ErrorCode::XphointMediaBoundsViolation)
            .count();
        let invalid = findings
            .iter()
            .filter(|error| error.code == ErrorCode::XphointBulletInvalid)
            .count();
        assert_eq!(
            bounds,
            usize::from(matches!(example, 2 | 4)),
            "E744_{example}: {findings:?}"
        );
        assert_eq!(
            invalid,
            usize::from(example == 7),
            "E744_{example}: {findings:?}"
        );
        assert_eq!(
            file.to_chat_string(),
            source,
            "invalid timing evidence must not be repaired"
        );
    }
}

/// Reserved codes do not override the adopted default bullet-timing policy.
#[test]
fn reserved_bullet_specs_keep_default_temporal_policy() {
    use talkbank_model::model::WriteChat;
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for code in ["E729", "E730", "E731", "E732"] {
        let source = std::fs::read_to_string(corpus.join(format!("{code}_1.cha")))
            .expect("canonical default-mode bullet control");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.into_vec().is_empty(), "{code}: real bullet syntax");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        assert!(
            errors.into_vec().is_empty(),
            "{code}: default temporal policy"
        );
        assert_eq!(
            file.to_chat_string(),
            source,
            "{code}: unchanged bullet spelling"
        );
    }
}

/// Grammar admission does not promise that a bounded numeric projection exists.
#[test]
fn timed_pause_specs_preserve_unrepresentable_numeric_projections() {
    use talkbank_model::model::{PauseDuration, PauseTimedDuration, UtteranceContent, WriteChat};
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, millis) in [(31, Some(4_294_967_295_123u64)), (32, None), (33, None)] {
        let source = std::fs::read_to_string(corpus.join(format!("E316_{example}.cha")))
            .expect("canonical pause projection spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.into_vec().is_empty(), "E316_{example}: clean syntax");
        let pauses: Vec<_> = file
            .utterances()
            .flat_map(|utterance| {
                utterance
                    .main
                    .content
                    .content
                    .iter()
                    .filter_map(|item| match item {
                        UtteranceContent::Pause(pause) => Some(pause),
                        _ => None,
                    })
            })
            .collect();
        assert_eq!(pauses.len(), 1);
        let PauseDuration::Timed(duration) = &pauses[0].duration else {
            panic!("numeric pause in canonical example");
        };
        assert_eq!(duration.total_millis(), millis);
        assert_eq!(
            matches!(duration, PauseTimedDuration::Parsed { .. }),
            millis.is_some()
        );
        let encoded = serde_json::to_string(duration).expect("pause JSON");
        let decoded: PauseTimedDuration =
            serde_json::from_str(&encoded).expect("pause JSON admission");
        assert_eq!(
            &decoded, duration,
            "wire admission retains projection state and spelling"
        );
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        assert!(
            errors.into_vec().is_empty(),
            "numeric range is not a CHAT syntax restriction"
        );
        assert_eq!(file.to_chat_string(), source);
    }
}

/// Nested commas depend on preceding speech, not later content in the group.
#[test]
fn nested_comma_specs_require_prior_spoken_content_at_the_actual_comma() {
    use talkbank_model::model::WriteChat;
    use talkbank_model::{ErrorCode, Span};
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for example in 9..=12 {
        let source = std::fs::read_to_string(corpus.join(format!("E259_{example}.cha")))
            .expect("canonical nested comma spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.into_vec().is_empty(), "E259_{example}: clean syntax");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let diagnostics = errors.into_vec();
        if matches!(example, 9 | 11) {
            assert!(diagnostics.is_empty(), "E259_{example}: {diagnostics:?}");
        } else {
            assert_eq!(diagnostics.len(), 1, "E259_{example}: {diagnostics:?}");
            assert_eq!(diagnostics[0].code, ErrorCode::CommaAfterNonSpokenContent);
            let comma = source.find(" , ").expect("canonical comma") + 1;
            assert_eq!(
                diagnostics[0].location.span,
                Span::from_usize(comma, comma + 1)
            );
        }
        assert_eq!(
            file.to_chat_string(),
            source,
            "validation does not rewrite commas"
        );
    }
}

/// Numeric date admission is a wire-format rule, not Rust integer syntax.
#[test]
fn date_specs_reject_signed_components_without_rewriting_source() {
    use talkbank_model::ErrorCode;
    use talkbank_model::model::{ChatDate, Header, WriteChat};
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (spec, control, invalid, code) in [
        ("E518", 7, &[8, 9, 10, 11][..], ErrorCode::InvalidDateFormat),
        ("E545", 5, &[6, 7][..], ErrorCode::InvalidBirthDateFormat),
    ] {
        for example in std::iter::once(control).chain(invalid.iter().copied()) {
            let source = std::fs::read_to_string(corpus.join(format!("{spec}_{example}.cha")))
                .expect("canonical date component spec");
            let errors = ErrorCollector::new();
            let mut file = parser.parse_chat_file_streaming(&source, &errors);
            assert!(
                errors.into_vec().is_empty(),
                "{spec}_{example}: clean syntax"
            );
            let dates: Vec<_> = file
                .headers()
                .filter_map(|header| match header {
                    Header::Date { date } if spec == "E518" => Some(date),
                    Header::Birth { date, .. } if spec == "E545" => Some(date),
                    _ => None,
                })
                .collect();
            assert_eq!(dates.len(), 1, "{spec}_{example}: date-bearing header");
            for date in dates {
                assert_eq!(
                    matches!(date, ChatDate::Valid { .. }),
                    example == control,
                    "{spec}_{example}: parsed date admission"
                );
                let encoded = serde_json::to_string(date).expect("date wire value");
                let decoded: ChatDate = serde_json::from_str(&encoded).expect("date wire decoding");
                assert_eq!(
                    &decoded, date,
                    "{spec}_{example}: wire admission and spelling"
                );
            }
            let errors = ErrorCollector::new();
            file.validate_with_alignment(&errors, TranscriptName::Anonymous);
            let diagnostics = errors.into_vec();
            if example == control {
                assert!(diagnostics.is_empty(), "{spec}_{example}: {diagnostics:?}");
            } else {
                assert_eq!(diagnostics.len(), 1, "{spec}_{example}: {diagnostics:?}");
                assert_eq!(diagnostics[0].code, code);
                assert!(diagnostics[0].message.contains("ASCII digits"));
                assert!(
                    diagnostics[0]
                        .suggestion
                        .as_deref()
                        .unwrap()
                        .contains("without a sign")
                );
            }
            assert_eq!(
                file.to_chat_string(),
                source,
                "date validation does not normalize input"
            );
        }
    }
}

/// Counts and reconstruction retain the same source positions after pauses.
#[test]
fn phoaln_pause_specs_preserve_independent_source_bindings() {
    use talkbank_model::ErrorCode;
    enum Expected {
        Clean(Vec<(Option<usize>, Option<usize>)>),
        Reconstruction(ErrorCode),
        Count(ErrorCode),
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (name, expected) in [
        (
            "E740_2",
            Expected::Clean(vec![(None, None), (Some(0), None), (Some(1), None)]),
        ),
        (
            "E741_3",
            Expected::Clean(vec![(None, None), (None, Some(0)), (None, Some(1))]),
        ),
        (
            "E740_4",
            Expected::Clean(vec![
                (Some(0), None),
                (Some(1), Some(0)),
                (None, Some(1)),
                (Some(2), Some(2)),
            ]),
        ),
        (
            "E740_3",
            Expected::Reconstruction(ErrorCode::PhoalnModReconstructionMismatch),
        ),
        (
            "E741_4",
            Expected::Reconstruction(ErrorCode::PhoalnPhoReconstructionMismatch),
        ),
        ("E727_3", Expected::Count(ErrorCode::PhoalnModCountMismatch)),
        ("E728_3", Expected::Count(ErrorCode::PhoalnPhoCountMismatch)),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("{name}.cha")))
            .expect("canonical Phon pause spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.into_vec().is_empty(), "{name}: clean syntax");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let diagnostics = errors.into_vec();
        match expected {
            Expected::Clean(pairs) => {
                assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}");
                let alignment = file
                    .utterances()
                    .next()
                    .unwrap()
                    .alignments
                    .as_ref()
                    .unwrap()
                    .phoaln
                    .as_ref()
                    .unwrap();
                let actual: Vec<_> = alignment
                    .pairs
                    .iter()
                    .map(|pair| {
                        (
                            pair.source_index.map(|i| i.as_usize()),
                            pair.target_index.map(|i| i.as_usize()),
                        )
                    })
                    .collect();
                assert_eq!(actual, pairs, "{name}: independent positions");
            }
            Expected::Reconstruction(code) => {
                assert_eq!(diagnostics.len(), 1, "{name}: {diagnostics:?}");
                assert_eq!(diagnostics[0].code, code);
                assert!(
                    diagnostics[0].message.contains("word 3"),
                    "{name}: {diagnostics:?}"
                );
            }
            Expected::Count(code) => {
                assert_eq!(diagnostics.len(), 1, "{name}: {diagnostics:?}");
                assert_eq!(diagnostics[0].code, code);
                assert!(
                    diagnostics[0]
                        .message
                        .contains("2 words (1 after excluding one-sided"),
                    "{name}: {diagnostics:?}"
                );
            }
        }
    }
}

/// Canonical scoped-overlap boundaries survive CHAT and JSON serialization.
#[test]
fn scoped_overlap_specs_preserve_admitted_indices_across_boundaries() {
    use talkbank_model::model::{ChatFile, WriteChat};
    use talkbank_model::{ChatParser, ParseOutcome, SemanticEq};
    use talkbank_parser_re2c::Re2cParser;
    use talkbank_parser_tests::test_error::strict_parse;
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for example in [3, 4, 6] {
        let source = std::fs::read_to_string(corpus.join(format!("E373_{example}.cha")))
            .expect("canonical scoped overlap spec");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("valid control");
        let reparsed = strict_parse(parser.parse_chat_file(&file.to_chat_string()))
            .expect("serialized overlap control");
        assert!(
            file.semantic_eq(&reparsed),
            "E373_{example}: CHAT roundtrip"
        );
        let wire = serde_json::to_string(&file).expect("encode overlap control");
        let decoded: ChatFile = serde_json::from_str(&wire).expect("decode admitted indices");
        assert!(file.semantic_eq(&decoded), "E373_{example}: JSON roundtrip");
        let errors = ErrorCollector::new();
        let ParseOutcome::Parsed(other) = Re2cParser::new().parse_chat_file(&source, 0, &errors)
        else {
            panic!("E373_{example}: compatibility parser rejected control")
        };
        assert!(
            errors.into_vec().is_empty(),
            "E373_{example}: compatibility diagnostics"
        );
        assert!(
            file.semantic_eq(&other),
            "E373_{example}: admitted indices differ by producer"
        );
    }
}

/// Registry policy applies to every candidate and retains the word's location.
#[test]
fn language_candidate_specs_report_at_the_marked_word() {
    use talkbank_model::ErrorCode;
    enum Claim {
        Valid,
        Invalid(&'static str),
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, claim) in [
        (10, Claim::Valid),
        (11, Claim::Invalid("hola@s:spa&qzz")),
        (12, Claim::Invalid("hola@s:qzz&fra")),
        (13, Claim::Valid),
        (14, Claim::Invalid("hola@s:spa+qzz")),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E519_{example}.cha")))
            .expect("canonical language candidate spec");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.into_vec().is_empty(), "E519_{example}");
        let errors = ErrorCollector::new();
        file.validate(&errors, TranscriptName::Anonymous);
        let diagnostics = errors.into_vec();
        match claim {
            Claim::Valid => assert!(diagnostics.is_empty(), "E519_{example}: {diagnostics:?}"),
            Claim::Invalid(word) => {
                assert_eq!(diagnostics.len(), 1, "E519_{example}: {diagnostics:?}");
                assert_eq!(diagnostics[0].code, ErrorCode::InvalidLanguageCode);
                let span = diagnostics[0].location.span;
                assert_eq!(&source[span.start as usize..span.end as usize], word);
            }
        }
    }
}

/// Postcode spelling cannot change its typed role into quotation syntax.
#[test]
fn postcode_specs_retain_opaque_labels_and_the_real_terminator() {
    use talkbank_model::model::Terminator;
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, labels) in [
        (4, vec!["\"/"]),
        (5, vec!["\"/."]),
        (6, vec!["\"/", "\"/."]),
        (7, vec!["\"/", "\"/. x"]),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E242_{example}.cha")))
            .expect("canonical postcode boundary");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.to_vec().is_empty());
        let utterance = file.utterances().next().expect("source utterance");
        assert!(matches!(
            utterance.main.content.terminator.as_ref(),
            Some(Terminator::Period { .. })
        ));
        let actual: Vec<_> = utterance
            .main
            .content
            .postcodes
            .iter()
            .map(|postcode| postcode.text.as_str())
            .collect();
        assert_eq!(actual, labels);
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        assert!(errors.into_vec().is_empty(), "E242_{example}");
    }
}

/// CLAN removes editor metadata from its displayed document. Verify the
/// source-coordinate boundary on actual reference lines, including all five
/// canonical hidden headers, rather than fabricated diagnostic objects.
#[test]
fn reference_clan_coordinates_skip_and_refuse_hidden_headers() {
    use talkbank_model::{SourceLocation, Span, resolve_clan_location};
    let corpus = ChatCorpus::reference().expect("canonical references");
    let mut hidden_witnesses = [0; 5];
    let mut visible_witnesses = 0;
    for fixture in corpus.fixtures() {
        let mut offset = 0;
        let mut visible = 0;
        for (index, line) in fixture.source().split_inclusive('\n').enumerate() {
            let content = line.trim_end_matches(['\r', '\n']);
            let name = content.split_once(':').map_or(content, |(name, _)| name);
            let hidden = ["@UTF8", "@PID", "@Font", "@Color words", "@Window"]
                .iter()
                .position(|candidate| *candidate == name)
                .filter(|kind| {
                    *kind != 2
                        || content
                            .split_once(':')
                            .is_some_and(|(_, value)| value.trim_start().starts_with("CAfont:"))
                });
            match hidden {
                Some(kind) => hidden_witnesses[kind] += 1,
                None => {
                    visible += 1;
                    visible_witnesses += 1;
                }
            }
            for (cached_line, cached_column) in [
                (None, None),
                (Some(index + 1), Some(1)),
                (Some(0), Some(1)),
                (Some(index + 1), None),
                (None, Some(1)),
            ] {
                let location = SourceLocation {
                    span: Span::from_usize(offset, offset + content.len()),
                    line: cached_line,
                    column: cached_column,
                };
                let mapped = resolve_clan_location(&location, fixture.source());
                if hidden.is_some() {
                    let error = mapped.expect_err("hidden header has no editor location");
                    assert_eq!(error.source_line, index + 1);
                    assert!(error.to_string().contains("hidden"));
                } else {
                    let mapped = mapped.expect("visible reference line");
                    assert_eq!(
                        (mapped.line, mapped.column),
                        (visible, 1),
                        "{} source line {}",
                        fixture.path().display(),
                        index + 1
                    );
                }
            }
            offset += line.len();
        }
    }
    assert!(hidden_witnesses.iter().all(|count| *count > 0));
    assert!(visible_witnesses > 0);
}

/// Mutation claims are independent of observed output; check retained words
/// and every diagnostic, not just whether a set contains E243 once.
#[test]
fn unicode_boundary_specs_preserve_words_and_locate_each_rejection() {
    use talkbank_model::alignment::helpers::{WordItem, walk_words};
    use talkbank_model::{ErrorCode, Span};
    enum Expectation {
        Accepted,
        Rejected,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, characters, expectation) in [
        (1, &['a', '\u{00b7}', '\u{d7ff}'][..], Expectation::Accepted),
        (
            2,
            &['\u{f170}', '\u{f264}', '\u{ff01}', '\u{ff5e}'][..],
            Expectation::Accepted,
        ),
        (
            3,
            &[
                '\u{e000}', '\u{f16f}', '\u{f265}', '\u{ff00}', '\u{ff5f}', '\u{ffff}',
            ][..],
            Expectation::Rejected,
        ),
        (4, &['\u{10000}'][..], Expectation::Accepted),
    ] {
        let source =
            std::fs::read_to_string(corpus.join(format!("E243_unicode_boundaries_{example}.cha",)))
                .expect("generated canonical Unicode boundary spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(
            errors.to_vec().is_empty(),
            "clean syntax in Unicode example {example}"
        );
        let expected: Vec<_> = characters
            .iter()
            .map(|character| {
                let text = format!("a{character}b");
                let start = source.find(&text).expect("authored word");
                let span = Span::from_usize(start, start + text.len());
                (text, span)
            })
            .collect();
        let mut observed_words = Vec::new();
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let WordItem::Word(word) = item else {
                    panic!("ordinary word required");
                };
                assert_eq!(
                    word.raw_text(),
                    word.cleaned_text(),
                    "retain the tested scalar"
                );
                observed_words.push((word.cleaned_text().to_owned(), word.span));
            });
        }
        assert_eq!(observed_words, expected, "Unicode example {example}");
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let observed = errors.into_vec();
        assert!(
            observed
                .iter()
                .all(|error| error.code == ErrorCode::IllegalCharactersInWord),
            "no unrelated errors: {observed:?}"
        );
        let expected_spans: Vec<_> = match expectation {
            Expectation::Accepted => Vec::new(),
            Expectation::Rejected => expected.iter().map(|(_, span)| *span).collect(),
        };
        assert_eq!(
            observed
                .iter()
                .map(|error| error.location.span)
                .collect::<Vec<_>>(),
            expected_spans,
            "one located error per rejected word in example {example}"
        );
    }
}

/// Lexical deletions cannot turn stress into a compound constituent. This is
/// a source-bound policy check over parsed words, including retained invalidity.
#[test]
fn compound_specs_require_spoken_material_in_every_part() {
    use talkbank_model::model::WriteChat;
    use talkbank_model::{ErrorCode, Span};
    enum Admission {
        Valid,
        Invalid {
            word: &'static str,
            codes: &'static [ErrorCode],
        },
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (fixture, admission) in [
        ("E232_2", Admission::Valid),
        (
            "E232_3",
            Admission::Invalid {
                word: "ˈ+cream",
                codes: &[ErrorCode::InvalidCompoundMarkerPosition],
            },
        ),
        ("E233_5", Admission::Valid),
        (
            "E233_6",
            Admission::Invalid {
                word: "ice+ˈ+cone",
                codes: &[ErrorCode::EmptyCompoundPart],
            },
        ),
        ("E233_7", Admission::Valid),
        (
            "E233_8",
            Admission::Invalid {
                word: "ice+ˈ",
                codes: &[
                    ErrorCode::EmptyCompoundPart,
                    ErrorCode::StressNotBeforeSpokenMaterial,
                ],
            },
        ),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("{fixture}.cha")))
            .expect("canonical compound spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.into_vec().is_empty(), "clean syntax: {fixture}");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let diagnostics = errors.into_vec();
        match admission {
            Admission::Valid => assert!(diagnostics.is_empty(), "{fixture}: {diagnostics:?}"),
            Admission::Invalid { word, codes } => {
                assert_eq!(
                    diagnostics
                        .iter()
                        .map(|error| error.code)
                        .collect::<Vec<_>>(),
                    codes,
                    "{fixture}: {diagnostics:?}"
                );
                let start = source.find(word).expect("authored compound word");
                let span = Span::from_usize(start, start + word.len());
                assert!(diagnostics.iter().all(|error| error.location.span == span));
            }
        }
        assert_eq!(
            file.to_chat_string(),
            source,
            "validation does not repair compound parts"
        );
    }
}

/// Parsed invalid words retain their source identity through validation;
/// neither Unicode punctuation nor replacement content is silently repaired.
#[test]
fn punctuation_specs_preserve_source_and_locate_every_word() {
    use talkbank_model::model::WriteChat;
    use talkbank_model::{ErrorCode, Span};
    enum Admission {
        Valid,
        Invalid(&'static [&'static str]),
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (spec, example, admission) in [
        ("ellipsis", 1, Admission::Valid),
        ("ellipsis", 2, Admission::Invalid(&["hello…"])),
        ("ellipsis", 3, Admission::Valid),
        ("ellipsis", 4, Admission::Invalid(&["one…", "children…"])),
        ("standalone_slash", 1, Admission::Valid),
        ("standalone_slash", 2, Admission::Invalid(&["/"])),
        ("standalone_slash", 3, Admission::Valid),
        ("standalone_slash", 4, Admission::Invalid(&["/", "/"])),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E243_{spec}_{example}.cha",)))
            .expect("canonical punctuation spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.into_vec().is_empty(), "clean syntax: {example}");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let diagnostics = errors.into_vec();
        match admission {
            Admission::Valid => assert!(diagnostics.is_empty(), "{diagnostics:?}"),
            Admission::Invalid(words) => {
                assert_eq!(diagnostics.len(), words.len(), "{diagnostics:?}");
                let mut previous_end = 0;
                for (error, word) in diagnostics.iter().zip(words) {
                    assert_eq!(error.code, ErrorCode::IllegalCharactersInWord);
                    let start = previous_end
                        + source[previous_end..]
                            .find(word)
                            .expect("authored invalid word");
                    assert_eq!(
                        error.location.span,
                        Span::from_usize(start, start + word.len())
                    );
                    previous_end = start + word.len();
                }
            }
        }
        assert_eq!(file.to_chat_string(), source, "validation is not repair");
    }
}

/// Spec claims pin presence/absence; this boundary contract additionally pins
/// multiplicity and source locations for the controlled overlap mutations.
#[test]
fn indexed_overlap_specs_locate_each_unmatched_speaker() {
    use talkbank_model::ErrorCode;
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, expected_speakers) in [
        (1, &["CHI"][..]),
        (2, &[][..]),
        (3, &["MOT"][..]),
        (4, &["CHI", "MOT"][..]),
        (5, &[][..]),
        (6, &[][..]),
        (7, &[][..]),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E347_{example}.cha")))
            .expect("generated canonical overlap spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.to_vec().is_empty(), "clean syntax in E347_{example}");
        let expected_spans: Vec<_> = expected_speakers
            .iter()
            .map(|speaker| {
                file.utterances()
                    .find(|utterance| utterance.main.speaker.as_str() == *speaker)
                    .expect("declared spec speaker")
                    .main
                    .span
            })
            .collect();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let observed = errors.into_vec();
        assert!(
            observed
                .iter()
                .all(|error| error.code == ErrorCode::UnbalancedOverlap),
            "unrelated diagnostic in E347_{example}: {observed:?}"
        );
        let spans: Vec<_> = observed.iter().map(|error| error.location.span).collect();
        assert_eq!(
            spans, expected_spans,
            "unmatched speakers in E347_{example}"
        );
    }
}

/// Lexical rejection locates every byte of forbidden attribute pairs in both
/// structured speech and free text; legal underline pairs are not rejected.
#[test]
fn attribute_specs_preserve_the_source_bound_lexical_boundary() {
    use talkbank_model::{ErrorCode, Span};
    enum Attribute {
        Italic,
        Underline,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, attribute) in [
        (4, Attribute::Italic),
        (5, Attribute::Underline),
        (6, Attribute::Italic),
        (7, Attribute::Underline),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E315_{example}.cha")))
            .expect("generated canonical attribute spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        let parsed = errors.into_vec();
        let lexical: Vec<_> = parsed
            .iter()
            .filter(|error| error.code == ErrorCode::InvalidControlCharacter)
            .collect();
        match attribute {
            Attribute::Italic => {
                let begin = source.find("\u{0002}\u{0003}").expect("italic start");
                let end = source.find("\u{0002}\u{0004}").expect("italic end");
                let expected = [begin, begin + 1, end, end + 1]
                    .map(|offset| Span::from_usize(offset, offset + 1));
                assert_eq!(
                    lexical
                        .iter()
                        .map(|error| error.location.span)
                        .collect::<Vec<_>>(),
                    expected
                );
                assert!(
                    lexical
                        .iter()
                        .all(|error| error.message.contains("underline marker"))
                );
            }
            Attribute::Underline => {
                assert!(parsed.is_empty());
                let validation = ErrorCollector::new();
                file.validate_with_alignment(&validation, TranscriptName::Anonymous);
                assert!(validation.into_vec().is_empty());
            }
        }
    }
}

/// Generated repeat slots preserve option order and unsupported payloads;
/// recovery must not manufacture an empty unsupported option.
#[test]
fn option_specs_preserve_typed_flags_through_recovery() {
    use ChatOptionFlag::{Ca, NoAlign, Unsupported};
    use talkbank_model::model::{ChatOptionFlag, Header};
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (fixture, expected) in [
        ("E533_1", vec![]),
        ("E534_2", vec![Ca, Unsupported("IPA".to_string())]),
        ("E534_3", vec![Ca, NoAlign]),
        ("E534_4", vec![NoAlign, Ca]),
        ("E316_24", vec![Ca, NoAlign]),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("{fixture}.cha")))
            .expect("generated canonical option spec");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        let mut headers = file.headers().filter_map(|header| match header {
            Header::Options { options } => Some(options),
            _ => None,
        });
        let actual: Vec<_> = headers
            .next()
            .expect("options header retained")
            .iter()
            .cloned()
            .collect();
        assert_eq!(actual, expected, "{fixture}");
        assert!(headers.next().is_none());
    }
}

/// Diagnostic wording is a public boundary: a recognized annotation shape
/// does not prove that a space is missing in the original source.
#[test]
fn annotation_specs_require_source_evidence_for_spacing_advice() {
    use talkbank_model::ErrorCode;
    enum Boundary {
        Glued,
        Separated,
        Valid,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, boundary) in [
        (14, Boundary::Glued),
        (16, Boundary::Separated),
        (18, Boundary::Separated),
        (15, Boundary::Valid),
        (17, Boundary::Valid),
        (19, Boundary::Valid),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E375_{example}.cha")))
            .expect("canonical annotation spec");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        let parsed = errors.into_vec();
        match boundary {
            Boundary::Glued | Boundary::Separated => {
                let annotations: Vec<_> = parsed
                    .iter()
                    .filter(|error| error.code == ErrorCode::ContentAnnotationParseError)
                    .collect();
                assert!(!annotations.is_empty(), "E375_{example}");
                for error in annotations {
                    assert_eq!(
                        error.message.contains("Space required"),
                        matches!(boundary, Boundary::Glued),
                        "E375_{example}: {error:?}"
                    );
                    assert_eq!(
                        error
                            .suggestion
                            .as_deref()
                            .is_some_and(|s| s.contains("Add a space")),
                        matches!(boundary, Boundary::Glued),
                        "E375_{example}: {error:?}"
                    );
                }
            }
            Boundary::Valid => {
                assert!(parsed.is_empty(), "E375_{example}: {parsed:?}");
                let validation = ErrorCollector::new();
                file.validate(&validation, TranscriptName::Anonymous);
                assert!(validation.into_vec().is_empty(), "E375_{example}");
            }
        }
    }
}

/// Non-ASCII syntax findings retain the speaker bytes, not a participant-join fault.
#[test]
fn speaker_specs_reject_non_ascii_at_the_source_boundary() {
    use talkbank_model::ErrorCode;
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, code) in [(4, "CHÎ"), (5, "子")] {
        let source = std::fs::read_to_string(corpus.join(format!("E307_{example}.cha")))
            .expect("canonical speaker spec");
        let errors = ErrorCollector::new();
        let _file = parser.parse_chat_file_streaming(&source, &errors);
        let diagnostics = errors.into_vec();
        let syntax: Vec<_> = diagnostics
            .iter()
            .filter(|error| error.code == ErrorCode::InvalidSpeaker)
            .collect();
        assert!(!syntax.is_empty(), "E307_{example}: {diagnostics:?}");
        for error in syntax {
            let span = error.location.span;
            assert_eq!(&source[span.start as usize..span.end as usize], code);
            assert!(error.message.contains("ASCII"));
        }
        assert!(
            diagnostics
                .iter()
                .all(|error| error.code != ErrorCode::SpeakerNotDefined)
        );
    }
}

/// Legacy count syntax must not be offered as the repair for its own error.
/// The modern explicit repetition is admitted through the same public parser.
#[test]
fn repetition_specs_recommend_supported_explicit_speech() {
    use talkbank_model::ErrorCode;
    enum Notation {
        LegacyComplete,
        LegacyFragmented,
        Explicit,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, notation) in [
        (9, Notation::LegacyComplete),
        (10, Notation::LegacyFragmented),
        (11, Notation::Explicit),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E375_{example}.cha")))
            .expect("generated canonical repetition spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        let parsed = errors.into_vec();
        match notation {
            Notation::LegacyComplete | Notation::LegacyFragmented => {
                assert!(
                    parsed
                        .iter()
                        .any(|error| error.code == ErrorCode::ContentAnnotationParseError)
                );
                if matches!(notation, Notation::LegacyComplete) {
                    assert!(
                        parsed
                            .iter()
                            .any(|error| error.message.contains("unsupported")
                                && error
                                    .suggestion
                                    .as_ref()
                                    .is_some_and(|text| text.contains("word [/] word"))),
                        "example {example}: {parsed:?}"
                    );
                }
                assert!(parsed.iter().all(|error| {
                    error
                        .suggestion
                        .as_ref()
                        .is_none_or(|text| !text.contains("[x N]"))
                }));
            }
            Notation::Explicit => {
                assert!(parsed.is_empty());
                let validation = ErrorCollector::new();
                file.validate_with_alignment(&validation, TranscriptName::Anonymous);
                assert!(validation.into_vec().is_empty());
            }
        }
    }
}

/// A marker inside shortening is not admitted as a valid word suffix. Its
/// recovery remains visible; moving the marker to the suffix admits its type.
#[test]
fn shortening_specs_distinguish_embedded_markers_from_admitted_suffixes() {
    use talkbank_model::ErrorCode;
    use talkbank_model::alignment::helpers::{WordItem, walk_words};
    enum Placement {
        Embedded,
        Absent,
        Suffix,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, placement) in [
        (3, Placement::Embedded),
        (4, Placement::Absent),
        (5, Placement::Suffix),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E203_{example}.cha")))
            .expect("generated canonical shortening-form spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        let parsed = errors.into_vec();
        let mut forms = Vec::new();
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let WordItem::Word(word) = item else {
                    panic!("ordinary shortened word");
                };
                forms.push(
                    word.form_type
                        .as_ref()
                        .map(|form| form.to_chat_marker().to_string()),
                );
            });
        }
        let validation = ErrorCollector::new();
        file.validate_with_alignment(&validation, TranscriptName::Anonymous);
        let validated = validation.into_vec();
        match placement {
            Placement::Embedded => {
                assert!(!parsed.is_empty());
                assert!(
                    parsed
                        .iter()
                        .all(|error| error.code == ErrorCode::UnparsableContent)
                );
                assert_eq!(forms, [None]);
                assert_eq!(validated.len(), 1);
                assert_eq!(validated[0].code, ErrorCode::InvalidFormType);
            }
            Placement::Absent => {
                assert!(parsed.is_empty() && validated.is_empty());
                assert_eq!(forms, [None]);
            }
            Placement::Suffix => {
                assert!(parsed.is_empty() && validated.is_empty());
                assert_eq!(forms, [Some("b".to_string())]);
            }
        }
    }
}

/// A trailing marker is actually retained in the typed word. This witness
/// prevents treating the validator arm as unreachable from canonical CHAT.
#[test]
fn compound_specs_preserve_typed_marker_placement_and_error_span() {
    use talkbank_model::alignment::helpers::{WordItem, walk_words};
    use talkbank_model::{ErrorCode, model::WordContent};
    enum Shape {
        Trailing,
        Plain,
        Internal,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, text, shape) in [
        (2, "hey+", Shape::Trailing),
        (3, "hey", Shape::Plain),
        (4, "un+do", Shape::Internal),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E233_{example}.cha")))
            .expect("generated canonical compound spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.to_vec().is_empty());
        let mut words = Vec::new();
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let WordItem::Word(word) = item else {
                    panic!("ordinary compound word");
                };
                assert_eq!(word.raw_text(), text);
                words.push(word);
            });
        }
        assert_eq!(words.len(), 1);
        let word = words[0];
        let expected_span = match shape {
            Shape::Trailing => {
                assert!(matches!(
                    word.content().last(),
                    Some(WordContent::CompoundMarker(_))
                ));
                Some(word.span)
            }
            Shape::Plain => {
                assert!(
                    !word
                        .content()
                        .iter()
                        .any(|part| matches!(part, WordContent::CompoundMarker(_)))
                );
                None
            }
            Shape::Internal => {
                assert!(
                    word.content()
                        .iter()
                        .any(|part| matches!(part, WordContent::CompoundMarker(_)))
                );
                assert!(!matches!(
                    word.content().last(),
                    Some(WordContent::CompoundMarker(_))
                ));
                None
            }
        };
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let observed = errors.into_vec();
        if let Some(span) = expected_span {
            assert_eq!(observed.len(), 1);
            assert_eq!(observed[0].code, ErrorCode::EmptyCompoundPart);
            assert_eq!(observed[0].location.span, span);
        } else {
            assert!(observed.is_empty());
        }
    }
}

/// Cross-header checks must identify the offending ID occurrence, not an
/// earlier identical header or a registry-valid language elsewhere in the file.
#[test]
fn header_specs_locate_duplicate_ids_and_undeclared_id_languages() {
    use talkbank_model::ErrorCode;
    enum Expectation {
        Accepted,
        DuplicateId,
        UndeclaredIdLanguage,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (spec, example, expectation) in [
        ("E549", 2, Expectation::DuplicateId),
        ("E549", 3, Expectation::Accepted),
        ("E549", 4, Expectation::Accepted),
        ("E519", 5, Expectation::UndeclaredIdLanguage),
        ("E519", 6, Expectation::Accepted),
        ("E519", 7, Expectation::Accepted),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("{spec}_{example}.cha")))
            .expect("generated canonical header-promotion spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.to_vec().is_empty(), "clean header syntax");
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let observed = errors.into_vec();
        let expected = match expectation {
            Expectation::Accepted => None,
            Expectation::DuplicateId => Some((ErrorCode::DuplicateSpeakerDeclaration, 1)),
            Expectation::UndeclaredIdLanguage => Some((ErrorCode::InvalidLanguageCode, 0)),
        };
        if let Some((code, occurrence)) = expected {
            assert_eq!(observed.len(), 1, "{spec}_{example}: {observed:?}");
            assert_eq!(observed[0].code, code);
            let start = source
                .match_indices("@ID:")
                .nth(occurrence)
                .expect("authored ID occurrence")
                .0;
            assert_eq!(observed[0].location.span.start as usize, start);
        } else {
            assert!(observed.is_empty(), "{spec}_{example}: {observed:?}");
        }
    }
}

/// A continuation tab preserves speech semantics; a mid-tier tab is a
/// structural parse error, not a failed retrace despite sharing E370.
#[test]
fn tab_specs_distinguish_continuations_from_mid_tier_recovery() {
    use talkbank_model::{ErrorCode, SemanticEq};
    enum TabRole {
        MidTier,
        Continuation,
        InterWordSpace,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let mut accepted = Vec::new();
    for (example, role) in [
        (3, TabRole::MidTier),
        (4, TabRole::Continuation),
        (5, TabRole::InterWordSpace),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E370_{example}.cha")))
            .expect("generated canonical tab-context spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        let parsed = errors.into_vec();
        match role {
            TabRole::MidTier => {
                assert_eq!(parsed.len(), 1);
                assert_eq!(parsed[0].code, ErrorCode::StructuralOrderError);
                let span = parsed[0].location.span;
                let fragment = source
                    .get(span.start as usize..span.end as usize)
                    .expect("diagnostic remains source-bound");
                assert!(fragment.contains('\t'), "locate the structural tab");
            }
            TabRole::Continuation | TabRole::InterWordSpace => {
                assert!(parsed.is_empty(), "E370_{example}: {parsed:?}");
                let errors = ErrorCollector::new();
                file.validate_with_alignment(&errors, TranscriptName::Anonymous);
                assert!(errors.into_vec().is_empty());
                accepted.push(file);
            }
        }
    }
    assert_eq!(accepted.len(), 2);
    assert!(
        accepted[0].semantic_eq(&accepted[1]),
        "continuation and space preserve the same speech"
    );
}

/// CHECK 36 controls remain invalid even though their E202 claim is negative.
/// This closes the gap between "no false form error" and actual rejection.
#[test]
fn delimiter_specs_reject_syntax_without_inventing_form_markers() {
    use talkbank_model::ErrorCode;
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for example in [11, 12, 13] {
        let source =
            std::fs::read_to_string(corpus.join(format!("E202_missing_form_type_{example}.cha",)))
                .expect("generated canonical delimiter-context spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        let parsed = errors.to_vec();
        assert!(
            !parsed.is_empty(),
            "misplaced delimiter must remain rejected"
        );
        assert!(
            parsed
                .iter()
                .all(|error| error.code == ErrorCode::UnparsableContent),
            "delimiter example {example}: {parsed:?}"
        );
        let validation = ErrorCollector::new();
        file.validate_with_alignment(&validation, TranscriptName::Anonymous);
        assert!(
            validation.into_vec().is_empty(),
            "no invented missing-form or missing-terminator errors"
        );
    }
}

/// E258 requires typed main-tier separator items. Dependent recovery remains
/// invalid, but raw punctuation cannot establish that semantic state.
#[test]
fn comma_specs_keep_main_tier_semantics_separate_from_dependent_recovery() {
    use talkbank_model::ErrorCode;
    enum Expectation {
        Accepted,
        ConsecutiveSeparators,
        DependentRecovery,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, expectation) in [
        (1, Expectation::ConsecutiveSeparators),
        (2, Expectation::Accepted),
        (3, Expectation::Accepted),
        (4, Expectation::DependentRecovery),
        (5, Expectation::Accepted),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E258_{example}.cha")))
            .expect("generated canonical comma-context spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        let parse_codes: Vec<_> = errors.to_vec().iter().map(|error| error.code).collect();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let all_codes: Vec<_> = errors.into_vec().iter().map(|error| error.code).collect();
        match expectation {
            Expectation::Accepted => assert!(all_codes.is_empty(), "E258_{example}: {all_codes:?}"),
            Expectation::ConsecutiveSeparators => {
                assert!(parse_codes.is_empty());
                assert_eq!(all_codes, [ErrorCode::ConsecutiveCommas]);
            }
            Expectation::DependentRecovery => {
                assert_eq!(parse_codes, [ErrorCode::UnparsableContent]);
                assert_eq!(
                    all_codes,
                    [ErrorCode::UnparsableContent, ErrorCode::TierValidationError]
                );
            }
        }
    }
}

/// Empty POS diagnostics must locate the original bytes, including UTF-8
/// and continued lines; accepted controls must not acquire recovery errors.
#[test]
fn morphology_specs_distinguish_empty_pos_from_split_tails() {
    use talkbank_model::{ErrorCode, Span};
    enum Expectation {
        Accepted,
        EmptyPos(&'static str),
        Malformed,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (spec, example, expectation) in [
        ("E760", 1, Expectation::EmptyPos("|we")),
        ("E760", 2, Expectation::EmptyPos("|home")),
        ("E760", 3, Expectation::Accepted),
        ("E760", 4, Expectation::EmptyPos("|go")),
        ("E760", 5, Expectation::Accepted),
        ("E760", 6, Expectation::EmptyPos("|café")),
        ("E702", 3, Expectation::Accepted),
        ("E702", 4, Expectation::Malformed),
        ("E702", 5, Expectation::Malformed),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("{spec}_{example}.cha")))
            .expect("generated canonical morphology spec");
        let errors = ErrorCollector::new();
        let _file = parser.parse_chat_file_streaming(&source, &errors);
        let observed = errors.into_vec();
        match expectation {
            Expectation::Accepted => assert!(observed.is_empty(), "{spec}_{example}: {observed:?}"),
            Expectation::EmptyPos(item) => {
                assert_eq!(observed.len(), 1, "{spec}_{example}: {observed:?}");
                let start = source.find(item).expect("authored offending item");
                assert_eq!(observed[0].code, ErrorCode::MorItemEmptyPos);
                assert_eq!(
                    observed[0].location.span,
                    Span::from_usize(start, start + item.len())
                );
            }
            Expectation::Malformed => {
                assert!(!observed.is_empty(), "{spec}_{example} must be rejected");
                assert!(
                    observed
                        .iter()
                        .all(|error| error.code == ErrorCode::InvalidMorphologyFormat),
                    "split tails must not become empty-POS errors: {observed:?}"
                );
            }
        }
    }
}

/// The CHECK short-name exemption applies to the user-defined namespace,
/// not arbitrary long labels. Preserve full labels and payloads at this boundary.
#[test]
fn tier_namespace_specs_preserve_complete_labels_and_diagnostics() {
    use talkbank_model::{ErrorCode, model::DependentTier};
    enum Namespace {
        Unsupported,
        UserDefined,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, label, namespace) in [
        (1, "foo", Namespace::Unsupported),
        (2, "xfoo", Namespace::UserDefined),
        (3, "xabcdefg", Namespace::UserDefined),
        (4, "xabcdefgh", Namespace::UserDefined),
        (5, "xtrial2", Namespace::UserDefined),
        (6, "zabcdefgh", Namespace::Unsupported),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E605_{example}.cha")))
            .expect("generated canonical tier-namespace spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.to_vec().is_empty(), "clean syntax in E605_{example}");
        let utterance = file.utterances().next().expect("retained speech");
        assert_eq!(utterance.dependent_tiers.len(), 1);
        let (payload, expected_count) = match (&utterance.dependent_tiers[0].tier, namespace) {
            (DependentTier::Unsupported(payload), Namespace::Unsupported) => (payload, 1),
            (DependentTier::UserDefined(payload), Namespace::UserDefined) => (payload, 0),
            (tier, _) => panic!("wrong namespace in E605_{example}: {tier:?}"),
        };
        assert_eq!(payload.label.as_str(), label);
        assert_eq!(payload.content.as_deref(), Some("unknown tier content"));
        let span = payload.span;
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let observed = errors.into_vec();
        assert_eq!(
            observed.len(),
            expected_count,
            "E605_{example}: {observed:?}"
        );
        for error in observed {
            assert_eq!(error.code, ErrorCode::UnsupportedDependentTier);
            assert_eq!(error.location.span, span);
        }
    }
}

/// Empty-content validation must not erase the authored tier or conflate an
/// unsupported label with the intentional user-defined namespace.
#[test]
fn empty_tier_specs_preserve_namespace_and_content_presence() {
    use talkbank_model::{ErrorCode, model::DependentTier};
    enum Namespace {
        Unsupported,
        UserDefined,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, namespace, content, report_empty) in [
        (7, Namespace::Unsupported, None, false),
        (8, Namespace::Unsupported, None, false),
        (9, Namespace::UserDefined, Some("\u{00a0}"), true),
        (
            10,
            Namespace::Unsupported,
            Some("unknown tier content"),
            false,
        ),
        (
            11,
            Namespace::UserDefined,
            Some("unknown tier content"),
            false,
        ),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("E756_{example}.cha")))
            .expect("generated canonical empty-tier spec");
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.to_vec().is_empty(), "clean syntax in E756_{example}");
        let utterance = file.utterances().next().expect("retained speech");
        assert_eq!(
            utterance.dependent_tiers.len(),
            1,
            "retain E756_{example} tier"
        );
        let tier = &utterance.dependent_tiers[0].tier;
        let (payload, label, unsupported) = match (tier, namespace) {
            (DependentTier::Unsupported(payload), Namespace::Unsupported) => (payload, "foo", true),
            (DependentTier::UserDefined(payload), Namespace::UserDefined) => {
                (payload, "xfoo", false)
            }
            _ => panic!("wrong tier namespace in E756_{example}: {tier:?}"),
        };
        assert_eq!(payload.label.as_str(), label);
        assert_eq!(payload.content.as_deref(), content);
        let span = payload.span;
        assert_eq!(tier.empty_content_span(), report_empty.then_some(span));
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let observed = errors.into_vec();
        let empty_errors: Vec<_> = observed
            .iter()
            .filter(|error| error.code == ErrorCode::EmptyDependentTier)
            .collect();
        assert_eq!(
            empty_errors.len(),
            usize::from(report_empty),
            "E756_{example}: {observed:?}"
        );
        if let Some(error) = empty_errors.first() {
            assert_eq!(error.location.span, span);
        }
        assert_eq!(
            observed.len(),
            usize::from(report_empty) + usize::from(unsupported),
            "preserve label/content reporting policy in E756_{example}: {observed:?}"
        );
        assert_eq!(
            observed
                .iter()
                .filter(|error| error.code == ErrorCode::UnsupportedDependentTier)
                .count(),
            usize::from(unsupported)
        );
    }
}

fn assert_document_rebasing(errors: &[talkbank_model::ParseError], source: &str) -> (usize, usize) {
    use talkbank_model::{ErrorSink, RebasedErrorSink, SourceLocation, Span};
    // Model a CHAT source embedded after a real prefix, without parsing the
    // envelope as CHAT or changing the diagnostic's independently owned snippet.
    let prefix = "embedded CHAT:\n";
    let delta = i32::try_from(prefix.len()).expect("bounded prefix");
    let mut checked = 0;
    let mut unlocated = 0;
    for error in errors {
        if source
            .get(error.location.span.start as usize..error.location.span.end as usize)
            .is_none()
        {
            continue;
        }
        let mut located = error.clone();
        if error.location.span.is_dummy() {
            unlocated += 1;
        } else {
            let (line, column) =
                SourceLocation::calculate_line_column(error.location.span.start as usize, source);
            located.location.line = Some(line);
            located.location.column = Some(column);
        }
        let received = ErrorCollector::new();
        RebasedErrorSink::new(&received, delta).report(located);
        let mut expected = error.clone();
        let shift = |span: Span| {
            if span.is_dummy() {
                return span;
            }
            Span::new(
                span.start
                    .checked_add(prefix.len() as u32)
                    .expect("source-sized offset"),
                span.end
                    .checked_add(prefix.len() as u32)
                    .expect("source-sized offset"),
            )
        };
        expected.location = SourceLocation::new(shift(error.location.span));
        for label in &mut expected.labels {
            label.span = shift(label.span);
        }
        let translated = received.into_vec();
        assert_eq!(
            translated,
            vec![expected],
            "moving spans invalidates cached display coordinates"
        );
        let restored = ErrorCollector::new();
        RebasedErrorSink::new(&restored, -delta).report_all(translated);
        let mut expected = error.clone();
        expected.location = SourceLocation::new(error.location.span);
        assert_eq!(
            restored.into_vec(),
            vec![expected],
            "inverse translation preserves primary/secondary spans and snippet evidence"
        );
        checked += 1;
    }
    (checked, unlocated)
}

/// Project actual diagnostics to the source slice they identify. Snippets have
/// their own coordinates and must not be guessed from relative text lengths.
fn assert_fragment_projection(
    errors: &[talkbank_model::ParseError],
    source: &str,
) -> (usize, usize) {
    use talkbank_model::{ErrorSink, OffsetAdjustingErrorSink, SourceLocation, Span};
    enum Delivery {
        Single,
        Vec,
        Inline,
    }
    let mut labeled = 0;
    let mut large_context = 0;
    for error in errors {
        let origin = error.location.span.start as usize;
        let Some(fragment) = source.get(origin..error.location.span.end as usize) else {
            continue;
        };
        let project = |span: Span| {
            Span::from_usize(
                (span.start as usize)
                    .saturating_sub(origin)
                    .min(fragment.len()),
                (span.end as usize)
                    .saturating_sub(origin)
                    .min(fragment.len()),
            )
        };
        let mut expected = error.clone();
        expected.location = SourceLocation::new(project(error.location.span));
        for label in &mut expected.labels {
            label.span = project(label.span);
        }
        for mode in [Delivery::Single, Delivery::Vec, Delivery::Inline] {
            let received = ErrorCollector::new();
            let sink = OffsetAdjustingErrorSink::new(&received, origin, fragment);
            match mode {
                Delivery::Single => sink.report(error.clone()),
                Delivery::Vec => sink.report_all(vec![error.clone()]),
                Delivery::Inline => sink.report_vec(std::iter::once(error.clone()).collect()),
            }
            assert_eq!(
                received.into_vec(),
                vec![expected.clone()],
                "fragment projection must preserve independently indexed context and translate labels"
            );
        }
        labeled += error.labels.len();
        large_context +=
            usize::from(error.context.as_ref().is_some_and(|context| {
                context.source_text.len() > fragment.len().saturating_mul(2)
            }));
    }
    (labeled, large_context)
}

#[test]
fn spec_diagnostics_preserve_meaning_and_gain_source_coordinates() {
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical spec corpus");
    let parser = TreeSitterParser::new().expect("parser");
    let mut rendered_count = 0;
    let mut labeled_count = 0;
    let mut projected_labels = 0;
    let mut projected_large_contexts = 0;
    let mut rebased_diagnostics = 0;
    let mut rebased_unlocated = 0;
    for fixture in corpus.fixtures() {
        let sink = ErrorCollector::new();
        let _admission = talkbank_transform::parse_validated_with_parser(
            &parser,
            fixture.source(),
            ValidationPolicy::new(
                RuleSelection::new(),
                AlignmentValidation::IncludeTierAlignment,
            ),
            TranscriptName::Anonymous,
            &sink,
        );
        let original = sink.into_vec();
        let (rebased, unlocated) = assert_document_rebasing(&original, fixture.source());
        rebased_diagnostics += rebased;
        rebased_unlocated += unlocated;
        let (labels, contexts) = assert_fragment_projection(&original, fixture.source());
        projected_labels += labels;
        projected_large_contexts += contexts;
        let mut rendered = original.clone();
        let index = SourceIndex::new(fixture.source());
        enhance_errors_with_index(&mut rendered, &index);
        assert_eq!(original.len(), rendered.len());
        // The public renderer accepts raw diagnostics and owns enhancement of
        // its clone. Never feed its display-coordinate result back as raw input.
        let untouched = original.clone();
        for mode in [
            talkbank_transform::RenderMode::Plain,
            talkbank_transform::RenderMode::Ansi,
        ] {
            let reports = talkbank_transform::render_diagnostics(
                &original,
                "spec.cha",
                fixture.source(),
                mode,
            );
            assert_eq!(reports.len(), rendered.len());
            assert_eq!(
                original, untouched,
                "rendering must leave raw evidence unchanged"
            );
            for (report, expected) in reports.iter().zip(&rendered) {
                assert_eq!(&report.error, expected, "{}", fixture.path().display());
                assert!(
                    !report.text.is_empty(),
                    "plain rendering must not disappear"
                );
                match mode {
                    talkbank_transform::RenderMode::Plain => assert!(report.ansi.is_none()),
                    talkbank_transform::RenderMode::Ansi => {
                        assert!(report.ansi.as_ref().is_some_and(|text| !text.is_empty()))
                    }
                }
            }
        }
        for (before, after) in original.iter().zip(&rendered) {
            assert_eq!(before.code, after.code);
            assert_eq!(before.severity, after.severity);
            assert_eq!(before.message, after.message);
            assert_eq!(before.suggestion, after.suggestion);
            assert_eq!(before.help_url, after.help_url);
            assert_eq!(before.labels.len(), after.labels.len());
            // Coordinates are bytes, not Unicode scalar or display columns.
            // Count source newlines independently of the indexed lookup.
            let offset = (after.location.span.start as usize).min(fixture.source().len() - 1);
            let prefix = &fixture.source().as_bytes()[..offset];
            let line = prefix.iter().filter(|byte| **byte == b'\n').count() + 1;
            let column = offset
                - prefix
                    .iter()
                    .rposition(|byte| *byte == b'\n')
                    .map_or(0, |position| position + 1)
                + 1;
            assert_eq!(
                after.location.line,
                Some(line),
                "{}",
                fixture.path().display()
            );
            assert_eq!(
                after.location.column,
                Some(column),
                "{}",
                fixture.path().display()
            );
            let context = after.context.as_ref().expect("rendered source context");
            assert!(
                context
                    .line_offset
                    .is_some_and(|first| first > 0 && first <= line)
            );
            assert!(
                context
                    .source_text
                    .get(context.span.start as usize..context.span.end as usize)
                    .is_some(),
                "invalid display span: {} {after:?}",
                fixture.path().display()
            );
            for label in &after.labels {
                assert!(
                    context
                        .source_text
                        .get(label.span.start as usize..label.span.end as usize)
                        .is_some(),
                    "invalid display label: {} {after:?}",
                    fixture.path().display()
                );
                labeled_count += 1;
            }
            rendered_count += 1;
        }
    }
    assert!(
        rendered_count > 0 && labeled_count > 0,
        "specs must witness both primary and labeled diagnostics"
    );
    assert!(
        projected_labels > 0 && projected_large_contexts > 0,
        "specs must witness label rebasing and independently indexed long snippets"
    );
    assert!(
        rebased_diagnostics > 0,
        "specs must exercise document translation"
    );
    assert!(
        rebased_unlocated > 0 && rebased_unlocated < rebased_diagnostics,
        "specs must exercise both located and explicitly unlocated diagnostics"
    );
}
