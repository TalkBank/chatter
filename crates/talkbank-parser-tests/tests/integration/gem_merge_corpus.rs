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

#[path = "gem_pair_order_corpus.rs"]
mod pair_order_contracts;

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

    let boundaries: Vec<_> = reference
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| match line {
            Line::Header { header, .. }
                if matches!(
                    header.as_ref(),
                    Header::BeginGem { .. } | Header::EndGem { .. }
                ) =>
            {
                Some(index)
            }
            _ => None,
        })
        .collect();
    let [opening, closing] = boundaries.as_slice() else {
        panic!("reference control has exactly one paired gem");
    };
    let mut reversed = reference.clone();
    let mut reversed_lines = reference.lines.to_vec();
    reversed_lines.swap(*opening, *closing);
    reversed.lines = reversed_lines.into();
    let mut empty = reference.clone();
    empty.lines = reference
        .lines
        .iter()
        .enumerate()
        .filter(|(index, _)| *index <= *opening || *index >= *closing)
        .map(|(_, line)| line.clone())
        .collect::<Vec<_>>()
        .into();
    for refused in [&reversed, &empty] {
        let before = refused.to_chat_string();
        assert!(
            matches!(
                bind().with_timed_gem_exterior(refused, label),
                Err(MergeError::InvalidGemExterior)
            ),
            "reversed or speechless sections cannot issue a timed extent"
        );
        assert_eq!(
            refused.to_chat_string(),
            before,
            "refusal must not repair the input"
        );
    }

    let gem_start = reference
        .utterances()
        .next()
        .expect("gem start")
        .main
        .content
        .bullet
        .as_ref()
        .expect("timed gem")
        .timing
        .start_ms;
    let gem_end = reference
        .utterances()
        .last()
        .expect("gem end")
        .main
        .content
        .bullet
        .as_ref()
        .expect("timed gem")
        .timing
        .end_ms;
    for before_gem in [true, false] {
        let mut touching = donor.clone();
        let index = if before_gem {
            0
        } else {
            donor.utterances().count() - 1
        };
        let row = (&mut touching.lines)
            .into_iter()
            .filter_map(|line| match line {
                Line::Utterance(row) => Some(row),
                Line::Header { .. } => None,
            })
            .nth(index)
            .expect("exterior donor row");
        let original = row.main.content.bullet.as_ref().expect("timed donor");
        // Deliberate input-boundary mutation, not an authorized output retiming.
        // The new interval has no claimed source span.
        row.main.content.bullet = Some(if before_gem {
            talkbank_model::model::Bullet::new(original.timing.start_ms, gem_start)
        } else {
            talkbank_model::model::Bullet::new(gem_end, original.timing.end_ms)
        });
        let wire = touching.to_chat_string();
        let selected = SourceBoundDonorSelection::bind(&touching, &touching, parents())
            .expect("mutated donor bound to its own coordinates")
            .with_timed_gem_exterior(&reference, label)
            .expect("unchanged timed gem");
        assert!(
            matches!(
                merge_chat_files_with_donor_selection(
                    &reference,
                    &selected,
                    std::slice::from_ref(&retained),
                    &[],
                ),
                Err(MergeError::AmbiguousSectionPlacement { .. })
            ),
            "endpoint equality is not strictly exterior placement"
        );
        assert_eq!(touching.to_chat_string(), wire);
    }

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
    let reconstructed: talkbank_model::model::ChatFile =
        serde_json::from_str(&serde_json::to_string(reported.file()).expect("merged JSON"))
            .expect("reconstruct merged JSON");
    assert!(reported.file().semantic_eq(&reconstructed));
    assert_eq!(reconstructed.to_chat_string(), wire);
}

/// Source timing brackets prove placement; speaker partitions do not authorize
/// guessing which side of a section owns a turn in an unbounded gap.
#[test]
fn reference_timed_gem_interleaving_preserves_sections_or_refuses_ambiguity() {
    use talkbank_model::model::TranscriptName;
    use talkbank_model::{ErrorCollector, SpeakerCode};

    let parser = TreeSitterParser::new().expect("parser");
    let source_text = std::fs::read_to_string(
        workspace_root().join("corpus/reference/edge-cases/timed-gem-interleaving.cha"),
    )
    .expect("authored timed section reference");
    let mut source = strict_parse(parser.parse_chat_file(&source_text)).expect("reference parses");
    let errors = ErrorCollector::new();
    source.validate_with_alignment(&errors, TranscriptName::Anonymous);
    assert!(
        errors.is_empty(),
        "reference validates: {:?}",
        errors.to_vec()
    );

    for retained in [SpeakerCode::new("CHI"), SpeakerCode::new("MOT")] {
        let mut reference = source.clone();
        reference.lines.retain(|line| match line {
            Line::Utterance(u) => u.main.speaker == retained,
            Line::Header { .. } => true,
        });
        let mut donor = source.clone();
        donor.lines.retain(|line| match line {
            Line::Utterance(u) => u.main.speaker != retained,
            Line::Header { header, .. } => !matches!(
                header.as_ref(),
                Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
            ),
        });
        // This projection intentionally supplies speech only. It is bound to
        // its own coordinates, not claimed to preserve the removed sections.
        let parents = (0..donor.utterances().count())
            .map(|i| DonorIdx::new(UtteranceIdx::new(i)))
            .collect();
        let selection = SourceBoundDonorSelection::bind(&donor, &donor, parents)
            .expect("unchanged selected donor");
        let source_order = merge_chat_files_with_donor_selection(
            &reference,
            &selection,
            std::slice::from_ref(&retained),
            &[],
        );
        assert!(
            matches!(
                source_order,
                Err(MergeError::AmbiguousSectionPlacement { .. })
            ),
            "source-order bounds cannot infer section placement within an anchor gap"
        );
        let merged = talkbank_transform::transcript_merge::merge_chat_files(
            &reference,
            &donor,
            std::slice::from_ref(&retained),
            &[],
        );
        if retained.as_str() == "MOT" {
            assert!(
                matches!(merged, Err(MergeError::AmbiguousSectionPlacement { .. })),
                "the 10ms donor turn cannot be placed around a section bounded by 5ms and 55ms"
            );
            continue;
        }
        let forward = merged.expect("both section boundaries have ordering witnesses");
        let reverse = talkbank_transform::transcript_merge::merge_chat_files(
            &donor,
            &reference,
            &[SpeakerCode::new("MOT")],
            &[],
        )
        .expect("section-bearing donor uses the same timing proof");
        fn structural(file: &talkbank_model::model::ChatFile) -> impl Iterator<Item = &Line> {
            file.lines.iter().filter(|line| match line {
                Line::Utterance(_) => true,
                Line::Header { header, .. } => matches!(
                    header.as_ref(),
                    Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
                ),
            })
        }
        for merged in [forward, reverse] {
            assert!(merged.bullet_edits().is_empty());
            assert!(merged.draft_order_reviews().is_empty());
            let reported = merged.report(|_, _| panic!("complementary projections omit no speech"));
            let mut actual = structural(reported.file());
            let mut expected = structural(&source);
            loop {
                match (actual.next(), expected.next()) {
                    (Some(actual), Some(expected)) => assert!(
                        actual.semantic_eq(expected),
                        "speech, dependent tiers, exact timing and section order all survive"
                    ),
                    (None, None) => break,
                    _ => panic!("structural event count changed"),
                }
            }
        }
    }
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
