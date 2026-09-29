//! Structural merge contracts from canonical source documents, not invented ASTs.
#![allow(clippy::expect_used, clippy::panic)]

#[path = "merge_timing_corpus.rs"]
mod timing_contracts;

#[path = "merge_metadata_corpus.rs"]
mod metadata_contracts;

use talkbank_model::model::{ChatFile, Header, Line, SemanticEq, TranscriptName};
use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{NullErrorSink, RuleSelection, UtteranceIdx, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};
use talkbank_transform::transcript_merge::{
    DonorFate, DonorIdx, DraftOrderReason, MergeError, MergeOrigin, ReferenceFate, ReferenceIdx,
    RelativeOrderConstraint, SourceBoundDonorSelection, default_strip_tiers,
    merge_chat_files_with_donor_selection,
};

#[test]
fn timed_reference_refuses_correspondence_that_reverses_disjoint_speech() {
    let parser = TreeSitterParser::new().expect("parser");
    let path = talkbank_parser_tests::repo_paths::workspace_root()
        .join("corpus/reference/content/media-bullets.cha");
    let source = std::fs::read_to_string(&path).expect("canonical timed speech");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference syntax");
    let valid = file
        .validate_with_policy(
            ValidationPolicy::new(
                RuleSelection::new(),
                AlignmentValidation::IncludeTierAlignment,
            ),
            &NullErrorSink,
            TranscriptName::for_path(&path),
        )
        .expect("valid reference");
    let file = valid.document();
    let before = file.to_chat_string();
    let turns: Vec<_> = file.utterances().collect();
    assert_eq!(turns.len(), 2);
    assert!(
        turns[0]
            .main
            .content
            .bullet
            .as_ref()
            .expect("first timing")
            .timing
            .end_ms
            < turns[1]
                .main
                .content
                .bullet
                .as_ref()
                .expect("second timing")
                .timing
                .start_ms
    );
    let retained = turns[0].main.speaker.clone();
    let mut reference = file.clone();
    reference
        .lines
        .retain(|line| !matches!(line, Line::Utterance(u) if u.main.speaker != retained));
    let mut donor = file.clone();
    donor
        .lines
        .retain(|line| !matches!(line, Line::Utterance(u) if u.main.speaker == retained));
    let reference_index = ReferenceIdx::new(UtteranceIdx::new(0));
    let donor_index = DonorIdx::new(UtteranceIdx::new(0));
    let selection =
        SourceBoundDonorSelection::bind(file, &donor, vec![DonorIdx::new(UtteranceIdx::new(1))])
            .expect("source-preserving speaker projection")
            .with_relative_order(
                &reference,
                vec![RelativeOrderConstraint::DonorBefore {
                    donor: donor_index,
                    reference: reference_index,
                }],
            )
            .expect("coordinate-consistent proposal is not a timing proof");
    let result = merge_chat_files_with_donor_selection(
        &reference,
        &selection,
        std::slice::from_ref(&retained),
        &[],
    );
    assert!(
        matches!(result, Err(MergeError::RelativeOrderTimingConflict { reference, donor })
        if reference == reference_index && donor == donor_index),
        "correspondence cannot reverse disjoint recorded speech: {result:?}"
    );
    let flagged = selection.with_flagged_draft_order(&reference);
    let result = merge_chat_files_with_donor_selection(&reference, &flagged, &[retained], &[]);
    assert!(
        matches!(result, Err(MergeError::RelativeOrderTimingConflict { reference, donor })
        if reference == reference_index && donor == donor_index),
        "review-draft permission cannot override contradictory timing: {result:?}"
    );
    assert_eq!(
        file.to_chat_string(),
        before,
        "refusal cannot retime the admitted source"
    );
}

