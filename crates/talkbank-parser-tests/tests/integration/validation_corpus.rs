//! Public validation boundaries over parsed canonical data and its JSON wire form.

#![allow(clippy::expect_used)]

use talkbank_model::model::{ChatFile, SemanticEq, TranscriptName};
use talkbank_model::validation::Validate;
use talkbank_model::{ErrorCode, ErrorCollector, Severity};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, repo_paths::workspace_root};

/// Spelling diagnostics stream in timestamp order only after numeric admission.
#[test]
fn leading_zero_specs_preserve_multiplicity_order_and_overflow_precedence() {
    let parser = TreeSitterParser::new().expect("parser");
    for (name, roles, overflow) in [
        ("E748_1.cha", &["start"][..], false),
        ("E748_2.cha", &["end"][..], false),
        ("E748_3.cha", &[][..], false),
        ("E748_4.cha", &["start", "end"][..], false),
        ("E748_5.cha", &[][..], true),
    ] {
        let source = std::fs::read_to_string(
            workspace_root()
                .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors")
                .join(name),
        )
        .expect("canonical timestamp specimen");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        let diagnostics = errors.into_vec();
        if overflow {
            assert_eq!(diagnostics.len(), 1, "{name}");
            assert_eq!(diagnostics[0].code, ErrorCode::InvalidMediaBullet, "{name}");
        } else {
            assert_eq!(diagnostics.len(), roles.len(), "{name}");
            let bullet = file
                .utterances()
                .next()
                .expect("utterance")
                .main
                .content
                .bullet
                .as_ref()
                .expect("numeric bullet retained");
            for (diagnostic, role) in diagnostics.iter().zip(roles) {
                assert_eq!(diagnostic.code, ErrorCode::LeadingZeroBulletTime, "{name}");
                assert!(
                    diagnostic
                        .message
                        .starts_with(&format!("Bullet {role} time")),
                    "{name}"
                );
                assert_eq!(diagnostic.location.span, bullet.span, "{name}");
            }
        }
    }
}

/// Indexed orphan diagnostics retain the originating main-tier span, not a
/// file-line index or the other speaker's tier. Headers precede every specimen.
#[test]
fn overlap_specs_preserve_orphan_origin_locations() {
    let parser = TreeSitterParser::new().expect("parser");
    for (name, orphan_indices) in [
        ("E347_1.cha", &[0usize][..]),
        ("E347_2.cha", &[][..]),
        ("E347_3.cha", &[1usize][..]),
        ("E347_4.cha", &[0usize, 1][..]),
        ("E347_5.cha", &[][..]),
        ("E347_6.cha", &[][..]),
        ("E347_7.cha", &[][..]),
    ] {
        let source = std::fs::read_to_string(
            workspace_root()
                .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors")
                .join(name),
        )
        .expect("canonical overlap specimen");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.is_empty(), "{name}");
        let expected: Vec<_> = file
            .utterances()
            .enumerate()
            .filter(|(index, _)| orphan_indices.contains(index))
            .map(|(_, utterance)| utterance.main.span)
            .collect();
        file.validate_with_rules(
            talkbank_model::RuleSelection::new(),
            &errors,
            TranscriptName::Anonymous,
        );
        let diagnostics = errors.into_vec();
        assert_eq!(diagnostics.len(), expected.len(), "{name}");
        for (diagnostic, span) in diagnostics.iter().zip(expected) {
            assert_eq!(diagnostic.code, ErrorCode::UnbalancedOverlap, "{name}");
            assert_eq!(diagnostic.location.span, span, "{name}");
        }
    }
}

