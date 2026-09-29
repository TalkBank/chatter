//! Filesystem boundary contracts, using canonical CHAT rather than synthetic files.

use talkbank_model::ParseValidateOptions;
use talkbank_model::model::{SemanticEq, TranscriptName};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;
use talkbank_transform::{PipelineError, parse_and_validate_named, parse_file_and_validate};

/// The collecting public API must retain the opt-in policy boundary: disabling
/// strict quotation checks must not disable indexed overlap diagnostics.
#[test]
fn canonical_cross_utterance_collection_preserves_rule_selection() {
    use std::sync::Arc;
    use talkbank_model::validation::cross_utterance::check_cross_utterance_patterns;
    use talkbank_model::validation::{SharedValidationData, ValidationContext};

    let parser = TreeSitterParser::new().expect("parser");
    for (spec, default_codes, strict_codes) in [
        ("E341_2", &[][..], &["E341"][..]),
        ("E341_3", &[][..], &[][..]),
        ("E341_5", &[][..], &[][..]),
        ("E346_3", &[][..], &[][..]),
        // The orphan opens the chain; its following quoted turn continues it.
        ("E346_4", &[][..], &["E346"][..]),
        ("E347_1", &["E347"][..], &["E347"][..]),
        ("E347_2", &[][..], &[][..]),
        ("E347_3", &["E347"][..], &["E347"][..]),
        ("E347_4", &["E347", "E347"][..], &["E347", "E347"][..]),
        ("E704_2", &[][..], &[][..]),
        ("E704_3", &["E704"][..], &["E704"][..]),
    ] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{spec}.cha",
        )))
        .expect("canonical quotation or overlap specimen");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("spec syntax parses");
        for (strict, expected) in [(false, default_codes), (true, strict_codes)] {
            // This phase reads only the rule-selection flag; header and word
            // validity remain the responsibility of full-file admission.
            let context = ValidationContext::from_shared(Arc::new(SharedValidationData {
                enable_quotation_validation: strict,
                ..SharedValidationData::default()
            }));
            let diagnostics = check_cross_utterance_patterns(&file, &context);
            let codes: Vec<_> = diagnostics
                .iter()
                .map(|error| error.code.to_string())
                .collect();
            assert_eq!(codes, expected, "{spec}, strict={strict}");
        }
    }
}

#[test]
fn spec_lenient_parsing_retains_recovery_and_primary_diagnostics() {
    use talkbank_model::model::DependentTier;
    use talkbank_model::{ErrorCode, ErrorCollector};
    use talkbank_transform::parse::{parse_lenient, parse_strict};
    let parser = TreeSitterParser::new().expect("parser");
    for (fixture, generated_recovery) in [("E702_1", true), ("E600_1", true), ("E370_3", false)] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{fixture}.cha",
        )))
        .expect("canonical malformed tier specimen");
        let errors = ErrorCollector::new();
        let original = parser.parse_chat_file_streaming(&source, &errors);
        let original_errors = errors.into_vec();
        assert!(
            !original_errors.is_empty(),
            "{fixture}: actual parse failure"
        );
        assert!(parse_strict(&parser, &source).is_err());
        let (mut recovered, retained) = parse_lenient(&parser, &source);
        assert!(
            original.semantic_eq(&recovered),
            "leniency cannot erase recovered model slots"
        );
        let generated_spans: Vec<_> = original
            .utterances()
            .flat_map(|utterance| {
                utterance
                    .dependent_tiers
                    .iter()
                    .filter_map(|entry| match &entry.tier {
                        DependentTier::Mor(_) | DependentTier::Gra(_) => Some(entry.span()),
                        _ => None,
                    })
            })
            .collect();
        match generated_recovery {
            true => {
                assert!(
                    retained.len() < original_errors.len(),
                    "{fixture}: generated diagnostics suppressed"
                );
                for error in &original_errors {
                    if !retained.contains(error) {
                        let offset = error.location.span.start;
                        assert!(
                            generated_spans
                                .iter()
                                .any(|span| span.start <= offset && offset < span.end),
                            "{fixture}: suppression must belong to a typed generated tier"
                        );
                    }
                }
                let validation = ErrorCollector::new();
                recovered.validate_with_alignment(&validation, TranscriptName::Anonymous);
                assert!(
                    validation
                        .into_vec()
                        .iter()
                        .any(|error| error.code == ErrorCode::TierValidationError),
                    "ignored parse diagnostics do not restore alignment trust"
                );
            }
            false => {
                assert_eq!(
                    retained, original_errors,
                    "primary speech errors remain visible"
                );
                assert!(
                    retained
                        .iter()
                        .any(|error| error.code == ErrorCode::StructuralOrderError)
                );
            }
        }
    }
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/core/basic-conversation.cha"),
    )
    .expect("clean reference control");
    let strict = parse_strict(&parser, &source).expect("clean source admitted");
    let (lenient, errors) = parse_lenient(&parser, &source);
    assert!(errors.is_empty());
    assert!(strict.semantic_eq(&lenient));
}