#[test]
fn reference_untimed_review_draft_preserves_speech_and_visible_uncertainty() {
    let path = talkbank_parser_tests::repo_paths::workspace_root()
        .join("corpus/reference/core/basic-conversation.cha");
    let parser = TreeSitterParser::new().expect("parser");
    let text = std::fs::read_to_string(&path).expect("reference");
    let file = strict_parse(parser.parse_chat_file(&text))
        .expect("reference parses")
        .validate_with_policy(
            ValidationPolicy::new(
                RuleSelection::new(),
                AlignmentValidation::IncludeTierAlignment,
            ),
            &NullErrorSink,
            TranscriptName::for_path(&path),
        )
        .expect("valid reference");
    let source = file.document();
    let retained = talkbank_model::SpeakerCode::new("CHI");
    let mut reference = source.clone();
    reference
        .lines
        .retain(|line| !matches!(line, Line::Utterance(u) if u.main.speaker != retained));
    let mut donor = source.clone();
    donor
        .lines
        .retain(|line| !matches!(line, Line::Utterance(u) if u.main.speaker == retained));
    assert!(source.utterances().all(|u| u.main.content.bullet.is_none()));
    let selection = SourceBoundDonorSelection::bind(
        source,
        &donor,
        vec![
            DonorIdx::new(UtteranceIdx::new(1)),
            DonorIdx::new(UtteranceIdx::new(3)),
        ],
    )
    .expect("original speaker projection");
    assert!(matches!(
        merge_chat_files_with_donor_selection(
            &reference,
            &selection,
            std::slice::from_ref(&retained),
            &[]
        ),
        Err(MergeError::AmbiguousUtteranceOrder { .. })
    ));
    let selection = selection.with_flagged_draft_order(&reference);
    assert!(
        matches!(
            merge_chat_files_with_donor_selection(
                &reference.clone(),
                &selection,
                std::slice::from_ref(&retained),
                &[]
            ),
            Err(MergeError::InvalidRelativeOrder)
        ),
        "permission belongs to the bound source"
    );
    let merged = merge_chat_files_with_donor_selection(&reference, &selection, &[retained], &[])
        .expect("explicitly flagged review draft");
    assert!(merged.bullet_edits().is_empty());
    assert_eq!(merged.draft_order_reviews().len(), 2);
    for (i, review) in merged.draft_order_reviews().iter().enumerate() {
        assert_eq!(review.before_output_utterance.utterances_before(), i);
        assert!(
            matches!(review.reason, DraftOrderReason::Utterances { reference, donor }
            if reference == MergeOrigin::Retained(ReferenceIdx::new(UtteranceIdx::new(i)))
            && donor == MergeOrigin::Inserted(DonorIdx::new(UtteranceIdx::new(0))))
        );
    }
    assert_eq!(
        merged.origins(),
        &[
            MergeOrigin::Retained(ReferenceIdx::new(UtteranceIdx::new(0))),
            MergeOrigin::Retained(ReferenceIdx::new(UtteranceIdx::new(1))),
            MergeOrigin::Inserted(DonorIdx::new(UtteranceIdx::new(0))),
            MergeOrigin::Inserted(DonorIdx::new(UtteranceIdx::new(1))),
        ]
    );
    for (u, origin) in merged.utterances_with_origin() {
        let original = match origin {
            MergeOrigin::Retained(i) => reference.utterances().nth(i.utterance().raw()),
            MergeOrigin::Inserted(i) => donor.utterances().nth(i.utterance().raw()),
        }
        .expect("source coordinate");
        assert!(
            u.semantic_eq(original),
            "drafting cannot edit speech or invent timing"
        );
    }
    let reported = merged.report(|_, _| panic!("complementary projections lose no speech"));
    for line in &reported.file().lines {
        if let Line::Header { header, .. } = line {
            assert!(
                reference.lines.iter().chain(&donor.lines).any(|original| {
                    matches!(original, Line::Header { header: source, .. }
                    if source.to_chat_string() == header.to_chat_string())
                }),
                "no invented review header"
            );
        }
    }
}

#[test]
fn missing_id_spec_refuses_merge_with_original_join_diagnostic() {
    use talkbank_transform::transcript_merge::merge_chat_files_by_source_order;
    let parser = TreeSitterParser::new().expect("parser");
    let path = talkbank_parser_tests::repo_paths::workspace_root()
        .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E522_2.cha");
    let source = std::fs::read_to_string(path).expect("authored missing-ID spec");
    let errors = talkbank_model::ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(&source, &errors);
    let original_diagnostics = errors.into_vec();
    assert_eq!(original_diagnostics.len(), 1);
    assert_eq!(
        original_diagnostics[0].code,
        talkbank_model::ErrorCode::SpeakerNotDefined
    );
    let before = file.to_chat_string();
    let retained = file.unique_utterance_speakers();
    let result = merge_chat_files_by_source_order(&file, &file, &retained, &[]);
    let Err(MergeError::InvalidParticipantJoin { diagnostics }) = result else {
        panic!("missing identity metadata must refuse before issuing a merge: {result:?}");
    };
    assert_eq!(
        diagnostics, original_diagnostics,
        "rejoining source headers must retain the exact diagnostic and location"
    );
    assert_eq!(
        file.to_chat_string(),
        before,
        "refusal cannot invent an ID or drop speech"
    );
    assert_eq!(file.utterances().count(), 2);
}

