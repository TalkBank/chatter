//! Refusal policy at the boundary between a finding and a safe repair.

use super::DiagnosedSource;
use talkbank_model::ErrorCode;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_transform::splice::{FixKind, admit_edits};

/// Finding presence and proposal safety are distinct policy verdicts.
#[derive(Clone, Copy)]
enum ExpectedFinding {
    Absent,
    Proposed,
    Refused,
}

#[test]
fn mixed_recovery_specs_preserve_findings_without_admitting_unsafe_repairs() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    use ExpectedFinding::{Absent, Proposed, Refused};
    for (code, clean, recovered_control, recovered_variant, recovered_finding) in [
        (
            ErrorCode::DuplicateHeader,
            "E501_3.cha",
            "E501_recovery_1.cha",
            "E501_recovery_2.cha",
            Refused,
        ),
        (
            ErrorCode::ConsecutiveCommas,
            "E258_1.cha",
            "E258_recovery_1.cha",
            "E258_recovery_2.cha",
            Refused,
        ),
        (
            ErrorCode::CommaAfterNonSpokenContent,
            "E259_1.cha",
            "E259_recovery_1.cha",
            "E259_recovery_2.cha",
            Refused,
        ),
        (
            ErrorCode::MissingTerminator,
            "E305_1.cha",
            "E305_recovery_1.cha",
            "E305_recovery_2.cha",
            Absent,
        ),
        (
            ErrorCode::ConsecutiveStressMarkers,
            "E244_stress_runs_2.cha",
            "E244_recovery_1.cha",
            "E244_recovery_2.cha",
            Proposed,
        ),
    ] {
        for (name, recovery, finding) in [
            (clean, false, Proposed),
            (recovered_control, true, Absent),
            (recovered_variant, true, recovered_finding),
        ] {
            let source = std::fs::read_to_string(root.join(name)).expect("authored spec");
            let observed = DiagnosedSource::observe(&parser, &source);
            assert_eq!(
                observed
                    .parsed_source()
                    .expect("source-bound parse")
                    .root_node()
                    .has_error(),
                recovery,
                "{name}"
            );
            let mut diagnostics = observed.findings().filter(|error| error.code == code);
            if recovery {
                assert!(
                    !observed.diagnostics().is_empty(),
                    "{name}: recovery is reported"
                );
            }
            match (finding, diagnostics.next()) {
                (Absent, None) => {}
                (Refused, Some(diagnostic)) => {
                    assert!(
                        matches!(
                            diagnostic.catalog(),
                            talkbank_transform::splice::CatalogResolution::Ineligible(
                                talkbank_transform::splice::CatalogIneligibility::RecoveredCarrier
                            )
                        ),
                        "{name}: unsafe carrier"
                    );
                }
                (Proposed, Some(diagnostic)) => {
                    let proposal = diagnostic
                        .catalog()
                        .into_offer()
                        .expect("clean local evidence");
                    if recovery {
                        let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) =
                            proposal.into_kind()
                        else {
                            panic!("{name}: stress proposal is deterministic");
                        };
                        let admission = admit_edits(observed.document(), edits);
                        assert!(admission.admitted().is_empty(), "{name}: tainted turn");
                        assert!(!admission.skipped().is_empty(), "{name}: retained refusal");
                    }
                }
                (Absent, Some(_)) => panic!("{name}: unexpected finding"),
                (Refused | Proposed, None) => panic!("{name}: missing finding"),
            }
            assert!(
                diagnostics.next().is_none(),
                "{name}: no duplicate findings"
            );
        }
    }
}

