//! Mechanical fix proposals from actual canonical spec diagnostics.
#![allow(clippy::expect_used, clippy::panic)]

#[path = "catalog_recovery_contracts.rs"]
mod recovery_contracts;
#[path = "catalog_selection_contracts.rs"]
mod selection_contracts;

use talkbank_model::model::TranscriptName;
use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, repo_paths::workspace_root};
use talkbank_transform::splice::{
    BatchSafety, CatalogIneligibility, CatalogResolution, EditProvenance, FixDiagnosis, FixKind,
    admit_edits, apply_edits_verified, mapped_edit_sites, verify_splice,
};

/// Keep observed diagnostics and the recovery model with the exact input used
/// to produce them. A recovered model is not a validity certificate.
struct DiagnosedSource<'a>(FixDiagnosis<'a>);
impl<'a> DiagnosedSource<'a> {
    fn observe(parser: &TreeSitterParser, source: &'a str) -> Self {
        Self::observe_named(parser, source, TranscriptName::Anonymous)
    }
    fn observe_named(parser: &TreeSitterParser, source: &'a str, name: TranscriptName<'a>) -> Self {
        Self(FixDiagnosis::observe(
            parser,
            source,
            ValidationPolicy::new(
                talkbank_model::RuleSelection::new(),
                AlignmentValidation::Structure,
            ),
            name,
        ))
    }
    fn into_diagnosis(self) -> FixDiagnosis<'a> {
        self.0
    }
}
impl<'a> std::ops::Deref for DiagnosedSource<'a> {
    type Target = FixDiagnosis<'a>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[test]
fn media_normalization_specs_repair_only_the_header_token() {
    use talkbank_model::model::FileStem;
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let input = std::fs::read_to_string(root.join("W109_2.cha")).expect("decomposed media spec");
    let expected =
        std::fs::read_to_string(root.join("W109_4.cha")).expect("canonical media control");
    for stem in ["Schlüssel", "Schlu\u{0308}ssel"] {
        let name = TranscriptName::Named(FileStem::from_stem(stem).expect("a stem"));
        let observed = DiagnosedSource::observe_named(&parser, &input, name);
        let warning = observed
            .findings()
            .find(|e| e.code.as_str() == "W109")
            .expect("normalization warning");
        let proposal = warning
            .catalog()
            .into_offer()
            .expect("source-bound token repair");
        assert!(matches!(proposal.safety(), BatchSafety::Mechanical));
        let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind() else {
            panic!("mechanical repair");
        };
        let admitted = admit_edits(observed.document(), edits);
        assert!(admitted.skipped().is_empty());
        assert_eq!(admitted.admitted().len(), 1);
        let output =
            apply_edits_verified(&input, admitted.admitted()).expect("byte-preserving repair");
        assert_eq!(output, expected, "no other source bytes may change");
        let repaired = DiagnosedSource::observe_named(&parser, &output, name);
        assert!(!repaired.findings().any(|e| e.code.as_str() == "E531"));
        let remaining = repaired.findings().find(|e| e.code.as_str() == "W109");
        assert_eq!(remaining.is_some(), stem != "Schlüssel");
        if let Some(warning) = remaining {
            assert!(
                matches!(
                    warning.catalog(),
                    CatalogResolution::Ineligible(CatalogIneligibility::MediaToken)
                ),
                "file-only warning must not propose a header edit"
            );
        }
    }
}

#[test]
fn empty_turn_deletion_never_reassigns_dependent_tiers() {
    use talkbank_transform::splice::EditTarget;
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (name, owns_dependent) in [
        ("E306_separator_only_2.cha", false),
        ("E306_separator_only_3.cha", true),
    ] {
        let source = std::fs::read_to_string(root.join(name)).expect("empty turn spec");
        let observed = DiagnosedSource::observe(&parser, &source);
        let diagnostic = observed
            .findings()
            .find(|error| error.code.as_str() == "E306")
            .expect("separator-only turn diagnosis");
        let proposal = diagnostic.catalog().into_offer();
        if owns_dependent {
            assert!(
                matches!(
                    diagnostic.catalog(),
                    CatalogResolution::Ineligible(CatalogIneligibility::DependentTiers)
                ),
                "deletion must not reattach or discard dependent text"
            );
            continue;
        }
        let proposal = proposal.expect("isolated empty main-tier proposal");
        assert_eq!(proposal.safety(), BatchSafety::Semantic);
        let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind() else {
            panic!("one semantic deletion");
        };
        let [edit] = edits.as_slice() else {
            panic!("one complete main tier")
        };
        let EditTarget::Replace(span) = edit.target() else {
            panic!("deletion range")
        };
        assert_eq!(&source[span.start()..span.end()], "*CHI:\t, .\n");
        assert!(edit.replacement().as_str().is_empty());
        let admitted = admit_edits(observed.document(), edits);
        assert!(admitted.skipped().is_empty());
        let output = apply_edits_verified(&source, admitted.admitted()).expect("exact deletion");
        let repaired = DiagnosedSource::observe(&parser, &output);
        assert!(
            repaired.diagnostics().is_empty(),
            "{:?}",
            repaired.diagnostics()
        );
        assert_eq!(repaired.document().utterances().count(), 1);
    }
}

#[test]
fn missing_end_spec_proposals_preserve_document_boundary_with_or_without_final_newline() {
    use talkbank_transform::splice::{EditTarget, SkipReason};
    let parser = TreeSitterParser::new().expect("parser");
    let fixture = include_str!("../error_corpus/validation_errors/E502_1.cha");
    let unterminated = fixture.strip_suffix('\n').expect("spec ends with newline");
    for ending in ["\n", "\r\n", "\r"] {
        for (text, replacement) in [
            (fixture, format!("@End{ending}")),
            (unterminated, format!("{ending}@End{ending}")),
        ] {
            let transported = text.replace('\n', ending);
            let source = transported.as_str();
            let observed = DiagnosedSource::observe(&parser, source);
            let findings: Vec<_> = observed.findings().collect();
            let [diagnostic] = findings.as_slice() else {
                panic!(
                    "missing end is the sole defect: {:?}",
                    observed.diagnostics()
                );
            };
            assert_eq!(diagnostic.code.as_str(), "E502");
            let proposal = diagnostic
                .catalog()
                .into_offer()
                .expect("document end repair");
            assert!(matches!(proposal.safety(), BatchSafety::Mechanical));
            let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind()
            else {
                panic!("document end repair needs no semantic choice");
            };
            let [edit] = edits.as_slice() else {
                panic!("one document-end insertion");
            };
            assert_eq!(edit.target(), &EditTarget::InsertAt(source.len()));
            assert_eq!(edit.replacement().as_str(), replacement);
            // Catalog policy does not grant automatic document-level admission.
            // Preserve the existing utterance-boundary refusal rather than
            // bypassing it merely because the proposal is mechanical.
            let admission = admit_edits(observed.document(), edits);
            assert!(admission.admitted().is_empty());
            let [skipped] = admission.skipped() else {
                panic!("the document-level proposal must be explicitly refused");
            };
            assert_eq!(skipped.reason, SkipReason::OutsideAnyUtterance);
            let output = apply_edits_verified(source, admission.admitted())
                .expect("refused proposal leaves source untouched");
            assert_eq!(output, source);
        }
    }
}

/// Tier order and continuation are grammar structure, not physical-line guesses.
/// This semantic proposal is exercised explicitly, never promoted to batch-safe.
#[test]
fn orphaned_gra_fix_preserves_intervening_tiers_and_removes_continuations() {
    let source = include_str!("../error_corpus/validation_errors/E604_gra_without_mor_2.cha");
    let expected = include_str!("../error_corpus/validation_errors/E604_gra_without_mor_3.cha");
    let parser = TreeSitterParser::new().expect("parser");
    let observed = DiagnosedSource::observe(&parser, source);
    let error = observed
        .findings()
        .find(|e| e.code.as_str() == "E604")
        .expect("authored missing MOR diagnostic");
    let proposal = error
        .catalog()
        .into_offer()
        .expect("structured GRA proposal");
    assert_eq!(proposal.safety(), BatchSafety::Semantic);
    let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind() else {
        panic!("explicit GRA deletion has one target");
    };
    let admitted = admit_edits(observed.document(), edits);
    assert!(
        admitted.skipped().is_empty(),
        "clean syntax, semantic invalidity only"
    );
    let output = apply_edits_verified(source, admitted.admitted()).expect("exact edit");
    assert_eq!(output, expected, "preserve all unrelated bytes");
    let repaired = DiagnosedSource::observe(&parser, &output);
    assert!(
        repaired.diagnostics().is_empty(),
        "{:?}",
        repaired.diagnostics()
    );
    let ambiguous = include_str!("../error_corpus/validation_errors/E604_gra_without_mor_4.cha");
    let observed = DiagnosedSource::observe(&parser, ambiguous);
    let error = observed
        .findings()
        .find(|e| e.code.as_str() == "E604")
        .expect("duplicate orphaned tiers retain missing MOR diagnostic");
    assert!(
        matches!(
            error.catalog(),
            CatalogResolution::Ineligible(CatalogIneligibility::NonUniqueCarrier)
        ),
        "multiple targets must refuse"
    );
}

#[test]
fn authored_error_specs_exercise_mechanical_catalog_proposals_and_admission() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::read(
        &workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors"),
    )
    .expect("canonical error-spec corpus");
    let mut repaired = 0;
    let mut refused = 0;
    let mut review_required = [0; 2];
    let mut no_proposal = 0;
    let mut missing_fact_witnesses = [0; 2];
    for fixture in corpus.fixtures() {
        let observed = DiagnosedSource::observe(&parser, fixture.source());
        for diagnostic in observed.findings() {
            let proposal = diagnostic.catalog().into_offer();
            if let Some(index) = [
                talkbank_model::ErrorCode::UndeclaredSpeaker,
                talkbank_model::ErrorCode::MissingRequiredHeader,
            ]
            .iter()
            .position(|code| *code == diagnostic.code)
            {
                assert!(
                    matches!(diagnostic.catalog(), CatalogResolution::Unsupported),
                    "missing facts cannot justify guessed data"
                );
                missing_fact_witnesses[index] += 1;
            }
            let Some(proposal) = proposal else {
                no_proposal += 1;
                continue;
            };
            let edits = match proposal.into_kind() {
                FixKind::Mechanical(edits) => edits,
                FixKind::Semantic(_) => {
                    review_required[0] += 1;
                    continue;
                }
                FixKind::Alternatives(_) => {
                    review_required[1] += 1;
                    continue;
                }
            };
            assert!(!edits.is_empty(), "a fix proposal must request a change");
            assert!(
                edits
                    .iter()
                    .all(|edit| edit.provenance() == &EditProvenance::Diagnostic(diagnostic.code))
            );
            mapped_edit_sites(observed.source(), &edits).unwrap_or_else(|e| {
                panic!(
                    "catalog coordinates {} {}: {e}",
                    fixture.path().display(),
                    diagnostic.code
                )
            });
            let admission = admit_edits(observed.document(), edits);
            if !admission.skipped().is_empty() {
                refused += admission.skipped().len();
                // Never turn a partially admitted proposal into a different fix.
                continue;
            }
            let output = apply_edits_verified(observed.source(), admission.admitted())
                .expect("mechanical proposal preserves all other bytes");
            assert_ne!(
                output,
                observed.source(),
                "admitted repair must not be a no-op"
            );
            verify_splice(observed.source(), &output, admission.admitted())
                .expect("recorded edit fidelity");
            let after = DiagnosedSource::observe(&parser, &output);
            let before_count = observed
                .findings()
                .filter(|e| e.code == diagnostic.code)
                .count();
            let after_count = after
                .findings()
                .filter(|e| e.code == diagnostic.code)
                .count();
            assert!(
                after_count < before_count,
                "admitted {} repair must reduce its diagnostic count: {} ({before_count} -> {after_count})",
                diagnostic.code,
                fixture.path().display()
            );
            repaired += 1;
        }
    }
    assert!(repaired > 0 && refused > 0 && no_proposal > 0);
    assert!(missing_fact_witnesses.iter().all(|count| *count > 0));
    assert!(
        review_required.iter().all(|count| *count > 0),
        "both nonautomatic policy classes have witnesses"
    );
    eprintln!(
        "catalog corpus: {repaired} admitted repairs, {refused} skipped edits, {review_required:?} review-only proposals, {no_proposal} declines"
    );
}