#[test]
fn backward_timing_spec_refuses_merge_without_sorting_source_turns() {
    use talkbank_transform::transcript_merge::{
        merge_chat_files, merge_chat_files_by_source_order,
    };
    let parser = TreeSitterParser::new().expect("parser");
    let path = talkbank_parser_tests::repo_paths::workspace_root()
        .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E362_2.cha");
    let source = std::fs::read_to_string(path).expect("authored backwards-timing spec");
    // This input is syntactically parsed, not validation-admitted. Both model
    // entry points must refuse its ordering defect before issuing a merge.
    let file = strict_parse(parser.parse_chat_file(&source)).expect("spec syntax");
    let starts: Vec<_> = file
        .utterances()
        .map(|u| {
            u.main
                .content
                .bullet
                .as_ref()
                .expect("spec timing")
                .timing
                .start_ms
        })
        .collect();
    assert_eq!(starts, [10000, 8000, 15000]);
    let retained = file.unique_utterance_speakers();
    let before = file.to_chat_string();
    type MergeOperation = fn(
        &ChatFile,
        &ChatFile,
        &[talkbank_model::SpeakerCode],
        &[String],
    )
        -> Result<talkbank_transform::transcript_merge::Merged, MergeError>;
    for operation in [
        merge_chat_files as MergeOperation,
        merge_chat_files_by_source_order,
    ] {
        let result = operation(&file, &file, &retained, &[]);
        assert!(
            matches!(result, Err(MergeError::SourceTimelineReversal {
            previous: MergeOrigin::Retained(previous), current: MergeOrigin::Retained(current),
        }) if previous == ReferenceIdx::new(UtteranceIdx::new(0))
            && current == ReferenceIdx::new(UtteranceIdx::new(1))),
            "reversal must identify its original neighboring turns: {result:?}"
        );
        assert_eq!(
            file.to_chat_string(),
            before,
            "refusal must not sort or retime source"
        );
    }
}

#[test]
fn canonical_mixed_timing_refuses_to_invent_retained_turn_positions() {
    use talkbank_transform::transcript_merge::merge_chat_files;
    let parser = TreeSitterParser::new().expect("parser");
    let path = talkbank_parser_tests::repo_paths::workspace_root()
        .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E732_1.cha");
    let source = std::fs::read_to_string(&path).expect("authored mixed-timing spec");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("spec syntax");
    let valid = file
        .validate_with_policy(
            ValidationPolicy::new(
                RuleSelection::new(),
                AlignmentValidation::IncludeTierAlignment,
            ),
            &NullErrorSink,
            TranscriptName::for_path(&path),
        )
        .expect("E732 default-mode control is legal CHAT");
    let file = valid.document();
    let times: Vec<_> = file
        .utterances()
        .map(|u| u.main.content.bullet.as_ref().map(|b| b.timing.start_ms))
        .collect();
    assert_eq!(times, [Some(1000), None]);
    let retained = file.unique_utterance_speakers();
    let before = file.to_chat_string();
    let result = merge_chat_files(file, file, &retained, &[]);
    assert!(
        matches!(result,
        Err(MergeError::UnpositionedUtterance { origin: MergeOrigin::Retained(index) })
            if index == ReferenceIdx::new(UtteranceIdx::new(1))),
        "timed merge must refuse original untimed turn 1: {result:?}"
    );
    assert_eq!(
        file.to_chat_string(),
        before,
        "refusal cannot change source timing"
    );
    let ordered = talkbank_transform::transcript_merge::merge_chat_files_by_source_order(
        file,
        file,
        &retained,
        &[],
    )
    .expect("one retained source already supplies the order of its own turns");
    assert_eq!(
        ordered.origins(),
        [
            MergeOrigin::Retained(ReferenceIdx::new(UtteranceIdx::new(0))),
            MergeOrigin::Retained(ReferenceIdx::new(UtteranceIdx::new(1))),
        ]
    );
    for ((actual, _), original) in ordered.utterances_with_origin().zip(file.utterances()) {
        assert!(
            actual.semantic_eq(original),
            "source order cannot invent a time bullet"
        );
    }
    assert!(ordered.bullet_edits().is_empty());
}

