//! Public transform wire contracts over canonical reference CHAT.

#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::ParseValidateOptions;
use talkbank_model::model::{ChatFile, SemanticEq};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::test_error::strict_parse;

#[path = "alignment_diagnostic_corpus.rs"]
mod alignment_diagnostic_contracts;
#[path = "build_chat_corpus.rs"]
mod build_chat_contracts;
#[path = "cache_corpus.rs"]
mod cache_contracts;
#[path = "catalog_corpus.rs"]
mod catalog_contracts;
#[path = "coordinated_corpus.rs"]
mod coordinated_contracts;
#[path = "diagnostic_corpus.rs"]
mod diagnostic_contracts;
#[path = "transform_file_corpus.rs"]
mod file_contracts;
#[path = "gem_merge_corpus.rs"]
mod gem_merge_contracts;
#[path = "transform_json_corpus.rs"]
mod json_contracts;
#[path = "transform_language_corpus.rs"]
mod language_contracts;
#[path = "merge_corpus.rs"]
mod merge_contracts;
#[path = "rediarize_corpus.rs"]
mod rediarize_contracts;
#[path = "runner_corpus.rs"]
mod runner_contracts;
#[path = "sanitize_corpus.rs"]
mod sanitize_contracts;
#[path = "semantic_diff_corpus.rs"]
mod semantic_diff_contracts;
#[path = "serialization_sink_corpus.rs"]
mod serialization_sink_contracts;
#[path = "splice_corpus.rs"]
mod splice_contracts;
#[path = "transform_tier_corpus.rs"]
mod tier_contracts;
#[path = "validation_corpus.rs"]
mod validation_contracts;
#[path = "wor_timing_corpus.rs"]
mod wor_timing_contracts;

#[test]
fn internal_bullet_spec_enters_linked_media_state() {
    use talkbank_parser_tests::repo_paths::workspace_root;
    use talkbank_transform::media_timing::{MediaTimingState, reconcile_media_timing};

    // E544's legal control establishes that an internal main-tier bullet is
    // timing evidence, even with no utterance-final or dependent-tier bullet.
    let path = workspace_root()
        .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E544_4.cha");
    let input = std::fs::read_to_string(path).expect("canonical internal-bullet spec");
    let parser = TreeSitterParser::new().expect("parser");
    let original = strict_parse(parser.parse_chat_file(&input)).expect("legal spec parses");
    let reconciled = reconcile_media_timing(original.clone()).expect("usable linked media");
    assert!(
        matches!(reconciled, MediaTimingState::Timed(_)),
        "internal timing must produce the linked-media capability"
    );
    assert!(
        original.semantic_eq(reconciled.as_chat_file()),
        "already linked media must preserve semantics"
    );

    // The companion E752 mutation removes the declaration but keeps the same
    // internal timing: it must refuse, not certify an untimed document.
    let missing = std::fs::read_to_string(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E752_2.cha"),
    )
    .expect("canonical missing-media spec");
    let missing = strict_parse(parser.parse_chat_file(&missing)).expect("spec parses");
    assert!(matches!(
        reconcile_media_timing(missing),
        Err(talkbank_transform::media_timing::MediaTimingError::MissingMedia)
    ));
}

#[test]
fn media_name_specs_keep_authored_identity_through_required_validation() {
    use talkbank_model::ErrorCollector;
    use talkbank_model::model::{FileStem, TranscriptName};
    use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
    use talkbank_parser_tests::repo_paths::workspace_root;
    use talkbank_spec_vocabulary::validation_manifest::{
        FixtureTranscriptName, ValidationManifest,
    };

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
        let input = std::fs::read_to_string(root.join(&entry.fixture))
            .expect("canonical media-name fixture");
        let name = match &entry.transcript_name {
            FixtureTranscriptName::Anonymous => TranscriptName::Anonymous,
            FixtureTranscriptName::Named(stem) => TranscriptName::Named(FileStem::from_stem(stem)),
        };
        let errors = ErrorCollector::new();
        let result = talkbank_transform::parse_validated_with_parser(
            &parser,
            &input,
            ValidationPolicy::new(
                entry.rules.selection(),
                AlignmentValidation::IncludeTierAlignment,
            ),
            name,
            &errors,
        );
        let diagnostics = errors.to_vec();
        assert!(
            entry.claim.satisfied_by(&entry.code, |code| diagnostics
                .iter()
                .any(|error| error.code.as_str() == code.as_str())),
            "authored name/claim: {} {:?}",
            entry.fixture,
            diagnostics
        );
        match result {
            Ok(accepted) => {
                assert!(!errors.has_errors());
                assert_eq!(accepted.name(), name);
                outcomes[0] += 1;
            }
            Err(talkbank_transform::ValidatedParseError::Validation(failure)) => {
                assert_eq!(failure.name(), name);
                assert!(errors.has_errors());
                outcomes[1] += 1;
            }
            Err(error) => panic!(
                "media-name examples must parse cleanly: {} {error}",
                entry.fixture
            ),
        }
    }
    assert!(
        outcomes.iter().all(|count| *count > 0),
        "media-name specs need both admission and refusal: {outcomes:?}"
    );
}