#[test]
fn duplicate_header_specs_require_identical_declarations_before_proposing_deletion() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (name, identical) in [
        ("E501_2.cha", true),
        ("E501_3.cha", true),
        ("E501_9.cha", false),
        ("E501_11.cha", false),
        ("E501_12.cha", false),
        ("E501_14.cha", true),
        ("E501_15.cha", false),
    ] {
        let source = std::fs::read_to_string(root.join(name)).expect("authored duplicate spec");
        let observed = DiagnosedSource::observe(&parser, &source);
        let diagnostic = observed
            .findings()
            .find(|error| error.code == talkbank_model::ErrorCode::DuplicateHeader)
            .expect("spec must diagnose the duplicate");
        let proposal = diagnostic.catalog().into_offer();
        if !identical {
            assert!(
                matches!(
                    diagnostic.catalog(),
                    CatalogResolution::Ineligible(CatalogIneligibility::HeaderConflict)
                ),
                "{name}: conflicting declarations need a user's facts"
            );
            continue;
        }
        let proposal = proposal.expect("identical declarations retain a proposal");
        assert!(matches!(proposal.safety(), BatchSafety::Mechanical));
        let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind() else {
            panic!("identical duplicate is deterministic");
        };
        assert_eq!(edits.len(), 1);
        if name == "E501_14.cha" {
            let talkbank_transform::splice::EditTarget::Replace(span) = edits[0].target() else {
                panic!("duplicate header requires replacement of its complete range");
            };
            assert_eq!(
                source.get(span.to_range()),
                Some("@Languages:\teng,\n\tfra\n")
            );
            assert_eq!(edits[0].replacement().as_str(), "");
        }
        mapped_edit_sites(&source, &edits).expect("source-bound complete header range");
        let admission = admit_edits(observed.document(), edits);
        assert!(
            admission.admitted().is_empty(),
            "header write restriction remains intact"
        );
        assert_eq!(admission.skipped().len(), 1);
    }
}

