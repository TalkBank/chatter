//! User-facing diagnostic rendering over canonical spec inputs.

#![allow(clippy::unwrap_used)] // Fixture assertions may fail loudly; production remains panic-free.

use talkbank_model::model::TranscriptName;
use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{ErrorCollector, RuleSelection, SourceIndex, enhance_errors_with_index};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::repo_paths::workspace_root;

#[path = "presentation_corpus.rs"]
mod presentation_contracts;

/// Imported age values preserve unsupported text without claiming source provenance.
#[test]
fn age_spec_values_preserve_the_string_wire_contract() {
    use talkbank_model::model::{AgeValue, Header, WriteChat};
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical error corpus");
    let mut supported = 0;
    let mut unsupported = 0;
    for fixture in corpus.fixtures().iter().filter(|fixture| {
        fixture
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("E517_"))
    }) {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("age specimen syntax");
        for header in file.headers() {
            let Header::ID(id) = header else {
                continue;
            };
            let Some(age) = &id.age else {
                continue;
            };
            match age {
                AgeValue::Valid { .. } => supported += 1,
                AgeValue::Unsupported(_) => unsupported += 1,
            }
            assert_eq!(AgeValue::from(age.as_str()), *age);
            assert_eq!(AgeValue::from(age.as_str().to_owned()), *age);
            assert_eq!(age.to_string(), age.as_str());
            assert_eq!(age.to_chat_string(), age.as_str());
            let wire = serde_json::to_string(age).expect("age JSON");
            assert_eq!(
                wire,
                serde_json::to_string(age.as_str()).expect("string JSON")
            );
            assert_eq!(
                serde_json::from_str::<AgeValue>(&wire).expect("age admission"),
                *age
            );
        }
    }
    assert!(
        supported > 0 && unsupported > 0,
        "both preserved states need canonical witnesses: {supported}/{unsupported}"
    );
    for wire in ["null", "0", "[]", "{}"] {
        assert!(serde_json::from_str::<AgeValue>(wire).is_err(), "{wire}");
    }
}

/// Independent incomplete syllable units must each retain their diagnostic.
#[test]
fn syllable_spec_deletions_report_each_malformed_word_without_rejecting_pause_control() {
    let parser = TreeSitterParser::new().expect("parser");
    for (fixture, expected_count) in [("E735_2.cha", 0), ("E735_3.cha", 3)] {
        let source = std::fs::read_to_string(
            workspace_root()
                .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors")
                .join(fixture),
        )
        .expect("syllable spec fixture");
        let mut file =
            talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(&source))
                .expect("tier body parses without recovery");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let errors = errors.into_vec();
        assert_eq!(errors.len(), expected_count, "{fixture}: {errors:?}");
        for (index, error) in errors.iter().enumerate() {
            assert_eq!(error.code, talkbank_model::ErrorCode::SylUnitMalformed);
            assert!(
                error.message.contains(&format!("word {}", index + 1)),
                "each changed word retains its position: {error:?}"
            );
        }
    }
}

/// Imported raw spelling remains subject to suffix validation after decoding.
#[test]
fn reference_word_wire_spelling_does_not_bypass_form_marker_validation() {
    use talkbank_model::ErrorCode;
    use talkbank_model::alignment::helpers::{WordItem, walk_words};
    use talkbank_model::model::Word;
    use talkbank_model::validation::{Validate, ValidationContext};
    let parser = TreeSitterParser::new().expect("parser");
    for (fixture, token) in [
        ("corpus/reference/core/basic-conversation.cha", "cookies"),
        (
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E203_5.cha",
            "c(a)t@b",
        ),
    ] {
        let source =
            std::fs::read_to_string(workspace_root().join(fixture)).expect("canonical source");
        let file = talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(&source))
            .expect("reference parses");
        let mut seed = None;
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                if let WordItem::Word(word) = item
                    && word.raw_text() == token
                {
                    seed = Some(serde_json::to_value(word).expect("reference wire"));
                }
            });
        }
        let seed = seed.expect("reference lexical control");
        // Display fields supplied by importers have no authority over structure.
        for spelling in [
            "cookies@",
            "cookies@@",
            "cookies)(",
            "c(a)t@bx",
            "c(a)t@b$n",
        ] {
            let mut wire = seed.clone();
            wire["raw_text"] = spelling.into();
            wire["cleaned_text"] = "stale".into();
            let word: Word = serde_json::from_value(wire).expect("legacy computed fields ignored");
            assert!(word.span.is_dummy());
            assert_eq!(word.raw_text(), token);
            assert_eq!(serde_json::to_value(&word).expect("derived wire"), seed);
        }
        let mut without_display = seed.clone();
        without_display
            .as_object_mut()
            .expect("word object")
            .remove("raw_text");
        let word: Word = serde_json::from_value(without_display).expect("structure alone suffices");
        assert_eq!(word.raw_text(), token);
        if token == "cookies" {
            for spelling in [
                "cookies",
                "cookies@",
                "cookies@@",
                "co(ok)ies",
                "cookies)(",
                "co\u{15}okies",
            ] {
                let mut wire = seed.clone();
                wire["content"][0]["content"] = spelling.into();
                let word: Word =
                    serde_json::from_value(wire).expect("unvalidated structured import");
                assert_eq!(word.raw_text(), spelling);
                let errors = ErrorCollector::new();
                word.validate(&ValidationContext::default(), &errors);
                assert_eq!(
                    errors
                        .into_vec()
                        .iter()
                        .any(|error| error.code == ErrorCode::IllegalCharactersInWord),
                    spelling != "cookies",
                    "{spelling:?}",
                );
            }
        }
    }
}