#[test]
fn media_specs_reconcile_recorded_timing_without_inventing_it() {
    use talkbank_parser_tests::repo_paths::workspace_root;
    use talkbank_transform::media_timing::{MediaTimingState, reconcile_media_timing};

    enum ExpectedTiming {
        Untimed,
        Linked,
    }
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    // E544's correspondence-only %wor is not timing. E552's two examples
    // carry real timing on distinct surfaces and require removing unlinked.
    for (fixture, expected) in [
        ("E544_1.cha", ExpectedTiming::Untimed),
        ("E544_3.cha", ExpectedTiming::Untimed),
        ("E544_5.cha", ExpectedTiming::Untimed),
        ("E531_2.cha", ExpectedTiming::Untimed),
        ("E552_1.cha", ExpectedTiming::Linked),
        ("E552_2.cha", ExpectedTiming::Linked),
        ("E535_2.cha", ExpectedTiming::Linked),
        ("E536_2.cha", ExpectedTiming::Linked),
    ] {
        let input = std::fs::read_to_string(root.join(fixture)).expect("canonical media spec");
        let original = strict_parse(parser.parse_chat_file(&input)).expect("spec parses cleanly");
        let reconciled = reconcile_media_timing(original.clone()).expect("reconcilable media");
        match (&expected, &reconciled) {
            (ExpectedTiming::Untimed, MediaTimingState::Untimed(file)) => {
                assert!(
                    original.semantic_eq(file.as_chat_file()),
                    "untimed preservation: {fixture}"
                );
            }
            (ExpectedTiming::Linked, MediaTimingState::Timed(file)) => {
                assert!(
                    file.as_chat_file()
                        .media
                        .as_ref()
                        .expect("linked media")
                        .status
                        .is_none()
                );
            }
            _ => panic!("wrong timing capability: {fixture}: {reconciled:?}"),
        }
        let reparsed = strict_parse(parser.parse_chat_file(&reconciled.to_chat_string()))
            .expect("reconciled CHAT parses cleanly");
        assert!(
            reconciled.as_chat_file().semantic_eq(&reparsed),
            "wire semantics: {fixture}"
        );
        let repeated = reconcile_media_timing(reparsed).expect("reconciliation is stable");
        assert_eq!(
            reconciled.to_chat_string(),
            repeated.to_chat_string(),
            "idempotence: {fixture}"
        );
    }
}

#[test]
fn timed_media_spec_mutations_refuse_ambiguous_or_unsupported_declarations() {
    use talkbank_model::model::{MediaStatus, MediaType};
    use talkbank_parser_tests::repo_paths::workspace_root;
    use talkbank_transform::media_timing::{MediaTimingError, reconcile_media_timing};

    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (fixture, expected) in [
        ("E501_7.cha", MediaTimingError::MultipleMedia { count: 2 }),
        (
            "E535_3.cha",
            MediaTimingError::UnusableMediaType {
                media_type: MediaType::Unsupported("badtype".into()),
            },
        ),
        (
            "E536_3.cha",
            MediaTimingError::IncompatibleMediaStatus {
                status: MediaStatus::Unsupported("badstatus".into()),
            },
        ),
    ] {
        let input = std::fs::read_to_string(root.join(fixture)).expect("canonical mutation");
        let parsed =
            strict_parse(parser.parse_chat_file(&input)).expect("mutation retains valid syntax");
        let refusal = reconcile_media_timing(parsed)
            .expect_err("ambiguous or unsupported declaration cannot link timing");
        assert_eq!(
            refusal, expected,
            "typed refusal preserves authored value: {fixture}"
        );
    }
}