#[test]
fn spec_lenient_parsing_does_not_hide_similarly_named_tier_errors() {
    use talkbank_model::{ErrorCode, ErrorCollector};
    use talkbank_transform::parse::parse_lenient;
    let parser = TreeSitterParser::new().expect("parser");
    for fixture in ["E315_8", "E315_9"] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{fixture}.cha",
        )))
        .expect("canonical misleading-prefix specimen");
        let errors = ErrorCollector::new();
        let original = parser.parse_chat_file_streaming(&source, &errors);
        let errors = errors.into_vec();
        assert!(
            errors
                .iter()
                .any(|error| error.code == ErrorCode::InvalidControlCharacter)
        );
        let (recovered, retained) = parse_lenient(&parser, &source);
        assert!(original.semantic_eq(&recovered));
        assert_eq!(
            retained, errors,
            "only exact generated-tier ownership permits suppression"
        );
    }
}

#[test]
fn reference_files_preserve_parsed_models() {
    use talkbank_model::model::{ChatFileLines, Header, WriteChat};
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("canonical reference corpus");
    let mut id_witnesses = 0;
    for fixture in corpus.fixtures() {
        let expected = strict_parse(parser.parse_chat_file(fixture.source()))
            .expect("reference parses cleanly");
        let actual = parse_file_and_validate(fixture.path(), ParseValidateOptions::default())
            .expect("reference file parses cleanly");
        assert!(
            expected.semantic_eq(&actual),
            "{}",
            fixture.path().display()
        );
        assert_eq!(expected.to_chat(), expected.to_chat_string());
        let mut ids = expected.id_headers();
        for header in expected.headers() {
            if let Header::ID(id) = header {
                assert!(std::ptr::eq(
                    ids.next().expect("ID view must retain each header"),
                    id
                ));
                id_witnesses += 1;
            }
        }
        assert!(
            ids.next().is_none(),
            "ID view cannot invent or duplicate headers"
        );
        // Rebuild an editor's ordered line buffer from the admitted source,
        // retaining interleaved headers, utterances, payloads and exact spans.
        // An empty buffer is a construction state, not an admitted CHAT file.
        let mut lines = ChatFileLines::new(Vec::new());
        assert!(lines.is_empty());
        for line in expected.lines.clone() {
            lines.insert(lines.len(), line);
        }
        assert!(!lines.is_empty(), "reference file must contain its headers");
        assert_eq!(lines, expected.lines);
        // Move out and restore boundary/interior lines through the public
        // collection operations; no arithmetic-derived index escapes this owner.
        for index in [0, lines.len() / 2, lines.len() - 1] {
            let removed = lines.remove(index);
            assert_eq!(&removed, &expected.lines[index]);
            lines.insert(index, removed);
            assert_eq!(
                lines, expected.lines,
                "line order and spans must survive editing"
            );
        }
    }
    assert!(
        id_witnesses > 0,
        "reference files must exercise participant ID views"
    );
}