/// Retrace model import preserves the violation but cannot restore source labels.
#[test]
fn retrace_specs_import_without_fabricating_source_labels() {
    use talkbank_model::ErrorCode;
    use talkbank_model::model::ChatFile;
    let parser = TreeSitterParser::new().expect("parser");
    for (fixture, code) in [
        ("E370_1.cha", ErrorCode::StructuralOrderError),
        ("E370_2.cha", ErrorCode::StructuralOrderError),
        ("E377_1.cha", ErrorCode::RetraceWithNoMaterial),
        ("E378_1.cha", ErrorCode::RetraceWithoutWords),
    ] {
        let source = std::fs::read_to_string(
            workspace_root()
                .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors")
                .join(fixture),
        )
        .expect("canonical retrace spec");
        let mut located =
            talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(&source))
                .expect("retrace specimen has clean syntax");
        let mut imported: ChatFile =
            serde_json::from_value(serde_json::to_value(&located).expect("encode spec model"))
                .expect("decode spec model");
        let collect = |file: &mut ChatFile| {
            let errors = ErrorCollector::new();
            file.validate_with_alignment(&errors, TranscriptName::Anonymous);
            errors
                .into_vec()
                .into_iter()
                .filter(|error| error.code == code)
                .collect::<Vec<_>>()
        };
        let before = collect(&mut located);
        let after = collect(&mut imported);
        assert!(!before.is_empty(), "{fixture}: declared violation");
        assert_eq!(before.len(), after.len());
        for (located, imported) in before.iter().zip(&after) {
            assert!(!located.location.span.is_dummy());
            assert!(!located.labels.is_empty());
            assert!(imported.location.span.is_dummy());
            assert!(imported.labels.is_empty(), "no byte-zero source label");
            assert_eq!(located.message, imported.message);
            assert_eq!(located.suggestion, imported.suggestion);
            assert_eq!(located.severity, imported.severity);
            assert!(located.context.is_none() && imported.context.is_none());
            assert!(located.help_url.is_none() && imported.help_url.is_none());
        }
    }
}

/// Optional recovery metadata survives import without inventing parser evidence.
#[test]
fn unknown_header_spec_import_preserves_optional_recovery_metadata() {
    use talkbank_model::ErrorCode;
    use talkbank_model::model::{ChatFile, Header, Line};
    let parser = TreeSitterParser::new().expect("parser");
    let source = std::fs::read_to_string(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E525_1.cha"),
    )
    .expect("unknown-header spec");
    let parse_errors = ErrorCollector::new();
    let seed = parser.parse_chat_file_streaming(&source, &parse_errors);
    // This recovery producer retains Header::Unknown and delegates E525 to
    // model validation; it does not emit a separate parse diagnostic.
    assert!(parse_errors.into_vec().is_empty());
    for retain_reason in [true, false] {
        for supplied_fix in [None, Some("Use @Comment for free text")] {
            let mut imported: ChatFile =
                serde_json::from_value(serde_json::to_value(&seed).expect("spec model wire"))
                    .expect("spec model import");
            let mut expected = None;
            for line in &mut imported.lines {
                if let Line::Header { header, .. } = line
                    && let Header::Unknown {
                        text,
                        parse_reason,
                        suggested_fix,
                    } = header.as_mut()
                {
                    assert!(expected.is_none(), "one unknown header in specimen");
                    assert!(parse_reason.is_some());
                    assert!(
                        suggested_fix.is_none(),
                        "unsupported-header producer has no fix"
                    );
                    if !retain_reason {
                        *parse_reason = None;
                    }
                    // Authored import advice follows E525's documented policy;
                    // it is not presented as advice emitted by this parser.
                    *suggested_fix = supplied_fix.map(str::to_owned);
                    let message = match parse_reason {
                        Some(reason) => {
                            format!("Unknown or malformed header: {} ({reason})", text.as_str())
                        }
                        None => format!("Unknown or malformed header: {}", text.as_str()),
                    };
                    expected = Some((message, suggested_fix.clone()));
                }
            }
            let (message, fix) = expected.expect("recovered unknown header");
            let errors = ErrorCollector::new();
            imported.validate_with_alignment(&errors, TranscriptName::Anonymous);
            let errors: Vec<_> = errors
                .into_vec()
                .into_iter()
                .filter(|error| error.code == ErrorCode::UnknownHeader)
                .collect();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].message, message);
            match fix {
                Some(fix) => assert_eq!(errors[0].suggestion.as_deref(), Some(fix.as_str())),
                None => assert_eq!(
                    errors[0].suggestion.as_deref(),
                    Some(
                        "Check the CHAT manual for valid header types: https://talkbank.org/0info/manuals/CHAT.html#File_Headers",
                    )
                ),
            }
        }
    }
}