/// Underline policy is utterance-wide, including replacement targets; the
/// shared traversal must preserve the authored marker's diagnostic location.
#[test]
fn underline_specs_preserve_pairing_and_marker_locations() {
    let parser = TreeSitterParser::new().expect("parser");
    for (name, expected) in [
        ("E356_2.cha", None),
        ("E356_5.cha", None),
        ("E356_7.cha", None),
        ("E356_3.cha", Some(ErrorCode::UnmatchedUnderlineBegin)),
        ("E356_4.cha", Some(ErrorCode::UnmatchedUnderlineBegin)),
        ("E356_6.cha", Some(ErrorCode::UnmatchedUnderlineBegin)),
        ("E356_8.cha", Some(ErrorCode::UnmatchedUnderlineBegin)),
        ("E357_3.cha", Some(ErrorCode::UnmatchedUnderlineEnd)),
        ("E357_4.cha", Some(ErrorCode::UnmatchedUnderlineEnd)),
        ("E357_6.cha", Some(ErrorCode::UnmatchedUnderlineEnd)),
    ] {
        let source = std::fs::read_to_string(
            workspace_root()
                .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors")
                .join(name),
        )
        .expect("canonical underline specimen");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.is_empty(), "{name}");
        file.validate_with_rules(
            talkbank_model::RuleSelection::new(),
            &errors,
            TranscriptName::Anonymous,
        );
        let diagnostics = errors.into_vec();
        assert_eq!(diagnostics.len(), usize::from(expected.is_some()), "{name}");
        for diagnostic in diagnostics {
            assert_eq!(Some(diagnostic.code), expected, "{name}");
            let marker = if diagnostic.code == ErrorCode::UnmatchedUnderlineBegin {
                "\u{2}\u{1}"
            } else {
                "\u{2}\u{2}"
            };
            let span = diagnostic.location.span;
            assert_eq!(
                &source[span.start as usize..span.end as usize],
                marker,
                "{name}"
            );
        }
    }
}

/// An absent SES field is not an unsupported comma-only value; neither state
/// may be silently replaced with a recognized demographic value.
#[test]
fn ses_specs_distinguish_absent_from_unsupported_separator() {
    use talkbank_model::model::{Header, SesValue};
    let parser = TreeSitterParser::new().expect("parser");
    for (name, expected) in [
        ("E546_vocabulary_7.cha", None),
        ("E546_vocabulary_8.cha", Some(",")),
    ] {
        let source = std::fs::read_to_string(
            workspace_root()
                .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors")
                .join(name),
        )
        .expect("canonical SES absence specimen");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.is_empty(), "{name}");
        let ids: Vec<_> = file
            .headers()
            .filter_map(|header| match header {
                Header::ID(id) => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(ids.len(), 1);
        match (&ids[0].ses, expected) {
            (None, None) => {}
            (Some(SesValue::Unsupported(text)), Some(expected)) => assert_eq!(text, expected),
            observed => panic!("unexpected SES state in {name}: {observed:?}"),
        }
        file.validate_with_rules(
            talkbank_model::RuleSelection::new(),
            &errors,
            TranscriptName::Anonymous,
        );
        let diagnostics = errors.into_vec();
        assert_eq!(diagnostics.len(), usize::from(expected.is_some()), "{name}");
        for diagnostic in diagnostics {
            assert_eq!(diagnostic.code, ErrorCode::UnsupportedSesValue);
            assert_eq!(diagnostic.severity, Severity::Error);
        }
    }
}

/// Replacement markers participate exactly once in utterance-wide pairing;
/// an unmatched target marker retains its source word location.
#[test]
fn ca_replacement_specs_count_each_authored_marker_once() {
    let parser = TreeSitterParser::new().expect("parser");
    for (name, expected) in [("E230_2.cha", 0), ("E230_3.cha", 1)] {
        let source = std::fs::read_to_string(
            workspace_root()
                .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors")
                .join(name),
        )
        .expect("canonical CA replacement specimen");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.is_empty(), "{name}");
        file.validate_with_rules(
            talkbank_model::RuleSelection::new(),
            &errors,
            TranscriptName::Anonymous,
        );
        let diagnostics = errors.into_vec();
        assert_eq!(diagnostics.len(), expected, "{name}");
        for diagnostic in diagnostics {
            assert_eq!(diagnostic.code, ErrorCode::UnbalancedCADelimiter);
            let span = diagnostic.location.span;
            assert_eq!(&source[span.start as usize..span.end as usize], "°quiet");
        }
    }
}

