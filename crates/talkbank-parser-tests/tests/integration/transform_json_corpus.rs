//! JSON schema policy must not bypass authored CHAT admission requirements.

use talkbank_model::model::{ChatFile, FileStem, SemanticEq, TranscriptName};
use talkbank_model::{ParseValidateOptions, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;
use talkbank_spec_vocabulary::validation_manifest::{FixtureTranscriptName, ValidationManifest};
use talkbank_transform::{JsonSchemaPolicy, PipelineError, chat_to_json_with_schema_policy};

#[path = "ca_omission_import_corpus.rs"]
mod ca_omission_import_corpus;

/// Unknown imported annotations retain their payload and owning source span;
/// deserialization is not validation, even for a nonempty annotation list.
#[test]
fn reference_unknown_annotation_imports_fail_admission_without_repair() {
    use talkbank_model::model::{ContentAnnotation, Line, UtteranceContent};
    use talkbank_model::{ErrorCode, ErrorCollector};

    let path = workspace_root().join("corpus/reference/annotation/groups-regular.cha");
    let source = std::fs::read_to_string(&path).expect("authored annotated words and groups");
    let parser = TreeSitterParser::new().expect("parser");
    let original = strict_parse(parser.parse_chat_file(&source)).expect("control syntax");
    let control_errors = ErrorCollector::new();
    original
        .clone()
        .validate_into(&control_errors, TranscriptName::Anonymous)
        .expect("unchanged control is valid");

    for text in ["", "imported detail"] {
        let wire = serde_json::json!({"type": "unknown", "marker": "qq", "text": text});
        let imported: ContentAnnotation = serde_json::from_value(wire.clone())
            .expect("unknown annotations are retained by the wire model");
        assert_eq!(
            serde_json::to_value(&imported).expect("retained annotation"),
            wire
        );
        // Lossless output is not admission: an imported unknown annotation
        // remains invalid even when its spelling can be preserved for an editor.
        let spelling = if text.is_empty() {
            "[qq]"
        } else {
            "[qq imported detail]"
        };
        assert_eq!(imported.to_string(), spelling);
        assert!(
            super::serialization_sink_contracts::assert_output_refusals(
                |mut writer| imported.write_chat(&mut writer),
                &path,
            ) > 0
        );
        let mut file = original.clone();
        let mut expected_spans = Vec::new();
        let mut word_hosts = 0;
        let mut group_hosts = 0;
        for line in &mut file.lines {
            let Line::Utterance(utterance) = line else {
                continue;
            };
            for item in &mut utterance.main.content.content {
                let (annotations, span) = match item {
                    UtteranceContent::AnnotatedWord(word) => {
                        word_hosts += 1;
                        (&mut word.scoped_annotations, word.span)
                    }
                    UtteranceContent::AnnotatedGroup(group) => {
                        group_hosts += 1;
                        (&mut group.scoped_annotations, group.span)
                    }
                    _ => continue,
                };
                // Mutate a payload through the nonempty owner's public iterator;
                // no empty annotated wrapper or parser recovery is fabricated.
                *annotations
                    .into_iter()
                    .next()
                    .expect("nonempty by construction") = imported.clone();
                expected_spans.push(span);
            }
        }
        assert!(word_hosts > 0 && group_hosts > 0);
        let before = serde_json::to_value(&file).expect("imported document");
        let errors = ErrorCollector::new();
        let refusal = file
            .validate_into(&errors, TranscriptName::Anonymous)
            .expect_err("unknown annotation cannot acquire a valid-document proof");
        assert!(!refusal.has_internal_failure());
        let findings = errors.into_vec();
        let unknown: Vec<_> = findings
            .iter()
            .filter(|error| error.code == ErrorCode::UnknownAnnotation)
            .collect();
        assert_eq!(unknown.len(), expected_spans.len(), "{findings:?}");
        let message = if text.is_empty() {
            "could not read [qq] as a scoped annotation"
        } else {
            "could not read [qq imported detail] as a scoped annotation"
        };
        for (finding, span) in unknown.into_iter().zip(expected_spans) {
            assert_eq!(finding.location.span, span);
            assert_eq!(finding.message, message);
        }
        assert_eq!(
            serde_json::to_value(refusal.document()).expect("retained rejected document"),
            before
        );
    }
}

#[test]
fn reference_json_content_deletion_requires_model_admission() {
    use talkbank_model::{ErrorCode, ErrorCollector};
    let path = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    let source = std::fs::read_to_string(&path).expect("reference source");
    let parser = TreeSitterParser::new().expect("parser");
    let original = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let wire = serde_json::to_value(&original).expect("external document shape");
    use talkbank_transform::json::{
        JsonError, is_schema_validation_available, schema_load_error, validate_json_string,
    };
    assert!(
        is_schema_validation_available(),
        "embedded schema must load for import"
    );
    assert_eq!(schema_load_error(), None);
    let encoded = serde_json::to_string(&wire).expect("reference JSON text");
    validate_json_string(&encoded).expect("unchanged reference satisfies the wire schema");
    let mut truncated = encoded.clone();
    assert_eq!(truncated.pop(), Some('}'), "document JSON is an object");
    let error = validate_json_string(&truncated).expect_err("truncated external JSON");
    assert!(
        matches!(error, JsonError::SerializationError(_)),
        "JSON syntax failure is not schema drift or CHAT invalidity"
    );
    assert!(error.to_string().starts_with("JSON serialization failed: "));
    let control: ChatFile = serde_json::from_value(wire.clone()).expect("unchanged JSON");
    assert!(control.semantic_eq(&original));
    let errors = ErrorCollector::new();
    // JSON retains model values, not parser provenance. Even the unchanged
    // control cannot mint ValidChatFile from unknown parse health.
    let control_failure = control
        .validate_into(&errors, TranscriptName::Anonymous)
        .expect_err("JSON does not confer parser-backed validity");
    assert!(control_failure.has_incomplete_parse());
    assert!(control_failure.diagnostics().is_empty());
    assert!(errors.is_empty());
    // Missing parser provenance cannot become speaker-match evidence, even
    // when the imported model has no CHAT diagnostics.
    use talkbank_transform::speaker_id::{
        RecordedInputFailureKind, RecordedSpeakerIdentificationAttempt,
        RecordedSpeakerIdentificationInput, RecordedSpeakerIdentificationOutcome,
    };
    let incomplete = PipelineError::IncompleteValidation(Box::new(control_failure));
    for input in [
        RecordedSpeakerIdentificationInput::Donor,
        RecordedSpeakerIdentificationInput::Reference,
    ] {
        let record = RecordedSpeakerIdentificationAttempt::input_rejected(input, &incomplete);
        assert!(matches!(
            &record.outcome,
            RecordedSpeakerIdentificationOutcome::InputRejected {
                input: recorded_input,
                failure_kind: RecordedInputFailureKind::IncompleteValidation,
                diagnostic_codes,
            } if *recorded_input == input && diagnostic_codes.is_empty()
        ));
        let wire = serde_json::to_value(record).expect("incomplete admission record");
        assert_eq!(wire["failure_kind"], "incomplete_validation");
        assert_eq!(wire["outcome"], "input_rejected");
        assert_eq!(wire["diagnostic_codes"], serde_json::json!([]));
        assert!(wire.get("match_report").is_none());
    }
    assert!(
        matches!(&incomplete, PipelineError::IncompleteValidation(failure)
        if failure.diagnostics().is_empty()
            && failure.document().semantic_eq(&original))
    );

    let mut empty = wire.clone();
    let turn = empty["lines"]
        .as_array_mut()
        .expect("document lines")
        .iter_mut()
        .find(|line| line["line_type"] == "utterance")
        .expect("actual reference turn");
    let content = &mut turn["main"]["content"]["content"];
    assert!(
        !content
            .as_array()
            .expect("reference content items")
            .is_empty()
    );
    *content = serde_json::json!([]);
    // This is deliberately malformed external JSON, not a parser-produced
    // empty tier and not permission to clear recovery provenance on an AST.
    let imported: ChatFile =
        serde_json::from_value(empty).expect("JSON shape alone admits an array");
    let errors = ErrorCollector::new();
    assert!(
        imported
            .validate_into(&errors, TranscriptName::Anonymous)
            .is_err(),
        "deserialization must not certify empty main-tier content"
    );
    let diagnostics = errors.into_vec();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, ErrorCode::EmptyUtterance);
    assert!(diagnostics[0].message.contains("no content after speaker"));

    for malformed in [
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!(42),
    ] {
        let mut invalid_shape = wire.clone();
        let turn = invalid_shape["lines"]
            .as_array_mut()
            .expect("lines")
            .iter_mut()
            .find(|line| line["line_type"] == "utterance")
            .expect("reference turn");
        turn["main"]["content"]["content"] = malformed;
        let invalid_text = serde_json::to_string(&invalid_shape).expect("syntactically valid JSON");
        let error = validate_json_string(&invalid_text).expect_err("wrong content shape");
        let JsonError::SchemaValidationError { message } = &error else {
            panic!("wrong wire shape must not masquerade as schema-load failure: {error}");
        };
        assert!(!message.is_empty());
        assert!(
            error
                .to_string()
                .starts_with("JSON schema validation failed: ")
        );
        assert!(
            serde_json::from_value::<ChatFile>(invalid_shape).is_err(),
            "non-array content must fail structural admission"
        );
    }
    assert_eq!(
        serde_json::to_value(&original).expect("unchanged reference"),
        wire
    );
}