#[test]
fn reference_donor_selection_binds_identity_and_refuses_invalid_coordinates() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut admitted = 0;
    let mut invalid_coordinates = 0;
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        let count = file.utterances().count();
        let parents: Vec<_> = (0..count)
            .map(|i| DonorIdx::new(UtteranceIdx::new(i)))
            .collect();
        let mut previous_start = None;
        let mut reversed = false;
        for bullet in file
            .utterances()
            .filter_map(|u| u.main.content.bullet.as_ref())
        {
            reversed |= previous_start.is_some_and(|start| start > bullet.timing.start_ms);
            previous_start = Some(bullet.timing.start_ms);
        }
        match SourceBoundDonorSelection::bind(&file, &file, parents.clone()) {
            Ok(selection) => {
                assert!(
                    !reversed,
                    "reversed original must refuse: {}",
                    fixture.path().display()
                );
                assert_eq!(selection.parents(), parents);
                admitted += 1;
            }
            Err(MergeError::InvalidDonorSelection) => assert!(
                reversed,
                "identity selection must retain valid source coordinates: {}",
                fixture.path().display()
            ),
            Err(error) => panic!("unexpected identity selection failure: {error}"),
        }
        let mut extra = parents.clone();
        extra.push(DonorIdx::new(UtteranceIdx::new(count)));
        assert!(
            matches!(
                SourceBoundDonorSelection::bind(&file, &file, extra),
                Err(MergeError::InvalidDonorSelection)
            ),
            "parent count must match selection"
        );
        if count > 0 {
            let mut outside = parents.clone();
            outside[0] = DonorIdx::new(UtteranceIdx::new(count));
            assert!(
                matches!(
                    SourceBoundDonorSelection::bind(&file, &file, outside),
                    Err(MergeError::InvalidDonorSelection)
                ),
                "parent must exist in bound original"
            );
            invalid_coordinates += 1;
        }
        if count > 1 {
            let mut backwards = parents;
            backwards.reverse();
            assert!(
                matches!(
                    SourceBoundDonorSelection::bind(&file, &file, backwards),
                    Err(MergeError::InvalidDonorSelection)
                ),
                "selection cannot reverse source parents"
            );
        }
    }
    assert!(
        admitted > 0 && invalid_coordinates > 0,
        "nonvacuous source-bound witnesses"
    );
}