/// Preserve the authored subtype and identify the invalid head in the
/// diagnostic, with its location bound to the actual parsed dependency tier.
#[test]
fn gra_subtype_specs_preserve_label_and_diagnostic_location() {
    let parser = TreeSitterParser::new().expect("parser");
    for (name, label, invalid) in [
        ("E761_4.cha", "NSUBJ-CUSTOM-PART", false),
        ("E761_5.cha", "NSUBJJ-CUSTOM-PART", true),
    ] {
        let source = std::fs::read_to_string(
            workspace_root()
                .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors")
                .join(name),
        )
        .expect("canonical dependency subtype specimen");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&source, &errors);
        assert!(errors.is_empty(), "{name}");
        let utterance = file.utterances().next().expect("utterance");
        let tier = utterance.gra_tier().expect("authored dependency tier");
        assert_eq!(
            tier.relations()
                .first()
                .expect("first relation")
                .relation
                .as_str(),
            label
        );
        file.validate_with_rules(
            talkbank_model::RuleSelection::new(),
            &errors,
            TranscriptName::Anonymous,
        );
        let diagnostics = errors.into_vec();
        assert_eq!(diagnostics.len(), usize::from(invalid), "{name}");
        for diagnostic in diagnostics {
            assert_eq!(diagnostic.code, ErrorCode::GraRelationHeadNotUniversal);
            assert_eq!(diagnostic.severity, Severity::Error);
            assert_eq!(diagnostic.location.span, tier.span);
            assert!(diagnostic.message.contains("1|2|NSUBJJ-CUSTOM-PART"));
            assert!(diagnostic.message.contains("(head \"NSUBJJ\")"));
        }
    }
}

/// CHECK accepts an unknown birth date; retain its actual typed header rather
/// than silently replacing it with a missing header or a fabricated date.
#[test]
fn empty_birth_date_spec_preserves_typed_header_and_validation() {
    use talkbank_model::model::Header;
    let source = std::fs::read_to_string(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E545_8.cha"),
    )
    .expect("canonical empty birth date");
    let parser = TreeSitterParser::new().expect("parser");
    let errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(&source, &errors);
    assert!(errors.is_empty());
    let births: Vec<_> = file
        .headers()
        .filter_map(|header| match header {
            Header::Birth { participant, date } => Some((participant, date)),
            _ => None,
        })
        .collect();
    assert_eq!(births.len(), 1);
    assert_eq!(births[0].0.as_str(), "CHI");
    assert_eq!(births[0].1.as_str(), "");
    file.validate_with_rules(
        talkbank_model::RuleSelection::new(),
        &errors,
        TranscriptName::Anonymous,
    );
    assert!(errors.is_empty());
}

/// Optional linker policy and diagnostic locations stay source-bound across
/// quotation attribution, first-turn completion, and interruption consumption.
#[test]
fn linker_specs_bind_lifecycle_locations_and_selected_policy() {
    use talkbank_model::RuleSelection;
    let parser = TreeSitterParser::new().expect("parser");
    for (name, code, turn, strict_count) in [
        (
            "E344_3.cha",
            ErrorCode::InvalidContentAnnotationNesting,
            2,
            0,
        ),
        (
            "E344_4.cha",
            ErrorCode::InvalidContentAnnotationNesting,
            2,
            1,
        ),
        ("E351_1.cha", ErrorCode::MissingQuoteBegin, 0, 1),
        ("E352_1.cha", ErrorCode::MissingQuoteEnd, 1, 1),
        ("E352_2.cha", ErrorCode::MissingQuoteEnd, 4, 0),
        ("E352_3.cha", ErrorCode::MissingQuoteEnd, 4, 1),
        ("E354_2.cha", ErrorCode::MissingTrailingOffTerminator, 1, 0),
        ("E354_3.cha", ErrorCode::MissingTrailingOffTerminator, 1, 1),
        ("E354_4.cha", ErrorCode::MissingTrailingOffTerminator, 1, 1),
    ] {
        let source = std::fs::read_to_string(
            workspace_root()
                .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors")
                .join(name),
        )
        .expect("canonical linker specimen");
        for strict in [false, true] {
            let parse_errors = ErrorCollector::new();
            let file = parser.parse_chat_file_streaming(&source, &parse_errors);
            assert!(parse_errors.is_empty(), "{name}");
            let target_span = file.utterances().nth(turn).expect("target turn").main.span;
            if code == ErrorCode::MissingTrailingOffTerminator {
                // The control has a real trailing-off token; both deletion
                // variants retain typed absence, including CA's legal absence.
                let predecessor = file.utterances().next().expect("preceding turn");
                assert_eq!(
                    predecessor.main.content.terminator.is_none(),
                    strict_count == 1
                );
            }
            let rules = if strict {
                RuleSelection::new().with_strict_linkers()
            } else {
                RuleSelection::new()
            };
            let errors = ErrorCollector::new();
            file.validate_with_rules(rules, &errors, TranscriptName::Anonymous);
            let diagnostics = errors.into_vec();
            let selected: Vec<_> = diagnostics
                .iter()
                .filter(|error| error.code == code)
                .collect();
            assert_eq!(
                selected.len(),
                if strict { strict_count } else { 0 },
                "{name}: strict={strict}"
            );
            for error in selected {
                assert_eq!(error.location.span, target_span, "{name}");
            }
        }
    }
}