/// Streaming is a reporting boundary, not permission to admit invalid CHAT.
/// The named path must retain the same validation decision even with no sink.
#[test]
fn canonical_streaming_boundaries_preserve_models_and_required_refusals() {
    use talkbank_model::{ErrorCollector, NullErrorSink};
    use talkbank_transform::{
        parse_and_validate_streaming, parse_and_validate_streaming_for_path,
        parse_and_validate_streaming_named, parse_and_validate_streaming_with_parser,
    };

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    for fixture in corpus.fixtures() {
        let expected =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        let errors = ErrorCollector::new();
        for actual in [
            parse_and_validate_streaming(
                fixture.source(),
                ParseValidateOptions::default(),
                &errors,
            ),
            parse_and_validate_streaming_with_parser(
                &parser,
                fixture.source(),
                ParseValidateOptions::default(),
                &errors,
            ),
            parse_and_validate_streaming_for_path(
                fixture.path(),
                fixture.source(),
                ParseValidateOptions::default(),
                &errors,
            ),
        ] {
            assert!(
                expected.semantic_eq(&actual.expect("clean streaming parse")),
                "{}",
                fixture.path().display()
            );
        }
        assert!(errors.into_vec().is_empty(), "{}", fixture.path().display());
    }

    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("spec corpus");
    let mut admitted = 0;
    let mut refused = 0;
    for fixture in corpus.fixtures() {
        let options = ParseValidateOptions::default()
            .with_alignment()
            .with_strict_linkers();
        let expected_errors = ErrorCollector::new();
        let actual_errors = ErrorCollector::new();
        let expected = parse_and_validate_streaming_named(
            &parser,
            fixture.source(),
            options.clone(),
            &expected_errors,
            TranscriptName::for_path(fixture.path()),
        );
        let actual = parse_and_validate_streaming_for_path(
            fixture.path(),
            fixture.source(),
            options.clone(),
            &actual_errors,
        );
        let silent = parse_and_validate_streaming_for_path(
            fixture.path(),
            fixture.source(),
            options,
            &NullErrorSink,
        );
        assert_eq!(
            expected_errors.into_vec(),
            actual_errors.into_vec(),
            "{}",
            fixture.path().display()
        );
        match (expected, actual, silent) {
            (Ok(expected), Ok(actual), Ok(silent)) => {
                assert!(expected.semantic_eq(&actual));
                assert!(expected.semantic_eq(&silent));
                admitted += 1;
            }
            (Err(expected), Err(actual), Err(silent)) => {
                assert_eq!(
                    std::mem::discriminant(&expected),
                    std::mem::discriminant(&actual)
                );
                assert_eq!(
                    std::mem::discriminant(&expected),
                    std::mem::discriminant(&silent)
                );
                refused += 1;
            }
            results => panic!(
                "streaming admission differs for {}: {results:?}",
                fixture.path().display()
            ),
        }
    }
    assert!(
        admitted > 0 && refused > 0,
        "both admission transitions exercised"
    );
}

#[test]
fn spec_files_preserve_named_admission_and_refusal_evidence() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical spec corpus");
    let mut outcomes = [0; 3];
    for fixture in corpus.fixtures() {
        // This is the storage-name boundary, not the manifest-authored name
        // used by the separate specification-claim runner.
        let name = TranscriptName::for_path(fixture.path());
        for options in [
            ParseValidateOptions::default(),
            ParseValidateOptions::default().with_validation(),
            ParseValidateOptions::default()
                .with_alignment()
                .with_strict_linkers(),
        ] {
            let expected =
                parse_and_validate_named(&parser, fixture.source(), options.clone(), name);
            let actual = parse_file_and_validate(fixture.path(), options.clone());
            if let (Err(expected), Err(actual)) = (&expected, &actual) {
                let summary = actual.to_string();
                assert!(
                    !summary.is_empty(),
                    "file refusal must have a user-facing summary"
                );
                assert_eq!(
                    summary,
                    expected.to_string(),
                    "file I/O must preserve the refusal summary"
                );
                match actual {
                    PipelineError::Parse(_) => assert!(summary.starts_with("Parse errors: ")),
                    PipelineError::Validation(errors) => assert_eq!(
                        summary,
                        format!("Validation failed with {} errors", errors.len()),
                    ),
                    PipelineError::IncompleteValidation(failure) => {
                        assert_eq!(summary, failure.to_string())
                    }
                    _ => {} // The exhaustive outcome comparison below rejects unexpected variants.
                }
            }
            match (expected, actual) {
                (Ok(expected), Ok(actual)) => {
                    assert!(
                        expected.semantic_eq(&actual),
                        "{}",
                        fixture.path().display()
                    );
                    outcomes[0] += 1;
                }
                (Err(PipelineError::Parse(expected)), Err(PipelineError::Parse(actual))) => {
                    assert_eq!(expected.errors, actual.errors);
                    outcomes[1] += 1;
                }
                (
                    Err(PipelineError::Validation(expected)),
                    Err(PipelineError::Validation(actual)),
                ) => {
                    assert_eq!(expected, actual);
                    outcomes[2] += 1;
                }
                (
                    Err(PipelineError::IncompleteValidation(expected)),
                    Err(PipelineError::IncompleteValidation(actual)),
                ) => {
                    assert_eq!(expected.name(), name);
                    assert_eq!(actual.name(), name);
                    assert_eq!(expected.policy(), actual.policy());
                    assert_eq!(expected.diagnostics(), actual.diagnostics());
                    assert_eq!(
                        expected.has_incomplete_parse(),
                        actual.has_incomplete_parse()
                    );
                    assert!(expected.document().semantic_eq(actual.document()));
                    outcomes[2] += 1;
                }
                (expected, actual) => panic!(
                    "file boundary differs: {} {options:?}: {expected:?} / {actual:?}",
                    fixture.path().display()
                ),
            }
        }
    }
    assert!(
        outcomes.iter().all(|count| *count > 0),
        "must witness admission and parse/validation refusals: {outcomes:?}"
    );
}

