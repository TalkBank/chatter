//! Paired section order must follow observed brackets, not a source preference.
use talkbank_model::model::{ChatFile, Header, Line, SemanticEq, TranscriptName};
use talkbank_model::{ErrorCollector, SpeakerCode, UtteranceIdx, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;
use talkbank_transform::transcript_merge::{
    DonorIdx, DraftOrderReason, MergeError, MergeOrigin, SourceBoundDonorSelection,
    merge_chat_files, merge_chat_files_by_source_order, merge_chat_files_with_donor_selection,
    merge_chat_files_with_donor_selection_draft,
};

fn parsed_reference(path: &str) -> ChatFile {
    let source = std::fs::read_to_string(workspace_root().join(path)).expect("authored reference");
    let parser = TreeSitterParser::new().expect("parser");
    let mut file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let errors = ErrorCollector::new();
    file.validate_with_alignment(&errors, TranscriptName::Anonymous);
    assert!(
        errors.is_empty(),
        "reference validates: {:?}",
        errors.to_vec()
    );
    file
}

fn partition(source: &ChatFile, speaker: &SpeakerCode, section: Option<&str>) -> ChatFile {
    let mut file = source.clone();
    file.lines.retain(|line| match line {
        Line::Utterance(u) => &u.main.speaker == speaker,
        Line::Header { header, .. } => match header.as_ref() {
            Header::BeginGem { label } | Header::EndGem { label } => {
                section.is_none_or(|section| {
                    label
                        .as_ref()
                        .is_some_and(|label| label.as_str() == section)
                })
            }
            _ => true,
        },
    });
    file
}

fn structural(file: &ChatFile) -> impl Iterator<Item = &Line> {
    file.lines.iter().filter(|line| match line {
        Line::Utterance(_) => true,
        Line::Header { header, .. } => matches!(
            header.as_ref(),
            Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
        ),
    })
}

#[test]
fn reference_distinct_gem_brackets_order_both_sources_without_retiming() {
    let source = parsed_reference("corpus/reference/edge-cases/timed-gem-pair-order.cha");
    let child = SpeakerCode::new("CHI");
    let adult = SpeakerCode::new("MOT");
    let child_source = partition(&source, &child, Some("child"));
    let adult_source = partition(&source, &adult, Some("adult"));
    for (reference, donor, retained) in [
        (&child_source, &adult_source, &child),
        (&adult_source, &child_source, &adult),
    ] {
        let merged = merge_chat_files(reference, donor, std::slice::from_ref(retained), &[])
            .expect("recorded brackets prove the same unique section order in both directions");
        assert!(merged.bullet_edits().is_empty());
        assert!(merged.draft_order_reviews().is_empty());
        let reported = merged.report(|_, _| panic!("complementary selections lose no speech"));
        let mut actual = structural(reported.file());
        let mut expected = structural(&source);
        loop {
            match (actual.next(), expected.next()) {
                (Some(actual), Some(expected)) => assert!(actual.semantic_eq(expected)),
                (None, None) => break,
                _ => panic!("section or utterance count changed"),
            }
        }
    }
}

#[test]
fn reference_shared_gem_brackets_refuse_unproved_cross_source_order() {
    let source = parsed_reference("corpus/reference/edge-cases/timed-gem-shared-bracket.cha");
    let child = SpeakerCode::new("CHI");
    let adult = SpeakerCode::new("MOT");
    let child_source = partition(&source, &child, None);
    let adult_source = partition(&source, &adult, None);
    for (reference, donor, retained) in [
        (&child_source, &adult_source, &child),
        (&adult_source, &child_source, &adult),
    ] {
        let error = merge_chat_files(reference, donor, std::slice::from_ref(retained), &[])
            .expect_err("equal section labels cannot supply ordering or authorize deduplication");
        assert!(
            matches!(error, MergeError::AmbiguousSectionOrder { .. }),
            "{error:?}"
        );
    }
}

#[test]
fn reference_crossed_gem_neighbors_refuse_invented_section_instant() {
    let source = parsed_reference("corpus/reference/edge-cases/timed-gem-crossed-neighbors.cha");
    let original = source.to_chat();
    let investigator = SpeakerCode::new("INV");
    let retained = [SpeakerCode::new("CHI"), SpeakerCode::new("MOT")];
    let mut reference = source.clone();
    reference.lines.retain(|line| match line {
        Line::Utterance(u) => retained.contains(&u.main.speaker),
        Line::Header { .. } => true,
    });
    let mut donor = partition(&source, &investigator, None);
    donor.lines.retain(|line| {
        !matches!(line,
            Line::Header { header, .. }
                if matches!(header.as_ref(), Header::BeginGem { .. } | Header::EndGem { .. })
        )
    });
    // The source validates: overlap belongs to different speakers. After
    // selecting speech, 45ms precedes this header and 30ms follows it.
    // Neither endpoint may be promoted to a section instant to order INV.
    for (left, right, speakers) in [
        (&reference, &donor, retained.as_slice()),
        (&donor, &reference, std::slice::from_ref(&investigator)),
    ] {
        let result = merge_chat_files(left, right, speakers, &[]);
        assert!(
            matches!(
                result,
                Err(MergeError::AmbiguousSectionPlacement {
                    previous_end: Some(45),
                    next_start: Some(30),
                    ..
                })
            ),
            "crossed neighbors cannot order the competing 25ms turn: {result:?}"
        );
    }
    assert_eq!(source.to_chat(), original);
}

#[test]
fn reference_shared_gem_source_coordinates_do_not_prove_cross_source_order() {
    let source = parsed_reference("corpus/reference/edge-cases/timed-gem-shared-bracket.cha");
    let original = source.to_chat();
    let child = SpeakerCode::new("CHI");
    let adult = SpeakerCode::new("MOT");
    for (retained, inserted) in [(&child, &adult), (&adult, &child)] {
        let reference = partition(&source, retained, None);
        let donor = partition(&source, inserted, None);
        let parents = source
            .utterances()
            .enumerate()
            .filter(|(_, u)| &u.main.speaker == inserted)
            .map(|(i, _)| DonorIdx::new(UtteranceIdx::new(i)))
            .collect();
        let selection = SourceBoundDonorSelection::bind(&source, &donor, parents)
            .expect("speaker projection preserves every header at its original boundary");
        // The donor retains actual header brackets even when a neighboring
        // speaker is omitted. Binding those coordinates cannot prove whether
        // competing speech belongs before or after a shared section marker.
        let projected = merge_chat_files_by_source_order(
            &reference,
            &donor,
            std::slice::from_ref(retained),
            &[],
        );
        assert!(
            matches!(
                projected,
                Err(MergeError::AmbiguousSectionPlacement {
                    previous_end: Some(20),
                    next_start: Some(30),
                    ..
                })
            ),
            "unbound projection uses its own neighbors: {projected:?}"
        );
        let bound = merge_chat_files_with_donor_selection(
            &reference,
            &selection,
            std::slice::from_ref(retained),
            &[],
        );
        // In the reversed direction the donor's original preceding neighbor
        // is MOT (ending at 25), omitted from the CHI projection (ending at 20).
        let expected_previous = if retained == &child { 20 } else { 25 };
        assert!(
            matches!(bound, Err(MergeError::AmbiguousSectionPlacement {
            previous_end: Some(previous), next_start: Some(30), ..
        }) if previous == expected_previous),
            "source-bound refusal must retain original bracket evidence: {bound:?}"
        );
    }
    assert_eq!(source.to_chat(), original);
}

#[test]
fn reference_gem_excerpt_preserves_headers_without_claiming_their_order() {
    let source = parsed_reference("corpus/reference/edge-cases/timed-gem-exterior.cha");
    let original = source.to_chat();
    let child = SpeakerCode::new("CHI");
    let reference = partition(&source, &child, None);
    let mut excerpt = source.clone();
    let final_turn = source.utterances().last().expect("exterior speech");
    assert_eq!(source.utterances().count(), 4);
    assert_eq!(final_turn.main.speaker.as_str(), "MOT");
    excerpt.lines.retain(|line| match line {
        Line::Utterance(u) => u.as_ref().semantic_eq(final_turn),
        Line::Header { .. } => true,
    });
    assert_eq!(excerpt.utterances().count(), 1);
    let selection = SourceBoundDonorSelection::bind(
        &source,
        &excerpt,
        vec![DonorIdx::new(UtteranceIdx::new(3))],
    )
    .expect("terminal speech excerpt preserves the original header coordinates");
    // Both projections start with the preserved gem. The original donor
    // bracket must not be replaced by the excerpt's distant remaining turn,
    // and equal labels authorize neither deduplication nor an ordering tie.
    let result = merge_chat_files_with_donor_selection(
        &reference,
        &selection,
        std::slice::from_ref(&child),
        &[],
    );
    assert!(
        matches!(result, Err(MergeError::AmbiguousSectionOrder { .. })),
        "excerpt admission is not section-order evidence: {result:?}"
    );
    let flagged = selection.with_flagged_draft_order(&reference);
    let draft = merge_chat_files_with_donor_selection_draft(&reference, &flagged, &[child], &[])
        .expect("explicitly requested review serialization, not validated output");
    assert_eq!(draft.origins().len(), 3);
    assert_eq!(draft.bullet_edits().count(), 0);
    for (u, origin) in draft.utterances_with_origin() {
        let original = match origin {
            MergeOrigin::Retained(i) => reference.utterances().nth(i.utterance().raw()),
            MergeOrigin::Inserted(i) => excerpt.utterances().nth(i.utterance().raw()),
        }
        .expect("original selected speech");
        assert!(u.semantic_eq(original));
    }
    let mut markers = Vec::new();
    assert!(
        draft.draft_order_reviews().iter().any(|review| {
            review.before_output_utterance.utterances_before() == 0
                && matches!(&review.reason, DraftOrderReason::Sections { reference, donor }
                if reference.trim() == "@Bg:\tplay" && donor.trim() == "@Bg:\tplay")
        }),
        "the unresolved pair is returned as data before validation"
    );
    for line in &draft.file().lines {
        let Line::Header { header, .. } = line else {
            continue;
        };
        match header.as_ref() {
            Header::BeginGem { label } => {
                markers.push(("begin", label.as_ref().map(|s| s.as_str())))
            }
            Header::EndGem { label } => markers.push(("end", label.as_ref().map(|s| s.as_str()))),
            Header::Comment { .. } => assert!(
                source.lines.iter().any(|original| {
                    matches!(original, Line::Header { header: source, .. }
                    if source.to_chat_string() == header.to_chat_string())
                }),
                "only source comments survive"
            ),
            _ => {}
        }
    }
    assert_eq!(
        markers,
        [
            ("begin", Some("play")),
            ("begin", Some("play")),
            ("end", Some("play")),
            ("end", Some("play")),
        ]
    );
    // Only the unvalidated draft is inspected here. Preserving conflicting
    // annotations is not a claim that they form a valid combined transcript.
    assert_eq!(source.to_chat(), original);
}

#[test]
fn reference_gem_review_drafts_report_crossed_and_missing_navigation_bounds() {
    struct ExpectedReview {
        output_boundary: usize,
        previous_end: Option<u64>,
        next_start: Option<u64>,
    }
    struct Case {
        path: &'static str,
        retained: &'static [&'static str],
        bounds: &'static [ExpectedReview],
    }
    for case in [
        Case {
            path: "corpus/reference/edge-cases/timed-gem-crossed-neighbors.cha",
            retained: &["CHI", "MOT"],
            bounds: &[ExpectedReview {
                output_boundary: 2,
                previous_end: Some(45),
                next_start: Some(30),
            }],
        },
        Case {
            path: "corpus/reference/edge-cases/timed-gem-exterior.cha",
            retained: &["CHI"],
            bounds: &[
                ExpectedReview {
                    output_boundary: 0,
                    previous_end: None,
                    next_start: Some(100),
                },
                ExpectedReview {
                    output_boundary: 3,
                    previous_end: Some(300),
                    next_start: None,
                },
            ],
        },
    ] {
        let source = parsed_reference(case.path);
        let retained: Vec<_> = case.retained.iter().map(|s| SpeakerCode::new(*s)).collect();
        let mut reference = source.clone();
        reference.lines.retain(|line| {
            !matches!(line, Line::Utterance(u)
            if !retained.contains(&u.main.speaker))
        });
        // The independent speech-only projection intentionally has no gem
        // annotations. Bind its own unchanged coordinates, not a claim that it
        // preserves section markers removed from the complete transcript.
        let mut donor = source.clone();
        donor.lines.retain(|line| match line {
            Line::Utterance(u) => !retained.contains(&u.main.speaker),
            Line::Header { header, .. } => !matches!(
                header.as_ref(),
                Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
            ),
        });
        let parents = (0..donor.utterances().count())
            .map(|i| DonorIdx::new(UtteranceIdx::new(i)))
            .collect();
        let selection = SourceBoundDonorSelection::bind(&donor, &donor, parents)
            .expect("unchanged independent donor");
        assert!(matches!(
            merge_chat_files_with_donor_selection(&reference, &selection, &retained, &[]),
            Err(MergeError::AmbiguousSectionPlacement { .. })
        ));
        let selection = selection.with_flagged_draft_order(&reference);
        let merged = merge_chat_files_with_donor_selection(&reference, &selection, &retained, &[])
            .expect("explicit review draft remains model-valid");
        assert!(merged.bullet_edits().is_empty());
        assert_eq!(merged.draft_order_reviews().len(), case.bounds.len());
        for (review, expected) in merged.draft_order_reviews().iter().zip(case.bounds) {
            assert_eq!(
                review.before_output_utterance.utterances_before(),
                expected.output_boundary
            );
            assert!(matches!(&review.reason, DraftOrderReason::Section {
                previous_end, next_start, competing: MergeOrigin::Inserted(_), ..
            } if *previous_end == expected.previous_end && *next_start == expected.next_start));
        }
        assert_eq!(merged.origins().len(), source.utterances().count());
        for (u, origin) in merged.utterances_with_origin() {
            let original = match origin {
                MergeOrigin::Retained(i) => reference.utterances().nth(i.utterance().raw()),
                MergeOrigin::Inserted(i) => donor.utterances().nth(i.utterance().raw()),
            }
            .expect("source origin");
            assert!(u.semantic_eq(original));
        }
        let reported = merged.report(|_, _| panic!("no speech omitted"));
        for line in &reported.file().lines {
            if let Line::Header { header, .. } = line {
                assert!(
                    source.lines.iter().any(|original| {
                        matches!(original, Line::Header { header: source, .. }
                        if source.to_chat_string() == header.to_chat_string())
                    }),
                    "no generated review header"
                );
            }
        }
    }
}
