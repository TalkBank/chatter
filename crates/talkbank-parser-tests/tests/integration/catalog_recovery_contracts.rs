//! Refusal policy at the boundary between a finding and a safe repair.

use super::DiagnosedSource;
use talkbank_model::ErrorCode;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_transform::splice::{FixKind, admit_edits, catalog_fix};

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
            assert_eq!(observed.parsed.root_node().has_error(), recovery, "{name}");
            let mut diagnostics = observed
                .diagnostics
                .iter()
                .filter(|error| error.code == code);
            if recovery {
                assert!(
                    !observed.diagnostics.is_empty(),
                    "{name}: recovery is reported"
                );
            }
            match (finding, diagnostics.next()) {
                (Absent, None) => {}
                (Refused, Some(diagnostic)) => {
                    assert!(
                        catalog_fix(diagnostic, &observed.parsed).is_none(),
                        "{name}: unsafe carrier"
                    );
                }
                (Proposed, Some(diagnostic)) => {
                    let proposal =
                        catalog_fix(diagnostic, &observed.parsed).expect("clean local evidence");
                    if recovery {
                        let FixKind::Deterministic(edits) = proposal.kind else {
                            panic!("{name}: stress proposal is deterministic");
                        };
                        let admission = admit_edits(&observed.file, edits);
                        assert!(admission.admitted.is_empty(), "{name}: tainted turn");
                        assert!(!admission.skipped.is_empty(), "{name}: retained refusal");
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