#[test]
fn missing_terminator_spec_alternatives_preserve_line_endings_and_resolve_the_finding() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for name in [
        "E305_standard_terminator_deletions_2.cha",
        "E305_timed_terminators_2.cha",
        "E305_postcodes_2.cha",
    ] {
        let original = std::fs::read_to_string(root.join(name)).expect("terminator deletion spec");
        for source in [original.clone(), original.replace('\n', "\r\n")] {
            let observed = DiagnosedSource::observe(&parser, &source);
            let error = observed
                .findings()
                .find(|error| error.code == talkbank_model::ErrorCode::MissingTerminator)
                .expect("authored missing terminator");
            let proposal = error.catalog().into_offer().expect("terminator choices");
            assert!(matches!(proposal.safety(), BatchSafety::Ambiguous));
            let FixKind::Alternatives(alternatives) = proposal.into_kind() else {
                panic!("the file cannot choose a terminator");
            };
            assert_eq!(alternatives.len(), 3);
            for alternative in alternatives {
                let admission = admit_edits(observed.document(), alternative.edits);
                assert!(admission.skipped().is_empty());
                let changed = apply_edits_verified(&source, admission.admitted())
                    .expect("only the proposed insertion changes source");
                assert_eq!(
                    changed.matches("\r\n").count(),
                    source.matches("\r\n").count(),
                    "{name}: insertion must not split a CRLF token"
                );
                let after = DiagnosedSource::observe(&parser, &changed);
                assert!(
                    !after
                        .findings()
                        .any(|error| error.code == talkbank_model::ErrorCode::MissingTerminator),
                    "{name}: chosen terminator must resolve E305: {:?}",
                    after.diagnostics()
                );
                assert!(
                    !after
                        .parsed_source()
                        .expect("source-bound parse")
                        .root_node()
                        .has_error(),
                    "repair must remain structurally parseable"
                );
            }
        }
    }
}