/// Validation evidence is an in-memory capability, not part of either wire
/// format. Editing consumes it; decoding JSON must not reconstruct it.
#[test]
fn canonical_validation_proofs_preserve_output_but_not_wire_authority() {
    use talkbank_model::{NullErrorSink, WriteChat};
    let parser = TreeSitterParser::new().expect("parser");
    let mut accepted = 0;
    let mut rejected = [0; 2];
    let mut wire_refused = 0;
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("canonical corpus");
        for fixture in corpus.fixtures() {
            let parsed = parser.parse_chat_file_streaming(fixture.source(), &ErrorCollector::new());
            // This is model admission only. It does not erase the independent
            // obligation to reject source parsing errors in a file pipeline.
            match parsed
                .clone()
                .validate_into(&NullErrorSink, TranscriptName::Anonymous)
            {
                Ok(proof) => {
                    let wire = serde_json::to_string(&proof).expect("accepted model serialization");
                    assert_eq!(
                        wire,
                        serde_json::to_string(proof.document()).expect("payload serialization")
                    );
                    let mut output = String::new();
                    proof.write_chat(&mut output).expect("accepted CHAT output");
                    let mut payload_output = String::new();
                    proof
                        .document()
                        .write_chat(&mut payload_output)
                        .expect("payload CHAT output");
                    assert_eq!(
                        output, payload_output,
                        "proof must not change CHAT spelling"
                    );
                    let policy = proof.policy();
                    let decoded: ChatFile = serde_json::from_str(&wire).expect("wire model");
                    assert!(decoded.semantic_eq(proof.document()));
                    if decoded.utterances().next().is_some() {
                        let failure = decoded
                            .validate_with_policy(policy, &NullErrorSink, TranscriptName::Anonymous)
                            .expect_err("wire content cannot reconstruct parser provenance");
                        assert!(failure.has_incomplete_parse());
                        assert!(
                            failure
                                .to_string()
                                .contains("unknown or recovered parse provenance")
                        );
                        wire_refused += 1;
                    }
                    let editable = proof.into_unchecked();
                    assert!(
                        editable.semantic_eq(&parsed),
                        "consuming proof retains authored content"
                    );
                    assert!(
                        editable
                            .validate_with_policy(policy, &NullErrorSink, TranscriptName::Anonymous)
                            .is_ok(),
                        "unchanged, source-origin content may be admitted again"
                    );
                    accepted += 1;
                }
                Err(failure) => {
                    let incomplete = failure.has_incomplete_parse();
                    let message = failure.to_string();
                    assert!(message.starts_with("model validation failed"));
                    assert_eq!(
                        message.contains(": unknown or recovered parse provenance"),
                        incomplete
                    );
                    for diagnostic in failure.diagnostics() {
                        assert!(
                            message.contains(&format!(
                                "\n  {} {}",
                                diagnostic.code.as_str(),
                                diagnostic.message
                            )),
                            "{}: rejection presentation retains every diagnostic",
                            fixture.path().display()
                        );
                    }
                    let editable = failure.into_unchecked();
                    assert!(
                        editable.semantic_eq(&parsed),
                        "repair admission retains rejected content"
                    );
                    rejected[usize::from(incomplete)] += 1;
                }
            }
        }
    }
    assert!(
        accepted > 0 && wire_refused > 0 && rejected.iter().all(|count| *count > 0),
        "canonical witnesses required: accepted={accepted}, wire_refused={wire_refused}, rejected={rejected:?}"
    );
}