#[test]
fn canonical_header_vocabularies_require_string_shaped_json() {
    use std::collections::BTreeSet;
    use talkbank_model::model::{Header, Line};
    use talkbank_parser_tests::chat_corpus::ChatCorpus;

    fn check<T>(value: &T, seen: &mut BTreeSet<String>)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let wire = serde_json::to_value(value).expect("header vocabulary wire");
        let text = wire
            .as_str()
            .expect("header vocabularies are string-valued");
        if !seen.insert(text.to_owned()) {
            return;
        }
        let restored: T = serde_json::from_value(wire.clone()).expect("string admission");
        assert_eq!(
            &restored, value,
            "recognized and unsupported tokens remain distinct"
        );
        // Mutate only the external JSON shape, not CHAT text or parser state.
        for malformed in [
            serde_json::Value::Null,
            serde_json::json!(42),
            serde_json::json!([wire.clone()]),
            serde_json::json!({"value": wire}),
        ] {
            assert!(
                serde_json::from_value::<T>(malformed).is_err(),
                "a wrapper or scalar cannot masquerade as a header token"
            );
        }
    }

    let parser = TreeSitterParser::new().expect("parser");
    let mut seen: [BTreeSet<String>; 6] = std::array::from_fn(|_| BTreeSet::new());
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("canonical corpus");
        for fixture in corpus.fixtures() {
            let errors = talkbank_model::ErrorCollector::new();
            let file = parser.parse_chat_file_streaming(fixture.source(), &errors);
            // Recovered models supply preserved token values, not validity proof.
            for line in &file.lines {
                let Line::Header { header, .. } = line else {
                    continue;
                };
                match header.as_ref() {
                    Header::ID(id) => {
                        if let Some(value) = &id.sex {
                            check(value, &mut seen[0]);
                        }
                    }
                    Header::RecordingQuality { quality } => check(quality, &mut seen[1]),
                    Header::Transcription { transcription } => check(transcription, &mut seen[2]),
                    Header::Media(media) => {
                        check(&media.media_type, &mut seen[3]);
                        if let Some(value) = &media.status {
                            check(value, &mut seen[4]);
                        }
                    }
                    Header::Number { number } => check(number, &mut seen[5]),
                    _ => {}
                }
            }
        }
    }
    assert!(
        seen.iter().all(|tokens| !tokens.is_empty()),
        "each of the six vocabulary families needs a canonical witness"
    );
}