#[test]
fn fixture_directory_is_an_io_refusal_not_empty_chat() {
    let directory = workspace_root().join("corpus/reference");
    assert!(
        directory.is_dir(),
        "canonical reference directory must exist"
    );
    // A filesystem root exists but cannot supply a transcript basename. Even
    // caller-supplied valid CHAT must not bypass that named-input admission.
    let root = directory
        .ancestors()
        .last()
        .expect("absolute reference path has a root");
    assert!(root.has_root());
    let source_path = directory.join("core/basic-conversation.cha");
    let source = std::fs::read_to_string(&source_path).expect("reference control");
    let errors = talkbank_model::ErrorCollector::new();
    let accepted = talkbank_transform::parse_and_validate_streaming_for_path(
        &source_path,
        &source,
        ParseValidateOptions::default(),
        &errors,
    );
    assert!(accepted.is_ok() && errors.is_empty());
    let refused = talkbank_transform::parse_and_validate_streaming_for_path(
        root,
        &source,
        ParseValidateOptions::default(),
        &errors,
    )
    .expect_err("a root has no transcript name");
    let PipelineError::Io(cause) = refused else {
        panic!("name admission must fail before CHAT processing");
    };
    assert_eq!(cause.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        errors.is_empty(),
        "I/O refusal must not invent CHAT diagnostics"
    );
    let error = parse_file_and_validate(&directory, ParseValidateOptions::default())
        .expect_err("directory is not a CHAT file");
    let PipelineError::Io(cause) = &error else {
        panic!("directory must be an I/O refusal: {error:?}");
    };
    assert_eq!(error.to_string(), format!("I/O error: {cause}"));
    use talkbank_transform::speaker_id::{
        RecordedSpeakerIdentificationAttempt, RecordedSpeakerIdentificationInput,
    };
    for (input, label) in [
        (RecordedSpeakerIdentificationInput::Donor, "donor"),
        (RecordedSpeakerIdentificationInput::Reference, "reference"),
    ] {
        let record = RecordedSpeakerIdentificationAttempt::input_rejected(input, &error);
        let wire = serde_json::to_value(record).expect("I/O refusal evidence");
        assert_eq!(wire["schema_version"], 1);
        assert_eq!(wire["outcome"], "input_rejected");
        assert_eq!(wire["input"], label);
        assert_eq!(wire["failure_kind"], "io");
        assert_eq!(wire["diagnostic_codes"], serde_json::json!([]));
        assert!(
            wire.get("match_report").is_none(),
            "unread input supplies no lexical evidence"
        );
    }
}

