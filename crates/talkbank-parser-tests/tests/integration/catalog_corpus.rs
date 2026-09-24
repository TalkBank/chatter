//! Mechanical fix proposals from actual canonical spec diagnostics.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::{
    ErrorCollector, ParseError,
    model::{ChatFile, TranscriptName},
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, repo_paths::workspace_root};
use talkbank_transform::splice::{
    BatchSafety, EditProvenance, FixKind, admit_edits, apply_edits_verified, catalog_fix,
    mapped_edit_sites, verify_splice,
};

/// Keep observed diagnostics and the recovery model with the exact input used
/// to produce them. A recovered model is not a validity certificate.
struct DiagnosedSource<'a> {
    source: &'a str,
    file: ChatFile,
    diagnostics: Vec<ParseError>,
}

impl<'a> DiagnosedSource<'a> {
    fn observe(parser: &TreeSitterParser, source: &'a str) -> Self {
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(source, &errors);
        file.validate(&errors, TranscriptName::Anonymous);
        Self {
            source,
            file,
            diagnostics: errors.into_vec(),
        }
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
        let name = TranscriptName::Named(FileStem::from_stem(stem));
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(&input, &errors);
        file.validate(&errors, name);
        let diagnostics = errors.into_vec();
        let warning = diagnostics
            .iter()
            .find(|e| e.code.as_str() == "W109")
            .expect("normalization warning");
        let proposal = catalog_fix(warning, &input).expect("source-bound token repair");
        assert!(matches!(proposal.safety, BatchSafety::Mechanical));
        let FixKind::Deterministic(edits) = proposal.kind else {
            panic!("mechanical repair");
        };
        let admitted = admit_edits(&file, edits);
        assert!(admitted.skipped.is_empty());
        assert_eq!(admitted.admitted.len(), 1);
        let output =
            apply_edits_verified(&input, &admitted.admitted).expect("byte-preserving repair");
        assert_eq!(output, expected, "no other source bytes may change");
        let errors = ErrorCollector::new();
        let repaired = parser.parse_chat_file_streaming(&output, &errors);
        repaired.validate(&errors, name);
        let after = errors.into_vec();
        assert!(!after.iter().any(|e| e.code.as_str() == "E531"));
        let remaining = after.iter().find(|e| e.code.as_str() == "W109");
        assert_eq!(remaining.is_some(), stem != "Schlüssel");
        if let Some(warning) = remaining {
            assert!(
                catalog_fix(warning, &output).is_none(),
                "file-only warning must not propose a header edit"
            );
        }
    }
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
    for fixture in corpus.fixtures() {
        let observed = DiagnosedSource::observe(&parser, fixture.source());
        for diagnostic in &observed.diagnostics {
            let Some(proposal) = catalog_fix(diagnostic, observed.source) else {
                no_proposal += 1;
                continue;
            };
            let edits = match (proposal.safety, proposal.kind) {
                (BatchSafety::Mechanical, FixKind::Deterministic(edits)) => edits,
                (BatchSafety::Mechanical, FixKind::Alternatives(_)) => {
                    panic!("mechanical fixes cannot require a semantic choice")
                }
                (BatchSafety::Semantic, _) => {
                    review_required[0] += 1;
                    continue;
                }
                (BatchSafety::Ambiguous, _) => {
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
            mapped_edit_sites(observed.source, &edits).unwrap_or_else(|e| {
                panic!(
                    "catalog coordinates {} {}: {e}",
                    fixture.path().display(),
                    diagnostic.code
                )
            });
            let admission = admit_edits(&observed.file, edits);
            if !admission.skipped.is_empty() {
                refused += admission.skipped.len();
                // Never turn a partially admitted proposal into a different fix.
                continue;
            }
            let output = apply_edits_verified(observed.source, &admission.admitted)
                .expect("mechanical proposal preserves all other bytes");
            assert_ne!(
                output, observed.source,
                "admitted repair must not be a no-op"
            );
            verify_splice(observed.source, &output, &admission.admitted)
                .expect("recorded edit fidelity");
            let after = DiagnosedSource::observe(&parser, &output);
            let before_count = observed
                .diagnostics
                .iter()
                .filter(|e| e.code == diagnostic.code)
                .count();
            let after_count = after
                .diagnostics
                .iter()
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
    assert!(
        review_required.iter().all(|count| *count > 0),
        "both nonautomatic policy classes have witnesses"
    );
    eprintln!(
        "catalog corpus: {repaired} admitted repairs, {refused} skipped edits, {review_required:?} review-only proposals, {no_proposal} declines"
    );
}