#[test]
fn reference_retain_all_merge_preserves_speech_and_total_provenance() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut admitted = 0;
    let mut ambiguous_speakers = 0;
    let mut stripped_tiers = 0;
    let mut repeated_drops = 0;
    let mut overlap_drop_refusals = 0;
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        // Body headers from two sources need a separate section-placement
        // ruling. This contract isolates speech retention, not header deduping.
        let mut in_body = false;
        let mut body_headers = false;
        for line in &file.lines {
            match line {
                Line::Utterance(_) => in_body = true,
                Line::Header { header, .. } => {
                    body_headers |= in_body && !matches!(header.as_ref(), Header::End);
                    body_headers |= matches!(
                        header.as_ref(),
                        Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
                    );
                }
            }
        }
        if body_headers || file.utterances().next().is_none() {
            continue;
        }
        let Ok(valid) = file.validate_with_policy(
            ValidationPolicy::new(
                RuleSelection::new(),
                AlignmentValidation::IncludeTierAlignment,
            ),
            &NullErrorSink,
            TranscriptName::Anonymous,
        ) else {
            continue;
        };
        let file = valid.document();
        let count = file.utterances().count();
        let parents = (0..count)
            .map(|i| DonorIdx::new(UtteranceIdx::new(i)))
            .collect();
        let selection = SourceBoundDonorSelection::bind(file, file, parents)
            .expect("validated source admits identity selection");
        let retained = file.unique_utterance_speakers();
        assert!(
            matches!(merge_chat_files_with_donor_selection(file, &selection, &[], &[]),
            Err(MergeError::RetainSpeakersMissing { retain }) if retain.is_empty()),
            "nonempty reference cannot admit an empty retain set"
        );
        if retained.len() > 1 {
            let first = &retained[..1];
            let conflicting = file
                .utterances()
                .find(|u| !first.contains(&u.main.speaker))
                .expect("second speaker witness");
            assert!(
                matches!(merge_chat_files_with_donor_selection(file, &selection, first, &[]),
                Err(MergeError::AmbiguousSpeaker { speaker }) if speaker == conflicting.main.speaker),
                "both sources carrying an unretained speaker requires adjudication"
            );
            ambiguous_speakers += 1;
        }
        let merged = merge_chat_files_with_donor_selection(file, &selection, &retained, &[])
            .unwrap_or_else(|error| {
                panic!("retain-all merge {}: {error}", fixture.path().display())
            });
        assert_eq!(
            merged.reference_fates(),
            vec![ReferenceFate::Retained; count]
        );
        assert_eq!(
            merged.donor_fates(),
            vec![DonorFate::ExcludedByRetain; count]
        );
        assert_eq!(merged.utterances_with_origin().count(), count);
        for (i, ((output, origin), original)) in merged
            .utterances_with_origin()
            .zip(file.utterances())
            .enumerate()
        {
            let index = ReferenceIdx::new(UtteranceIdx::new(i));
            assert_eq!(origin, MergeOrigin::Retained(index));
            assert!(
                original.semantic_eq(output),
                "selected speech and dependent tiers survive"
            );
            assert_eq!(merged.reference_fate(index), Some(&ReferenceFate::Retained));
            assert_eq!(
                merged.donor_fate(DonorIdx::new(UtteranceIdx::new(i))),
                Some(&DonorFate::ExcludedByRetain)
            );
        }
        assert_eq!(
            merged.reference_fate(ReferenceIdx::new(UtteranceIdx::new(count))),
            None
        );
        assert_eq!(
            merged.donor_fate(DonorIdx::new(UtteranceIdx::new(count))),
            None
        );
        assert_eq!(merged.dropped_not_retained().count(), 0);
        assert_eq!(merged.excluded_by_retain().count(), count);
        assert!(merged.bullet_edits().is_empty());
        assert!(merged.draft_order_reviews().is_empty());
        assert!(merged.gem_exterior_placements().is_empty());
        let reported = merged.report(|_, _| panic!("retain-all cannot drop speech"));
        let output = reported.file().to_chat_string();
        let reparsed =
            strict_parse(parser.parse_chat_file(&output)).expect("reported merge parses");
        assert!(
            reported.file().semantic_eq(&reparsed),
            "reported wire semantics"
        );
        assert!(
            reported.into_file().semantic_eq(&reparsed),
            "consuming report preserves semantics"
        );
        stripped_tiers += donor_insertion_preserves_selected_payload(file, &selection, &parser);
        empty_donor_selection_preserves_participant_refusal(file);
        if retained.len() > 1 {
            reconstruct_speaker_partition(file, &parser);
            if fixture.path().ends_with("core/basic-conversation.cha")
                || fixture.path().ends_with("languages/eng-conversation.cha")
                || fixture.path().ends_with("ca/overlaps.cha")
            {
                repeated_drops += selected_reference_speech_reports_every_drop(
                    file,
                    &parser,
                    fixture.path().ends_with("ca/overlaps.cha"),
                );
                overlap_drop_refusals += usize::from(fixture.path().ends_with("ca/overlaps.cha"));
            }
        }
        admitted += 1;
    }
    assert!(
        admitted > 0,
        "reference corpus must witness validated retain-all merges"
    );
    assert!(
        ambiguous_speakers > 0,
        "reference corpus must witness ambiguous speaker refusal"
    );
    assert!(
        stripped_tiers > 0,
        "reference corpus must witness actual donor-tier removal"
    );
    assert_eq!(
        repeated_drops, 2,
        "both conversation controls must report repeated speaker loss"
    );
    assert_eq!(
        overlap_drop_refusals, 1,
        "CA control must refuse unmatched overlap"
    );
    eprintln!(
        "validated retain-all merge witnesses: {admitted}; ambiguous speaker refusals: {ambiguous_speakers}"
    );
}