#[cfg(unix)]
#[test]
fn media_spec_stored_identity_survives_argument_aliases_and_directory_changes() {
    use talkbank_model::model::FileStem;
    use talkbank_transform::paths::{StoredNameResolver, StoredTranscript};
    let source = std::fs::read(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/W109_4.cha"),
    )
    .expect("canonical media control");
    let directory = tempfile::tempdir().expect("isolated filesystem boundary");
    let stored_path = directory.path().join("Schlu\u{0308}ssel.cha");
    std::fs::write(&stored_path, &source).expect("write canonical bytes");
    let entry = std::fs::read_dir(directory.path())
        .expect("directory")
        .next()
        .expect("entry")
        .expect("read entry");
    let admitted = StoredTranscript::from_entry(entry).expect("directory entry admission");
    assert_eq!(admitted.path(), stored_path);
    assert_eq!(
        admitted.name(),
        TranscriptName::Named(FileStem::from_stem("Schlu\u{0308}ssel"))
    );
    // Establish an old directory timestamp without a timing-dependent sleep.
    let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    std::fs::File::open(directory.path())
        .expect("directory handle")
        .set_times(std::fs::FileTimes::new().set_modified(old))
        .expect("directory timestamp");
    let mut resolver = StoredNameResolver::default();
    for spelling in ["Schlu\u{0308}ssel.cha", "Schlüssel.cha", "SCHLüSSEL.cha"] {
        let requested = directory.path().join(spelling);
        if requested.exists() {
            let resolved = resolver
                .resolve(&requested)
                .expect("existing normalized alias");
            assert_eq!(resolved.path(), stored_path);
            assert_eq!(resolved.name(), admitted.name());
        } else {
            assert_eq!(
                resolver
                    .resolve(&requested)
                    .expect_err("not a filesystem alias")
                    .kind(),
                std::io::ErrorKind::NotFound
            );
        }
    }
    // A differently named file cannot reuse the old directory snapshot.
    let renamed = directory.path().join("other.cha");
    std::fs::rename(&stored_path, &renamed).expect("rename test copy");
    let resolved = resolver
        .resolve(&renamed)
        .expect("refresh changed directory");
    assert_eq!(resolved.path(), renamed);
    assert_eq!(
        resolved.name(),
        TranscriptName::Named(FileStem::from_stem("other"))
    );
    assert_eq!(
        std::fs::read(resolved.path()).expect("unchanged bytes"),
        source
    );
    assert_eq!(
        resolver
            .resolve(&stored_path)
            .expect_err("removed name is not cached admission")
            .kind(),
        std::io::ErrorKind::NotFound
    );
}

#[cfg(target_os = "linux")]
#[test]
fn media_spec_non_utf8_disk_names_refuse_instead_of_becoming_anonymous() {
    use std::os::unix::ffi::OsStringExt;
    use talkbank_transform::paths::StoredTranscript;
    let source = std::fs::read(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/W109_4.cha"),
    )
    .expect("canonical media control");
    let directory = tempfile::tempdir().expect("isolated filesystem boundary");
    let path = directory
        .path()
        .join(std::ffi::OsString::from_vec(b"name\xff.cha".to_vec()));
    std::fs::write(&path, source).expect("Linux permits non-UTF-8 filenames");
    assert_eq!(
        StoredTranscript::resolve(&path)
            .expect_err("requested name refusal")
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    let entry = std::fs::read_dir(directory.path())
        .expect("directory")
        .next()
        .expect("entry")
        .expect("read entry");
    assert_eq!(
        StoredTranscript::from_entry(entry)
            .expect_err("stored name refusal")
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert!(matches!(
        parse_file_and_validate(&path, ParseValidateOptions::default().with_validation()),
        Err(PipelineError::Io(_))
    ));
}

#[cfg(unix)]
#[test]
fn media_spec_symlink_keeps_its_own_named_validation_context() {
    use talkbank_transform::paths::StoredTranscript;
    let target = workspace_root()
        .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/W109_4.cha");
    let directory = tempfile::tempdir().expect("isolated filesystem boundary");
    let link = directory.path().join("Schlüssel.cha");
    std::os::unix::fs::symlink(&target, &link).expect("transcript link");
    let stored = StoredTranscript::resolve(&link).expect("resolve link name");
    assert_eq!(
        stored.path(),
        link,
        "do not canonicalize to the target's basename"
    );
    parse_file_and_validate(&link, ParseValidateOptions::default().with_validation())
        .expect("media matches the link name, not W109_4");
    std::fs::remove_file(&link).expect("remove test link only");
    assert!(
        target.exists(),
        "canonical fixture survives removal of the link"
    );
    assert!(matches!(
        parse_file_and_validate(&link, ParseValidateOptions::default()),
        Err(PipelineError::Io(_))
    ));
    assert_eq!(
        StoredTranscript::resolve(std::path::Path::new("/"))
            .expect_err("no transcript basename")
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
}