/// External imports may preserve unsupported lexical values for diagnosis;
/// deleting a required participant field must never become valid metadata.
#[test]
fn reference_participant_imports_reject_empty_required_fields_without_repair() {
    use talkbank_model::model::{Header, ParticipantEntry};
    use talkbank_model::validation::{Validate, ValidationContext};
    use talkbank_model::{ErrorCode, ErrorCollector};

    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/core/headers-speaker-info.cha"),
    )
    .expect("participant reference");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference syntax");
    let (header, participant) = file
        .headers()
        .find_map(|header| match header {
            Header::Participants { entries } => entries.iter().next().map(|entry| (header, entry)),
            _ => None,
        })
        .expect("reference participant");
    let context = ValidationContext::default();
    let errors = ErrorCollector::new();
    header.validate(&context, &errors);
    participant.validate(&context, &errors);
    assert!(
        errors.is_empty(),
        "unmodified metadata is the legal control"
    );

    let wire = serde_json::to_value(participant).expect("participant wire payload");
    for (field, code) in [
        ("speaker_code", ErrorCode::EmptyParticipantCode),
        ("role", ErrorCode::EmptyParticipantRole),
    ] {
        let mut mutated = wire.clone();
        *mutated.get_mut(field).expect("required wire field") =
            serde_json::Value::String(String::new());
        let imported: ParticipantEntry = serde_json::from_value(mutated.clone())
            .expect("lexical import retains unsupported strings for validation");
        let errors = ErrorCollector::new();
        imported.validate(&context, &errors);
        let findings = errors.into_vec();
        assert!(
            findings.iter().any(|error| error.code == code),
            "{field}: {findings:?}"
        );
        assert!(
            findings
                .iter()
                .all(|error| error.code != ErrorCode::InternalError)
        );
        assert_eq!(
            serde_json::to_value(&imported).expect("retained refused payload"),
            mutated
        );
    }
}