/// Parser rejection and write admission are distinct from clean CST syntax.
#[test]
fn missing_mor_terminator_spec_does_not_bypass_tainted_utterance_admission() {
    use talkbank_transform::splice::SkipReason;

    let parser = TreeSitterParser::new().expect("parser");
    let original = std::fs::read_to_string(workspace_root().join(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E305_mor_terminator_2.cha",
    ))
    .expect("morphology terminator deletion spec");
    for source in [original.clone(), original.replace('\n', "\r\n")] {
        let observed = DiagnosedSource::observe(&parser, &source);
        assert!(
            !observed
                .parsed_source()
                .expect("source-bound parse")
                .root_node()
                .has_error()
        );
        let error = observed
            .findings()
            .find(|error| error.code == talkbank_model::ErrorCode::MissingTerminator)
            .expect("the parser rejects missing morphology termination");
        let proposal = error.catalog().into_offer().expect("source-bound proposal");
        assert!(matches!(proposal.safety(), BatchSafety::Ambiguous));
        let FixKind::Alternatives(alternatives) = proposal.into_kind() else {
            panic!("terminator is not an automatic choice");
        };
        assert_eq!(alternatives.len(), 3);
        for alternative in alternatives {
            let admission = admit_edits(observed.document(), alternative.edits);
            assert!(admission.admitted().is_empty());
            assert_eq!(admission.skipped().len(), 1);
            assert_eq!(admission.skipped()[0].reason, SkipReason::TaintedUtterance);
            assert_eq!(
                apply_edits_verified(&source, admission.admitted()).expect("no edits"),
                source
            );
        }
    }
}