/// Editor lookup must select the actual owning turn, including continuation
/// and dependent-tier bytes, without swallowing headers or the next turn.
#[test]
fn reference_editor_offsets_preserve_tier_ownership_and_half_open_boundaries() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut main_tiers = 0;
    let mut dependent_tiers = 0;
    for fixture in corpus.fixtures() {
        let file = talkbank_parser_tests::test_error::strict_parse(
            parser.parse_chat_file(fixture.source()),
        )
        .expect("reference syntax");
        assert_eq!(file.header_count(), file.headers().count());
        assert_eq!(file.utterance_count(), file.utterances().count());
        for utterance in file.utterances() {
            let mut end = utterance.main.span.end;
            for (span, marker) in std::iter::once((utterance.main.span, b'*')).chain(
                utterance
                    .dependent_tiers
                    .iter()
                    .map(|entry| (entry.tier.span(), b'%')),
            ) {
                assert!(
                    !span.is_dummy() && span.start < span.end,
                    "reference tier has source coordinates"
                );
                assert_eq!(
                    fixture.source().as_bytes()[span.start as usize],
                    marker,
                    "tier span begins at its source marker: {}",
                    fixture.path().display()
                );
                for offset in [span.start, span.start + 1, span.end - 1] {
                    let selected = file
                        .utterance_containing(offset)
                        .expect("tier byte has an owner");
                    assert!(
                        std::ptr::eq(selected, utterance),
                        "lookup returns the owning object, not a similar turn"
                    );
                }
                end = end.max(span.end);
                if marker == b'*' {
                    main_tiers += 1;
                } else {
                    dependent_tiers += 1;
                }
            }
            if let Some(selected) = file.utterance_containing(end) {
                assert!(
                    !std::ptr::eq(selected, utterance),
                    "exclusive end cannot select the finished turn"
                );
            }
        }
        for (_, span) in file.headers_with_spans() {
            if !span.is_dummy() {
                assert!(
                    file.utterance_containing(span.start).is_none(),
                    "header is not speech"
                );
            }
        }
        assert!(
            file.utterance_containing(fixture.source().len() as u32)
                .is_none()
        );
    }
    assert!(
        main_tiers > 0 && dependent_tiers > 0,
        "both editor lookup routes are witnessed"
    );
}

/// An invalid ID join does not erase the declaration or create missing data.
#[test]
fn missing_id_spec_keeps_declared_speaker_visible_without_metadata() {
    let parser = TreeSitterParser::new().expect("parser");
    let source = std::fs::read_to_string(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E522_2.cha"),
    )
    .expect("canonical missing-ID spec");
    let errors = ErrorCollector::new();
    let mut file = parser.parse_chat_file_streaming(&source, &errors);
    file.validate_with_alignment(&errors, TranscriptName::Anonymous);
    assert!(
        errors
            .to_vec()
            .iter()
            .any(|error| error.code.to_string() == "E522"),
        "usable declaration access must not certify invalid CHAT"
    );
    let declared: Vec<_> = file.declared_speakers().collect();
    assert_eq!(declared.len(), 2);
    assert_eq!(declared[0].code().as_str(), "CHI");
    assert_eq!(declared[0].name().map(|name| name.as_str()), Some("Ruth"));
    assert_eq!(declared[0].role().as_str(), "Target_Child");
    assert!(declared[0].id_metadata().is_none());
    assert_eq!(declared[1].code().as_str(), "MOT");
    assert_eq!(declared[1].role().as_str(), "Mother");
    assert!(declared[1].id_metadata().is_some());
    assert!(file.get_participant("CHI").is_none());
    assert!(file.get_participant("MOT").is_some());
    assert_eq!(file.participant_count(), 1);
    assert_eq!(file.all_participants().len(), 1);
}

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
    let cases = (1..=15)
        .map(|example| ("E540", example, matches!(example, 6 | 7 | 10)))
        .chain((1..=11).map(|example| ("E540_numeric_boundaries", example, example == 1)));
    for (stem, example, legal) in cases {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{stem}_{example}.cha",
        )))
        .expect("canonical duration spec");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("duration spec parses");
        let errors = ErrorCollector::new();
        file.validate_headers_only(&errors, TranscriptName::Anonymous);
        let findings = errors.into_vec();
        assert_eq!(
            findings.len(),
            usize::from(!legal),
            "{stem}_{example}: {findings:?}"
        );
        assert!(
            findings
                .iter()
                .all(|error| error.code == ErrorCode::InvalidTimeDuration)
        );
        assert_eq!(file.to_chat_string(), source);

        let wire = serde_json::to_string(&file).expect("serialize original duration");
        let decoded: ChatFile = serde_json::from_str(&wire).expect("restore original duration");
        assert_eq!(decoded.to_chat_string(), source, "{stem}_{example}");
        let errors = ErrorCollector::new();
        decoded.validate_headers_only(&errors, TranscriptName::Anonymous);
        let restored_findings = errors.into_vec();
        assert!(
            restored_findings
                .iter()
                .map(|error| (error.code, &error.message))
                .eq(findings.iter().map(|error| (error.code, &error.message))),
            "JSON must not change duration assessment: {stem}_{example}"
        );

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