/// Characterize the current lexical import boundary, not a commitment to keep
/// unchecked deserialization in a future checked-payload API.
#[test]
fn reference_lexical_text_imports_require_nonempty_validation() {
    use talkbank_model::alignment::helpers::{WordItem, walk_words};
    use talkbank_model::model::{NonEmptyString, WordContents};
    use talkbank_model::validation::{Validate, ValidationContext};
    use talkbank_model::{ErrorCode, ErrorCollector, Span};

    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/core/basic-conversation.cha"),
    )
    .expect("reference conversation");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference syntax");
    let mut seed = None;
    for utterance in file.utterances() {
        walk_words(&utterance.main.content.content, None, &mut |item| {
            if let WordItem::Word(word) = item
                && word.raw_text() == "cookies"
            {
                seed = Some(word);
            }
        });
    }
    let word = seed.expect("reference lexical witness");
    // The content collection is an importable draft, not a nonempty-word
    // certificate. Delete its external payload without fabricating a Word or
    // choosing how inconsistent raw text should be repaired.
    let mut content_wire = serde_json::to_value(word.content()).expect("source content wire");
    let control_content: WordContents =
        serde_json::from_value(content_wire.clone()).expect("reference content import");
    let control_errors = ErrorCollector::new();
    control_content.validate(&ValidationContext::default(), &control_errors);
    assert!(control_errors.is_empty());
    let serde_json::Value::Array(items) = &mut content_wire else {
        panic!("word content wire must be an array")
    };
    assert!(!items.is_empty());
    items.clear();
    let imported_content: WordContents = serde_json::from_value(content_wire.clone())
        .expect("content imports retain draft state until validation");
    for (context, span) in [
        (ValidationContext::default(), Span::from_usize(0, 0)),
        (
            ValidationContext::default()
                .with_field_span(word.span)
                .with_field_text(word.raw_text())
                .with_field_label("word content"),
            word.span,
        ),
    ] {
        let errors = ErrorCollector::new();
        imported_content.validate(&context, &errors);
        let findings = errors.into_vec();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, ErrorCode::EmptyWordContent);
        assert_eq!(findings[0].location.span, span);
        assert_eq!(
            serde_json::to_value(&imported_content).expect("unchanged rejected draft"),
            content_wire
        );
    }
    let admitted =
        NonEmptyString::try_from(word.raw_text().as_str()).expect("nonempty derived text");
    assert_eq!(AsRef::<str>::as_ref(&admitted), word.raw_text());
    assert_eq!(admitted.clone().into_inner(), word.raw_text());
    let errors = ErrorCollector::new();
    admitted.validate(&ValidationContext::default(), &errors);
    assert!(errors.is_empty());

    let mut wire = serde_json::to_value(&admitted).expect("lexical wire payload");
    let serde_json::Value::String(text) = &mut wire else {
        panic!("lexical wire must be a string")
    };
    text.clear();
    assert!(
        NonEmptyString::try_from(text.as_str()).is_err(),
        "constructor admission refuses deletion"
    );
    let imported: NonEmptyString = serde_json::from_value(wire.clone())
        .expect("current wire policy retains emptiness for validation");
    for (context, span) in [
        (ValidationContext::default(), Span::from_usize(0, 0)),
        (
            ValidationContext::default()
                .with_field_span(word.span)
                .with_field_text(word.raw_text())
                .with_field_label("word text")
                .with_field_error_code(ErrorCode::EmptyString),
            word.span,
        ),
    ] {
        let errors = ErrorCollector::new();
        imported.validate(&context, &errors);
        let findings = errors.into_vec();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, ErrorCode::EmptyString);
        assert_eq!(findings[0].location.span, span);
        assert_eq!(
            serde_json::to_value(&imported).expect("preserved refusal payload"),
            wire
        );
    }
}