/// The convenience entry point must honor the same requested rule selection as
/// explicit validation, including strict linkers and alignment-derived state.
#[test]
fn corpus_pipeline_options_preserve_requested_validation_policy() {
    use talkbank_model::{ParseValidateOptions, RuleSelection, validate_chat_file_with_options};
    let parser = TreeSitterParser::new().expect("parser");
    let mut compared = 0;
    let mut strict_witnesses = 0;
    let mut accepted = 0;
    let mut refused = 0;
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("canonical corpus");
        for fixture in corpus.fixtures() {
            let parsed = parser.parse_chat_file_streaming(fixture.source(), &ErrorCollector::new());
            for validate in [false, true] {
                for alignment in [false, true] {
                    for strict_linkers in [false, true] {
                        let options = ParseValidateOptions {
                            validate,
                            alignment,
                            strict_linkers,
                        };
                        let mut actual = parsed.clone();
                        let result = validate_chat_file_with_options(&mut actual, &options);
                        let mut expected = parsed.clone();
                        let errors = ErrorCollector::new();
                        if validate || alignment {
                            let rules = if strict_linkers {
                                RuleSelection::new().with_strict_linkers()
                            } else {
                                RuleSelection::new()
                            };
                            if alignment {
                                expected.validate_with_alignment_and_rules(
                                    rules,
                                    &errors,
                                    TranscriptName::Anonymous,
                                );
                            } else {
                                expected.validate_with_rules(
                                    rules,
                                    &errors,
                                    TranscriptName::Anonymous,
                                );
                            }
                        }
                        let diagnostics = errors.into_vec();
                        strict_witnesses += diagnostics
                            .iter()
                            .filter(|error| error.code == ErrorCode::MissingQuoteBegin)
                            .count();
                        match &result {
                            Ok(()) => accepted += 1,
                            Err(_) => refused += 1,
                        }
                        assert_eq!(
                            result.is_ok(),
                            diagnostics.is_empty(),
                            "{} {options:?}",
                            fixture.path().display()
                        );
                        assert_eq!(
                            result.err().unwrap_or_default(),
                            diagnostics,
                            "requested policy in {} {options:?}",
                            fixture.path().display()
                        );
                        assert!(
                            actual.semantic_eq(&expected),
                            "validation cannot change CHAT semantics"
                        );
                        // Wire equality additionally includes serialized derived alignment state.
                        assert_eq!(
                            serde_json::to_value(&actual).expect("actual model"),
                            serde_json::to_value(&expected).expect("expected model")
                        );
                        compared += 1;
                    }
                }
            }
        }
    }
    assert!(compared > 0 && strict_witnesses > 0 && accepted > 0 && refused > 0);
}