#[test]
fn initial_comma_specs_repair_to_the_spoken_control() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let control = std::fs::read_to_string(root.join("E259_initial_comma_1.cha")).expect("control");
    for name in ["E259_initial_comma_2.cha", "E259_initial_comma_3.cha"] {
        let original = std::fs::read_to_string(root.join(name)).expect("comma spec");
        for (source, expected) in [
            (original.clone(), control.clone()),
            (
                original.replace('\n', "\r\n"),
                control.replace('\n', "\r\n"),
            ),
        ] {
            let observed = DiagnosedSource::observe(&parser, &source);
            let error = observed
                .findings()
                .find(|error| error.code == talkbank_model::ErrorCode::CommaAfterNonSpokenContent)
                .expect("initial comma diagnosis");
            let proposal = error.catalog().into_offer().expect("semantic proposal");
            assert!(matches!(proposal.safety(), BatchSafety::Semantic));
            let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind()
            else {
                panic!("one deletion");
            };
            let admission = admit_edits(observed.document(), edits);
            assert!(admission.skipped().is_empty());
            let changed =
                apply_edits_verified(&source, admission.admitted()).expect("faithful deletion");
            assert_eq!(changed, expected, "{name}: preserve spoken control exactly");
            let after = DiagnosedSource::observe(&parser, &changed);
            assert!(after.diagnostics().is_empty(), "{:?}", after.diagnostics());
        }
    }
}

#[test]
fn interior_comma_specs_preserve_the_following_separator() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for name in ["E259_1.cha", "E259_8.cha", "E259_10.cha"] {
        let source = std::fs::read_to_string(root.join(name)).expect("interior comma spec");
        let observed = DiagnosedSource::observe(&parser, &source);
        let error = observed
            .findings()
            .find(|error| error.code == talkbank_model::ErrorCode::CommaAfterNonSpokenContent)
            .expect("comma diagnosis");
        let proposal = error.catalog().into_offer().expect("interior proposal");
        let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind() else {
            panic!("one deletion");
        };
        let sites = mapped_edit_sites(&source, &edits).expect("bound edits");
        assert_eq!(sites.len(), 1);
        let admission = admit_edits(observed.document(), edits);
        assert!(admission.skipped().is_empty());
        let changed =
            apply_edits_verified(&source, admission.admitted()).expect("faithful deletion");
        assert_eq!(
            changed,
            source.replacen(',', "", 1),
            "{name}: delete only the comma"
        );
        let after = DiagnosedSource::observe(&parser, &changed);
        assert!(
            after.diagnostics().is_empty(),
            "{name}: {:?}",
            after.diagnostics()
        );
    }
}