/// Two-component duration ranges are hours/minutes, not the minutes/seconds
/// convention used by general time values. Pin the typed meaning of real CHAT.
#[test]
fn duration_reference_retains_hour_minute_endpoint_meaning() {
    use talkbank_model::model::{Header, TimeDurationValue, TimeSegment, WriteChat};
    use talkbank_parser_tests::test_error::strict_parse;
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/core/headers-time-and-types.cha"
    ));
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(source)).expect("reference parses");
    let segments: Vec<_> = file
        .headers()
        .filter_map(|header| match header {
            Header::TimeDuration { duration } => Some(duration.segments()),
            _ => None,
        })
        .flatten()
        .collect();
    let [TimeSegment::Range { start, end }] = segments.as_slice() else {
        panic!("reference must supply one duration range");
    };
    assert_eq!((start.hours, start.minutes, start.seconds), (17, 30, 0));
    assert_eq!((end.hours, end.minutes, end.seconds), (18, 0, 0));
    let Header::TimeDuration { duration } = file
        .headers()
        .find(|header| matches!(header, Header::TimeDuration { .. }))
        .expect("authored duration")
    else {
        unreachable!()
    };
    assert_eq!(TimeDurationValue::from(duration.as_str()), *duration);
    assert_eq!(
        TimeDurationValue::from(duration.as_str().to_owned()),
        *duration
    );
    assert_eq!(duration.to_chat_string(), "17:30-18:00");
    assert_eq!(
        serde_json::to_string(duration).expect("duration string JSON"),
        "\"17:30-18:00\""
    );
    for invalid in ["0", "null", "[]", "{}"] {
        assert!(serde_json::from_str::<TimeDurationValue>(invalid).is_err());
    }
}