#[test]
fn media_normalization_recovery_specs_require_a_clean_header_carrier() {
    use talkbank_model::model::{FileStem, TranscriptName};
    use talkbank_transform::splice::apply_edits_verified;

    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let name = TranscriptName::Named(FileStem::from_stem("Schlüssel").expect("authored stem"));
    let control = std::fs::read_to_string(root.join("W109_recovery_1.cha"))
        .expect("canonical media with unrelated recovery");
    for (fixture, finding) in [
        ("W109_recovery_1.cha", ExpectedFinding::Absent),
        ("W109_recovery_2.cha", ExpectedFinding::Proposed),
        ("W109_recovery_3.cha", ExpectedFinding::Absent),
        ("W109_recovery_4.cha", ExpectedFinding::Refused),
    ] {
        let source = std::fs::read_to_string(root.join(fixture)).expect("authored recovery spec");
        let observed = DiagnosedSource::observe_named(&parser, &source, name);
        assert!(
            observed
                .parsed_source()
                .expect("source-bound parse")
                .root_node()
                .has_error(),
            "{fixture}"
        );
        assert!(
            observed.findings().any(|e| e.code.as_str() == "E316"),
            "{fixture}: retained recovery diagnostic"
        );
        let warnings: Vec<_> = observed
            .findings()
            .filter(|error| error.code == ErrorCode::MediaFilenameNonCanonicalUnicode)
            .collect();
        match finding {
            ExpectedFinding::Absent => assert!(warnings.is_empty(), "{fixture}"),
            ExpectedFinding::Refused => {
                assert_eq!(warnings.len(), 1, "{fixture}");
                assert!(matches!(
                    warnings[0].catalog(),
                    talkbank_transform::splice::CatalogResolution::Ineligible(
                        talkbank_transform::splice::CatalogIneligibility::RecoveredCarrier
                    )
                ));
            }
            ExpectedFinding::Proposed => {
                assert_eq!(warnings.len(), 1, "{fixture}");
                let proposal = warnings[0]
                    .catalog()
                    .into_offer()
                    .expect("the header, not the entire document, is clean");
                let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind()
                else {
                    panic!("normalization is a deterministic token edit");
                };
                let admission = admit_edits(observed.document(), edits);
                assert_eq!(admission.admitted().len(), 1);
                assert!(admission.skipped().is_empty());
                let output = apply_edits_verified(&source, admission.admitted())
                    .expect("all other bytes retained");
                assert_eq!(output, control);
                let after = DiagnosedSource::observe_named(&parser, &output, name);
                assert!(
                    after
                        .parsed_source()
                        .expect("source-bound parse")
                        .root_node()
                        .has_error()
                );
                assert!(after.findings().any(|e| e.code.as_str() == "E316"));
                assert!(
                    !after
                        .findings()
                        .any(|e| { e.code == ErrorCode::MediaFilenameNonCanonicalUnicode })
                );
            }
        }
    }
}