#[test]
fn canonical_validation_preserves_header_scope_and_wire_provenance() {
    let parser = TreeSitterParser::new().expect("parser");
    let mut alignment_errors = 0;
    let mut provenance_warnings = 0;
    let mut header_diagnostics = 0;
    let mut metadata_warnings = [0; 8];
    let mut cached_pairs = 0;
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("canonical corpus");
        for fixture in corpus.fixtures() {
            // Retain the real recovery model too; taint must suppress checks
            // whose input is incomplete, not turn partial output into proof.
            let parse_errors = ErrorCollector::new();
            let file = parser.parse_chat_file_streaming(fixture.source(), &parse_errors);
            let headers = ErrorCollector::new();
            let context = file.validate_headers_only(&headers, TranscriptName::Anonymous);
            let full = ErrorCollector::new();
            file.validate(&full, TranscriptName::Anonymous);
            let header_errors = headers.to_vec();
            assert!(
                full.to_vec().starts_with(&header_errors),
                "header-only checks must be full validation's initial phase: {}",
                fixture.path().display()
            );
            header_diagnostics += header_errors.len();
            let through_trait = ErrorCollector::new();
            Validate::validate(&file, &context, &through_trait);
            assert_eq!(
                full.to_vec(),
                through_trait.to_vec(),
                "anonymous trait validation: {}",
                fixture.path().display()
            );

            assert!(
                file.utterances()
                    .all(|utterance| !utterance.parse_health.is_unknown()),
                "parser must establish provenance: {}",
                fixture.path().display()
            );
            let parsed_alignment = file.validate_alignments();
            assert!(
                parsed_alignment
                    .iter()
                    .all(|error| error.severity == Severity::Error),
                "parsed alignment must not report unknown provenance: {}",
                fixture.path().display()
            );
            alignment_errors += parsed_alignment.len();

            let json = serde_json::to_string(&file).expect("serialize parsed corpus model");
            let decoded: ChatFile = serde_json::from_str(&json).expect("decode corpus wire model");
            assert!(
                file.semantic_eq(&decoded),
                "wire semantics: {}",
                fixture.path().display()
            );
            let mut expected_warnings = 0;
            for utterance in decoded.utterances() {
                assert!(
                    utterance.parse_health.is_unknown(),
                    "wire data cannot certify parser provenance"
                );
                expected_warnings += usize::from(utterance.mor_tier().is_some());
                expected_warnings +=
                    usize::from(utterance.mor_tier().is_some() && utterance.gra_tier().is_some());
                expected_warnings += usize::from(utterance.pho_tier().is_some());
                expected_warnings += usize::from(utterance.sin_tier().is_some());
            }
            let warnings = decoded.validate_alignments();
            assert_eq!(
                warnings.len(),
                expected_warnings,
                "wire alignment: {}",
                fixture.path().display()
            );
            assert!(
                warnings
                    .iter()
                    .all(|warning| warning.code == ErrorCode::TierValidationError
                        && warning.severity == Severity::Warning),
                "unknown wire provenance must not become alignment error/proof: {}",
                fixture.path().display()
            );
            provenance_warnings += warnings.len();
            // Exercise the derived-metadata producer, not only the separate
            // validation facade. Recomputed wire alignments must not reuse
            // parser-origin evidence which JSON deliberately cannot preserve.
            let wire_context =
                decoded.validate_headers_only(&ErrorCollector::new(), TranscriptName::Anonymous);
            for utterance in decoded.utterances() {
                let mut computed = utterance.clone();
                computed.compute_alignments(&wire_context);
                assert!(computed.parse_health.is_unknown());
                assert!(
                    computed.semantic_eq(utterance),
                    "metadata computation cannot edit content"
                );
                let metadata = computed
                    .alignments
                    .as_ref()
                    .expect("alignment metadata produced");
                for (total, count) in metadata_warnings
                    .iter_mut()
                    .zip(unknown_metadata(&computed))
                {
                    *total += count;
                }
                assert!(
                    metadata.wor_timings.is_none(),
                    "unknown provenance cannot produce a timing-sidecar binding"
                );
                assert_eq!(
                    metadata
                        .collect_errors()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>(),
                    computed.alignment_diagnostics
                );
                assert_eq!(
                    metadata.is_error_free(),
                    computed.alignment_diagnostics.is_empty()
                );
                let first = metadata.clone();
                let diagnostics = computed.alignment_diagnostics.clone();
                computed.compute_alignments(&wire_context);
                assert_eq!(
                    computed.alignments.as_ref(),
                    Some(&first),
                    "recomputation is stable"
                );
                assert_eq!(
                    computed.alignment_diagnostics, diagnostics,
                    "warnings must not accumulate on recomputation"
                );
            }
            for utterance in file.utterances() {
                let mut computed = utterance.clone();
                computed.compute_alignments(&context);
                assert_eq!(
                    computed.parse_health, utterance.parse_health,
                    "metadata cannot promote parser recovery"
                );
                assert!(computed.semantic_eq(utterance));
                let metadata = computed
                    .alignments
                    .as_ref()
                    .expect("parsed metadata produced");
                assert_eq!(
                    metadata
                        .collect_errors()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>(),
                    computed.alignment_diagnostics
                );
                assert_eq!(
                    metadata.is_error_free(),
                    computed.alignment_diagnostics.is_empty()
                );
                let first = metadata.clone();
                let diagnostics = computed.alignment_diagnostics.clone();
                computed.compute_alignments(&context);
                assert_eq!(computed.alignments.as_ref(), Some(&first));
                assert_eq!(computed.alignment_diagnostics, diagnostics);
                // A wire model may already contain serialized alignment pairs.
                // Their presence cannot restore the parser's provenance proof.
                let mut cached: talkbank_model::Utterance = serde_json::from_str(
                    &serde_json::to_string(&computed).expect("serialize computed metadata"),
                )
                .expect("decode cached metadata");
                assert!(cached.parse_health.is_unknown());
                assert!(cached.semantic_eq(&computed));
                let held = cached
                    .alignments
                    .as_ref()
                    .expect("wire retains legacy alignment payload");
                let pair_count = held.mor.as_ref().map_or(0, |a| a.pairs.len())
                    + held.gra.as_ref().map_or(0, |a| a.pairs.len())
                    + held.pho.as_ref().map_or(0, |a| a.pairs.len())
                    + held.mod_.as_ref().map_or(0, |a| a.pairs.len())
                    + held.sin.as_ref().map_or(0, |a| a.pairs.len())
                    + held.modsyl.as_ref().map_or(0, |a| a.pairs.len())
                    + held.phosyl.as_ref().map_or(0, |a| a.pairs.len())
                    + held.phoaln.as_ref().map_or(0, |a| a.pairs.len());
                cached_pairs += pair_count;
                cached.compute_alignments(&context);
                unknown_metadata(&cached);
                assert!(
                    cached
                        .alignments
                        .as_ref()
                        .expect("recomputed metadata")
                        .wor_timings
                        .is_none()
                );
                assert!(
                    cached.semantic_eq(&computed),
                    "discarding cached trust cannot edit content"
                );
            }
        }
    }
    assert!(
        alignment_errors > 0,
        "specs must witness diagnosed alignment faults"
    );
    assert!(
        provenance_warnings > 0,
        "wire models must witness provenance warnings"
    );
    assert!(
        header_diagnostics > 0,
        "specs must witness header diagnostics"
    );
    assert!(
        metadata_warnings.iter().all(|count| *count > 0),
        "every structural metadata family needs blocked-provenance witnesses: {metadata_warnings:?}"
    );
    assert!(
        cached_pairs > 0,
        "serialized real alignment pairs must witness cache invalidation"
    );
}

