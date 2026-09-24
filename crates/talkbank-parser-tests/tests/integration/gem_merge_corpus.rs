//! Source-bound gem placement over authored reference CHAT.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{Header, Line, SemanticEq};
use talkbank_model::{UtteranceIdx, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};
use talkbank_spec_vocabulary::validation_manifest::ValidationManifest;
use talkbank_transform::transcript_merge::{
    DonorIdx, GemExterior, MergeError, SourceBoundDonorSelection,
    merge_chat_files_with_donor_selection,
};

#[test]
fn reference_timed_gem_reconstructs_exterior_speech_without_retiming() {
    let parser = TreeSitterParser::new().expect("parser");
    let source = strict_parse(parser.parse_chat_file(include_str!(
        "../../../../corpus/reference/edge-cases/timed-gem-exterior.cha",
    )))
    .expect("authored timed gem parses");
    let (label, retained) = source
        .lines
        .iter()
        .enumerate()
        .find_map(|(index, line)| {
            let Line::Header { header, .. } = line else {
                return None;
            };
            let Header::BeginGem { label: Some(label) } = header.as_ref() else {
                return None;
            };
            let speaker = source.lines[index + 1..]
                .iter()
                .find_map(|line| match line {
                    Line::Utterance(u) => Some(u.main.speaker.clone()),
                    Line::Header { .. } => None,
                })
                .expect("gem speech");
            Some((label.as_str(), speaker))
        })
        .expect("named gem");
    let mut reference = source.clone();
    reference.lines.retain(|line| match line {
        Line::Utterance(u) => u.main.speaker == retained,
        Line::Header { .. } => true,
    });
    // This donor projection deliberately has no section annotations; it is
    // bound to its own unchanged row/header coordinates, not to a source from
    // which it claims to have preserved removed gem markers.
    let mut donor = source.clone();
    donor.lines.retain(|line| match line {
        Line::Utterance(u) => u.main.speaker != retained,
        Line::Header { header, .. } => !matches!(
            header.as_ref(),
            Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
        ),
    });
    let parents = || {
        (0..donor.utterances().count())
            .map(|i| DonorIdx::new(UtteranceIdx::new(i)))
            .collect()
    };
    let bind = || {
        SourceBoundDonorSelection::bind(&donor, &donor, parents())
            .expect("unchanged donor coordinates")
    };
    let selection = bind()
        .with_timed_gem_exterior(&reference, label)
        .expect("fully timed paired gem");
    let equivalent_reference = reference.clone();
    assert!(
        matches!(
            merge_chat_files_with_donor_selection(
                &equivalent_reference,
                &selection,
                std::slice::from_ref(&retained),
                &[],
            ),
            Err(MergeError::InvalidGemExterior)
        ),
        "content equality cannot replace source identity"
    );
    assert!(
        matches!(
            bind().with_timed_gem_exterior(&reference, "absent"),
            Err(MergeError::InvalidGemExterior)
        ),
        "missing named extent is not evidence"
    );

    let merged = merge_chat_files_with_donor_selection(
        &reference,
        &selection,
        std::slice::from_ref(&retained),
        &[],
    )
    .expect("strictly exterior donor speech is placeable");
    let placements = merged.gem_exterior_placements();
    assert_eq!(
        placements.len(),
        2,
        "both exterior placements need receipts"
    );
    for (placement, expected) in placements
        .iter()
        .zip([GemExterior::Before, GemExterior::After])
    {
        assert_eq!(placement.exterior, expected);
        assert_eq!(placement.label, label);
        assert_eq!(placement.reference_interval, 100..=300);
        let original = donor
            .utterances()
            .nth(placement.donor.utterance().raw())
            .expect("receipt names actual selected donor row");
        let bullet = original.main.content.bullet.as_ref().expect("timed donor");
        assert_eq!(
            placement.donor_interval,
            bullet.timing.start_ms..=bullet.timing.end_ms
        );
    }
    assert!(merged.bullet_edits().is_empty());
    assert!(merged.draft_order_reviews().is_empty());
    let reported = merged.report(|_, _| panic!("complementary projections drop no speech"));
    let actual: Vec<_> = reported.file().utterances().collect();
    let original: Vec<_> = source.utterances().collect();
    assert_eq!(actual.len(), original.len());
    for (actual, original) in actual.iter().zip(original) {
        assert!(
            actual.semantic_eq(original),
            "payload and exact timing survive reconstruction"
        );
    }
    let structural = |file: &talkbank_model::model::ChatFile| {
        file.lines
            .iter()
            .filter(|line| match line {
                Line::Utterance(_) => true,
                Line::Header { header, .. } => matches!(
                    header.as_ref(),
                    Header::BeginGem { .. } | Header::EndGem { .. }
                ),
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    assert!(
        structural(reported.file()).semantic_eq(&structural(&source)),
        "the original speech/gem order is reconstructed, not just utterance order"
    );
    let wire = reported.file().to_chat_string();
    let reparsed = strict_parse(parser.parse_chat_file(&wire)).expect("merged wire parses");
    assert!(reported.file().semantic_eq(&reparsed));
}

#[test]
fn reference_untimed_gems_cannot_issue_timed_exterior_capability() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut refusals = 0;
    for fixture in corpus.fixtures() {
        let source =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        if source.utterances().any(|u| u.main.content.bullet.is_some()) {
            continue;
        }
        for header in source.lines.iter().filter_map(|line| match line {
            Line::Header { header, .. } => Some(header.as_ref()),
            Line::Utterance(_) => None,
        }) {
            let Header::BeginGem { label: Some(label) } = header else {
                continue;
            };
            let parents = (0..source.utterances().count())
                .map(|i| DonorIdx::new(UtteranceIdx::new(i)))
                .collect();
            let selection = SourceBoundDonorSelection::bind(&source, &source, parents)
                .expect("untimed source coordinates remain valid");
            assert!(
                matches!(
                    selection.with_timed_gem_exterior(&source, label.as_str()),
                    Err(MergeError::InvalidGemExterior)
                ),
                "untimed extent must refuse: {}",
                fixture.path().display()
            );
            refusals += 1;
        }
    }
    assert!(refusals > 0, "canonical untimed gem refusal witnesses");
}

#[test]
fn gem_spec_controls_and_mutations_preserve_exterior_admission_boundary() {
    let parser = TreeSitterParser::new().expect("parser");
    let root =
        workspace_root().join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let manifest: ValidationManifest = serde_json::from_str(
        &std::fs::read_to_string(root.join("manifest.json")).expect("canonical manifest"),
    )
    .expect("producer-owned manifest schema");
    let mut covered_codes = std::collections::BTreeSet::new();
    let mut attempts = 0;
    for entry in manifest.fixtures.iter().filter(|entry| {
        matches!(
            entry.code.as_str(),
            "E526" | "E527" | "E528" | "E529" | "E530"
        )
    }) {
        let input = std::fs::read_to_string(root.join(&entry.fixture)).expect("gem spec fixture");
        let diagnostics = talkbank_model::ErrorCollector::new();
        let source = parser.parse_chat_file_streaming(&input, &diagnostics);
        source.validate(
            &diagnostics,
            talkbank_model::model::TranscriptName::Anonymous,
        );
        let emitted = diagnostics.to_vec();
        assert!(
            entry.claim.satisfied_by(&entry.code, |code| emitted
                .iter()
                .any(|error| error.code.to_string() == code.as_str())),
            "authored claim must hold before testing placement refusal: {}",
            entry.fixture
        );
        // Syntax-error examples retain their recovery model only to test the
        // refusal boundary. Passing a spec claim is not clean-parse evidence
        // and never licenses a merge or promotes recovery to valid CHAT.
        // These controls and single-rule mutations are untimed. Even a legal
        // nested or unlabelled gem is not a unique named, fully timed extent.
        // This admission contract does not relabel legal CHAT as invalid.
        assert!(
            source.utterances().all(|u| u.main.content.bullet.is_none()),
            "review admission expectations when timed examples are added: {}",
            entry.fixture
        );
        let mut labels = std::collections::BTreeSet::new();
        for line in &source.lines {
            if let Line::Header { header, .. } = line {
                match header.as_ref() {
                    Header::BeginGem { label } | Header::EndGem { label } => {
                        labels.insert(label.as_ref().map(|label| label.as_str()).unwrap_or(""));
                    }
                    _ => {}
                }
            }
        }
        // A lazy-only file has no paired extent to name.
        if labels.is_empty() {
            labels.insert("");
        }
        for label in labels {
            let parents = (0..source.utterances().count())
                .map(|i| DonorIdx::new(UtteranceIdx::new(i)))
                .collect();
            let selected = SourceBoundDonorSelection::bind(&source, &source, parents)
                .expect("identity projection retains malformed section structure too");
            assert!(
                matches!(
                    selected.with_timed_gem_exterior(&source, label),
                    Err(MergeError::InvalidGemExterior)
                ),
                "no timed extent: {} ({label:?})",
                entry.fixture
            );
            attempts += 1;
        }
        covered_codes.insert(entry.code.as_str().to_owned());
    }
    assert_eq!(
        covered_codes.len(),
        5,
        "all five authored gem-rule populations"
    );
    assert!(
        attempts > covered_codes.len(),
        "controls and mutations both exercised"
    );
}