#[test]
fn clock_context_reference_keeps_start_and_timing_in_minutes_seconds() {
    use talkbank_model::model::{
        ChatFile, DependentTier, Header, TimeSegment, TimeStartValue, WriteChat,
    };
    use talkbank_parser_tests::test_error::strict_parse;
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/core/clock-contexts.cha"
    ));
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(source)).expect("clock reference parses");
    for header in file.headers() {
        if let Header::TimeStart { start } = header {
            assert_eq!(TimeStartValue::from(start.as_str()), *start);
            assert_eq!(TimeStartValue::from(start.as_str().to_owned()), *start);
            assert_eq!(start.to_chat_string(), start.as_str());
            assert_eq!(
                serde_json::to_string(start).expect("start JSON"),
                serde_json::to_string(start.as_str()).expect("string JSON")
            );
        }
    }
    for invalid in ["0", "null", "[]", "{}"] {
        assert!(serde_json::from_str::<TimeStartValue>(invalid).is_err());
    }
    let wire = serde_json::to_string(&file).expect("serialize clock contexts");
    let decoded: ChatFile = serde_json::from_str(&wire).expect("restore clock contexts");
    for file in [&file, &decoded] {
        let mut seen = [0; 3];
        for header in file.headers() {
            match header {
                Header::TimeStart { start: time } => {
                    let TimeStartValue::Parsed {
                        hours,
                        minutes,
                        seconds,
                        ..
                    } = time
                    else {
                        panic!("structured start time");
                    };
                    assert_eq!((*hours, *minutes, *seconds), (0, 59, 59));
                    seen[0] += 1;
                }
                Header::TimeDuration { duration } => {
                    let [TimeSegment::Range { start, end }] = duration.segments() else {
                        panic!("structured duration range");
                    };
                    assert_eq!((start.hours, start.minutes, start.seconds), (17, 30, 0));
                    assert_eq!((end.hours, end.minutes, end.seconds), (18, 0, 0));
                    seen[1] += 1;
                }
                _ => {}
            }
        }
        for utterance in file.utterances() {
            for entry in &utterance.dependent_tiers {
                if let DependentTier::Tim(tier) = &entry.tier {
                    let [talkbank_model::model::dependent_tier::TimSegment::Single(time)] =
                        tier.segments()
                    else {
                        panic!("structured timing value")
                    };
                    assert_eq!((time.hours, time.minutes, time.seconds), (0, 59, 59));
                    seen[2] += 1;
                }
            }
        }
        assert_eq!(seen, [1, 1, 1]);
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
            matches!(duration, PauseTimedDuration::Parsed(_)),
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
    use talkbank_model::validation::{Validate, ValidationContext};
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
                    matches!(date, ChatDate::Valid(_)),
                    example == control,
                    "{spec}_{example}: parsed date admission"
                );
                let encoded = serde_json::to_string(date).expect("date wire value");
                assert_eq!(encoded, serde_json::to_string(date.as_str()).unwrap());
                assert_eq!(ChatDate::new(date.as_str()), *date);
                assert_eq!(ChatDate::from(date.as_str()), *date);
                assert_eq!(ChatDate::from(date.as_str().to_owned()), *date);
                assert_eq!(date.to_chat_string(), date.as_str());
                if let ChatDate::Valid(checked) = date {
                    assert_eq!(checked.as_str(), date.as_str());
                    assert_eq!(
                        format!(
                            "{:02}-{}-{:04}",
                            checked.day(),
                            checked.month().as_str(),
                            checked.year()
                        ),
                        date.as_str(),
                        "admitted components and preserved text agree"
                    );
                }
                let decoded: ChatDate = serde_json::from_str(&encoded).expect("date wire decoding");
                assert_eq!(
                    &decoded, date,
                    "{spec}_{example}: wire admission and spelling"
                );
                // A standalone imported value has neither a header role nor
                // document coordinates. Full-file validation below retains both.
                let value_errors = ErrorCollector::new();
                decoded.validate(&ValidationContext::default(), &value_errors);
                let value_errors = value_errors.into_vec();
                if example == control {
                    assert!(value_errors.is_empty());
                } else {
                    assert_eq!(value_errors.len(), 1);
                    assert_eq!(value_errors[0].code, ErrorCode::InvalidDateFormat);
                    assert_eq!(value_errors[0].severity, talkbank_model::Severity::Error);
                    assert_eq!(
                        value_errors[0].location.span,
                        talkbank_model::Span::from_usize(0, 0)
                    );
                    assert!(value_errors[0].message.contains(date.as_str()));
                }
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
    for invalid_wire in ["null", "0", "[]", "{}"] {
        assert!(
            serde_json::from_str::<ChatDate>(invalid_wire).is_err(),
            "date wire admission requires a string: {invalid_wire}"
        );
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
        PrivateUse,
        Noncharacter,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, characters, expectation) in [
        (
            1,
            &['\u{61}', '\u{b7}', '\u{d7ff}'][..],
            Expectation::Accepted,
        ),
        (
            2,
            &[
                '\u{e000}',
                '\u{f170}',
                '\u{f264}',
                '\u{f8ff}',
                '\u{f0000}',
                '\u{ffffd}',
                '\u{100000}',
                '\u{10fffd}',
            ][..],
            Expectation::PrivateUse,
        ),
        (
            3,
            &[
                '\u{fdd0}',
                '\u{fdd1}',
                '\u{fdd2}',
                '\u{fdd3}',
                '\u{fdd4}',
                '\u{fdd5}',
                '\u{fdd6}',
                '\u{fdd7}',
                '\u{fdd8}',
                '\u{fdd9}',
                '\u{fdda}',
                '\u{fddb}',
                '\u{fddc}',
                '\u{fddd}',
                '\u{fdde}',
                '\u{fddf}',
                '\u{fde0}',
                '\u{fde1}',
                '\u{fde2}',
                '\u{fde3}',
                '\u{fde4}',
                '\u{fde5}',
                '\u{fde6}',
                '\u{fde7}',
                '\u{fde8}',
                '\u{fde9}',
                '\u{fdea}',
                '\u{fdeb}',
                '\u{fdec}',
                '\u{fded}',
                '\u{fdee}',
                '\u{fdef}',
                '\u{fffe}',
                '\u{ffff}',
                '\u{1fffe}',
                '\u{1ffff}',
                '\u{2fffe}',
                '\u{2ffff}',
                '\u{3fffe}',
                '\u{3ffff}',
                '\u{4fffe}',
                '\u{4ffff}',
                '\u{5fffe}',
                '\u{5ffff}',
                '\u{6fffe}',
                '\u{6ffff}',
                '\u{7fffe}',
                '\u{7ffff}',
                '\u{8fffe}',
                '\u{8ffff}',
                '\u{9fffe}',
                '\u{9ffff}',
                '\u{afffe}',
                '\u{affff}',
                '\u{bfffe}',
                '\u{bffff}',
                '\u{cfffe}',
                '\u{cffff}',
                '\u{dfffe}',
                '\u{dffff}',
                '\u{efffe}',
                '\u{effff}',
                '\u{ffffe}',
                '\u{fffff}',
                '\u{10fffe}',
                '\u{10ffff}',
            ][..],
            Expectation::Noncharacter,
        ),
        (4, &['\u{10000}'][..], Expectation::Accepted),
        (
            5,
            &[
                '\u{f900}', '\u{fb00}', '\u{fd50}', '\u{fdf0}', '\u{fe70}', '\u{ff01}', '\u{ff5e}',
                '\u{ff5f}', '\u{fffd}',
            ][..],
            Expectation::Accepted,
        ),
        (
            6,
            &['\u{f900}', '\u{fdcf}', '\u{fdf0}', '\u{1fffd}', '\u{efffd}'][..],
            Expectation::Accepted,
        ),
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
        use talkbank_model::WriteChat;
        assert_eq!(
            file.to_chat_string(),
            source,
            "byte-exact Unicode preservation"
        );
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let observed = errors.into_vec();
        assert!(
            observed
                .iter()
                .all(|error| error.code == ErrorCode::IllegalCharactersInWord),
            "no unrelated errors: {observed:?}"
        );
        let category = match expectation {
            Expectation::Accepted => None,
            Expectation::PrivateUse => Some("Unicode private-use character"),
            Expectation::Noncharacter => Some("Unicode noncharacter"),
        };
        if let Some(category) = category {
            assert!(
                observed
                    .iter()
                    .all(|error| error.message.contains(category))
            );
        }
        let expected_spans: Vec<_> = match expectation {
            Expectation::Accepted => Vec::new(),
            Expectation::PrivateUse | Expectation::Noncharacter => {
                expected.iter().map(|(_, span)| *span).collect()
            }
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
                // Editor buffers can end inside either attribute pair. Use
                // exact prefixes of the legal source; do not fabricate CHAT
                // scaffolding or infer whole-document validity from E315.
                for marker in ["\u{0002}\u{0001}", "\u{0002}\u{0002}"] {
                    let start = source.find(marker).expect("canonical underline marker");
                    for length in [1, marker.len()] {
                        let prefix = source.get(..start + length).expect("ASCII marker boundary");
                        let errors = ErrorCollector::new();
                        let _recovered = parser.parse_chat_file_streaming(prefix, &errors);
                        let lexical: Vec<_> = errors
                            .into_vec()
                            .into_iter()
                            .filter(|error| error.code == ErrorCode::InvalidControlCharacter)
                            .collect();
                        match length {
                            1 => {
                                assert_eq!(lexical.len(), 1, "incomplete pair at EOF");
                                assert_eq!(
                                    lexical[0].location.span,
                                    Span::from_usize(start, start + 1)
                                );
                                assert!(lexical[0].message.contains("U+0002"));
                            }
                            _ => assert!(lexical.is_empty(), "complete pair is lexically admitted"),
                        }
                    }
                }
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

/// Removed recovery scanners must not turn malformed CHAT into accepted input.
/// Retired E311 examples remain executable contracts rather than deferred evidence.
#[test]
fn marker_recovery_specs_preserve_admission_without_text_scanners() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (spec, invalid, valid) in [
        (
            "E202_missing_form_type",
            &[1, 4, 6, 8, 10][..],
            &[5, 7, 9][..],
        ),
        ("E311", &[1, 2][..], &[3][..]),
    ] {
        for example in invalid {
            let source = std::fs::read_to_string(corpus.join(format!("{spec}_{example}.cha")))
                .expect("canonical invalid marker fixture");
            let errors = ErrorCollector::new();
            let _file = parser.parse_chat_file_streaming(&source, &errors);
            assert!(
                errors
                    .into_vec()
                    .iter()
                    .any(|e| e.severity == talkbank_model::Severity::Error),
                "{spec}_{example} must remain rejected"
            );
        }
        for example in valid {
            let source = std::fs::read_to_string(corpus.join(format!("{spec}_{example}.cha")))
                .expect("canonical valid marker control");
            let errors = ErrorCollector::new();
            let file = parser.parse_chat_file_streaming(&source, &errors);
            assert!(errors.into_vec().is_empty(), "{spec}_{example}");
            let validation = ErrorCollector::new();
            file.validate(&validation, TranscriptName::Anonymous);
            assert!(validation.into_vec().is_empty(), "{spec}_{example}");
        }
    }
}

/// Malformed brackets do not establish any maximum legal nesting depth.
/// Keep rejection and valid controls while forbidding unsupported repair advice.
#[test]
fn malformed_bracket_specs_do_not_invent_a_nesting_limit() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for example in [1, 6, 8] {
        let source = std::fs::read_to_string(corpus.join(format!("E375_{example}.cha")))
            .expect("canonical malformed bracket spec");
        let errors = ErrorCollector::new();
        let _file = parser.parse_chat_file_streaming(&source, &errors);
        let observed = errors.into_vec();
        assert!(
            observed
                .iter()
                .any(|e| e.severity == talkbank_model::Severity::Error),
            "E375_{example} must remain rejected"
        );
        for error in observed {
            assert!(!error.message.contains("Quadruple nested"), "{error:?}");
            assert!(
                error
                    .suggestion
                    .as_deref()
                    .is_none_or(|s| !s.contains("triple nested")),
                "{error:?}"
            );
        }
    }
    for example in [5, 7] {
        let source = std::fs::read_to_string(corpus.join(format!("E375_{example}.cha")))
            .expect("canonical valid bracket control");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.into_vec().is_empty(), "E375_{example}");
        let validation = ErrorCollector::new();
        file.validate(&validation, TranscriptName::Anonymous);
        assert!(validation.into_vec().is_empty(), "E375_{example}");
    }
}

