//! Structural merge contracts from canonical source documents, not invented ASTs.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{ChatFile, Header, Line, SemanticEq, TranscriptName};
use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
use talkbank_model::{NullErrorSink, RuleSelection, UtteranceIdx, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};
use talkbank_transform::transcript_merge::{
    DonorFate, DonorIdx, MergeError, MergeOrigin, ReferenceFate, ReferenceIdx,
    RelativeOrderConstraint, SourceBoundDonorSelection, default_strip_tiers,
    merge_chat_files_with_donor_selection,
};

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
    eprintln!(
        "validated retain-all merge witnesses: {admitted}; ambiguous speaker refusals: {ambiguous_speakers}"
    );
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