#[test]
fn spec_validation_preserves_admission_and_streamed_evidence() {
    use talkbank_model::model::TranscriptName;
    use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
    use talkbank_model::{ErrorCollector, RuleSelection};
    use talkbank_parser_tests::repo_paths::workspace_root;
    use talkbank_transform::{PipelineError, ValidatedParseError};

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical spec fixtures");
    let mut outcomes = [0; 3];
    for fixture in corpus.fixtures() {
        for alignment in [
            AlignmentValidation::Structure,
            AlignmentValidation::IncludeTierAlignment,
        ] {
            let policy = ValidationPolicy::new(RuleSelection::new(), alignment);
            let options = match alignment {
                AlignmentValidation::Structure => ParseValidateOptions::default().with_validation(),
                AlignmentValidation::IncludeTierAlignment => {
                    ParseValidateOptions::default().with_alignment()
                }
            };
            let admitted_errors = ErrorCollector::new();
            let admitted = talkbank_transform::parse_validated_with_parser(
                &parser,
                fixture.source(),
                policy,
                TranscriptName::Anonymous,
                &admitted_errors,
            );
            let streamed_errors = ErrorCollector::new();
            let streamed = talkbank_transform::parse_and_validate_streaming_with_parser(
                &parser,
                fixture.source(),
                options,
                &streamed_errors,
            );
            assert_eq!(
                streamed_errors.to_vec(),
                admitted_errors.to_vec(),
                "streamed evidence: {} {alignment:?}",
                fixture.path().display()
            );
            match (admitted, streamed) {
                (Ok(accepted), Ok(model)) => {
                    assert_eq!(accepted.policy(), policy);
                    assert!(!admitted_errors.has_errors());
                    assert!(
                        accepted.document().semantic_eq(&model),
                        "accepted model: {}",
                        fixture.path().display()
                    );
                    outcomes[0] += 1;
                }
                (Err(ValidatedParseError::Parse(product)), Err(PipelineError::Parse(errors))) => {
                    assert_eq!(product.diagnostics(), errors.errors.as_slice());
                    assert!(
                        admitted_errors.has_errors(),
                        "parse refusal needs evidence: {}",
                        fixture.path().display()
                    );
                    outcomes[1] += 1;
                }
                (
                    Err(ValidatedParseError::Validation(failure)),
                    Err(PipelineError::Validation(errors)),
                ) => {
                    assert_eq!(failure.policy(), policy);
                    assert!(!failure.has_incomplete_parse());
                    assert_eq!(failure.diagnostics(), errors.as_slice());
                    outcomes[2] += 1;
                }
                (
                    Err(ValidatedParseError::Validation(failure)),
                    Err(PipelineError::IncompleteValidation(other)),
                ) => {
                    assert!(failure.has_incomplete_parse());
                    assert_eq!(failure.policy(), other.policy());
                    assert_eq!(failure.diagnostics(), other.diagnostics());
                    assert!(failure.document().semantic_eq(other.document()));
                    outcomes[2] += 1;
                }
                (admitted, streamed) => panic!(
                    "admission differs: {} {alignment:?}: {admitted:?} / {streamed:?}",
                    fixture.path().display()
                ),
            }
        }
    }
    assert!(
        outcomes.iter().all(|count| *count > 0),
        "spec corpus must witness acceptance and both rejection phases: {outcomes:?}"
    );
}

#[test]
fn reference_normalization_preserves_models_and_is_idempotent() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("canonical reference corpus");
    for fixture in corpus.fixtures() {
        let original = strict_parse(parser.parse_chat_file(fixture.source()))
            .expect("reference parses cleanly");
        // Rewrite is the public admission capability for loss-checked output.
        // Do not bypass it by serializing a model directly in this workflow.
        let rewrite =
            talkbank_transform::normalize_chat(fixture.source(), ParseValidateOptions::default())
                .unwrap_or_else(|error| panic!("normalize {}: {error}", fixture.path().display()));
        let reparsed = strict_parse(parser.parse_chat_file(rewrite.text()))
            .expect("normalized reference parses cleanly");
        assert!(
            original.semantic_eq(&reparsed),
            "semantics: {}",
            fixture.path().display()
        );
        let repeated =
            talkbank_transform::normalize_chat(rewrite.text(), ParseValidateOptions::default())
                .expect("normalized output remains rewritable");
        assert_eq!(
            repeated.text(),
            rewrite.text(),
            "idempotence: {}",
            fixture.path().display()
        );
    }
}