/// Exercise the mandatory reporting transition with an empty donor projection.
/// All speech, metadata and expected counts come from the admitted reference.
fn selected_reference_speech_reports_every_drop(
    source: &ChatFile,
    parser: &TreeSitterParser,
    leaves_unmatched_overlap: bool,
) -> usize {
    use talkbank_transform::transcript_merge::merge_chat_files_by_source_order;

    let retained = &source
        .utterances()
        .next()
        .expect("nonempty source")
        .main
        .speaker;
    let mut donor = source.clone();
    donor.lines.retain(|line| match line {
        Line::Utterance(_) => false,
        Line::Header { header, .. } => match header.as_ref() {
            Header::ID(id) => &id.speaker == retained,
            _ => true,
        },
    });
    for line in &mut donor.lines {
        if let Line::Header { header, .. } = line
            && let Header::Participants { entries } = header.as_mut()
        {
            *entries = talkbank_model::model::ParticipantEntries::new(
                entries
                    .iter()
                    .filter(|entry| &entry.speaker_code == retained)
                    .cloned()
                    .collect(),
            );
        }
    }
    let expected_drops: Vec<_> = source
        .utterances()
        .enumerate()
        .filter(|(_, utterance)| &utterance.main.speaker != retained)
        .map(|(index, _)| ReferenceIdx::new(UtteranceIdx::new(index)))
        .collect();
    let expected_counts: Vec<_> = source
        .unique_utterance_speakers()
        .into_iter()
        .filter(|speaker| speaker != retained)
        .map(|speaker| {
            let count = source
                .utterances()
                .filter(|u| u.main.speaker == speaker)
                .count();
            (speaker, count)
        })
        .collect();
    let result =
        merge_chat_files_by_source_order(source, &donor, std::slice::from_ref(retained), &[]);
    if leaves_unmatched_overlap {
        assert!(
            matches!(result, Err(MergeError::InvalidOutput(failure))
            if !failure.has_internal_failure() && failure.diagnostics().iter()
                .any(|error| error.code == talkbank_model::ErrorCode::UnbalancedOverlap)),
            "dropping the other half of an overlap cannot produce an admitted merge"
        );
        return 0;
    }
    let merged = result.expect("explicit reference selection with no competing donor speech");
    assert_eq!(
        merged.dropped_not_retained().collect::<Vec<_>>(),
        expected_drops
    );
    assert_eq!(merged.dropped_speakers(), expected_counts);
    for (index, utterance) in source.utterances().enumerate() {
        let expected = if &utterance.main.speaker == retained {
            ReferenceFate::Retained
        } else {
            ReferenceFate::DroppedNotRetained {
                speaker: utterance.main.speaker.clone(),
            }
        };
        assert_eq!(
            merged.reference_fate(ReferenceIdx::new(UtteranceIdx::new(index))),
            Some(&expected)
        );
    }
    let mut reports = Vec::new();
    let reported = merged.report(|speaker, count| reports.push((speaker.clone(), count)));
    assert_eq!(
        reports, expected_counts,
        "report transition delivers each loss exactly once"
    );
    let expected_speech: Vec<_> = source
        .utterances()
        .filter(|u| &u.main.speaker == retained)
        .collect();
    let output = reported.into_file();
    assert_eq!(output.utterances().count(), expected_speech.len());
    for (actual, expected) in output.utterances().zip(expected_speech) {
        assert!(
            actual.semantic_eq(expected),
            "selected speech and tiers stay unchanged"
        );
    }
    let reparsed = strict_parse(parser.parse_chat_file(&output.to_chat_string()))
        .expect("reported selection output parses");
    assert!(output.semantic_eq(&reparsed));
    expected_counts
        .iter()
        .filter(|(_, count)| *count > 1)
        .count()
}