/// Checked construction and JSON import are distinct admission boundaries.
/// Imported strings remain inspectable, but unrepresentable names must reach
/// the header-only validator used by editors, not just whole-file validation.
#[test]
fn media_filename_mutations_retain_constructor_and_json_refusal_evidence() {
    use talkbank_model::model::{Header, Line, MediaFilename, MediaFilenameProblem};
    use talkbank_model::{ErrorCode, ErrorCollector};

    fn filename(file: &ChatFile) -> &MediaFilename {
        file.lines
            .iter()
            .find_map(|line| match line {
                Line::Header { header, .. } => match header.as_ref() {
                    Header::Media(media) => Some(&media.filename),
                    _ => None,
                },
                _ => None,
            })
            .expect("authored media header")
    }

    let parser = TreeSitterParser::new().expect("parser");
    for (path, noncanonical, remote) in [
        ("corpus/reference/core/headers-media.cha", false, false),
        ("corpus/reference/core/headers-media-url.cha", false, true),
        (
            "corpus/reference/core/headers-media-http-url.cha",
            false,
            true,
        ),
        (
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/W109_2.cha",
            true,
            false,
        ),
    ] {
        let source =
            std::fs::read_to_string(workspace_root().join(path)).expect("canonical media control");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("media syntax");
        let name = filename(&file);
        assert_eq!(
            MediaFilename::parse(name.as_str()).expect("checked admission"),
            *name
        );
        assert_eq!(name.needs_unicode_normalization(), noncanonical);
        assert_eq!(name.is_remote_url(), remote);
        assert_eq!(
            file.to_chat_string(),
            source,
            "media inspection preserves source spelling"
        );
        if remote {
            let errors = ErrorCollector::new();
            file.validate_headers_only(&errors, TranscriptName::Anonymous);
            assert!(
                errors.into_vec().is_empty(),
                "opaque URLs need no local-filename repair"
            );
        }
    }

    let source =
        std::fs::read_to_string(workspace_root().join("corpus/reference/core/headers-media.cha"))
            .expect("media seed");
    let original = strict_parse(parser.parse_chat_file(&source)).expect("seed syntax");
    let seed = filename(&original).as_str();
    for (text, reason) in [
        (String::new(), MediaFilenameProblem::Empty),
        (
            format!(" {seed}"),
            MediaFilenameProblem::SurroundingWhitespace,
        ),
        (format!("{seed}\npart"), MediaFilenameProblem::LineBreak),
        (format!("{seed},part"), MediaFilenameProblem::UnquotedComma),
        (format!("{seed}\"part"), MediaFilenameProblem::StrayQuote),
    ] {
        let rejected = MediaFilename::parse(&text).expect_err("unrepresentable edit");
        assert_eq!(rejected.reason, reason);
        assert_eq!(rejected.value, text, "refusal retains the attempted input");
        let imported: MediaFilename = serde_json::from_value(serde_json::json!(text))
            .expect("JSON preserves lexical input for validation");
        let mut file = original.clone();
        let mut replacements = 0;
        for line in &mut file.lines {
            if let Line::Header { header, .. } = line
                && let Header::Media(media) = header.as_mut()
            {
                media.filename = imported.clone();
                replacements += 1;
            }
        }
        assert_eq!(replacements, 1);
        let errors = ErrorCollector::new();
        file.validate_headers_only(&errors, TranscriptName::Anonymous);
        let findings = errors.into_vec();
        let representability: Vec<_> = findings
            .iter()
            .filter(|error| error.code == ErrorCode::MediaFilenameNotRepresentable)
            .collect();
        assert_eq!(representability.len(), 1, "{reason}: {findings:?}");
        assert_eq!(representability[0].message, reason.to_string());
        let admission_errors = ErrorCollector::new();
        assert!(
            file.clone()
                .validate_into(&admission_errors, TranscriptName::Anonymous)
                .is_err()
        );
        assert_eq!(
            admission_errors
                .into_vec()
                .iter()
                .filter(|error| error.code == ErrorCode::MediaFilenameNotRepresentable)
                .count(),
            1,
            "whole-file admission retains the same header refusal",
        );
        assert_eq!(
            filename(&file).as_str(),
            text,
            "validation must not repair imported content"
        );
    }
}