#[test]
fn reference_language_retagging_is_reversible_or_refuses_atomically() {
    use talkbank_model::model::LanguageCode;
    use talkbank_transform::retag_language::{RetagRefusal, retag_language};

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("canonical reference corpus");
    let temporary = LanguageCode::new("qaa").expect("nonempty language code");
    let probe_target = LanguageCode::new("qab").expect("nonempty language code");
    let mut accepted = 0;
    let mut refused = 0;
    for fixture in corpus.fixtures() {
        let original = strict_parse(parser.parse_chat_file(fixture.source()))
            .expect("reference parses cleanly");
        // Establish a collision-free temporary name over the transform's
        // complete supported domain, including undeclared word markers.
        let mut probe = original.clone();
        let probe_stats = retag_language(&mut probe, &temporary, &probe_target)
            .expect("temporary code absent from unsupported spans");
        assert!(
            probe_stats.is_empty(),
            "temporary-name collision: {}",
            fixture.path().display()
        );
        assert!(original.semantic_eq(&probe));
        for from in original.languages.iter() {
            let mut unchanged = original.clone();
            assert!(
                !unchanged.languages.retag(from, from),
                "the language-list owner must preserve an identity declaration"
            );
            assert!(original.semantic_eq(&unchanged));
            let identity = retag_language(&mut unchanged, from, from)
                .expect("identity retagging needs no unsupported-span rewrite");
            assert!(
                identity.is_empty(),
                "identity retagging must report no changes"
            );
            assert!(
                original.semantic_eq(&unchanged),
                "identity must preserve declarations and content"
            );
            let mut renamed = original.clone();
            match retag_language(&mut renamed, from, &temporary) {
                Ok(stats) => {
                    assert!(stats.declarations > 0, "declared source must move");
                    let restored_stats = retag_language(&mut renamed, &temporary, from)
                        .expect("supported rename must be reversible");
                    assert_eq!(
                        stats,
                        restored_stats,
                        "notation counts: {}",
                        fixture.path().display()
                    );
                    assert!(
                        original.semantic_eq(&renamed),
                        "inverse semantics: {}",
                        fixture.path().display()
                    );
                    accepted += 1;
                }
                Err(RetagRefusal::NamesCodeInSpan) => {
                    assert!(
                        original.semantic_eq(&renamed),
                        "refusal must be atomic: {}",
                        fixture.path().display()
                    );
                    refused += 1;
                }
            }
        }
    }
    assert!(
        accepted > 0,
        "reference corpus must exercise successful retagging"
    );
    assert!(refused > 0, "reference corpus must exercise span refusal");
}

#[test]
fn language_retagging_matches_authored_reference_output() {
    use talkbank_model::model::{LanguageCode, WriteChat};
    use talkbank_parser_tests::repo_paths::workspace_root;
    use talkbank_transform::retag_language::{RetagStats, retag_language};

    let root = workspace_root().join("corpus/reference/word-features");
    let source = std::fs::read_to_string(root.join("retag-source.cha")).expect("authored source");
    let expected_text =
        std::fs::read_to_string(root.join("retag-expected.cha")).expect("authored target");
    let parser = TreeSitterParser::new().expect("parser");
    for text in [&source, &expected_text] {
        let errors = talkbank_model::ErrorCollector::new();
        talkbank_transform::parse_validated_with_parser(
            &parser,
            text,
            talkbank_model::validation::ValidationPolicy::new(
                talkbank_model::RuleSelection::new(),
                talkbank_model::validation::AlignmentValidation::IncludeTierAlignment,
            ),
            talkbank_model::model::TranscriptName::Anonymous,
            &errors,
        )
        .expect("authored retagging pair must pass required validation");
        assert!(!errors.has_errors());
    }
    let mut actual = strict_parse(parser.parse_chat_file(&source)).expect("source parses");
    let expected = strict_parse(parser.parse_chat_file(&expected_text)).expect("target parses");
    let stats = retag_language(
        &mut actual,
        &LanguageCode::new("sun").expect("source code"),
        &LanguageCode::new("fin").expect("target code"),
    )
    .expect("all authored notations supported");
    assert_eq!(
        stats,
        RetagStats {
            declarations: 1,
            utterance_scopes: 1,
            word_markers: 4
        }
    );
    assert_eq!(actual.to_chat_string(), expected.to_chat_string());
    // The transformed model retains source-spelling provenance in raw_text.
    // Compare models at the wire boundary, where both have new source text.
    let reparsed = strict_parse(parser.parse_chat_file(&actual.to_chat_string()))
        .expect("retagged wire output parses");
    assert!(
        reparsed.semantic_eq(&expected),
        "wire model must match independently authored target"
    );
}