/// Both projections descend from one source, so original adjacency is evidence
/// of structural order. It is not an assertion about acoustic precedence.
fn reconstruct_speaker_partition(source: &ChatFile, parser: &TreeSitterParser) {
    let retained = source
        .utterances()
        .next()
        .expect("nonempty source")
        .main
        .speaker
        .clone();
    let mut reference = source.clone();
    reference.lines.retain(|line| match line {
        Line::Header { .. } => true,
        Line::Utterance(u) => u.main.speaker == retained,
    });
    let mut donor = source.clone();
    donor.lines.retain(|line| match line {
        Line::Header { .. } => true,
        Line::Utterance(u) => u.main.speaker != retained,
    });
    let mut parents = Vec::new();
    let mut origins = Vec::new();
    let mut proposals = Vec::new();
    let mut reference_count = 0;
    let mut donor_count = 0;
    for (parent, utterance) in source.utterances().enumerate() {
        let origin = if utterance.main.speaker == retained {
            let index = ReferenceIdx::new(UtteranceIdx::new(reference_count));
            reference_count += 1;
            MergeOrigin::Retained(index)
        } else {
            let index = DonorIdx::new(UtteranceIdx::new(donor_count));
            donor_count += 1;
            parents.push(DonorIdx::new(UtteranceIdx::new(parent)));
            MergeOrigin::Inserted(index)
        };
        if let Some(previous) = origins.last().copied() {
            match (previous, origin) {
                (MergeOrigin::Retained(reference), MergeOrigin::Inserted(donor)) => {
                    proposals.push(RelativeOrderConstraint::ReferenceBefore { reference, donor })
                }
                (MergeOrigin::Inserted(donor), MergeOrigin::Retained(reference)) => {
                    proposals.push(RelativeOrderConstraint::DonorBefore { donor, reference })
                }
                (MergeOrigin::Retained(_), MergeOrigin::Retained(_))
                | (MergeOrigin::Inserted(_), MergeOrigin::Inserted(_)) => {}
            }
        }
        origins.push(origin);
    }
    assert!(reference_count > 0 && donor_count > 0 && !proposals.is_empty());
    let bind = || {
        SourceBoundDonorSelection::bind(source, &donor, parents.clone())
            .expect("speaker projection preserves source/header coordinates")
    };
    let mut contradictory = proposals.clone();
    contradictory.push(match proposals[0] {
        RelativeOrderConstraint::ReferenceBefore { reference, donor } => {
            RelativeOrderConstraint::DonorBefore { donor, reference }
        }
        RelativeOrderConstraint::DonorBefore { donor, reference } => {
            RelativeOrderConstraint::ReferenceBefore { reference, donor }
        }
    });
    assert!(
        matches!(
            bind().with_relative_order(&reference, contradictory),
            Err(MergeError::InvalidRelativeOrder)
        ),
        "contradictory source order must refuse"
    );
    let selection = bind()
        .with_relative_order(&reference, proposals)
        .expect("original source adjacency is consistent");
    let unrelated_copy = reference.clone();
    assert!(
        matches!(
            merge_chat_files_with_donor_selection(
                &unrelated_copy,
                &selection,
                std::slice::from_ref(&retained),
                &[],
            ),
            Err(MergeError::InvalidRelativeOrder)
        ),
        "equal contents cannot substitute for bound source identity"
    );
    let merged = merge_chat_files_with_donor_selection(
        &reference,
        &selection,
        std::slice::from_ref(&retained),
        &[],
    )
    .expect("complementary speaker projections reconstruct source order");
    assert_eq!(merged.origins(), origins);
    assert_eq!(
        merged.reference_fates(),
        vec![ReferenceFate::Retained; reference_count]
    );
    assert_eq!(
        merged.donor_fates(),
        vec![DonorFate::Inserted { tiers_stripped: 0 }; donor_count]
    );
    for ((actual, _), original) in merged.utterances_with_origin().zip(source.utterances()) {
        assert!(
            actual.semantic_eq(original),
            "partition reconstruction preserves all speech and tiers"
        );
    }
    assert!(merged.bullet_edits().is_empty());
    assert!(merged.draft_order_reviews().is_empty());
    let reported = merged.report(|_, _| panic!("complementary partition omits no speech"));
    let parsed = strict_parse(parser.parse_chat_file(&reported.file().to_chat_string()))
        .expect("reconstructed partition output parses");
    assert!(reported.file().semantic_eq(&parsed));
}