/// Recovery must reject malformed annotations without reconstructing their
/// syntax from a text prefix and the preceding character.
#[test]
fn annotation_specs_require_source_evidence_for_spacing_advice() {
    enum Boundary {
        Invalid,
        Valid,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (example, boundary) in [
        (14, Boundary::Invalid),
        (16, Boundary::Invalid),
        (18, Boundary::Invalid),
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
            Boundary::Invalid => {
                assert!(
                    parsed
                        .iter()
                        .any(|error| error.severity == talkbank_model::Severity::Error),
                    "E375_{example} must remain rejected"
                );
                for error in parsed {
                    assert!(!error.message.contains("Space required"), "{error:?}");
                    assert!(
                        error
                            .suggestion
                            .as_deref()
                            .is_none_or(|s| !s.contains("Add a space")),
                        "{error:?}"
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
        let file = parser.parse_chat_file_streaming(&source, &errors);
        let parsed = errors.to_vec();
        assert!(
            parsed
                .iter()
                .any(|error| error.code == ErrorCode::UnparsableContent)
        );
        assert!(
            parsed
                .iter()
                .all(|error| error.code != ErrorCode::SpeakerNotDefined)
        );
        // Typed ID fields retain E307; recovery may also leave incomplete
        // participant declarations, whose subsequent join checks are separate.
        file.validate(&errors, TranscriptName::Anonymous);
        let diagnostics = errors.into_vec();
        let syntax: Vec<_> = diagnostics
            .iter()
            .filter(|error| error.code == ErrorCode::InvalidSpeaker)
            .collect();
        assert!(!syntax.is_empty(), "E307_{example}: {diagnostics:?}");
        for error in syntax {
            let span = error.location.span;
            assert!(source[span.start as usize..span.end as usize].contains(code));
            assert!(error.message.contains("ASCII"));
        }
    }
}

/// Legacy count syntax must not be offered as the repair for its own error.
/// The modern explicit repetition is admitted through the same public parser.
#[test]
fn repetition_specs_reject_legacy_counts_without_guessing_repairs() {
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
                        .any(|error| error.code == ErrorCode::UnparsableContent)
                );
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
                assert!(
                    validated.is_empty(),
                    "parser rejection needs no raw-text rescan"
                );
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

/// Grammar-rejected morphology stays rejected without reparsing recovery text
/// to distinguish empty POS from split tails; valid controls remain clean.
#[test]
fn morphology_specs_distinguish_empty_pos_from_split_tails() {
    use talkbank_model::ErrorCode;
    enum Expectation {
        Accepted,
        Malformed,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (spec, example, expectation) in [
        ("E760", 1, Expectation::Malformed),
        ("E760", 2, Expectation::Malformed),
        ("E760", 3, Expectation::Accepted),
        ("E760", 4, Expectation::Malformed),
        ("E760", 5, Expectation::Accepted),
        ("E760", 6, Expectation::Malformed),
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

#[test]
fn reference_excerpt_admission_distinguishes_empty_from_invalid_ranges() {
    use talkbank_model::{ErrorContext, SourceExcerptError, SourceLocation, Span};
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/audio/chinese-adult-conversation.cha"),
    )
    .expect("Unicode reference source");
    let (start, character) = source
        .char_indices()
        .find(|(_, ch)| ch.len_utf8() > 1)
        .expect("reference contains a multibyte scalar");
    let end = start + character.len_utf8();
    for range in [0..0, start..end, source.len()..source.len()] {
        let checked = SourceLocation::from_offsets_in_source(range.start, range.end, &source)
            .expect("reference range");
        let line = checked.line.expect("checked source line");
        let column = checked.column.expect("checked source column");
        let explicit =
            SourceLocation::from_offsets_with_position(range.start, range.end, line, column);
        assert_eq!(explicit, checked);
        let relative = SourceLocation::from_range(range);
        assert_eq!(relative.span, checked.span);
        assert_eq!((relative.line, relative.column), (None, None));
        assert_eq!(relative.with_position(line, column), checked);
        let from_span: SourceLocation = checked.span.into();
        assert_eq!(from_span.span, checked.span);
        assert_eq!((from_span.line, from_span.column), (None, None));
    }
    let excerpt = ErrorContext::from_source_with_span(&source, start, end, &source[start..end])
        .expect("whole scalar is a valid source excerpt");
    assert_eq!(excerpt.source_text, character.to_string());
    assert_eq!(excerpt.span, Span::from_usize(0, character.len_utf8()));
    for offset in [0, start, source.len()] {
        let empty = ErrorContext::from_source_with_span(&source, offset, offset, "")
            .expect("valid insertion range");
        assert!(empty.source_text.is_empty());
        assert_eq!(empty.span, Span::from_usize(0, 0));
    }
    for (start, end) in [
        (1, 0),
        (0, source.len() + 1),
        (start + 1, end),
        (start, end - 1),
        (usize::MAX, usize::MAX),
    ] {
        let error = ErrorContext::from_source_with_span(&source, start, end, "")
            .expect_err("invalid range cannot produce even an empty context");
        let SourceExcerptError::InvalidRange {
            start: actual_start,
            end: actual_end,
            source_len,
        } = error
        else {
            panic!("invalid source range must be distinguished from coordinate capacity");
        };
        assert_eq!(
            (actual_start, actual_end, source_len),
            (start, end, source.len())
        );
        assert!(error.to_string().contains("not a UTF-8 slice"));
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
        if let Some(original_context) = error.context.as_ref()
            && !error.location.span.is_dummy()
        {
            // A confirmed source range permits a fresh, snippet-relative
            // context. Keep the diagnostic's found/expected evidence verbatim;
            // do not confuse this excerpt with any reconstructed old context.
            let excerpt = talkbank_model::ErrorContext::from_source_with_span(
                source,
                origin,
                error.location.span.end as usize,
                original_context.found.clone(),
            )
            .expect("confirmed source range")
            .with_expected(original_context.expected.clone());
            assert_eq!(excerpt.source_text, fragment);
            assert_eq!(excerpt.span, Span::from_usize(0, fragment.len()));
            assert_eq!(
                excerpt.line_offset,
                Some(
                    source.as_bytes()[..origin]
                        .iter()
                        .filter(|byte| **byte == b'\n')
                        .count()
                        + 1
                )
            );
            assert_eq!(excerpt.found, original_context.found);
            assert_eq!(excerpt.expected, original_context.expected);
            let decoded: talkbank_model::ErrorContext = serde_json::from_value(
                serde_json::to_value(&excerpt).expect("source excerpt wire"),
            )
            .expect("decode source excerpt");
            assert_eq!(decoded, excerpt);
        }
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
    use talkbank_model::SourceLocation;
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
        // Source-aware API admission is separate from display enrichment:
        // it must preserve real diagnostic spans, not clamp malformed input.
        for diagnostic in &original {
            for label in &diagnostic.labels {
                let located = SourceLocation::from_offsets_in_source(
                    label.span.start as usize,
                    label.span.end as usize,
                    fixture.source(),
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "{}: {} label must belong to its source: {error:?}",
                        fixture.path().display(),
                        diagnostic.code,
                    )
                });
                assert_eq!(located.span, label.span);
                assert!(
                    fixture
                        .source()
                        .get(label.span.start as usize..label.span.end as usize,)
                        .is_some(),
                    "label must select complete UTF-8 source bytes"
                );
            }
            let span = diagnostic.location.span;
            if span.is_dummy() {
                continue;
            }
            let located = SourceLocation::from_offsets_in_source(
                span.start as usize,
                span.end as usize,
                fixture.source(),
            )
            .expect("concrete diagnostic range belongs to its source");
            let point =
                SourceLocation::from_offset_in_source(span.start as usize, fixture.source())
                    .expect("diagnostic start belongs to its source");
            let (line, column) = index.line_col_of(span.start);
            assert_eq!(located.span, span);
            assert_eq!(
                (located.line, located.column),
                (Some(line + 1), Some(column + 1))
            );
            assert_eq!((point.line, point.column), (located.line, located.column));
            assert_eq!(point.span.start, point.span.end);
        }
        let end = fixture.source().len();
        let eof = SourceLocation::from_offset_in_source(end, fixture.source())
            .expect("zero-width EOF location is legitimate");
        assert_eq!(eof.span.start as usize, end);
        assert_eq!(eof.span.start, eof.span.end);
        assert!(SourceLocation::from_offset_in_source(end + 1, fixture.source()).is_err());
        assert!(SourceLocation::from_offsets_in_source(1, 0, fixture.source()).is_err());
        assert!(SourceLocation::from_offsets_in_source(0, end + 1, fixture.source()).is_err());
        enhance_errors_with_index(&mut rendered, &index);
        assert_eq!(original.len(), rendered.len());
        let shared_source =
            miette::NamedSource::new("spec.cha", std::sync::Arc::new(fixture.source().to_owned()));
        for (raw, enhanced) in original.iter().zip(&rendered) {
            // Embedded display context and full-file source are different
            // coordinate spaces. The standalone helper consumes the former;
            // the shared-source helper must also accept untouched diagnostics.
            let standalone = talkbank_transform::render_error_with_miette(enhanced);
            let shared_raw =
                talkbank_transform::render_error_with_miette_with_named_source(raw, &shared_source);
            let shared_enhanced = talkbank_transform::render_error_with_miette_with_named_source(
                enhanced,
                &shared_source,
            );
            for text in [&standalone, &shared_raw, &shared_enhanced] {
                assert!(
                    text.contains(&raw.code.to_string()),
                    "diagnostic code lost: {text}"
                );
            }
            assert_eq!(
                shared_raw,
                talkbank_transform::render_error_with_miette_with_source(
                    raw,
                    "spec.cha",
                    fixture.source(),
                ),
                "shared ownership must not change source resolution"
            );
            assert_eq!(
                shared_enhanced,
                talkbank_transform::render_error_with_miette_with_source(
                    enhanced,
                    "spec.cha",
                    fixture.source(),
                ),
                "embedded display context must win over fallback source"
            );
        }
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
                assert!(report.text.contains(&expected.code.to_string()));
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
            let offset = (after.location.span.start as usize).min(fixture.source().len());
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

/// Retired delimiter codes must not defer away the actual source boundary.
#[test]
fn delimiter_specs_reject_recovery_without_inventing_a_balance_diagnosis() {
    use talkbank_model::{ErrorCode, Severity};
    let parser = TreeSitterParser::new().expect("parser");
    let corpus =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (spec, example, valid) in [
        ("E312", 1, false),
        ("E312", 2, false),
        ("E313", 1, false),
        ("E313", 2, true),
        ("E313", 3, false),
    ] {
        let source = std::fs::read_to_string(corpus.join(format!("{spec}_{example}.cha")))
            .expect("canonical delimiter spec");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        file.validate(&errors, TranscriptName::Anonymous);
        let diagnostics = errors.into_vec();
        assert_eq!(
            diagnostics.iter().any(|e| e.severity == Severity::Error),
            !valid,
            "{spec}_{example}: {diagnostics:?}"
        );
        assert!(diagnostics.iter().all(|e| !matches!(
            e.code,
            ErrorCode::UnclosedBracket | ErrorCode::UnclosedParenthesis
        )));
    }
}