#[test]
fn stress_spec_repair_finds_the_later_duplicate_without_moving_other_stress() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let source = std::fs::read_to_string(root.join("E244_3.cha")).expect("duplicate stress spec");
    let control =
        std::fs::read_to_string(root.join("E244_4.cha")).expect("distinct stress control");
    let observed = DiagnosedSource::observe(&parser, &source);
    let error = observed
        .findings()
        .find(|error| error.code == talkbank_model::ErrorCode::ConsecutiveStressMarkers)
        .expect("stress diagnosis");
    let proposal = error
        .catalog()
        .into_offer()
        .expect("duplicate primary stress proposal");
    assert!(matches!(proposal.safety(), BatchSafety::Mechanical));
    let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind() else {
        panic!("duplicate removal");
    };
    let admission = admit_edits(observed.document(), edits);
    assert!(admission.skipped().is_empty());
    let changed = apply_edits_verified(&source, admission.admitted()).expect("faithful edit");
    assert_eq!(changed, control);
    let after = DiagnosedSource::observe(&parser, &changed);
    assert!(
        !after
            .findings()
            .any(|error| error.code == talkbank_model::ErrorCode::ConsecutiveStressMarkers)
    );
}

#[test]
fn duplicate_comma_specs_batch_distinct_token_deletions() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (input, control) in [("E258_6.cha", "E258_2.cha"), ("E258_7.cha", "E258_8.cha")] {
        let source = std::fs::read_to_string(root.join(input)).expect("triple comma spec");
        let expected = std::fs::read_to_string(root.join(control)).expect("single comma control");
        let observed = DiagnosedSource::observe(&parser, &source);
        let mut proposed = Vec::new();
        for error in observed
            .findings()
            .filter(|error| error.code == talkbank_model::ErrorCode::ConsecutiveCommas)
        {
            let proposal = error.catalog().into_offer().expect("typed comma proposal");
            assert!(matches!(proposal.safety(), BatchSafety::Mechanical));
            let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind()
            else {
                panic!("duplicate deletion");
            };
            proposed.extend(edits);
        }
        assert_eq!(
            proposed.len(),
            2,
            "{input}: each extra comma is diagnosed once"
        );
        let admission = admit_edits(observed.document(), proposed);
        assert!(admission.skipped().is_empty());
        let changed = apply_edits_verified(&source, admission.admitted())
            .expect("nonoverlapping token deletions");
        assert_eq!(changed, expected);
        let after = DiagnosedSource::observe(&parser, &changed);
        assert!(after.diagnostics().is_empty(), "{:?}", after.diagnostics());
    }
}

#[test]
fn stress_run_specs_preserve_controls_and_refuse_unselected_policies() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (input, control) in [
        (2, Some(1)),
        (4, Some(3)),
        (6, Some(5)),
        (7, None),
        (8, None),
        (9, Some(3)),
    ] {
        let source = std::fs::read_to_string(root.join(format!("E244_stress_runs_{input}.cha")))
            .expect("stress-run spec");
        let observed = DiagnosedSource::observe(&parser, &source);
        let mut proposed = Vec::new();
        let errors: Vec<_> = observed
            .findings()
            .filter(|error| error.code == talkbank_model::ErrorCode::ConsecutiveStressMarkers)
            .collect();
        assert_eq!(
            errors.len(),
            1,
            "case {input}: one word-level E244 required"
        );
        for error in errors {
            let proposal = error.catalog().into_offer();
            if control.is_none() {
                assert!(
                    matches!(
                        error.catalog(),
                        CatalogResolution::Ineligible(CatalogIneligibility::NoRepairableToken)
                    ),
                    "case {input}: no approved primary repair"
                );
                continue;
            }
            let proposal = proposal.expect("primary duplicate proposal");
            assert!(matches!(proposal.safety(), BatchSafety::Mechanical));
            let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind()
            else {
                panic!("duplicate removal");
            };
            proposed.extend(edits);
        }
        let admission = admit_edits(observed.document(), proposed);
        assert!(admission.skipped().is_empty());
        let changed =
            apply_edits_verified(&source, admission.admitted()).expect("batch of stress repairs");
        if let Some(control) = control {
            let expected =
                std::fs::read_to_string(root.join(format!("E244_stress_runs_{control}.cha")))
                    .expect("paired control");
            assert_eq!(changed, expected, "case {input}: exact paired control");
            let after = DiagnosedSource::observe(&parser, &changed);
            assert!(
                !after
                    .findings()
                    .any(|error| error.code == talkbank_model::ErrorCode::ConsecutiveStressMarkers)
            );
        } else {
            assert_eq!(changed, source, "declined policies leave source unchanged");
        }
    }
}