/// Project an actual document to its existing opening metadata, then insert
/// its source-bound speech. No timing, lexical content or speaker is invented.
fn donor_insertion_preserves_selected_payload(
    donor: &ChatFile,
    selection: &SourceBoundDonorSelection<'_>,
    parser: &TreeSitterParser,
) -> usize {
    let mut reference = donor.clone();
    reference.lines = reference
        .lines
        .into_iter()
        .filter(|line| matches!(line, Line::Header { .. }))
        .collect::<Vec<_>>()
        .into();
    let default_strip = default_strip_tiers();
    let mut total_stripped = 0;
    for strip in [&[][..], default_strip.as_slice()] {
        let merged = merge_chat_files_with_donor_selection(&reference, selection, &[], strip)
            .expect("header-only reference admits source-bound donor speech");
        assert!(merged.reference_fates().is_empty());
        assert_eq!(merged.donor_fates().len(), donor.utterances().count());
        assert_eq!(
            merged.utterances_with_origin().count(),
            donor.utterances().count()
        );
        assert_eq!(merged.excluded_by_retain().count(), 0);
        for (i, ((output, origin), source)) in merged
            .utterances_with_origin()
            .zip(donor.utterances())
            .enumerate()
        {
            let index = DonorIdx::new(UtteranceIdx::new(i));
            assert_eq!(origin, MergeOrigin::Inserted(index));
            assert!(
                output.main.semantic_eq(&source.main),
                "donor main tier must be untouched"
            );
            assert!(
                output
                    .preceding_headers
                    .semantic_eq(&source.preceding_headers)
            );
            let kept = source
                .dependent_tiers
                .iter()
                .filter(|tier| !strip.iter().any(|kind| kind == tier.kind()));
            let removed = source.dependent_tiers.len() - kept.clone().count();
            assert_eq!(
                merged.donor_fate(index),
                Some(&DonorFate::Inserted {
                    tiers_stripped: removed
                })
            );
            assert_eq!(output.dependent_tiers.len(), kept.clone().count());
            for (actual, expected) in output.dependent_tiers.iter().zip(kept) {
                assert!(
                    actual.semantic_eq(expected),
                    "unstripped tiers retain content and order"
                );
            }
            if strip.is_empty() {
                assert!(
                    output.semantic_eq(source),
                    "no-strip donor insertion is unchanged"
                );
            }
            total_stripped += removed;
        }
        assert!(
            merged.bullet_edits().is_empty(),
            "insertion cannot invent timestamps"
        );
        let reported = merged.report(|_, _| panic!("header-only reference cannot drop speech"));
        let reparsed = strict_parse(parser.parse_chat_file(&reported.file().to_chat_string()))
            .expect("inserted donor output parses");
        assert!(
            reported.file().semantic_eq(&reparsed),
            "inserted donor wire semantics"
        );
    }
    total_stripped
}

/// Omitting donor speech does not erase its original participant declarations.
fn empty_donor_selection_preserves_participant_refusal(reference: &ChatFile) {
    let mut selected = reference.clone();
    selected.lines = selected
        .lines
        .into_iter()
        .filter(|line| matches!(line, Line::Header { .. }))
        .collect::<Vec<_>>()
        .into();
    let empty_donor = SourceBoundDonorSelection::bind(reference, &selected, Vec::new())
        .expect("omitting all donor speech retains original opening header coordinates");
    let retained = reference
        .utterances()
        .next()
        .expect("admitted nonempty source")
        .main
        .speaker
        .clone();
    let result = merge_chat_files_with_donor_selection(
        reference,
        &empty_donor,
        std::slice::from_ref(&retained),
        &default_strip_tiers(),
    );
    if reference.utterances().any(|u| u.main.speaker != retained) {
        match result {
            Err(MergeError::ParticipantAlreadyDeclared {
                speaker,
                file1_role,
                donor_role,
            }) => {
                assert_ne!(speaker, retained);
                assert!(reference.utterances().any(|u| u.main.speaker == speaker));
                assert_eq!(
                    file1_role, donor_role,
                    "even identical metadata cannot erase live speech"
                );
            }
            other => panic!("omitted speech must not hide participant collision: {other:?}"),
        }
    } else {
        let merged = result.expect("all live reference speakers retained");
        assert!(merged.donor_fates().is_empty());
        assert_eq!(
            merged.reference_fates(),
            vec![ReferenceFate::Retained; reference.utterances().count()]
        );
    }
}
