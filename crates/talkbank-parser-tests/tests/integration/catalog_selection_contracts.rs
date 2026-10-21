//! Canonical inputs travel the diagnosis-owned selection/application phases.

use super::{DiagnosedSource, TreeSitterParser, workspace_root};
use talkbank_model::ErrorCode;
use talkbank_transform::splice::{FixCodeSelection, SelectionSkip};

#[test]
fn verified_repairs_retain_complete_findings_policy_and_name() {
    use talkbank_model::{
        RuleSelection,
        model::{FileStem, TranscriptName},
        validation::{AlignmentValidation, ValidationPolicy},
    };
    use talkbank_transform::splice::FixDiagnosis;
    let parser = TreeSitterParser::new().expect("parser");
    let source = include_str!("../error_corpus/validation_errors/W109_2.cha");
    let expected = include_str!("../error_corpus/validation_errors/W109_4.cha");
    let stem = FileStem::from_stem("Schlu\u{0308}ssel").expect("authored stem");
    for alignment in [
        AlignmentValidation::Structure,
        AlignmentValidation::IncludeTierAlignment,
    ] {
        let policy = ValidationPolicy::new(RuleSelection::new(), alignment);
        let diagnosis = FixDiagnosis::observe(&parser, source, policy, TranscriptName::Named(stem));
        let complete_codes: Vec<_> = diagnosis
            .diagnostics()
            .iter()
            .map(|finding| finding.code)
            .collect();
        assert!(complete_codes.contains(&ErrorCode::MediaFilenameNonCanonicalUnicode));
        let selected = diagnosis.select(&FixCodeSelection::Only(
            [ErrorCode::MediaFilenameNonCanonicalUnicode]
                .into_iter()
                .collect(),
        ));
        let admitted = selected.admit();
        assert_eq!(admitted.admitted().len(), 1);
        assert!(admitted.skipped().is_empty());
        let verified = admitted.apply_verified().expect("owned verified repair");
        assert_eq!(verified.bytes().text(), expected);
        assert_eq!(verified.diagnosis().policy(), policy);
        assert_eq!(verified.diagnosis().name(), TranscriptName::Named(stem));
        assert_eq!(
            verified
                .diagnosis()
                .diagnostics()
                .iter()
                .map(|finding| finding.code)
                .collect::<Vec<_>>(),
            complete_codes,
            "selection must not prune the complete baseline"
        );
        // The decomposed stored name still warns after the header is repaired.
        let after = FixDiagnosis::observe(
            &parser,
            verified.bytes().text(),
            verified.diagnosis().policy(),
            verified.diagnosis().name(),
        );
        assert!(
            after
                .findings()
                .any(|finding| finding.code == ErrorCode::MediaFilenameNonCanonicalUnicode)
        );
    }
}

#[test]
fn explicit_semantic_selection_preserves_the_authored_control() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let source =
        std::fs::read_to_string(root.join("E259_initial_comma_2.cha")).expect("semantic spec");
    let expected =
        std::fs::read_to_string(root.join("E259_initial_comma_1.cha")).expect("spoken control");
    let selected = DiagnosedSource::observe(&parser, &source)
        .into_diagnosis()
        .select(&FixCodeSelection::All);
    assert!(selected.is_empty());
    assert!(
        selected
            .skipped()
            .iter()
            .any(|skip| skip.code == ErrorCode::CommaAfterNonSpokenContent
                && matches!(skip.reason, SelectionSkip::NeedsCode))
    );
    let selected = DiagnosedSource::observe(&parser, &source)
        .into_diagnosis()
        .select(&FixCodeSelection::Only(
            [ErrorCode::CommaAfterNonSpokenContent]
                .into_iter()
                .collect(),
        ));
    let admission = selected.admit();
    assert!(admission.skipped().is_empty());
    let verified = admission.apply_verified().expect("verified bytes");
    assert_eq!(verified.bytes().text(), expected);
    assert_eq!(verified.diagnosis().source(), source);
    assert_eq!(verified.bytes().plan().source(), source);
    assert_eq!(verified.bytes().plan().sites().len(), 1);
}

#[test]
fn naming_an_ambiguous_code_never_selects_its_alternatives() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let source = std::fs::read_to_string(root.join("E305_standard_terminator_deletions_2.cha"))
        .expect("alternatives spec");
    for naming in [
        FixCodeSelection::All,
        FixCodeSelection::Only([ErrorCode::MissingTerminator].into_iter().collect()),
    ] {
        let selected = DiagnosedSource::observe(&parser, &source)
            .into_diagnosis()
            .select(&naming);
        assert!(selected.is_empty());
        let skip = selected
            .skipped()
            .iter()
            .find(|skip| skip.code == ErrorCode::MissingTerminator)
            .expect("actual E305");
        let SelectionSkip::Alternatives(choices) = &skip.reason else {
            panic!("choices remain unselected");
        };
        assert_eq!(choices.len(), 3);
        assert!(choices.iter().all(|label| !label.is_empty()));
    }
}

#[test]
fn accumulated_mechanical_selection_retains_source_and_deletion_sites() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let original = std::fs::read_to_string(root.join("E258_6.cha")).expect("two deletions");
    let control = std::fs::read_to_string(root.join("E258_2.cha")).expect("single comma control");
    for (source, expected) in [
        (original.clone(), control.clone()),
        (
            original.replace('\n', "\r\n"),
            control.replace('\n', "\r\n"),
        ),
    ] {
        let selected = DiagnosedSource::observe(&parser, &source)
            .into_diagnosis()
            .select(&FixCodeSelection::All);
        assert!(!selected.is_empty());
        let admission = selected.admit();
        assert!(admission.skipped().is_empty());
        assert_eq!(admission.admitted().len(), 2);
        let verified = admission.apply_verified().expect("verified bytes");
        assert_eq!(verified.bytes().text(), expected);
        assert_eq!(verified.diagnosis().source(), source);
        assert_eq!(verified.bytes().plan().source(), source);
        assert_eq!(verified.bytes().plan().sites().len(), 2);
        assert!(
            verified
                .bytes()
                .plan()
                .sites()
                .iter()
                .all(|site| site.spliced().start() == site.spliced().end())
        );
    }
}
