//! Donor metadata admission and declaration placement from reference sources.

use super::{
    ChatFile, Header, Line, MergeError, SemanticEq, TreeSitterParser, WriteChat, strict_parse,
};

#[test]
fn language_specs_preserve_merge_admission_and_input_identity() {
    use talkbank_transform::transcript_merge::{
        LanguageDeclarationProblem, MergeInput, merge_chat_files_by_source_order,
    };
    let parser = TreeSitterParser::new().expect("parser");
    let root = talkbank_parser_tests::repo_paths::workspace_root()
        .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors");
    let load = |name: &str| {
        let text = std::fs::read_to_string(root.join(name)).expect("canonical language spec");
        let errors = talkbank_model::ErrorCollector::new();
        parser.parse_chat_file_streaming(&text, &errors)
    };
    let single = load("E507_3.cha");
    let multiple = load("E507_6.cha");
    let retained = single.unique_utterance_speakers();
    for (fixture, problem) in [
        ("E504_2.cha", LanguageDeclarationProblem::Missing),
        ("E501_3.cha", LanguageDeclarationProblem::Repeated),
        ("E507_2.cha", LanguageDeclarationProblem::Empty),
    ] {
        let invalid = load(fixture);
        let original = invalid.to_chat_string();
        for (reference, donor, expected_input) in [
            (&invalid, &single, MergeInput::Reference),
            (&single, &invalid, MergeInput::Donor),
        ] {
            let result = merge_chat_files_by_source_order(reference, donor, &retained, &[]);
            assert!(
                matches!(result,
                Err(MergeError::InvalidLanguageDeclaration { input, problem: actual })
                    if input == expected_input && actual == problem),
                "{fixture}: {expected_input:?} must refuse {problem:?}: {result:?}"
            );
        }
        assert_eq!(
            invalid.to_chat_string(),
            original,
            "refusal cannot repair declarations"
        );
    }
    let refused = merge_chat_files_by_source_order(&single, &multiple, &retained, &[]);
    assert!(
        matches!(refused, Err(MergeError::LanguageMismatch { file1, file2 })
        if file1.as_slice().iter().map(|c| c.as_str()).collect::<Vec<_>>() == ["eng"]
            && file2.as_slice().iter().map(|c| c.as_str()).collect::<Vec<_>>() == ["eng", "spa"]),
        "donor must not introduce an undeclared reference language"
    );
    let merged = merge_chat_files_by_source_order(&multiple, &single, &retained, &[])
        .expect("donor language subset is allowed");
    let reported = merged.report(|_, _| panic!("retain-all subset merge loses no speaker"));
    let output = reported.into_file();
    assert_eq!(output.utterances().count(), multiple.utterances().count());
    for (actual, expected) in output.utterances().zip(multiple.utterances()) {
        assert!(actual.semantic_eq(expected));
    }
}