#[test]
fn group_edge_specs_repair_only_owned_whitespace_runs() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (input, control, count) in [(2, 1, 2), (3, 4, 4)] {
        let source = std::fs::read_to_string(root.join(format!("E750_group_edges_{input}.cha")))
            .expect("group edge spec");
        let expected =
            std::fs::read_to_string(root.join(format!("E750_group_edges_{control}.cha")))
                .expect("paired group control");
        let observed = DiagnosedSource::observe(&parser, &source);
        let mut proposed = Vec::new();
        for error in observed
            .findings()
            .filter(|error| error.code == talkbank_model::ErrorCode::SpaceInsideAngleGroup)
        {
            let proposal = error.catalog().into_offer().expect("group-owned repair");
            assert!(matches!(proposal.safety(), BatchSafety::Mechanical));
            let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind()
            else {
                panic!("edge whitespace deletion");
            };
            proposed.extend(edits);
        }
        assert_eq!(proposed.len(), count);
        let admission = admit_edits(observed.document(), proposed);
        assert!(
            admission.skipped().is_empty(),
            "own recovery repair remains admissible"
        );
        let changed =
            apply_edits_verified(&source, admission.admitted()).expect("distinct group edges");
        assert_eq!(changed, expected);
        let after = DiagnosedSource::observe(&parser, &changed);
        assert!(after.diagnostics().is_empty(), "{:?}", after.diagnostics());
    }
}

#[test]
fn marker_specs_preserve_comment_and_structural_word_boundaries() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    for (input, control, count) in [(2, Some(1), 6), (3, None, 2)] {
        let source =
            std::fs::read_to_string(root.join(format!("E241_marker_boundaries_{input}.cha")))
                .expect("marker spec");
        let observed = DiagnosedSource::observe(&parser, &source);
        let diagnostics: Vec<_> = observed
            .findings()
            .filter(|error| error.code == talkbank_model::ErrorCode::IllegalUntranscribed)
            .collect();
        assert_eq!(diagnostics.len(), count);
        let mut proposed = Vec::new();
        for error in diagnostics {
            let proposal = error.catalog().into_offer();
            if control.is_none() {
                assert!(
                    matches!(
                        error.catalog(),
                        CatalogResolution::Ineligible(CatalogIneligibility::NoRepairableToken)
                    ),
                    "structural notation must not be erased"
                );
                continue;
            }
            let proposal = proposal.expect("whole marker proposal");
            assert!(matches!(proposal.safety(), BatchSafety::Mechanical));
            let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind()
            else {
                panic!("canonical spelling");
            };
            proposed.extend(edits);
        }
        let admission = admit_edits(observed.document(), proposed);
        assert!(admission.skipped().is_empty());
        let changed =
            apply_edits_verified(&source, admission.admitted()).expect("distinct marker edits");
        if let Some(control) = control {
            let expected =
                std::fs::read_to_string(root.join(format!("E241_marker_boundaries_{control}.cha")))
                    .expect("canonical control");
            assert_eq!(changed, expected);
            let after = DiagnosedSource::observe(&parser, &changed);
            assert!(after.diagnostics().is_empty(), "{:?}", after.diagnostics());
        } else {
            assert_eq!(changed, source);
        }
    }
}