#[test]
fn reference_json_wire_roundtrip_preserves_typed_models() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("canonical reference corpus");
    for fixture in corpus.fixtures() {
        let original = strict_parse(parser.parse_chat_file(fixture.source()))
            .expect("reference parses cleanly");
        for pretty in [false, true] {
            // This is a Rust wire-format roundtrip, not analysis of CHAT through
            // a JSON representation. The expected semantics stay in the AST.
            let wire = talkbank_transform::chat_to_json_unvalidated(
                fixture.source(),
                ParseValidateOptions::default(),
                pretty,
            )
            .unwrap_or_else(|error| panic!("JSON {}: {error}", fixture.path().display()));
            let restored: ChatFile = serde_json::from_str(&wire).expect("typed JSON model");
            assert!(
                original.semantic_eq(&restored),
                "JSON semantics: {}",
                fixture.path().display()
            );
            let checked = talkbank_transform::chat_to_json(
                fixture.source(),
                ParseValidateOptions::default(),
                pretty,
            )
            .unwrap_or_else(|error| {
                panic!("schema-checked JSON {}: {error}", fixture.path().display())
            });
            assert_eq!(
                wire,
                checked,
                "schema checking must not alter output: {}",
                fixture.path().display()
            );
        }
    }
}

#[test]
fn reference_regenerated_tiers_replace_in_place_or_append_cleanly() {
    use talkbank_model::model::{DependentTier, TierSeparator};
    use talkbank_transform::dependent_tiers::replace_or_add_tier;

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("canonical reference corpus");
    let mut witnessed = [0; 4];
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        for utterance in file.utterances() {
            for (index, entry) in utterance.dependent_tiers.iter().enumerate() {
                let family = match &entry.tier {
                    DependentTier::Mor(_) => 0,
                    DependentTier::Gra(_) => 1,
                    DependentTier::Wor(_) => 2,
                    DependentTier::UserDefined(_) => 3,
                    _ => continue, // Only the helper's documented regeneration families.
                };
                let mut replaced = utterance.dependent_tiers.clone();
                replace_or_add_tier(&mut replaced, entry.tier.clone());
                assert_eq!(
                    replaced,
                    utterance.dependent_tiers,
                    "identity replacement preserves order and provenance: {}",
                    fixture.path().display()
                );

                let mut appended = utterance.dependent_tiers.clone();
                let removed = appended.remove(index);
                let retained = appended.clone();
                replace_or_add_tier(&mut appended, removed.tier.clone());
                assert_eq!(appended.len(), retained.len() + 1);
                assert_eq!(&appended[..retained.len()], retained.as_slice());
                let last = appended.last().expect("newly appended entry");
                assert_eq!(last.tier, removed.tier);
                assert_eq!(last.separator, TierSeparator::CLEAN);
                witnessed[family] += 1;
            }
        }
    }
    assert!(
        witnessed.iter().all(|count| *count > 0),
        "reference corpus must exercise all regeneration families: {witnessed:?}"
    );
}

#[test]
fn regenerated_tier_keeps_spec_separator_diagnostic() {
    use talkbank_model::model::{Line, TierSeparator, TranscriptName};
    use talkbank_parser_tests::repo_paths::workspace_root;
    let input = std::fs::read_to_string(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E758_4.cha"),
    )
    .expect("canonical non-CA morphology separator mutation");
    let parser = TreeSitterParser::new().expect("parser");
    let mut file = strict_parse(parser.parse_chat_file(&input)).expect("spec syntax parses");
    let mut witnessed = false;
    for line in &mut file.lines {
        if let Line::Utterance(utterance) = line {
            for entry in utterance.dependent_tiers.clone() {
                assert_ne!(entry.separator, TierSeparator::CLEAN);
                talkbank_transform::dependent_tiers::replace_or_add_tier(
                    &mut utterance.dependent_tiers,
                    entry.tier,
                );
                assert_eq!(utterance.dependent_tiers[0].separator, entry.separator);
                witnessed = true;
            }
        }
    }
    assert!(witnessed);
    let errors = talkbank_model::ErrorCollector::new();
    file.validate(&errors, TranscriptName::Anonymous);
    assert!(
        errors
            .to_vec()
            .iter()
            .any(|error| error.code.as_str() == "E758"),
        "payload replacement must not erase source-spacing evidence"
    );
}