#[test]
fn stress_word_recovery_specs_separate_typed_findings_from_carrier_safety() {
    use ExpectedFinding::{Absent, Proposed, Refused};
    use talkbank_parser::generated_traversal::{AsRawNode, StandaloneWordNode};
    use talkbank_transform::splice::apply_edits_verified;

    enum ExpectedCarrier {
        Clean(&'static str),
        Recovered(&'static str),
    }
    use ExpectedCarrier::{Clean, Recovered};

    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let control = std::fs::read_to_string(root.join("E244_word_recovery_1.cha"))
        .expect("complete separated-stress control");
    for (fixture, recovery, finding, carrier) in [
        (
            "E244_word_recovery_1.cha",
            false,
            Absent,
            Clean("ˈhelˌl(o)"),
        ),
        (
            "E244_word_recovery_2.cha",
            false,
            Proposed,
            Clean("ˈˈhelˌl(o)"),
        ),
        ("E244_word_recovery_3.cha", true, Absent, Clean("ˈhelˌl")),
        ("E244_word_recovery_4.cha", true, Proposed, Clean("ˈˈhelˌl")),
        (
            "E244_word_recovery_5.cha",
            true,
            Absent,
            Recovered("ˈhelˌl(o,)"),
        ),
        (
            "E244_word_recovery_6.cha",
            true,
            Refused,
            Recovered("ˈˈhelˌl(o,)"),
        ),
    ] {
        let source = std::fs::read_to_string(root.join(fixture)).expect("authored stress spec");
        let observed = DiagnosedSource::observe(&parser, &source);
        assert_eq!(
            observed
                .parsed_source()
                .expect("source-bound parse")
                .root_node()
                .has_error(),
            recovery,
            "{fixture}"
        );
        if recovery {
            assert!(
                observed.findings().any(|e| e.code.as_str() == "E316"),
                "{fixture}: shortening recovery remains reported"
            );
        } else if matches!(finding, Absent) {
            assert!(
                observed.diagnostics().is_empty(),
                "{fixture}: valid control"
            );
        }
        let root = observed
            .parsed_source()
            .expect("source-bound parse")
            .root()
            .expect("source-bound root");
        let mut words = root.descendants().filter_map(|node| {
            node.expect("source-bound descendant")
                .typed::<StandaloneWordNode>()
        });
        let word = words.next().expect("retained stress-bearing word");
        assert!(words.next().is_none(), "{fixture}: one source word");
        match carrier {
            Clean(text) => {
                assert_eq!(word.text(), text, "{fixture}: complete local carrier");
                assert!(!word.raw_node().has_error(), "{fixture}");
            }
            Recovered(text) => {
                assert_eq!(word.text(), text, "{fixture}: recovery is inside the word");
                assert!(word.raw_node().has_error(), "{fixture}");
            }
        }
        let mut findings = observed
            .findings()
            .filter(|e| e.code == ErrorCode::ConsecutiveStressMarkers);
        match (finding, findings.next()) {
            (Absent, None) => {}
            (Proposed, Some(diagnostic)) => {
                assert_eq!(
                    diagnostic.location.span().expect("located diagnostic"),
                    talkbank_model::Span::new(
                        word.raw_node().start_byte(),
                        word.raw_node().end_byte()
                    ),
                    "{fixture}: finding owns this exact carrier"
                );
                let proposal = diagnostic
                    .catalog()
                    .into_offer()
                    .expect("stress-bearing word retains clean local evidence");
                let (FixKind::Mechanical(edits) | FixKind::Semantic(edits)) = proposal.into_kind()
                else {
                    panic!("{fixture}: stress proposal is deterministic");
                };
                let admission = admit_edits(observed.document(), edits);
                if recovery {
                    assert!(admission.admitted().is_empty(), "{fixture}: tainted turn");
                    assert_eq!(admission.skipped().len(), 1, "{fixture}: retained refusal");
                } else {
                    assert_eq!(admission.admitted().len(), 1, "{fixture}");
                    assert!(admission.skipped().is_empty(), "{fixture}");
                    let output = apply_edits_verified(&source, admission.admitted())
                        .expect("source-bound marker deletion");
                    assert_eq!(output, control, "{fixture}: all other bytes retained");
                    let after = DiagnosedSource::observe(&parser, &output);
                    assert!(
                        after.diagnostics().is_empty(),
                        "{fixture}: repaired control"
                    );
                }
            }
            (Refused, Some(diagnostic)) => {
                assert_eq!(
                    diagnostic.location.span().expect("located diagnostic"),
                    talkbank_model::Span::new(
                        word.raw_node().start_byte(),
                        word.raw_node().end_byte()
                    ),
                    "{fixture}: finding retains this recovered carrier"
                );
                assert!(
                    matches!(
                        diagnostic.catalog(),
                        talkbank_transform::splice::CatalogResolution::Ineligible(
                            talkbank_transform::splice::CatalogIneligibility::RecoveredCarrier
                        )
                    ),
                    "{fixture}: the word itself is recovered"
                );
            }
            (Absent, Some(_)) => panic!("{fixture}: unexpected stress finding"),
            (Proposed | Refused, None) => panic!("{fixture}: missing stress finding"),
        }
        assert!(findings.next().is_none(), "{fixture}: one stress finding");
    }
}