#[test]
fn underline_specs_keep_semantics_without_fabricating_wire_locations() {
    use talkbank_model::alignment::helpers::{ContentItem, walk_content};
    use talkbank_model::model::{UnderlineMarker, Word, WordContent};

    fn inspect_word(word: &Word, inspect: &mut impl FnMut(&UnderlineMarker)) {
        for piece in word.content() {
            if let WordContent::UnderlineBegin(marker) | WordContent::UnderlineEnd(marker) = piece {
                inspect(marker);
            }
        }
    }

    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let manifest: ValidationManifest = serde_json::from_str(
        &std::fs::read_to_string(root.join("manifest.json")).expect("canonical manifest"),
    )
    .expect("producer-owned manifest schema");
    let parser = TreeSitterParser::new().expect("parser");
    for entry in manifest
        .fixtures
        .iter()
        .filter(|entry| matches!(entry.code.as_str(), "E356" | "E357"))
    {
        let source = std::fs::read_to_string(root.join(&entry.fixture)).expect("underline fixture");
        let original =
            strict_parse(parser.parse_chat_file(&source)).expect("underline syntax parses");
        let wire =
            talkbank_transform::json::to_json_validated(&original).expect("schema accepts markers");
        let restored: ChatFile = serde_json::from_str(&wire).expect("typed wire model");
        assert!(
            original.semantic_eq(&restored),
            "source locations are not semantics"
        );
        for (file, located) in [(&original, true), (&restored, false)] {
            let mut markers = 0;
            let mut inspect = |marker: &UnderlineMarker| {
                assert_eq!(marker.span().is_some(), located, "{}", entry.fixture);
                markers += 1;
            };
            for utterance in file.utterances() {
                walk_content(
                    &utterance.main.content.content,
                    None,
                    &mut |item| match item {
                        ContentItem::UnderlineBegin(marker) | ContentItem::UnderlineEnd(marker) => {
                            inspect(marker)
                        }
                        ContentItem::Word(word) => inspect_word(word, &mut inspect),
                        ContentItem::ReplacedWord(replaced) => {
                            inspect_word(&replaced.word, &mut inspect);
                            for word in &replaced.replacement.words {
                                inspect_word(word, &mut inspect);
                            }
                        }
                        ContentItem::Separator(_)
                        | ContentItem::Event(_)
                        | ContentItem::Pause(_)
                        | ContentItem::Action(_)
                        | ContentItem::OverlapPoint(_)
                        | ContentItem::OtherSpokenEvent(_)
                        | ContentItem::Freecode(_)
                        | ContentItem::InternalBullet(_)
                        | ContentItem::LongFeatureBegin(_)
                        | ContentItem::LongFeatureEnd(_)
                        | ContentItem::NonvocalBegin(_)
                        | ContentItem::NonvocalEnd(_)
                        | ContentItem::NonvocalSimple(_) => {}
                    },
                );
            }
            assert!(markers > 0, "spec must witness markers: {}", entry.fixture);
            let errors = talkbank_model::ErrorCollector::new();
            file.validate(&errors, TranscriptName::Anonymous);
            let diagnostics = errors.to_vec();
            assert!(
                entry.claim.satisfied_by(&entry.code, |code| diagnostics
                    .iter()
                    .any(|error| error.code.as_str() == code.as_str())),
                "underline claim after wire transition: {} {diagnostics:?}",
                entry.fixture
            );
        }
    }
}