fn unknown_metadata(utterance: &talkbank_model::Utterance) -> [usize; 8] {
    assert!(utterance.parse_health.is_unknown());
    let metadata = utterance.alignments.as_ref().expect("computed metadata");
    [
        unknown_alignment(metadata.mor.as_ref(), utterance.mor_tier().is_some()),
        unknown_alignment(
            metadata.gra.as_ref(),
            utterance.mor_tier().is_some() && utterance.gra_tier().is_some(),
        ),
        unknown_alignment(metadata.pho.as_ref(), utterance.pho_tier().is_some()),
        unknown_alignment(metadata.mod_.as_ref(), utterance.mod_tier().is_some()),
        unknown_alignment(metadata.sin.as_ref(), utterance.sin_tier().is_some()),
        unknown_alignment(
            metadata.modsyl.as_ref(),
            utterance.modsyl_tier().is_some() && utterance.mod_tier().is_some(),
        ),
        unknown_alignment(
            metadata.phosyl.as_ref(),
            utterance.phosyl_tier().is_some() && utterance.pho_tier().is_some(),
        ),
        unknown_alignment(metadata.phoaln.as_ref(), utterance.phoaln_tier().is_some()),
    ]
}

fn unknown_alignment<T: talkbank_model::alignment::TierAlignmentResult>(
    alignment: Option<&T>,
    expected: bool,
) -> usize {
    assert_eq!(
        alignment.is_some(),
        expected,
        "tier presence determines whether alignment is attempted"
    );
    let Some(alignment) = alignment else {
        return 0;
    };
    assert!(
        alignment.pairs().is_empty(),
        "unknown provenance cannot produce trusted index pairs"
    );
    assert_eq!(
        alignment.errors().len(),
        1,
        "one explicit provenance warning per blocked alignment"
    );
    let warning = &alignment.errors()[0];
    assert_eq!(warning.code, ErrorCode::TierValidationError);
    assert_eq!(warning.severity, Severity::Warning);
    assert!(warning.message.contains("provenance is unknown"));
    1
}