#[test]
fn reference_speaker_projection_restores_donor_id_before_opening_comments() {
    use super::{
        DonorIdx, ReferenceIdx, RelativeOrderConstraint, SourceBoundDonorSelection, UtteranceIdx,
        merge_chat_files_with_donor_selection,
    };
    use talkbank_model::model::TranscriptName;
    use talkbank_model::{ErrorCollector, SpeakerCode};

    let parser = TreeSitterParser::new().expect("parser");
    let source = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
    let donor = strict_parse(parser.parse_chat_file(source)).expect("reference syntax");
    let retained = SpeakerCode::new("CHI");
    // A typed speaker projection removes only records owned by the other
    // speaker; all retained values still come from the authored reference.
    let mut reference = donor.clone();
    reference.lines.retain(|line| match line {
        Line::Utterance(u) => u.main.speaker == retained,
        Line::Header { header, .. } => match header.as_ref() {
            Header::ID(id) => id.speaker == retained,
            _ => true,
        },
    });
    reference.lines = reference
        .lines
        .into_iter()
        .map(|mut line| {
            if let Line::Header { header, .. } = &mut line
                && let Header::Participants { entries } = header.as_mut()
            {
                *entries = entries
                    .iter()
                    .filter(|entry| entry.speaker_code == retained)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into();
            }
            line
        })
        .collect::<Vec<_>>()
        .into();
    let errors = ErrorCollector::new();
    reference.validate(&errors, TranscriptName::Anonymous);
    assert!(
        !errors.has_errors(),
        "projection remains valid: {:?}",
        errors.to_vec()
    );
    // Explicit correspondence comes from the original source order. Untimed
    // inputs alone do not authorize the merger to invent relative placement.
    let parents: Vec<_> = donor
        .utterances()
        .enumerate()
        .map(|(index, _)| DonorIdx::new(UtteranceIdx::new(index)))
        .collect();
    let retained_parents: Vec<_> = donor
        .utterances()
        .enumerate()
        .filter_map(|(index, u)| (u.main.speaker == retained).then_some(index))
        .collect();
    let mut order = Vec::new();
    for (reference_index, parent) in retained_parents.iter().enumerate() {
        for (donor_index, u) in donor.utterances().enumerate() {
            if u.main.speaker == retained {
                continue;
            }
            let reference = ReferenceIdx::new(UtteranceIdx::new(reference_index));
            let donor = DonorIdx::new(UtteranceIdx::new(donor_index));
            order.push(if *parent < donor_index {
                RelativeOrderConstraint::ReferenceBefore { reference, donor }
            } else {
                RelativeOrderConstraint::DonorBefore { donor, reference }
            });
        }
    }
    let selection = SourceBoundDonorSelection::bind(&donor, &donor, parents)
        .expect("original donor coordinates")
        .with_relative_order(&reference, order)
        .expect("original source correspondence");
    let merged = merge_chat_files_with_donor_selection(&reference, &selection, &[retained], &[])
        .expect("source-bound donor insertion");
    let reported = merged.report(|_, _| panic!("both declared speakers remain represented"));
    let output = reported.file();
    let ids = |file: &ChatFile| {
        file.headers()
            .filter(|h| matches!(h, Header::ID(_)))
            .map(WriteChat::to_chat_string)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ids(output),
        ids(&donor),
        "donor ID follows retained ID without fabrication"
    );
    let participants = |file: &ChatFile| {
        file.headers()
            .filter(|h| matches!(h, Header::Participants { .. }))
            .map(WriteChat::to_chat_string)
            .collect::<Vec<_>>()
    };
    assert_eq!(participants(output), participants(&donor));
    let mut comment_seen = false;
    for header in output.headers() {
        match header {
            Header::Comment { .. } => comment_seen = true,
            Header::ID(_) => assert!(!comment_seen, "new ID must precede opening comments"),
            _ => {}
        }
    }
    assert!(
        comment_seen,
        "authored comments exercise the ordering boundary"
    );
    for speaker in donor.unique_utterance_speakers() {
        let before: Vec<_> = donor
            .utterances()
            .filter(|u| u.main.speaker == speaker)
            .collect();
        let after: Vec<_> = output
            .utterances()
            .filter(|u| u.main.speaker == speaker)
            .collect();
        assert_eq!(after.len(), before.len());
        assert!(
            before
                .iter()
                .zip(after)
                .all(|(before, after)| before.semantic_eq(after))
        );
    }
    let errors = ErrorCollector::new();
    output.validate(&errors, TranscriptName::Anonymous);
    assert!(
        !errors.has_errors(),
        "merged declarations validate: {:?}",
        errors.to_vec()
    );
}

#[test]
fn reference_donor_metadata_order_is_preserved_or_refused_without_repair() {
    use talkbank_transform::transcript_merge::merge_chat_files_by_source_order;
    let parser = TreeSitterParser::new().expect("parser");
    let source = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
    let reference = strict_parse(parser.parse_chat_file(source)).expect("reference syntax");
    let retained = reference.unique_utterance_speakers();
    let accepted = merge_chat_files_by_source_order(&reference, &reference, &retained, &[])
        .expect("original ID-before-comment order is admitted");
    let reported = accepted.report(|_, _| panic!("retain-all loses no speaker"));
    assert_eq!(
        reported.file().utterances().count(),
        reference.utterances().count()
    );
    let comments = |file: &ChatFile| -> Vec<_> {
        file.headers()
            .filter(|header| matches!(header, Header::Comment { .. }))
            .map(WriteChat::to_chat_string)
            .collect()
    };
    let original_comments = comments(&reference);
    assert_eq!(original_comments.len(), 2, "authored opening comments");
    assert_eq!(
        comments(reported.file()),
        original_comments
            .iter()
            .chain(&original_comments)
            .cloned()
            .collect::<Vec<_>>(),
        "legal merge preserves comments from both inputs in source order"
    );

    let id_indices: Vec<_> = reference
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            matches!(line,
            Line::Header { header, .. } if matches!(header.as_ref(), Header::ID(_)))
            .then_some(index)
        })
        .collect();
    assert_eq!(
        id_indices.len(),
        2,
        "both authored identities are witnesses"
    );
    for index in id_indices {
        let mut lines: Vec<_> = reference.lines.iter().cloned().collect();
        let id = lines.remove(index);
        let comment = lines
            .iter()
            .position(|line| {
                matches!(line,
            Line::Header { header, .. } if matches!(header.as_ref(), Header::Comment { .. }))
            })
            .expect("authored opening comment");
        lines.insert(comment + 1, id);
        let mut donor = reference.clone();
        donor.lines = lines.into();
        let before = donor.to_chat_string();
        let result = merge_chat_files_by_source_order(&reference, &donor, &retained, &[]);
        assert!(
            matches!(result, Err(MergeError::DonorMetadataOrder)),
            "moving either ID after a comment must refuse, even for retained speakers"
        );
        assert_eq!(
            donor.to_chat_string(),
            before,
            "refusal cannot silently reorder donor headers"
        );
        assert_eq!(donor.utterances().count(), reference.utterances().count());
        assert!(
            donor
                .utterances()
                .zip(reference.utterances())
                .all(|(actual, expected)| actual.semantic_eq(expected))
        );
    }
}