#[test]
fn media_name_spec_claims_survive_both_schema_policies() {
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let manifest: ValidationManifest = serde_json::from_str(
        &std::fs::read_to_string(root.join("manifest.json")).expect("canonical manifest"),
    )
    .expect("producer-owned manifest schema");
    let parser = TreeSitterParser::new().expect("parser");
    let mut outcomes = [0; 2];
    for entry in manifest
        .fixtures
        .iter()
        .filter(|entry| entry.code.as_str() == "E531")
    {
        let source =
            std::fs::read_to_string(root.join(&entry.fixture)).expect("canonical name fixture");
        let original =
            strict_parse(parser.parse_chat_file(&source)).expect("name examples parse cleanly");
        let name = match &entry.transcript_name {
            FixtureTranscriptName::Anonymous => TranscriptName::Anonymous,
            FixtureTranscriptName::Named(stem) => TranscriptName::Named(FileStem::from_stem(stem)),
        };
        for schema in [JsonSchemaPolicy::Validate, JsonSchemaPolicy::Skip] {
            for pretty in [false, true] {
                match chat_to_json_with_schema_policy(
                    &source,
                    ParseValidateOptions::default().with_validation(),
                    pretty,
                    name,
                    schema,
                ) {
                    Ok(wire) => {
                        assert!(
                            entry.claim.satisfied_by(&entry.code, |_| false),
                            "conversion admitted a violation: {}",
                            entry.fixture
                        );
                        let restored: ChatFile =
                            serde_json::from_str(&wire).expect("typed wire model");
                        assert!(original.semantic_eq(&restored));
                        outcomes[0] += 1;
                    }
                    Err(PipelineError::Validation(errors)) => {
                        assert!(
                            entry.claim.satisfied_by(&entry.code, |code| errors
                                .iter()
                                .any(|error| error.code.as_str() == code.as_str())),
                            "authored name claim: {} {errors:?}",
                            entry.fixture
                        );
                        assert!(
                            errors.iter().any(|error| error.code.as_str() == "E531"),
                            "name mismatch must survive schema policy: {}",
                            entry.fixture
                        );
                        outcomes[1] += 1;
                    }
                    other => panic!(
                        "unexpected conversion outcome: {} {schema:?} {other:?}",
                        entry.fixture
                    ),
                }
            }
        }
    }
    assert!(
        outcomes.iter().all(|count| *count > 0),
        "must witness accepted and refused name mutations: {outcomes:?}"
    );
}
